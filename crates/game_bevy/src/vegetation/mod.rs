// crates/game_bevy/src/vegetation/mod.rs
mod grass;

use bevy::prelude::*;
use std::sync::Arc;
use terrain_generation::RecipeDensitySource;
use voxel_core::ChunkCoord;

use crate::data::{ConfigRegistryResource, UserSetupPrefs};
use crate::environment::BiomeCatalog;
use crate::environment::biomes::{BiomeKind, classify_biome};
use crate::lod::LodPolicy;
use crate::physics::NeedsGroundSnap;
use crate::player::Player;
use crate::state::AppState;
use crate::terrain::{
    ChunkState, TerrainPipelineState, TerrainWorldRuntime,
    residency::{
        chunk_chebyshev_distance, within_decoration_radius, within_high_detail_radius,
    },
    world_position_in_decoration_radius, world_position_in_high_detail_radius,
};
use crate::ui::{EcologyTweaks, WorldTweaks};
use crate::world::requested_world_id;
use game_data::VegetationRuleDefinition;
use voxel_core::CHUNK_CELLS;

pub use grass::{GrassPatch, GrassPlugin};

#[derive(Component)]
pub struct VegetationInstance {
    pub chunk: ChunkCoord,
}

#[derive(Component)]
pub struct PropInstance;

pub struct VegetationPlugin;

impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnvironmentMeshLibrary>()
            .add_systems(Startup, init_environment_mesh_library)
            .add_plugins(GrassPlugin)
            .add_systems(
                Update,
                (
                    despawn_vegetation_outside_residency,
                    respawn_ecology_on_density_change,
                    spawn_environment_when_ready,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn despawn_vegetation_outside_residency(
    mut commands: Commands,
    runtime: Res<TerrainWorldRuntime>,
    world_tweaks: Res<WorldTweaks>,
    vegetation: Query<(Entity, &VegetationInstance)>,
) {
    let center = runtime.interest_center;
    for (entity, instance) in &vegetation {
        if !within_decoration_radius(center, instance.chunk, &world_tweaks) {
            commands.entity(entity).despawn();
        }
    }
}

#[derive(Component)]
struct EnvironmentSpawned;

const ECOLOGY_DENSITY_RESPAWN_THRESHOLD: f32 = 0.05;

fn respawn_ecology_on_density_change(
    mut commands: Commands,
    ecology: Res<EcologyTweaks>,
    mut last_density: Local<f32>,
    players: Query<Entity, (With<Player>, With<EnvironmentSpawned>)>,
    vegetation: Query<Entity, With<VegetationInstance>>,
    props: Query<Entity, With<PropInstance>>,
) {
    if players.is_empty() {
        *last_density = ecology.vegetation_density;
        return;
    }
    if (ecology.vegetation_density - *last_density).abs() <= ECOLOGY_DENSITY_RESPAWN_THRESHOLD {
        return;
    }
    *last_density = ecology.vegetation_density;
    for entity in vegetation.iter().chain(props.iter()) {
        commands.entity(entity).despawn();
    }
    for player in &players {
        commands.entity(player).remove::<EnvironmentSpawned>();
    }
}

fn spawn_environment_when_ready(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    policy: Res<LodPolicy>,
    pipeline: Res<TerrainPipelineState>,
    biomes: Res<BiomeCatalog>,
    ecology: Res<EcologyTweaks>,
    world_tweaks: Res<WorldTweaks>,
    runtime: Res<TerrainWorldRuntime>,
    players: Query<
        (&Transform, Entity),
        (
            With<Player>,
            Without<NeedsGroundSnap>,
            Without<EnvironmentSpawned>,
        ),
    >,
    mesh_library: Res<EnvironmentMeshLibrary>,
    mut last_density: Local<f32>,
) {
    let Ok((player_tf, player)) = players.single() else {
        return;
    };
    let Some(source) = pipeline.density_source.as_ref().map(Arc::clone) else {
        return;
    };
    let Some(lib) = mesh_library.0.as_ref() else {
        return;
    };
    let Some(spawn_chunk) = pipeline.spawn_chunk else {
        return;
    };
    let spawn_ready = pipeline
        .chunks
        .values()
        .any(|c| c.coord == spawn_chunk && c.state == ChunkState::Ready && c.entity.is_some());
    if !spawn_ready {
        return;
    }

    commands.entity(player).insert(EnvironmentSpawned);
    *last_density = ecology.vegetation_density;
    spawn_vegetation_and_props(
        &mut commands,
        &registry,
        &prefs,
        source.as_ref(),
        lib,
        &biomes,
        ecology.vegetation_density,
        &world_tweaks,
        &policy,
        runtime.interest_center,
        player_tf.translation,
    );
}

struct MeshLibrary {
    grass: Handle<Mesh>,
    shrub: Handle<Mesh>,
    tree: Handle<Mesh>,
    rock: Handle<Mesh>,
    moss: Handle<Mesh>,
    fungus: Handle<Mesh>,
    driftwood: Handle<Mesh>,
    cave_stone: Handle<Mesh>,
    grass_mat: Handle<StandardMaterial>,
    shrub_mat: Handle<StandardMaterial>,
    tree_mat: Handle<StandardMaterial>,
    rock_mat: Handle<StandardMaterial>,
    moss_mat: Handle<StandardMaterial>,
    fungus_mat: Handle<StandardMaterial>,
    driftwood_mat: Handle<StandardMaterial>,
    cave_stone_mat: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct EnvironmentMeshLibrary(Option<MeshLibrary>);

fn init_environment_mesh_library(
    mut library: ResMut<EnvironmentMeshLibrary>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if library.0.is_none() {
        library.0 = Some(build_mesh_library(&mut meshes, &mut materials));
    }
}

struct SpawnOccupancyGrid {
    cell_size: f32,
    cells: std::collections::HashMap<(i32, i32), Vec<(f32, f32)>>,
}

impl SpawnOccupancyGrid {
    fn new(min_spacing: f32) -> Self {
        Self {
            cell_size: min_spacing.max(1.0),
            cells: std::collections::HashMap::new(),
        }
    }

    fn allows(&self, x: f32, z: f32, spacing: f32) -> bool {
        let spacing_sq = spacing * spacing;
        let gx = (x / self.cell_size).floor() as i32;
        let gz = (z / self.cell_size).floor() as i32;
        for dx in -1..=1 {
            for dz in -1..=1 {
                let Some(entries) = self.cells.get(&(gx + dx, gz + dz)) else {
                    continue;
                };
                for (ox, oz) in entries {
                    let ddx = x - ox;
                    let ddz = z - oz;
                    if ddx * ddx + ddz * ddz < spacing_sq {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn insert(&mut self, x: f32, z: f32) {
        let gx = (x / self.cell_size).floor() as i32;
        let gz = (z / self.cell_size).floor() as i32;
        self.cells.entry((gx, gz)).or_default().push((x, z));
    }
}

fn build_mesh_library(
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> MeshLibrary {
    MeshLibrary {
        grass: meshes.add(Cuboid::new(0.15, 0.35, 0.15)),
        shrub: meshes.add(Cuboid::new(0.4, 0.55, 0.4)),
        tree: meshes.add(Cylinder::new(0.2, 2.5)),
        rock: meshes.add(Sphere::new(0.4)),
        moss: meshes.add(Cuboid::new(0.2, 0.15, 0.2)),
        fungus: meshes.add(Sphere::new(0.25)),
        driftwood: meshes.add(Cuboid::new(0.25, 0.12, 1.2)),
        cave_stone: meshes.add(Cuboid::new(0.5, 0.35, 0.45)),
        grass_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.55, 0.18),
            ..default()
        }),
        shrub_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.48, 0.16),
            ..default()
        }),
        tree_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.15, 0.35, 0.12),
            ..default()
        }),
        rock_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.5, 0.48, 0.45),
            ..default()
        }),
        moss_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.28, 0.14),
            ..default()
        }),
        fungus_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.9, 0.5),
            emissive: LinearRgba::from(Color::srgb(0.15, 0.55, 0.25)),
            ..default()
        }),
        driftwood_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.38, 0.28),
            ..default()
        }),
        cave_stone_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.38, 0.36, 0.34),
            ..default()
        }),
    }
}

