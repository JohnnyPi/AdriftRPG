use bevy::prelude::*;
use terrain_generation::RecipeDensitySource;

use crate::data::ConfigRegistryResource;
use crate::environment::biomes::{classify_biome, BiomeCatalog, BiomeKind};
use crate::physics::SpawnTerrainReleased;
use crate::player::Player;
use crate::state::AppState;
use crate::terrain::{ChunkState, TerrainPipelineState};

#[derive(Component)]
pub struct VegetationInstance;

pub struct VegetationPlugin;

impl Plugin for VegetationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            spawn_vegetation_when_ready.run_if(in_state(AppState::Running)),
        );
    }
}

fn spawn_vegetation_when_ready(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    pipeline: Res<TerrainPipelineState>,
    biomes: Res<BiomeCatalog>,
    players: Query<Entity, (With<Player>, With<SpawnTerrainReleased>, Without<VegetationSpawned>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(player) = players.single() else {
        return;
    };
    let Some(source) = pipeline.density_source.clone() else {
        return;
    };
    let Some(spawn_chunk) = pipeline.spawn_chunk else {
        return;
    };
    let spawn_ready = pipeline.chunks.iter().any(|c| {
        c.coord == spawn_chunk && c.state == ChunkState::Ready && c.entity.is_some()
    });
    if !spawn_ready {
        return;
    }

    commands.entity(player).insert(VegetationSpawned);
    spawn_vegetation_instances(
        &mut commands,
        &registry,
        &source,
        &biomes,
        &mut meshes,
        &mut materials,
    );
}

#[derive(Component)]
struct VegetationSpawned;

fn spawn_vegetation_instances(
    commands: &mut Commands,
    registry: &ConfigRegistryResource,
    source: &RecipeDensitySource,
    biomes: &BiomeCatalog,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let vegetation = registry
        .0
        .vegetation
        .get(&shared::StableId::new("vegetation.vertical_slice"))
        .or_else(|| registry.0.vegetation.values().next());
    let perf = registry.0.active_performance().expect("performance");
    let density_mult = perf.vegetation_density_multiplier;
    let max_dist = perf.vegetation_maximum_distance_m;
    let sea_level = source.recipe().sea_level;
    let seed = source.recipe().seed;

    let grass_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.55, 0.18),
        ..default()
    });
    let rock_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.48, 0.45),
        ..default()
    });
    let tree_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.35, 0.12),
        ..default()
    });
    let moss_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.28, 0.14),
        ..default()
    });

    let grass_mesh = meshes.add(Cuboid::new(0.15, 0.35, 0.15));
    let rock_mesh = meshes.add(Sphere::new(0.4));
    let tree_mesh = meshes.add(Cylinder::new(0.2, 2.5));

    let rules = vegetation.map(|v| v.rules.as_slice()).unwrap_or(&[]);
    let mut rng_state = seed;
    for x in (-40..40).step_by(3) {
        for z in (-40..40).step_by(3) {
            if (x as f32 * x as f32 + z as f32 * z as f32).sqrt() > max_dist {
                continue;
            }
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let wx = x as f32;
            let wz = z as f32;
            let surface_y = source.surface_height_at(wx, wz);
            let density = source.density_at(wx, surface_y, wz);
            if density > 0.0 {
                continue;
            }
            let biome = classify_biome(biomes, sea_level, wx, surface_y, wz, density);
            if biome == BiomeKind::Cave && rules.is_empty() {
                continue;
            }

            let rule_density = rules
                .first()
                .map(|r| r.density)
                .unwrap_or(0.35);
            if (rng_state % 1000) as f32 / 1000.0 > density_mult * rule_density {
                continue;
            }

            let (mesh, mat) = match biome {
                BiomeKind::RockyUpland => (rock_mesh.clone(), rock_mat.clone()),
                BiomeKind::Cave => (grass_mesh.clone(), moss_mat.clone()),
                BiomeKind::Grassland if (rng_state % 5) == 0 => {
                    (tree_mesh.clone(), tree_mat.clone())
                }
                _ => (grass_mesh.clone(), grass_mat.clone()),
            };

            commands.spawn((
                VegetationInstance,
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                Transform::from_xyz(wx, surface_y, wz),
            ));
        }
    }
}
