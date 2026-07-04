// crates/terrain_generation/src/lib.rs
//! Deterministic terrain density generation. No Bevy dependency.

pub mod atlas_bake;
mod chunk_gen;
mod density_ops;
pub mod field2d;
pub mod field_stack;
pub mod hydrology;
pub mod island_atlas;
pub mod island_gen;
pub mod noise;
pub mod recipe;
pub mod resolution;
pub mod river;
mod spawn;
pub mod surface_height;
pub mod topology;
mod traversal_tests;
pub mod water_body;
mod world_setup;

pub use atlas_bake::{
    ATLAS_BAKE_SCHEMA_VERSION, AtlasBakeError, AtlasBakeManifest, atlas_content_hash,
    load_baked_atlas, resolve_baked_atlas_path, try_load_baked_atlas, write_baked_atlas,
};
pub use chunk_gen::{
    chunk_axis_range, fill_padded_samples, generate_chunk, generate_padded_samples,
    iter_world_chunk_coords, padded_index,
};
pub use density_ops::{
    capsule_sdf, ellipsoid_sdf, plane_density, solid_subtract, solid_union, sphere_density,
};
pub use field_stack::{FieldStackParams, build_coast_mask, ridge_field, valley_field};
pub use field2d::{Field2D, FieldTier, add_residual, residual_from_absolute};
pub use hydrology::{HydrologyBackend, RiverHydrology};
pub use island_atlas::{BiomeWeights, IslandAtlas};
pub use island_gen::{
    BeachParams, CaveParams, CoastParams, ErosionParams, HydrologyParams, IslandGenParams,
    IslandShapeParams, PREVIEW_OUTPUT_MAX, PREVIEW_OUTPUT_MIN, PREVIEW_PIXEL_SPACING_M,
    SurfaceNoiseParams, ValidationReport, VolcanoParams, build_island_atlas,
    clamp_preview_output_side, colorize_preview, colorize_preview_with_heights,
    colorize_runtime_preview, min_peak_elevation_m, preview_grid_for_atlas, sample_atlas_surface,
};
pub use noise::ValueNoise;
pub use recipe::{
    CombineOp, RecipeDensitySource, RecipeOp, RiverCarveContext, TerrainRecipe, WorldVolumeBounds,
    coastal_inland_factor, default_vertical_slice_recipe, distance_to_river_m, distance_to_water_m,
};
pub use resolution::{GenerationResolution, ResolutionError};
pub use river::{
    RiverGenConfig, distance_to_river_centerline, generate_river_spline, river_carve_offset,
    river_channel_at,
};
pub use spawn::{PLAYER_SPAWN_MIN_CLEARANCE_M, SPAWN_FLOOR_EPSILON_M, SpawnValidationReport};
pub use surface_height::{CoastModifierKind, island_land_factor_warped, land_surface_height};
pub use topology::{
    CAVITY_EXTERIOR_MARGIN, FOUNDATION_DEPTH_M, apply_foundation_seal, cavity_sdf_at,
    coastal_surface_height, count_outdoor_void_columns, outside_declared_cavities,
};
pub use water_body::{
    HorizontalFootprint, RiverControlPoint, RiverSpline, WaterBody, WaterBodyId, WaterBodyKind,
    WaterBodyRegistry, WaterQuery, WaterSample, WaterSurfaceDefinition,
};
pub use world_setup::{
    WorldSetupError, build_atlas_density_source, build_atlas_density_source_for_world,
    compile_terrain_recipe, compile_terrain_recipe_with_island, effective_sea_level_m,
    island_params_from_compiled, resolve_island_atlas, validate_island_world_budget,
};

pub trait DensitySource: Send + Sync {
    fn sample_density(&self, world_x: f32, world_y: f32, world_z: f32) -> f32;
}