fn spawn_vegetation_and_props(
    commands: &mut Commands,
    registry: &ConfigRegistryResource,
    prefs: &UserSetupPrefs,
    source: &RecipeDensitySource,
    lib: &MeshLibrary,
    biomes: &BiomeCatalog,
    ecology_density: f32,
    world_tweaks: &WorldTweaks,
    policy: &LodPolicy,
    interest_center: voxel_core::ChunkCoord,
    player_position: Vec3,
) {
    let world_id = requested_world_id(prefs);
    let vegetation = registry
        .0
        .effective_world(Some(&world_id))
        .ok()
        .and_then(|world| registry.0.effective_vegetation(world))
        .or_else(|| {
            registry
                .0
                .vegetation
                .get(&shared::StableId::new("vegetation.vertical_slice"))
        })
        .or_else(|| registry.0.vegetation.values().next());
    let perf = registry.0.active_performance().expect("performance");
    let density_mult = perf.vegetation_density_multiplier * ecology_density;
    let max_dist = policy
        .content
        .vegetation_max_distance_m
        .min(perf.vegetation_maximum_distance_m);
    let seed = source.recipe().seed;

    let default_rules = vegetation.map(|v| v.rules.as_slice()).unwrap_or(&[]);
    let mut rng_state = seed;
    let mut occupied = SpawnOccupancyGrid::new(2.0);

    let decoration = world_tweaks.decoration_radius;
    for dz in -decoration..=decoration {
        for dy in -decoration..=decoration {
            for dx in -decoration..=decoration {
                let chunk_coord = ChunkCoord::new(
                    interest_center.x + dx,
                    interest_center.y + dy,
                    interest_center.z + dz,
                );
                let chunk_dist = chunk_chebyshev_distance(interest_center, chunk_coord);
                if chunk_dist > decoration {
                    continue;
                }
                let veg_lod = vegetation_lod_tier(interest_center, chunk_coord, world_tweaks);
                if veg_lod >= 3 {
                    continue;
                }
                let chunk_origin_x = chunk_coord.x as f32 * CHUNK_CELLS as f32;
                let chunk_origin_z = chunk_coord.z as f32 * CHUNK_CELLS as f32;
                for sx in (0..CHUNK_CELLS).step_by(2) {
                    for sz in (0..CHUNK_CELLS).step_by(2) {
                        let wx = chunk_origin_x + sx as f32;
                        let wz = chunk_origin_z + sz as f32;
                        if !world_position_in_decoration_radius(
                            interest_center,
                            Vec3::new(wx, 0.0, wz),
                            world_tweaks,
                        ) {
                            continue;
                        }
                        let dist_from_player =
                            Vec2::new(wx - player_position.x, wz - player_position.z).length();
                        if dist_from_player > decoration as f32 * CHUNK_CELLS as f32 {
                            continue;
                        }
                        let detail_scale = if world_position_in_high_detail_radius(
                            interest_center,
                            Vec3::new(wx, 0.0, wz),
                            world_tweaks,
                        ) {
                            1.0
                        } else {
                            0.55
                        };
                        if dist_from_player > max_dist {
                            continue;
                        }
                        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
                        let surface_y = source.surface_height_at(wx, wz);
                        let density = source.density_at(wx, surface_y, wz);
                        if density > 0.0 {
                            continue;
                        }
                        let biome = classify_biome(biomes, source, wx, surface_y, wz, density);
                        let slope = estimate_slope_deg(source, wx, surface_y, wz);
                        let biome_rules = biomes
                            .vegetation_profile_for(biome)
                            .and_then(|id| registry.0.vegetation.get(&id))
                            .map(|v| v.rules.as_slice())
                            .unwrap_or(default_rules);

                        for rule in biome_rules {
                            if !rule_matches_biome(rule, biome) {
                                continue;
                            }
                            if slope > rule.slope_max_deg {
                                continue;
                            }
                            if !occupied.allows(wx, wz, rule.spacing_m) {
                                continue;
                            }
                            let roll = (rng_state % 1000) as f32 / 1000.0;
                            if roll > density_mult * rule.density * detail_scale {
                                continue;
                            }
                            let (mesh, mat, y_off, base_scale, rot_y) =
                                prototype_for_rule(&lib, &rule.category);
                            let mesh = vegetation_lod_mesh(veg_lod, &lib, mesh);
                            let scale = vegetation_lod_scale(veg_lod, base_scale);
                            occupied.insert(wx, wz);
                            commands.spawn((
                                VegetationInstance { chunk: chunk_coord },
                                Mesh3d(mesh),
                                MeshMaterial3d(mat),
                                Transform::from_xyz(wx, surface_y + y_off, wz)
                                    .with_rotation(Quat::from_rotation_y(rot_y))
                                    .with_scale(scale),
                            ));
                            break;
                        }

                        if biome == BiomeKind::Beach && (rng_state % 17) == 0 {
                            let rot = (rng_state % 360) as f32 * 0.05;
                            commands.spawn((
                                PropInstance,
                                Mesh3d(lib.driftwood.clone()),
                                MeshMaterial3d(lib.driftwood_mat.clone()),
                                Transform::from_xyz(wx, surface_y + 0.06, wz)
                                    .with_rotation(Quat::from_rotation_y(rot)),
                            ));
                        }
                        if biome == BiomeKind::Cave && surface_y < 2.0 && (rng_state % 11) == 0 {
                            commands.spawn((
                                PropInstance,
                                Mesh3d(lib.cave_stone.clone()),
                                MeshMaterial3d(lib.cave_stone_mat.clone()),
                                Transform::from_xyz(wx, surface_y + 0.2, wz),
                            ));
                        }
                    }
                }
            }
        }
    }
}

