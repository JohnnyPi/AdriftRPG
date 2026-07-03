// crates/game_bevy/src/terrain/mod.rs
mod editing;
mod features;
mod island_params;
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
pub use island_params::island_params_from_compiled;
pub use material::{TerrainMaterialHandle, TerrainMaterialPlugin};
pub use metrics::{TerrainPipelineMetrics, WorldSeedOverride};
pub use recipe::build_density_source_from_prefs;
pub use terrain_generation::compile_terrain_recipe;
#[cfg(test)]
pub(crate) use recipe::build_density_source;
pub use pipeline::{
    regen_terrain_with_seed, TerrainPipelineState, TerrainPlugin, TerrainRecipeRevision,
    TerrainRegenPending, TerrainSpawnPoint, TerrainWorldInitSet,
};
pub use residency::{
    draw_residency_rings, spawn_terrain_collider_ready, spawn_terrain_uploaded,
    world_position_in_decoration_radius, world_position_in_high_detail_radius,
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

/// Per-chunk material slot palette used to refresh procedural textures after bake.
#[derive(Component, Debug, Clone, Copy)]
pub struct TerrainChunkPalette(pub terrain_surface::ChunkSlotPalette);

/// Marker for chunks that own a per-chunk material instance (chunk-local slot
/// remap uniforms baked into its `MeshMaterial3d` handle).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct TerrainChunkMaterial;

#[derive(Resource, Debug)]
pub struct TerrainRevision {
    pub value: u64,
}

impl Default for TerrainRevision {
    fn default() -> Self {
        Self { value: 1 }
    }
}
