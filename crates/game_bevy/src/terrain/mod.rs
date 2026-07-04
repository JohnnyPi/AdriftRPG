// crates/game_bevy/src/terrain/mod.rs
mod editing;
mod features;
mod horizon_skirt;
mod material;
mod mesh_convert;
mod metrics;
mod pipeline;
mod recipe;
pub mod residency;

pub use editing::TerrainEditingPlugin;
pub use features::{
    CameraWaterState, TerrainFeaturePlugin, TerrainFeatureRegistry, effective_runtime_sea_level_m,
};
pub use material::{TerrainMaterialHandle, TerrainMaterialPlugin, queue_compiled_palette_reload};
pub use mesh_convert::{globalize_material_vertices, insert_terrain_material_attributes};
pub use metrics::{TerrainPipelineMetrics, WorldSeedOverride};
pub use recipe::{build_density_source, build_density_source_from_prefs};
pub use terrain_generation::compile_terrain_recipe;

/// Cheap probe that the terrain recipe (and optional atlas) can be built without user prefs.
pub fn validate_density_source_buildable(
    registry: &game_data::ConfigRegistry,
    world_id: Option<&shared::StableId>,
    seed_override: Option<u64>,
    field_stack: terrain_generation::FieldStackParams,
) -> Result<(), String> {
    let source = build_density_source(registry, world_id, seed_override, field_stack);
    let _ = source.recipe();
    if let Some(atlas) = source.atlas() {
        if !atlas.validation_passed {
            return Err(format!(
                "island atlas validation failed: {}",
                atlas.validation_messages.join("; ")
            ));
        }
    }
    Ok(())
}
pub use pipeline::{
    TerrainPipelineState, TerrainPlugin, TerrainRecipeRevision, TerrainRegenPending,
    TerrainSpawnPoint, TerrainWorldInitSet, regen_terrain_with_seed,
};
pub use residency::{
    ChunkResidencyPlugin, TerrainWorldRuntime, chunk_world_center, draw_residency_rings,
    spawn_terrain_collider_ready, spawn_terrain_uploaded, world_position_in_decoration_radius,
    world_position_in_high_detail_radius,
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
    /// Density samples cached; mesh only when inside render radius.
    DensityReady,
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
