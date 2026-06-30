mod editing;
mod mesh_convert;
mod material;
mod metrics;
mod pipeline;
mod recipe;

pub use editing::TerrainEditingPlugin;
pub use material::TerrainMaterialPlugin;
pub use metrics::{TerrainPipelineMetrics, WorldSeedOverride};
pub use pipeline::{
    regen_terrain_with_seed, TerrainPipelineState, TerrainPlugin, TerrainRecipeRevision,
    TerrainSpawnPoint,
};

use bevy::prelude::*;
use voxel_core::ChunkCoord;

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
