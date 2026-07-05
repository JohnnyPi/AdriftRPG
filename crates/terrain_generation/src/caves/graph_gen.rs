//! Procedural cave graph generation from suitability fields.

use game_data::CompiledCavesRecipe;

use crate::contract::coordinates::{WorldPosition, WorldXZ};
use crate::contract::version::derive_seed;
use crate::fields::scalar::ScalarField;

use super::graph::{
    CaveEdge, CaveFamily, CaveGraphRegistry, CaveNode, CaveNodeKind, CaveSystem, WallNoiseParams,
};
use super::suitability::{CaveSuitabilityFields, family_field};

pub fn generate_cave_systems(
    fields: &CaveSuitabilityFields,
    elevation: &ScalarField,
    land_mask: &ScalarField,
    river_mask: &ScalarField,
    recipe: &CompiledCavesRecipe,
    world_seed: u64,
    sea_level_m: f32,
    island_center: WorldXZ,
) -> CaveGraphRegistry {
    let mut systems = Vec::new();
    let families = [
        (CaveFamily::LavaTube, recipe.lava_tube.systems_max.max(1)),
        (CaveFamily::Limestone, recipe.limestone.systems_max),
        (CaveFamily::SeaCave, recipe.sea_cave.systems_max),
    ];
    let mut system_index = 0u32;
    for (family, max_count) in families {
        if max_count == 0 {
            continue;
        }
        let field = family_field(fields, family);
        for local in 0..max_count {
            let family_label = cave_family_label(family);
            let seed = derive_seed(world_seed, family_label, None, local as u64);
            if let Some(system) = generate_one_system(
                family,
                field,
                elevation,
                land_mask,
                river_mask,
                recipe,
                seed,
                sea_level_m,
                island_center,
                system_index,
                local,
            ) {
                systems.push(system);
                system_index += 1;
            }
        }
    }
    if systems.is_empty() {
        if let Some(system) = forced_lava_system(
            fields,
            elevation,
            land_mask,
            recipe,
            world_seed,
            sea_level_m,
            island_center,
        ) {
            systems.push(system);
        }
    }
    CaveGraphRegistry { systems }
}

fn forced_lava_system(
    fields: &CaveSuitabilityFields,
    elevation: &ScalarField,
    land_mask: &ScalarField,
    recipe: &CompiledCavesRecipe,
    world_seed: u64,
    sea_level_m: f32,
    island_center: WorldXZ,
) -> Option<CaveSystem> {
    let field = &fields.lava_tube;
    let desc = &field.descriptor;
    let mut best: Option<(u32, u32)> = None;
    let mut best_land = 0.0f32;
    for z in 0..desc.height {
        for x in 0..desc.width {
            let wx = desc.origin_x() + x as f64 * desc.cell_size_m;
            let wz = desc.origin_z() + z as f64 * desc.cell_size_m;
            let land = land_mask.sample_at_world(WorldXZ::new(wx, wz));
            if land > best_land {
                best_land = land;
                best = Some((x, z));
            }
        }
    }
    let (x, z) = best?;
    let wx = desc.origin_x() + x as f64 * desc.cell_size_m + desc.cell_size_m * 0.5;
    let wz = desc.origin_z() + z as f64 * desc.cell_size_m + desc.cell_size_m * 0.5;
    let seed = derive_seed(world_seed, "cave_forced_lava", None, 0);
    generate_one_system(
        CaveFamily::LavaTube,
        field,
        elevation,
        land_mask,
        &ScalarField::zeros(desc.clone()),
        recipe,
        seed,
        sea_level_m,
        island_center,
        0,
        0,
    )
    .or_else(|| {
        let profile = super::recipe::profile_for(recipe, CaveFamily::LavaTube);
        let entrance_pos = WorldPosition::new(
            wx,
            (elevation.sample_at_world(WorldXZ::new(wx, wz)) - profile.minimum_cover_m) as f64,
            wz,
        );
        let mut nodes = vec![CaveNode {
            kind: CaveNodeKind::Entrance,
            position: entrance_pos,
            radius_m: profile.passage_radius_min_m.max(1.0),
        }];
        let end = WorldPosition::new(wx + 8.0, entrance_pos.0.y - 4.0, wz + 6.0);
        nodes.push(CaveNode {
            kind: CaveNodeKind::Terminus,
            position: end,
            radius_m: profile.passage_radius_min_m,
        });
        Some(CaveSystem {
            id: "cave.lava_tube.forced".into(),
            family: CaveFamily::LavaTube,
            nodes,
            edges: vec![CaveEdge {
                from: 0,
                to: 1,
                radius_m: profile.passage_radius_min_m,
                noise: WallNoiseParams::default(),
            }],
            entrance_world: [entrance_pos.0.x, entrance_pos.0.y, entrance_pos.0.z],
            overhang_enabled: profile.overhang_enabled,
        })
    })
}

