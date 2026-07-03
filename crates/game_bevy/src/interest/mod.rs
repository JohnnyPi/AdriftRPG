//! Pluggable chunk interest provider (player + velocity prefetch).

use bevy::prelude::*;
use std::collections::BTreeSet;
use voxel_core::{
    ChunkCoord, ChunkInterestProvider, SimulationLodProvider,
};

use crate::lod::LodPolicy;
use crate::staging::InterestVelocity;
use crate::terrain::{residency::chunk_chebyshev_distance, TerrainWorldRuntime};

pub struct ChunkInterestPlugin;

impl Plugin for ChunkInterestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerChunkInterestProvider>()
            .init_resource::<WorldSimulationLodProvider>()
            .add_systems(Update, sync_interest_provider);
    }
}

#[derive(Resource, Default, Debug, Clone)]
pub struct PlayerChunkInterestProvider {
    pub desired: BTreeSet<ChunkCoord>,
}

impl ChunkInterestProvider for PlayerChunkInterestProvider {
    fn desired_chunks(&self) -> BTreeSet<ChunkCoord> {
        self.desired.clone()
    }
}
impl PlayerChunkInterestProvider {
    pub fn compute(
        center: ChunkCoord,
        policy: &LodPolicy,
        velocity: &InterestVelocity,
    ) -> BTreeSet<ChunkCoord> {
        let mut out = BTreeSet::new();
        let radius = policy.density_radius;
        for dz in -radius..=radius {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let coord = ChunkCoord::new(center.x + dx, center.y + dy, center.z + dz);
                    if chunk_chebyshev_distance(center, coord) <= radius {
                        out.insert(coord);
                    }
                }
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
                        if chunk_chebyshev_distance(center, coord) <= radius {
                            out.insert(coord);
                        }
                    }
                }
            }
        }

        out
    }
}

fn sync_interest_provider(
    runtime: Res<TerrainWorldRuntime>,
    policy: Res<LodPolicy>,
    velocity: Res<InterestVelocity>,
    mut provider: ResMut<PlayerChunkInterestProvider>,
    mut sim_lod: ResMut<WorldSimulationLodProvider>,
) {
    provider.desired =
        PlayerChunkInterestProvider::compute(runtime.interest_center, &policy, &velocity);
    sim_lod.sync_policy(&policy);
}

/// Simulation LOD: full density inside policy radius, none outside.
#[derive(Resource, Default, Debug, Clone)]
pub struct WorldSimulationLodProvider {
    pub detail_radius: i32,
}

impl SimulationLodProvider for WorldSimulationLodProvider {
    fn detail_radius_chunks(&self) -> i32 {
        self.detail_radius.max(1)
    }
}

impl WorldSimulationLodProvider {
    pub fn sync_policy(&mut self, policy: &LodPolicy) {
        self.detail_radius = policy.density_radius;
    }

    pub fn should_simulate_chunk(
        center: ChunkCoord,
        coord: ChunkCoord,
        policy: &LodPolicy,
    ) -> bool {
        chunk_chebyshev_distance(center, coord) <= policy.density_radius
    }
}
