//! Deterministic terrain density generation. No Bevy dependency.

mod chunk_gen;
mod density_ops;
mod noise;
pub mod recipe;
pub mod vertical_slice;

pub use chunk_gen::{chunk_axis_range, generate_chunk, generate_padded_samples, iter_world_chunk_coords, padded_index};
pub use density_ops::{capsule_sdf, ellipsoid_sdf, plane_density, solid_subtract, solid_union, sphere_density};
pub use recipe::{
    default_vertical_slice_recipe, CombineOp, RecipeDensitySource, RecipeOp, TerrainRecipe,
};
pub use vertical_slice::VerticalSliceDensitySource;

pub trait DensitySource: Send + Sync {
    fn sample_density(&self, world_x: f32, world_y: f32, world_z: f32) -> f32;
}
