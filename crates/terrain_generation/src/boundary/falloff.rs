//! Ocean basin elevation profile from boundary distance.

use game_data::CompiledBoundaryRecipe;

use crate::contract::coordinates::WorldXZ;
use crate::fields::descriptor::FieldDescriptor;
use crate::fields::scalar::ScalarField;
use crate::noise::ValueNoise;

pub fn generate_ocean_basin(
    descriptor: FieldDescriptor,
    boundary_distance: &ScalarField,
    recipe: &CompiledBoundaryRecipe,
    world_seed: u64,
) -> ScalarField {
    let half_w = descriptor.extent_x_m() * 0.5 + descriptor.origin_x().abs();
    let half_extent = half_w.max(descriptor.extent_z_m() * 0.5);
    let start = half_extent * recipe.ocean_edge_start_fraction as f64;
    let max_depth = recipe.maximum_depth_m;
    let noise = ValueNoise::new(world_seed.wrapping_add(0xBEEF));

    let mut out = ScalarField::zeros(descriptor.clone());
    for z in 0..descriptor.height {
        for x in 0..descriptor.width {
            let wx = descriptor.origin_x() + x as f64 * descriptor.cell_size_m;
            let wz = descriptor.origin_z() + z as f64 * descriptor.cell_size_m;
            let edge_d = boundary_distance.sample_at_world(WorldXZ::new(wx, wz));
            let norm = if start > 0.0 {
                (edge_d as f64 / start).clamp(0.0, 1.0) as f32
            } else {
                0.0
            };
            let depth = -max_depth * (1.0 - smoothstep01(norm));
            let variation = if recipe.variation_amplitude_m > 0.0 {
                let n = noise.sample(
                    wx as f32 * recipe.variation_frequency as f32,
                    0.0,
                    wz as f32 * recipe.variation_frequency as f32,
                );
                (n - 0.5) * 2.0 * recipe.variation_amplitude_m * (1.0 - norm)
            } else {
                0.0
            };
            out.set(x, z, depth + variation);
        }
    }
    out
}

fn smoothstep01(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
