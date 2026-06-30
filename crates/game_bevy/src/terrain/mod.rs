mod editing;
mod features;
mod mesh_convert;
mod material;
mod metrics;
mod pipeline;
mod recipe;
mod residency;

pub use editing::TerrainEditingPlugin;
pub use features::{CameraWaterState, TerrainFeaturePlugin, TerrainFeatureRegistry};
/// VS2 §20 — river spline generation lives in [`TerrainFeaturePlugin`].
#[allow(dead_code)]
pub type RiverGenerationPlugin = TerrainFeaturePlugin;
/// VS2 §20 — water occupancy registry lives in [`TerrainFeaturePlugin`].
#[allow(dead_code)]
pub type WaterBodyPlugin = TerrainFeaturePlugin;
pub use material::{TerrainMaterialHandle, TerrainMaterialPlugin, TerrainTriplanarMaterial};
pub use metrics::{TerrainPipelineMetrics, WorldSeedOverride};
#[cfg(test)]
pub use recipe::build_density_source;
pub use pipeline::{
    regen_terrain_with_seed, TerrainPipelineState, TerrainPlugin, TerrainRecipeRevision,
    TerrainRegenPending, TerrainSpawnPoint, TerrainWorldInitSet,
};
pub use residency::{
    draw_residency_rings, world_position_in_decoration_radius, world_position_in_high_detail_radius,
    ChunkResidencyPlugin, TerrainWorldRuntime,
};

use bevy::prelude::*;
use voxel_core::ChunkCoord;

/// Bevy resource wrapping the voxel edit overlay.
#[derive(Resource, Clone, Debug, Default)]
pub struct TerrainEditStore(pub voxel_core::TerrainEditStore);

impl TerrainEditStore {
    pub fn clear(&mut self) {
        self.0 = voxel_core::TerrainEditStore::new();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChunkState {
    #[default]
    Unrequested,
    GeneratingDensity,
    Meshing,
    AwaitingUpload,
    Ready,
    Failed,
}

#[derive(Component, Debug)]
pub struct TerrainChunkEntity {
    pub coord: ChunkCoord,
}

#[derive(Resource, Debug)]
pub struct TerrainRevision {
    pub value: u64,
}

impl Default for TerrainRevision {
    fn default() -> Self {
        Self { value: 1 }
    }
}
