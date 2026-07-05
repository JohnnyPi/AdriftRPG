//! Procedural grass patches — cluster mesh per terrain chunk with distance LOD.
//!
//! Phase 1 scaffold (see `ProceduralGrass`): one parent entity per chunk/band with
//! CPU-generated blade children. Future phases add instancing, wind shaders, and
//! suitability filtering from the terrain surface.

use bevy::prelude::*;
use std::collections::HashMap;
use voxel_core::{CHUNK_CELLS, ChunkCoord};

use crate::lod::{LodPolicy, grass_lod_band};
use crate::player::Player;
use crate::state::AppState;
use crate::terrain::{
    ChunkState, TerrainPipelineState, TerrainWorldRuntime, chunk_world_center,
    residency::within_decoration_radius,
};
use crate::ui::WorldTweaks;

/// One grass cluster owned by a terrain chunk at a specific distance LOD band.
#[derive(Component, Clone, Debug)]
pub struct GrassPatch {
    pub terrain_chunk: ChunkCoord,
    /// Distance band from [`grass_lod_band`] (0 = near, 1 = mid, 2 = far).
    pub lod_band: u8,
    /// Blade instances in this patch (Phase 1 cluster mesh count).
    pub instance_count: u32,
}

#[derive(Component)]
struct GrassBlade;

#[derive(Resource)]
struct GrassRenderAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct GrassPatchIndex(HashMap<(i32, i32, i32, u8), Entity>);

pub struct GrassPlugin;

impl Plugin for GrassPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GrassPatchIndex>()
            .add_systems(Startup, init_grass_render_assets)
            .add_systems(
                Update,
                (cleanup_stale_grass_patches, sync_grass_patches)
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn init_grass_render_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(GrassRenderAssets {
        mesh: meshes.add(Cuboid::new(0.08, 0.25, 0.08)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.52, 0.16),
            ..default()
        }),
    });
}

/// Despawn patches whose chunk left residency or is no longer terrain-ready.
fn cleanup_stale_grass_patches(
    mut commands: Commands,
    runtime: Res<TerrainWorldRuntime>,
    world_tweaks: Res<WorldTweaks>,
    pipeline: Res<TerrainPipelineState>,
    patches: Query<(Entity, &GrassPatch)>,
    mut patch_index: ResMut<GrassPatchIndex>,
) {
    let center = runtime.interest_center;
    for (entity, patch) in &patches {
        let stale = !within_decoration_radius(center, patch.terrain_chunk, &world_tweaks)
            || pipeline
                .chunks
                .get(&patch.terrain_chunk)
                .is_none_or(|chunk| chunk.state != ChunkState::Ready || chunk.entity.is_none());
        if !stale {
            continue;
        }
        let key = (
            patch.terrain_chunk.x,
            patch.terrain_chunk.y,
            patch.terrain_chunk.z,
            patch.lod_band,
        );
        patch_index.0.remove(&key);
        commands.entity(entity).despawn();
    }
}

fn sync_grass_patches(
    mut commands: Commands,
    policy: Res<LodPolicy>,
    runtime: Res<TerrainWorldRuntime>,
    world_tweaks: Res<WorldTweaks>,
    pipeline: Res<TerrainPipelineState>,
    player: Query<&Transform, With<Player>>,
    grass_assets: Res<GrassRenderAssets>,
    mut patch_index: ResMut<GrassPatchIndex>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let center = runtime.interest_center;
    let decoration = world_tweaks.decoration_radius;
    let cell_size_m = runtime.cell_size_m;
    let mut desired: HashMap<(i32, i32, i32, u8), ChunkCoord> = HashMap::new();

    for dz in -decoration..=decoration {
        for dy in -decoration..=decoration {
            for dx in -decoration..=decoration {
                let coord = ChunkCoord::new(center.x + dx, center.y + dy, center.z + dz);
                if !within_decoration_radius(center, coord, &world_tweaks) {
                    continue;
                }
                let Some(chunk) = pipeline.chunks.get(&coord) else {
                    continue;
                };
                if chunk.state != ChunkState::Ready || chunk.entity.is_none() {
                    continue;
                }
                let chunk_center = chunk_world_center(coord, cell_size_m);
                let dist = player_tf.translation.distance(chunk_center);
                let band = grass_lod_band(&policy, dist);
                if band >= 3 {
                    continue;
                }
                desired.insert((coord.x, coord.y, coord.z, band), coord);
            }
        }
    }

    patch_index.0.retain(|key, entity| {
        if desired.contains_key(key) {
            return true;
        }
        commands.entity(*entity).despawn();
        false
    });

    for ((cx, cy, cz, band), coord) in desired {
        let key = (cx, cy, cz, band);
        if patch_index.0.contains_key(&key) {
            continue;
        }
        let extent = CHUNK_CELLS as f32 * cell_size_m;
        let origin = Vec3::new(coord.x as f32 * extent, 0.0, coord.z as f32 * extent);
        let instance_count = grass_blade_count_for_band(band);
        let entity = commands
            .spawn((
                GrassPatch {
                    terrain_chunk: coord,
                    lod_band: band,
                    instance_count,
                },
                Transform::from_translation(origin),
                Visibility::default(),
            ))
            .with_children(|parent| {
                for i in 0..instance_count {
                    let hash = stable_hash(coord, i);
                    let x = (hash % 100) as f32 * 0.16;
                    let z = ((hash / 100) % 100) as f32 * 0.16;
                    parent.spawn((
                        GrassBlade,
                        Mesh3d(grass_assets.mesh.clone()),
                        MeshMaterial3d(grass_assets.material.clone()),
                        Transform::from_xyz(x, 0.12, z),
                    ));
                }
            })
            .id();
        patch_index.0.insert(key, entity);
    }
}

fn grass_blade_count_for_band(band: u8) -> u32 {
    match band {
        0 => 48,
        1 => 24,
        _ => 12,
    }
}

fn stable_hash(coord: ChunkCoord, i: u32) -> u32 {
    let mut h = (coord.x as u32)
        ^ (coord.y as u32).wrapping_mul(374761)
        ^ (coord.z as u32).wrapping_mul(668265);
    h ^= i.wrapping_mul(2246822519);
    h
}
