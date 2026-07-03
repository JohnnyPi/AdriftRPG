// crates/game_bevy/src/vegetation/mod.rs
use bevy::prelude::*;
use terrain_generation::RecipeDensitySource;

use crate::data::ConfigRegistryResource;
use crate::environment::biomes::{classify_biome, BiomeCatalog, BiomeKind};
use crate::physics::SpawnTerrainReleased;
use crate::player::Player;
use crate::state::AppState;
use crate::terrain::{
    world_position_in_decoration_radius, world_position_in_high_detail_radius, ChunkState,
    TerrainPipelineState, TerrainWorldRuntime,
};
use crate::ui::{EcologyTweaks, WorldTweaks};
use game_data::VegetationRuleDefinition;
use voxel_core::CHUNK_CELLS;

#[derive(Component)]
pub struct VegetationInstance;

#[derive(Component)]
pub struct PropInstance;

pub struct VegetationPlugin;

impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            spawn_environment_when_ready.run_if(in_state(AppState::Running)),
        );
    }
}

#[derive(Component)]
struct EnvironmentSpawned;

fn spawn_environment_when_ready(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    pipeline: Res<TerrainPipelineState>,
    biomes: Res<BiomeCatalog>,
    ecology: Res<EcologyTweaks>,
    world_tweaks: Res<WorldTweaks>,
    runtime: Res<TerrainWorldRuntime>,
    players: Query<(&Transform, Entity), (With<Player>, With<SpawnTerrainReleased>, Without<EnvironmentSpawned>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok((player_tf, player)) = players.single() else {
        return;
    };
    let Some(source) = pipeline.density_source.clone() else {
        return;
    };
    let Some(spawn_chunk) = pipeline.spawn_chunk else {
        return;
    };
    let spawn_ready = pipeline.chunks.values().any(|c| {
        c.coord == spawn_chunk && c.state == ChunkState::Ready && c.entity.is_some()
    });
    if !spawn_ready {
        return;
    }

    commands.entity(player).insert(EnvironmentSpawned);
    spawn_vegetation_and_props(
        &mut commands,
        &registry,
        &source,
        &biomes,
        ecology.vegetation_density,
        &world_tweaks,
        runtime.interest_center,
        player_tf.translation,
        &mut meshes,
        &mut materials,
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
    source: &RecipeDensitySource,
    biomes: &BiomeCatalog,
    ecology_density: f32,
    world_tweaks: &WorldTweaks,
    interest_center: voxel_core::ChunkCoord,
    player_position: Vec3,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let vegetation = registry
        .0
        .vegetation
        .get(&shared::StableId::new("vegetation.vertical_slice"))
        .or_else(|| registry.0.vegetation.values().next());
    let perf = registry.0.active_performance().expect("performance");
    let density_mult = perf.vegetation_density_multiplier * ecology_density;
    let max_dist = perf.vegetation_maximum_distance_m;
    let seed = source.recipe().seed;
    let lib = build_mesh_library(meshes, materials);

    let rules = vegetation.map(|v| v.rules.as_slice()).unwrap_or(&[]);
    let mut rng_state = seed;
    let mut occupied: Vec<(f32, f32)> = Vec::new();

    let decoration_m = world_tweaks.decoration_radius as f32 * CHUNK_CELLS as f32;

    for x in (-48..48).step_by(2) {
        for z in (-48..48).step_by(2) {
            let wx = x as f32;
            let wz = z as f32;
            if !world_position_in_decoration_radius(interest_center, Vec3::new(wx, 0.0, wz), world_tweaks) {
                continue;
            }
            let dist_from_player = Vec2::new(wx - player_position.x, wz - player_position.z).length();
            if dist_from_player > decoration_m {
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
            if (wx * wx + wz * wz).sqrt() > max_dist {
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

            for rule in rules {
                if !rule_matches_biome(rule, biome) {
                    continue;
                }
                if slope > rule.slope_max_deg {
                    continue;
                }
                if !spacing_allows(wx, wz, rule.spacing_m, &occupied) {
                    continue;
                }
                let roll = (rng_state % 1000) as f32 / 1000.0;
                if roll > density_mult * rule.density * detail_scale {
                    continue;
                }
                let (mesh, mat, y_off, scale, rot_y) = prototype_for_rule(&lib, &rule.category);
                occupied.push((wx, wz));
                commands.spawn((
                    VegetationInstance,
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(wx, surface_y + y_off, wz)
                        .with_rotation(Quat::from_rotation_y(rot_y))
                        .with_scale(scale),
                ));
                break;
            }

            // Props: driftwood on beach, cave stones near cave entrance
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

fn rule_matches_biome(rule: &VegetationRuleDefinition, biome: BiomeKind) -> bool {
    if rule.biomes.is_empty() {
        return true;
    }
    let id = biome_id(biome);
    rule.biomes.iter().any(|b| b == id)
}

fn biome_id(kind: BiomeKind) -> &'static str {
    match kind {
        BiomeKind::Beach => "beach",
        BiomeKind::Grassland => "grassland",
        BiomeKind::RockyUpland => "rocky_upland",
        BiomeKind::Cave => "cave",
        BiomeKind::ShallowWater => "shallow_water",
        BiomeKind::Wetland => "wetland",
        BiomeKind::Riverbank => "riverbank",
        BiomeKind::Forest => "forest",
        BiomeKind::Scrub => "scrub",
        BiomeKind::Alpine => "mountain_alpine",
        BiomeKind::CoastalScrub => "coastal_scrub",
        BiomeKind::DeepWater => "deep_water",
        BiomeKind::OffshoreShelf => "offshore_shelf",
    }
}

fn prototype_for_rule(
    lib: &MeshLibrary,
    category: &str,
) -> (Handle<Mesh>, Handle<StandardMaterial>, f32, Vec3, f32) {
    match category {
        "tree" => (
            lib.tree.clone(),
            lib.tree_mat.clone(),
            1.25,
            Vec3::ONE,
            0.0,
        ),
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

fn spacing_allows(wx: f32, wz: f32, spacing: f32, occupied: &[(f32, f32)]) -> bool {
    let min_dist = spacing.max(1.0);
    occupied
        .iter()
        .all(|(ox, oz)| (wx - ox).hypot(wz - oz) >= min_dist)
}
