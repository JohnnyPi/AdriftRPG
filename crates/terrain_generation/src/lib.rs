//! Deterministic terrain density generation. No Bevy dependency.

mod chunk_gen;
mod density_ops;
pub mod field_stack;
pub mod hydrology;
pub mod noise;
pub mod recipe;
pub mod river;
pub mod topology;
pub mod traversal_tests;
pub mod vertical_slice;
pub mod water_body;

pub use chunk_gen::{chunk_axis_range, generate_chunk, generate_padded_samples, iter_world_chunk_coords, padded_index};
pub use density_ops::{capsule_sdf, ellipsoid_sdf, plane_density, solid_subtract, solid_union, sphere_density};
pub use noise::ValueNoise;
pub use recipe::{
    coastal_inland_factor, default_vertical_slice_recipe, distance_to_water_m, CombineOp,
    RecipeDensitySource, RecipeOp, RiverCarveContext, TerrainRecipe,
};
pub use topology::{
    apply_foundation_seal, cavity_sdf_at, coastal_surface_height, count_outdoor_void_columns,
    outside_declared_cavities, CAVITY_EXTERIOR_MARGIN, FOUNDATION_DEPTH_M,
};
pub use field_stack::{build_coast_mask, stack_surface_height, FieldStackParams, TerrainMask};
pub use river::{
    distance_to_river_centerline, generate_river_spline, river_carve_offset, river_channel_at,
    RiverGenConfig,
};
pub use water_body::{
    RiverControlPoint, RiverSpline, WaterBody, WaterBodyId, WaterBodyKind, WaterBodyRegistry,
    WaterQuery, WaterSample, WaterSurfaceDefinition,
};
pub use vertical_slice::VerticalSliceDensitySource;
pub use hydrology::{HydrologyBackend, RiverHydrology, StubHydrology};

pub trait DensitySource: Send + Sync {
    fn sample_density(&self, world_x: f32, world_y: f32, world_z: f32) -> f32;
}
