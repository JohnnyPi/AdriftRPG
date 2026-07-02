// crates/game_bevy/src/terrain/metrics.rs
use bevy::prelude::*;

/// Rolling pipeline stage timings and queue depths for the F1 debug panel.
#[derive(Resource, Clone, Debug, Default)]
pub struct TerrainPipelineMetrics {
    pub last_density_ms: f32,
    pub last_mesh_ms: f32,
    pub last_upload_ms: f32,
    pub density_queue: usize,
    pub mesh_queue: usize,
    pub upload_queue: usize,
    pub collider_queue: usize,
    pub colliders_built_this_frame: u32,
}

impl TerrainPipelineMetrics {
    pub fn record_density_ms(&mut self, ms: f32) {
        self.last_density_ms = ms;
    }

    pub fn record_mesh_ms(&mut self, ms: f32) {
        self.last_mesh_ms = ms;
    }

    pub fn record_upload_ms(&mut self, ms: f32) {
        self.last_upload_ms = ms;
    }

    pub fn within_vs_budget(&self, chunk_count: usize) -> bool {
        crate::performance::terrain_pipeline_within_budget(
            self.last_density_ms,
            self.last_mesh_ms,
            chunk_count,
        )
    }
}

/// Overrides the procedural world seed (F9 increments, F8 keeps current).
#[derive(Resource, Clone, Debug)]
pub struct WorldSeedOverride {
    pub seed: u64,
}

impl Default for WorldSeedOverride {
    fn default() -> Self {
        Self { seed: 0 }
    }
}
