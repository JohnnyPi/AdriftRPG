// crates/terrain_generation/src/lib.rs
//! Deterministic terrain density generation. No Bevy dependency.

pub mod atlas_bake;
pub mod biomes;
pub mod boundary;
pub mod caves;
mod chunk_gen;
pub mod climate;
pub mod coast;
pub mod compiler;
pub mod contract;
mod density_ops;
pub mod diagnostics;
pub mod erosion;
pub mod field2d;
pub mod field_stack;
pub mod fields;
pub mod geology;
pub mod hydrology;
pub mod island_atlas;
pub mod island_gen;
pub mod islands;
pub mod macro_terrain;
pub mod noise;
pub mod provider;
pub mod recipe;
pub mod regional;
pub mod resolution;
pub mod river;
pub mod soil;
mod spawn;
pub mod strata;
pub mod surface_height;
pub mod topology;
mod traversal_tests;
pub mod water_body;
pub mod world;
mod world_setup;

pub use atlas_bake::{
    ATLAS_BAKE_SCHEMA_VERSION, AtlasBakeError, AtlasBakeManifest, atlas_content_hash,
    load_baked_atlas, resolve_baked_atlas_path, try_load_baked_atlas, write_baked_atlas,
};
pub use biomes::BiomePass;
pub use biomes::id::{BiomeBlendCell, CompilerBiomeId};
pub use caves::{
    CaveFamily, CaveGraphRegistry, CaveNodeKind, CavePass, CaveSubtractOps, CaveSystem,
};
pub use chunk_gen::{
    chunk_axis_range, fill_padded_samples, generate_chunk, generate_padded_samples,
    iter_world_chunk_coords, padded_index,
};
pub use climate::ClimatePass;
pub use coast::CoastPass;
pub use compiler::{
    CompileOptions, CompileStage, CompiledWorld, DefaultWorldCompiler, WorldCompiler,
    WorldgenError, compile_world_from_bundle, compile_world_from_recipe,
};
pub use contract::{
    CellSizeMeters, ColumnSample, ElevationMeters, GENERATOR_VERSION, GeologySample, RecipeHash,
    SurfaceSample, TileCoord, WorldDensityProvider, WorldExtent, WorldManifest, WorldMetadata,
    WorldPosition, WorldXZ, derive_seed, grid_cell_to_world, surface_density, world_to_grid_coords,
};
pub use density_ops::{
    capsule_sdf, ellipsoid_sdf, plane_density, solid_subtract, solid_union, sphere_density,
};
pub use erosion::ErosionPass;
pub use field_stack::{FieldStackParams, build_coast_mask, ridge_field, valley_field};
pub use field2d::{Field2D, FieldTier, add_residual, residual_from_absolute};
pub use fields::{FieldDescriptor, FieldKey, FieldRegistry, ScalarField};
pub use hydrology::{
    CompiledHydrologyProducts, HydrologyBackend, HydrologyFinalizePass, HydrologyGraph,
    HydrologyPass, RiverHydrology, WaterCarvePass,
};
pub use island_atlas::{BiomeWeights, IslandAtlas};
pub use island_gen::{
    BeachParams, CaveParams, CoastParams, ErosionParams, HydrologyParams, IslandGenParams,
    IslandShapeParams, PREVIEW_OUTPUT_MAX, PREVIEW_OUTPUT_MIN, PREVIEW_PIXEL_SPACING_M,
    SurfaceNoiseParams, ValidationReport, VolcanoParams, build_island_atlas,
    clamp_preview_output_side, colorize_preview, colorize_preview_with_heights,
    colorize_runtime_preview, min_peak_elevation_m, preview_grid_for_atlas, sample_atlas_surface,
};
pub use noise::ValueNoise;
pub use provider::{AtlasWorldProvider, RecipeDensityProviderAdapter, VolumetricWorldProvider};
pub use recipe::{
    CombineOp, RecipeDensitySource, RecipeOp, RiverCarveContext, TerrainRecipe, WorldVolumeBounds,
    coastal_inland_factor, default_vertical_slice_recipe, distance_to_river_m, distance_to_water_m,
};
pub use resolution::{GenerationResolution, ResolutionError};
pub use river::{
    RiverGenConfig, distance_to_river_centerline, generate_river_spline, river_carve_offset,
    river_channel_at,
};
pub use soil::SoilPass;
pub use spawn::{
    PLAYER_SPAWN_MIN_CLEARANCE_M, SPAWN_FLOOR_EPSILON_M, SpawnValidationReport,
    resolve_spawn_from_provider,
};
pub use strata::StrataPass;
pub use strata::material::StrataMaterialId;
pub use surface_height::{CoastModifierKind, island_land_factor_warped, land_surface_height};
pub use topology::{
    CAVITY_EXTERIOR_MARGIN, FOUNDATION_DEPTH_M, apply_foundation_seal, cavity_sdf_at,
    coastal_surface_height, count_outdoor_void_columns, outside_declared_cavities,
};
pub use water_body::{
    HorizontalFootprint, RiverControlPoint, RiverSpline, WaterBody, WaterBodyId, WaterBodyKind,
    WaterBodyRegistry, WaterQuery, WaterSample, WaterSurfaceDefinition,
};
pub use world::{GraphRegistry, WorldAtlas};
pub use world_setup::{
    WorldSetupError, build_atlas_density_source, build_atlas_density_source_for_world,
    compile_terrain_recipe, compile_terrain_recipe_with_island, effective_sea_level_m,
    island_params_from_compiled, resolve_island_atlas, validate_island_world_budget,
};

