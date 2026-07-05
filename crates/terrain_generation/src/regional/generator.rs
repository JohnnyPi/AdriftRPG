//! Geology-aware regional residual generator.

use crate::contract::coordinates::WorldXZ;
use crate::contract::version::derive_seed;
use crate::fields::descriptor::FieldDescriptor;
use crate::fields::scalar::ScalarField;
use crate::noise::ValueNoise;

pub struct RegionalPatch {
    pub residual: ScalarField,
}

pub fn generate_patch_residual(
    patch_desc: FieldDescriptor,
    world_seed: u64,
    patch_index: u64,
    amplitude_m: f32,
    hardness: &ScalarField,
    erodibility: &ScalarField,
    coast_distance: &ScalarField,
    value_constraint: &ScalarField,
    coast_preserve_start_m: f32,
    coast_preserve_end_m: f32,
) -> RegionalPatch {
    let noise_seed = derive_seed(world_seed, "regional", None, patch_index);
    let noise = ValueNoise::new(noise_seed);
    let mut residual = ScalarField::zeros(patch_desc.clone());

    for z in 0..patch_desc.height {
        for x in 0..patch_desc.width {
            let wx = patch_desc.origin_x() + x as f64 * patch_desc.cell_size_m;
            let wz = patch_desc.origin_z() + z as f64 * patch_desc.cell_size_m;
            let world = WorldXZ::new(wx, wz);

            let h = hardness.sample_at_world(world);
            let e = erodibility.sample_at_world(world);
            let coast_d = coast_distance.sample_at_world(world);
            let constraint = value_constraint.sample_at_world(world);

            let amp = amplitude_m * (0.5 + e * 0.5) * (1.0 - h * 0.3);
            let n = noise.fbm_2d(wx as f32 * 0.0003, wz as f32 * 0.0003, 4);
            let coast_fade =
                coast_preservation(coast_d, coast_preserve_start_m, coast_preserve_end_m);
            let allowed = (1.0 - constraint).clamp(0.0, 1.0);
            let v = n * amp * coast_fade * allowed;
            residual.set(x, z, v);
        }
    }

    RegionalPatch { residual }
}

fn coast_preservation(coast_distance_m: f32, start_m: f32, end_m: f32) -> f32 {
    if end_m <= start_m {
        return 1.0;
    }
    let t = ((coast_distance_m - start_m) / (end_m - start_m)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