fn vegetation_lod_tier(center: ChunkCoord, coord: ChunkCoord, tweaks: &WorldTweaks) -> u8 {
    if within_high_detail_radius(center, coord, tweaks) {
        0
    } else if within_decoration_radius(center, coord, tweaks) {
        1
    } else {
        3
    }
}

fn vegetation_lod_mesh(lod: u8, lib: &MeshLibrary, mesh: Handle<Mesh>) -> Handle<Mesh> {
    if lod >= 2 { lib.grass.clone() } else { mesh }
}

fn vegetation_lod_scale(lod: u8, base: Vec3) -> Vec3 {
    match lod {
        0 => base,
        1 => base * 0.7,
        2 => Vec3::new(base.x.max(0.5) * 1.5, 0.06, base.z.max(0.5) * 1.5),
        _ => base,
    }
}

fn rule_matches_biome(rule: &VegetationRuleDefinition, biome: BiomeKind) -> bool {
    if rule.biomes.is_empty() {
        return true;
    }
    let id = biome.as_rule_id();
    rule.biomes.iter().any(|b| b == id)
}

fn prototype_for_rule(
    lib: &MeshLibrary,
    category: &str,
) -> (Handle<Mesh>, Handle<StandardMaterial>, f32, Vec3, f32) {
    match category {
        "tree" => (lib.tree.clone(), lib.tree_mat.clone(), 1.25, Vec3::ONE, 0.0),
        "rock" => (
            lib.rock.clone(),
            lib.rock_mat.clone(),
            0.2,
            Vec3::splat(0.8 + (category.len() as f32 * 0.01)),
            0.0,
        ),
        "shrub" => (
            lib.shrub.clone(),
            lib.shrub_mat.clone(),
            0.28,
            Vec3::ONE,
            0.0,
        ),
        "cave_moss" | "fungus" => (
            if category == "fungus" {
                lib.fungus.clone()
            } else {
                lib.moss.clone()
            },
            if category == "fungus" {
                lib.fungus_mat.clone()
            } else {
                lib.moss_mat.clone()
            },
            0.08,
            Vec3::ONE,
            0.0,
        ),
        _ => (
            lib.grass.clone(),
            lib.grass_mat.clone(),
            0.18,
            Vec3::ONE,
            0.0,
        ),
    }
}

fn estimate_slope_deg(source: &RecipeDensitySource, wx: f32, y: f32, wz: f32) -> f32 {
    let h = 0.5;
    let yx = source.surface_height_at(wx + h, wz);
    let yz = source.surface_height_at(wx, wz + h);
    let dx = yx - y;
    let dz = yz - y;
    (dx * dx + dz * dz).sqrt().atan().to_degrees()
}
