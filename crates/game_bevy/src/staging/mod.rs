//! Asset preload/staging queue for LOD transitions.

use bevy::prelude::*;
use std::collections::VecDeque;
use voxel_core::ChunkCoord;

use crate::data::{ConfigRegistryResource, UserSetupPrefs};
use crate::lod::LodPolicy;
use crate::state::AppState;
use crate::terrain::{
    TerrainPipelineState, TerrainSpawnPoint,
    residency::{chunk_chebyshev_distance, spawn_terrain_collider_ready, spawn_terrain_uploaded},
};

pub struct StagingPlugin;

impl Plugin for StagingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssetStagingQueue>()
            .init_resource::<StagingGate>()
            .init_resource::<InterestVelocity>()
            .add_systems(
                Update,
                (
                    track_interest_velocity,
                    update_staging_queue,
                    update_staging_gate,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct InterestVelocity {
    pub chunk_delta: IVec3,
    pub last_center: Option<ChunkCoord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StagingPriority {
    Background = 0,
    Prefetch = 1,
    Immediate = 2,
    Critical = 3,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagingJobKind {
    AtlasLoad,
    MaterialArrayBake,
    DensityJob { coord: ChunkCoord },
    MeshJob { coord: ChunkCoord },
    VegetationPatch { coord: ChunkCoord },
    WaterTile { coord: ChunkCoord },
}

#[derive(Clone, Debug)]
pub struct StagingJob {
    pub priority: StagingPriority,
    pub kind: StagingJobKind,
}

#[derive(Resource, Default, Debug)]
pub struct AssetStagingQueue {
    pub pending: VecDeque<StagingJob>,
    pub enqueued_this_frame: u32,
}

impl AssetStagingQueue {
    pub fn enqueue(&mut self, job: StagingJob) {
        if self
            .pending
            .iter()
            .any(|existing| existing.kind == job.kind)
        {
            return;
        }
        let insert_idx = self
            .pending
            .iter()
            .position(|existing| existing.priority < job.priority)
            .unwrap_or(self.pending.len());
        self.pending.insert(insert_idx, job);
        self.enqueued_this_frame += 1;
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

/// Blocks player spawn until critical staging jobs complete.
#[derive(Resource, Debug, Clone)]
pub struct StagingGate {
    pub spawn_allowed: bool,
    pub waiting_for: Vec<String>,
}

impl Default for StagingGate {
    fn default() -> Self {
        Self {
            spawn_allowed: false,
            waiting_for: vec!["atlas".into(), "spawn_chunk".into()],
        }
    }
}

fn track_interest_velocity(
    runtime: Res<crate::terrain::TerrainWorldRuntime>,
    mut velocity: ResMut<InterestVelocity>,
) {
    let center = runtime.interest_center;
    if let Some(last) = velocity.last_center {
        velocity.chunk_delta = IVec3::new(center.x - last.x, center.y - last.y, center.z - last.z);
    }
    velocity.last_center = Some(center);
}

fn update_staging_queue(
    mut queue: ResMut<AssetStagingQueue>,
    policy: Res<LodPolicy>,
    runtime: Res<crate::terrain::TerrainWorldRuntime>,
    velocity: Res<InterestVelocity>,
    pipeline: Res<TerrainPipelineState>,
    _spawn_point: Res<TerrainSpawnPoint>,
) {
    queue.enqueued_this_frame = 0;
    let center = runtime.interest_center;

    if policy.preload_atlas && pipeline.density_source.is_none() {
        queue.enqueue(StagingJob {
            priority: StagingPriority::Critical,
            kind: StagingJobKind::AtlasLoad,
        });
    }

    if let Some(spawn_chunk) = pipeline.spawn_chunk {
        if !pipeline.has_density_cached(spawn_chunk) {
            queue.enqueue(StagingJob {
                priority: StagingPriority::Critical,
                kind: StagingJobKind::DensityJob { coord: spawn_chunk },
            });
        }
        if !spawn_terrain_uploaded(&pipeline, spawn_chunk) {
            queue.enqueue(StagingJob {
                priority: StagingPriority::Critical,
                kind: StagingJobKind::MeshJob { coord: spawn_chunk },
            });
        }
    }

    let prefetch = policy.prefetch_chunks_ahead.max(0);
    if prefetch > 0 && velocity.chunk_delta != IVec3::ZERO {
        let ahead = ChunkCoord::new(
            center.x + velocity.chunk_delta.x * prefetch,
            center.y + velocity.chunk_delta.y * prefetch,
            center.z + velocity.chunk_delta.z * prefetch,
        );
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let coord = ChunkCoord::new(ahead.x + dx, ahead.y + dy, ahead.z + dz);
                    if chunk_chebyshev_distance(center, coord) > policy.density_radius {
                        continue;
                    }
                    if !pipeline.has_density_cached(coord) {
                        queue.enqueue(StagingJob {
                            priority: StagingPriority::Prefetch,
                            kind: StagingJobKind::DensityJob { coord },
                        });
                    }
                }
            }
        }
    }

    for dz in -policy.render_radius..=policy.render_radius {
        for dy in -policy.render_radius..=policy.render_radius {
            for dx in -policy.render_radius..=policy.render_radius {
                let coord = ChunkCoord::new(center.x + dx, center.y + dy, center.z + dz);
                if chunk_chebyshev_distance(center, coord) > policy.render_radius {
                    continue;
                }
                if !pipeline.has_density_cached(coord) {
                    queue.enqueue(StagingJob {
                        priority: StagingPriority::Immediate,
                        kind: StagingJobKind::DensityJob { coord },
                    });
                }
            }
        }
    }

    if policy.preload_material_arrays {
        queue.enqueue(StagingJob {
            priority: StagingPriority::Background,
            kind: StagingJobKind::MaterialArrayBake,
        });
    }

    for dz in -policy.decoration_radius..=policy.decoration_radius {
        for dy in -policy.decoration_radius..=policy.decoration_radius {
            for dx in -policy.decoration_radius..=policy.decoration_radius {
                let coord = ChunkCoord::new(center.x + dx, center.y + dy, center.z + dz);
                if chunk_chebyshev_distance(center, coord) > policy.decoration_radius {
                    continue;
                }
                queue.enqueue(StagingJob {
                    priority: StagingPriority::Prefetch,
                    kind: StagingJobKind::VegetationPatch { coord },
                });
            }
        }
    }

    let ocean_snap = ChunkCoord::new(center.x, center.y, center.z);
    queue.enqueue(StagingJob {
        priority: StagingPriority::Background,
        kind: StagingJobKind::WaterTile { coord: ocean_snap },
    });
}

fn update_staging_gate(
    mut gate: ResMut<StagingGate>,
    pipeline: Res<TerrainPipelineState>,
    _spawn_point: Res<TerrainSpawnPoint>,
    colliders: Query<Entity, With<avian3d::prelude::Collider>>,
    _registry: Res<ConfigRegistryResource>,
    _prefs: Res<UserSetupPrefs>,
) {
    let mut waiting = Vec::new();

    if pipeline.density_source.is_none() {
        waiting.push("atlas".into());
    }

    let spawn_chunk = pipeline.spawn_chunk;
    if let Some(spawn) = spawn_chunk {
        if !spawn_terrain_uploaded(&pipeline, spawn) {
            waiting.push("spawn_mesh".into());
        }
        if !spawn_terrain_collider_ready(&pipeline, spawn, &colliders) {
            waiting.push("spawn_collider".into());
        }
    } else if pipeline.density_source.is_some() {
        waiting.push("spawn_chunk".into());
    }

    gate.waiting_for = waiting;
    gate.spawn_allowed = gate.waiting_for.is_empty();
}