fn generate_one_system(
    family: CaveFamily,
    suitability: &ScalarField,
    elevation: &ScalarField,
    land_mask: &ScalarField,
    river_mask: &ScalarField,
    recipe: &CompiledCavesRecipe,
    seed: u64,
    sea_level_m: f32,
    island_center: WorldXZ,
    system_index: u32,
    local_index: u32,
) -> Option<CaveSystem> {
    let profile = super::recipe::profile_for(recipe, family);
    let entrance = pick_entrance(
        suitability,
        elevation,
        land_mask,
        river_mask,
        sea_level_m,
        family,
        seed,
        profile.entrance_threshold,
    )?;
    let chamber_count = profile.chamber_count_min
        + (hash_unit(seed, 1) * (profile.chamber_count_max - profile.chamber_count_min) as f32)
            .floor() as u32;
    let chamber_count = chamber_count.max(2).min(8) as usize;
    let passage_r = lerp(
        profile.passage_radius_min_m,
        profile.passage_radius_max_m,
        0.4 + 0.4 * hash_unit(seed, 2),
    );

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let entrance_pos = WorldPosition::new(
        entrance.0,
        entrance_elevation(
            elevation,
            entrance,
            family,
            sea_level_m,
            profile.minimum_cover_m,
        ),
        entrance.1,
    );
    nodes.push(CaveNode {
        kind: CaveNodeKind::Entrance,
        position: entrance_pos,
        radius_m: passage_r * 1.1,
    });

    let mut prev_idx = 0usize;
    for i in 0..chamber_count {
        let t = if chamber_count <= 1 {
            0.5
        } else {
            i as f32 / (chamber_count - 1) as f32
        };
        let (wx, wz) = next_node_xz(family, entrance, island_center, seed, i, t, suitability);
        let wy = chamber_height(
            elevation,
            wx,
            wz,
            sea_level_m,
            profile.minimum_cover_m,
            profile.maximum_depth_m,
            t,
        );
        let pos = WorldPosition::new(wx, wy, wz);
        let kind = if i == chamber_count - 1 {
            if hash_unit(seed, 10 + i as u32) > 0.7 {
                CaveNodeKind::Pool
            } else {
                CaveNodeKind::Terminus
            }
        } else if i == 0 {
            CaveNodeKind::Chamber
        } else if hash_unit(seed, 20 + i as u32) > 0.75 {
            CaveNodeKind::Junction
        } else {
            CaveNodeKind::Chamber
        };
        let radius = passage_r * (0.9 + 0.3 * hash_unit(seed, 30 + i as u32));
        let node_idx = nodes.len();
        nodes.push(CaveNode {
            kind,
            position: pos,
            radius_m: radius,
        });
        edges.push(CaveEdge {
            from: prev_idx,
            to: node_idx,
            radius_m: (passage_r * 0.85).max(0.6),
            noise: WallNoiseParams {
                frequency: 0.3 + 0.2 * hash_unit(seed, 40 + i as u32),
                amplitude_m: 0.25 + 0.2 * hash_unit(seed, 50 + i as u32),
            },
        });
        prev_idx = node_idx;

        if family == CaveFamily::Limestone
            && hash_unit(seed, 60 + i as u32) > 0.6
            && i > 0
            && i < chamber_count - 1
        {
            let branch_x = wx + f64::from((hash_unit(seed, 70 + i as u32) - 0.5) * 12.0);
            let branch_z = wz + f64::from((hash_unit(seed, 80 + i as u32) - 0.5) * 12.0);
            let branch_y = chamber_height(
                elevation,
                branch_x,
                branch_z,
                sea_level_m,
                profile.minimum_cover_m,
                profile.maximum_depth_m * 0.7,
                t,
            );
            let branch_idx = nodes.len();
            nodes.push(CaveNode {
                kind: CaveNodeKind::Chamber,
                position: WorldPosition::new(branch_x, branch_y, branch_z),
                radius_m: radius * 0.8,
            });
            edges.push(CaveEdge {
                from: prev_idx,
                to: branch_idx,
                radius_m: passage_r * 0.65,
                noise: WallNoiseParams::default(),
            });
        }
    }

    Some(CaveSystem {
        id: format!(
            "cave.{}.{}.{}",
            cave_family_label(family),
            system_index,
            local_index
        ),
        family,
        nodes,
        edges,
        entrance_world: [entrance_pos.0.x, entrance_pos.0.y, entrance_pos.0.z],
        overhang_enabled: profile.overhang_enabled,
    })
}