pub trait DensitySource: Send + Sync {
    fn sample_density(&self, world_x: f32, world_y: f32, world_z: f32) -> f32;
}

/// Bridge DensitySource to WorldDensityProvider for incremental migration.
pub struct DensitySourceAdapter<D: DensitySource> {
    inner: D,
    metadata: WorldMetadata,
    sea_level_m: f32,
}

impl<D: DensitySource + 'static> DensitySourceAdapter<D> {
    pub fn new(inner: D, metadata: WorldMetadata, sea_level_m: f32) -> Self {
        Self {
            inner,
            metadata,
            sea_level_m,
        }
    }

    pub fn into_provider(self) -> std::sync::Arc<dyn WorldDensityProvider> {
        std::sync::Arc::new(self)
    }
}

impl<D: DensitySource + Send + Sync + 'static> WorldDensityProvider for DensitySourceAdapter<D> {
    fn world_metadata(&self) -> &WorldMetadata {
        &self.metadata
    }

    fn sample_density(&self, position: WorldPosition) -> f32 {
        self.inner.sample_density(
            position.0.x as f32,
            position.0.y as f32,
            position.0.z as f32,
        )
    }

    fn sample_surface(&self, horizontal: WorldXZ) -> SurfaceSample {
        let mut lo = self.sea_level_m - 500.0;
        let mut hi = self.sea_level_m + 3000.0;
        for _ in 0..24 {
            let mid = (lo + hi) * 0.5;
            let d = self.sample_density(WorldPosition::new(
                horizontal.x(),
                mid as f64,
                horizontal.z(),
            ));
            if d < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let elevation_m = (lo + hi) * 0.5;
        SurfaceSample {
            elevation_m,
            slope: 0.0,
            macro_normal: glam::Vec3::Y,
            land_mask: if elevation_m > self.sea_level_m {
                1.0
            } else {
                0.0
            },
            coast_distance_m: 0.0,
            island_id: Some(0),
        }
    }

    fn sample_geology(&self, _position: WorldPosition) -> GeologySample {
        GeologySample::default()
    }

    fn sample_column(&self, horizontal: WorldXZ) -> ColumnSample {
        ColumnSample {
            surface: self.sample_surface(horizontal),
            ..Default::default()
        }
    }
}
