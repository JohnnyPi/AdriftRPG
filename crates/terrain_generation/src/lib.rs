// crates/terrain_generation/src/lib.rs
//! Deterministic terrain density generation. No Bevy dependency.

mod chunk_gen;
mod density_ops;
pub mod field2d;
pub mod field_stack;
pub mod hydrology;
pub mod island_atlas;
pub mod island_gen;
pub mod noise;
pub mod resolution;
pub mod recipe;
pub mod river;
pub mod surface_height;
pub mod topology;
mod spawn;
mod traversal_tests;
pub mod water_body;
mod world_setup;

pub use chunk_gen::{
    chunk_axis_range, fill_padded_samples, generate_chunk, generate_padded_samples,
    iter_world_chunk_coords, padded_index,
};
pub use density_ops::{
    capsule_sdf, ellipsoid_sdf, plane_density, solid_subtract, solid_union, sphere_density,
};
pub use field2d::{add_residual, residual_from_absolute, Field2D, FieldTier};
pub use field_stack::{build_coast_mask, ridge_field, valley_field, FieldStackParams};
pub use hydrology::{HydrologyBackend, RiverHydrology, StubHydrology};
pub use island_atlas::{BiomeWeights, IslandAtlas};
pub use island_gen::{
    build_island_atlas, colorize_preview, colorize_preview_with_heights, colorize_runtime_preview,
    sample_atlas_surface, BeachParams, CaveParams, CoastParams, ErosionParams, HydrologyParams,
    IslandGenParams, IslandShapeParams, PREVIEW_PIXEL_SPACING_M, SurfaceNoiseParams,
    ValidationReport, VolcanoParams,
};
pub use resolution::{GenerationResolution, ResolutionError};
pub use noise::ValueNoise;
pub use recipe::{
    coastal_inland_factor, default_vertical_slice_recipe, distance_to_river_m, distance_to_water_m,
    CombineOp, RecipeDensitySource, RecipeOp, RiverCarveContext, TerrainRecipe,
};
pub use river::{
    distance_to_river_centerline, generate_river_spline, river_carve_offset, river_channel_at,
    RiverGenConfig,
};
pub use surface_height::{island_land_factor_warped, land_surface_height, CoastModifierKind};
pub use topology::{
    apply_foundation_seal, cavity_sdf_at, coastal_surface_height, count_outdoor_void_columns,
    outside_declared_cavities, CAVITY_EXTERIOR_MARGIN, FOUNDATION_DEPTH_M,
};
pub use spawn::{
    SpawnValidationReport, PLAYER_SPAWN_MIN_CLEARANCE_M, SPAWN_FLOOR_EPSILON_M,
};
pub use water_body::{
    RiverControlPoint, RiverSpline, WaterBody, WaterBodyId, WaterBodyKind, WaterBodyRegistry,
    WaterQuery, WaterSample, WaterSurfaceDefinition,
};
pub use world_setup::{
    append_generated_island_caves, build_atlas_density_source, compile_terrain_recipe,
    island_params_from_compiled,
};

pub trait DensitySource: Send + Sync {
    fn sample_density(&self, world_x: f32, world_y: f32, world_z: f32) -> f32;
}