fn pick_entrance(
    suitability: &ScalarField,
    elevation: &ScalarField,
    land_mask: &ScalarField,
    river_mask: &ScalarField,
    sea_level_m: f32,
    family: CaveFamily,
    seed: u64,
    threshold: f32,
) -> Option<(f64, f64)> {
    let desc = &suitability.descriptor;
    let mut best_score = threshold;
    let mut best: Option<(f64, f64)> = None;
    let mut fallback_score = 0.0f32;
    let mut fallback: Option<(f64, f64)> = None;
    let w = desc.width;
    let h = desc.height;
    for z in (0..h).step_by(2) {
        for x in (0..w).step_by(2) {
            let wx = desc.origin_x() + x as f64 * desc.cell_size_m + desc.cell_size_m * 0.5;
            let wz = desc.origin_z() + z as f64 * desc.cell_size_m + desc.cell_size_m * 0.5;
            let world = WorldXZ::new(wx, wz);
            if land_mask.sample_at_world(world) < 0.3 {
                continue;
            }
            if river_mask.sample_at_world(world) > 0.35 {
                continue;
            }
            let elev = elevation.sample_at_world(world);
            if family != CaveFamily::SeaCave && elev < sea_level_m + 3.0 {
                continue;
            }
            let score = suitability.get(x, z);
            if score > fallback_score {
                fallback_score = score;
                fallback = Some((wx, wz));
            }
            let jitter = (hash_unit(seed, x + z * 131) - 0.5) * 0.05;
            let adjusted = score + jitter;
            if adjusted > best_score {
                best_score = adjusted;
                best = Some((wx, wz));
            }
        }
    }
    best.or(if fallback_score > 0.02 {
        fallback
    } else {
        None
    })
}

fn next_node_xz(
    family: CaveFamily,
    entrance: (f64, f64),
    island_center: WorldXZ,
    seed: u64,
    index: usize,
    t: f32,
    suitability: &ScalarField,
) -> (f64, f64) {
    match family {
        CaveFamily::SeaCave => {
            let angle = f64::from(hash_unit(seed, index as u32 + 100)) * std::f64::consts::TAU;
            let dist = 4.0 + t as f64 * 10.0;
            (
                entrance.0 + angle.cos() * dist,
                entrance.1 + angle.sin() * dist * 0.3,
            )
        }
        CaveFamily::LavaTube => {
            let to_center_x = island_center.x() - entrance.0;
            let to_center_z = island_center.z() - entrance.1;
            let len = (to_center_x * to_center_x + to_center_z * to_center_z)
                .sqrt()
                .max(1.0);
            let step = 8.0 + t as f64 * 14.0;
            let jx = (hash_unit(seed, index as u32 + 200) - 0.5) * 6.0;
            let jz = (hash_unit(seed, index as u32 + 300) - 0.5) * 6.0;
            (
                entrance.0 + (to_center_x / len) * step * (index as f64 + 1.0) * 0.35 + jx as f64,
                entrance.1 + (to_center_z / len) * step * (index as f64 + 1.0) * 0.35 + jz as f64,
            )
        }
        CaveFamily::Limestone | CaveFamily::Fracture | CaveFamily::Talus => {
            let desc = &suitability.descriptor;
            let mut best = (entrance.0, entrance.1);
            let mut best_s = 0.0f32;
            for dz in -3i32..=3 {
                for dx in -3i32..=3 {
                    let gx = ((entrance.0 - desc.origin_x()) / desc.cell_size_m) as i32 + dx * 2;
                    let gz = ((entrance.1 - desc.origin_z()) / desc.cell_size_m) as i32 + dz * 2;
                    if gx < 0 || gz < 0 || gx as u32 >= desc.width || gz as u32 >= desc.height {
                        continue;
                    }
                    let s = suitability.get(gx as u32, gz as u32);
                    if s > best_s {
                        best_s = s;
                        best = (
                            desc.origin_x() + gx as f64 * desc.cell_size_m,
                            desc.origin_z() + gz as f64 * desc.cell_size_m,
                        );
                    }
                }
            }
            let jx = (hash_unit(seed, index as u32 + 400) - 0.5) * 8.0;
            let jz = (hash_unit(seed, index as u32 + 500) - 0.5) * 8.0;
            (
                best.0 + jx as f64 + t as f64 * 5.0,
                best.1 + jz as f64 + t as f64 * 5.0,
            )
        }
    }
}

fn entrance_elevation(
    elevation: &ScalarField,
    entrance: (f64, f64),
    family: CaveFamily,
    sea_level_m: f32,
    cover_m: f32,
) -> f64 {
    let surface = elevation.sample_at_world(WorldXZ::new(entrance.0, entrance.1));
    match family {
        CaveFamily::SeaCave => (sea_level_m + cover_m * 0.5).max(sea_level_m) as f64,
        _ => (surface - cover_m * 0.5).max(sea_level_m + cover_m) as f64,
    }
}

fn chamber_height(
    elevation: &ScalarField,
    wx: f64,
    wz: f64,
    sea_level_m: f32,
    cover_m: f32,
    max_depth_m: f32,
    t: f32,
) -> f64 {
    let surface = elevation.sample_at_world(WorldXZ::new(wx, wz));
    let floor = sea_level_m + cover_m + 1.0;
    let depth = max_depth_m * (0.2 + 0.25 * t);
    (surface - depth).max(floor as f32).min(surface * 0.85) as f64
}

fn hash_unit(seed: u64, salt: u32) -> f32 {
    let h = derive_seed(seed, "cave_hash", None, salt as u64);
    (h as f64 / u64::MAX as f64) as f32
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn cave_family_label(family: CaveFamily) -> &'static str {
    match family {
        CaveFamily::LavaTube => "lava_tube",
        CaveFamily::Limestone => "limestone",
        CaveFamily::SeaCave => "sea_cave",
        CaveFamily::Fracture => "fracture",
        CaveFamily::Talus => "talus",
    }
}
