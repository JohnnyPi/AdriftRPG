//! Procedural grass patches — cluster mesh per terrain chunk with distance LOD.

use bevy::prelude::*;
use voxel_core::{ChunkCoord, CHUNK_CELLS};

use crate::lod::{grass_lod_band, LodPolicy};
use crate::player::Player;
use crate::state::AppState;
use crate::terrain::{
    residency::chunk_chebyshev_distance, ChunkState, TerrainPipelineState, TerrainWorldRuntime,
};
use crate::ui::WorldTweaks;

#[derive(Component)]
pub struct GrassPatch {
    pub terrain_chunk: ChunkCoord,
    pub lod_band: u8,
}

#[derive(Component)]
struct GrassBlade;

pub struct GrassPlugin;

impl Plugin for GrassPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            sync_grass_patches.run_if(in_state(AppState::Running)),
        );
    }
}

fn sync_grass_patches(
    mut commands: Commands,
    policy: Res<LodPolicy>,
    runtime: Res<TerrainWorldRuntime>,
    world_tweaks: Res<WorldTweaks>,
    pipeline: Res<TerrainPipelineState>,
    player: Query<&Transform, With<Player>>,
    existing: Query<(Entity, &GrassPatch)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let center = runtime.interest_center;
    let decoration = world_tweaks.decoration_radius;
    let mut desired: std::collections::HashMap<ChunkCoord, u8> = std::collections::HashMap::new();

    for dz in -decoration..=decoration {
        for dy in -decoration..=decoration {
            for dx in -decoration..=decoration {
                let coord = ChunkCoord::new(center.x + dx, center.y + dy, center.z + dz);
                if chunk_chebyshev_distance(center, coord) > decoration {
                    continue;
                }
                let Some(chunk) = pipeline.chunks.get(&coord) else {
                    continue;
                };
                if chunk.state != ChunkState::Ready {
                    continue;
                }
                let chunk_center = Vec3::new(
                    coord.x as f32 * CHUNK_CELLS as f32 + CHUNK_CELLS as f32 * 0.5,
                    0.0,
                    coord.z as f32 * CHUNK_CELLS as f32 + CHUNK_CELLS as f32 * 0.5,
                );
                let dist = player_tf.translation.distance(chunk_center);
                let band = grass_lod_band(&policy, dist);
                if band >= 3 {
                    continue;
                }
                desired.insert(coord, band);
            }
        }
    }

    for (entity, patch) in &existing {
        match desired.get(&patch.terrain_chunk) {
            Some(&band) if band == patch.lod_band => {}
            _ => commands.entity(entity).despawn(),
        }
    }

    let grass_mesh = meshes.add(Cuboid::new(0.08, 0.25, 0.08));
    let grass_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.52, 0.16),
        ..default()
    });

    for (coord, band) in desired {
        if existing.iter().any(|(_, p)| p.terrain_chunk == coord && p.lod_band == band) {
            continue;
        }
        let origin = Vec3::new(
            coord.x as f32 * CHUNK_CELLS as f32,
            0.0,
            coord.z as f32 * CHUNK_CELLS as f32,
        );
        let count = match band {
            0 => 48,
            1 => 24,
            _ => 12,
        };
        commands
            .spawn((
                GrassPatch {
                    terrain_chunk: coord,
                    lod_band: band,
                },
                Transform::from_translation(origin),
                Visibility::default(),
            ))
            .with_children(|parent| {
                for i in 0..count {
                    let hash = stable_hash(coord, i);
                    let x = (hash % 100) as f32 * 0.16;
                    let z = ((hash / 100) % 100) as f32 * 0.16;
                    parent.spawn((
                        GrassBlade,
                        Mesh3d(grass_mesh.clone()),
                        MeshMaterial3d(grass_mat.clone()),
                        Transform::from_xyz(x, 0.12, z),
                    ));
                }
            });
    }
}

fn stable_hash(coord: ChunkCoord, i: u32) -> u32 {
    let mut h = (coord.x as u32)
        ^ (coord.y as u32).wrapping_mul(374761)
        ^ (coord.z as u32).wrapping_mul(668265);
    h ^= i.wrapping_mul(2246822519);
    h
}
