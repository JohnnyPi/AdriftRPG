//! Beach berm elevation shaping.

use game_data::CompiledCoastRecipe;

use crate::contract::derive_seed;
use crate::fields::scalar::ScalarField;
use crate::noise::ValueNoise;

fn range_mix(min: f32, max: f32, t: f32) -> f32 {
    min + (max - min) * t.clamp(0.0, 1.0)
}

fn seeded_unit(noise: &ValueNoise, wx: f32, wz: f32) -> f32 {
    noise.sample(wx, 0.0, wz).rem_euclid(1.0)
}

pub fn apply_beach_berms(
    elevation: &mut ScalarField,
    beach: &ScalarField,
    coast_distance: &ScalarField,
    land_mask: &ScalarField,
    recipe: &CompiledCoastRecipe,
    sea_level_m: f32,
    world_seed: u64,
) {
    let berm_noise = ValueNoise::new(derive_seed(world_seed, "coast_berm", None, 0xBEAC_0001));
    let spacing = elevation.descriptor.cell_size_m as f32;

    for z in 0..elevation.descriptor.height {
        for x in 0..elevation.descriptor.width {
            if land_mask.get(x, z) < 0.3 {
                continue;
            }
            let mask = beach.get(x, z);
            if mask < 0.15 {
                continue;
            }
            let wx = elevation.descriptor.origin_x() as f32 + x as f32 * spacing;
            let wz = elevation.descriptor.origin_z() as f32 + z as f32 * spacing;
            let coast = coast_distance.get(x, z).max(0.0);
            let berm = range_mix(
                recipe.berm_height_min_m,
                recipe.berm_height_max_m,
                seeded_unit(&berm_noise, wx, wz),
            );
            let berm_distance = recipe.beach_width_max_m.max(spacing * 2.0);
            let target = if coast <= berm_distance {
                sea_level_m + berm * (coast / berm_distance)
            } else {
                sea_level_m + berm
            };
            let current = elevation.get(x, z);
            elevation.set(x, z, current + (target - current) * (mask * 0.6));
        }
    }
}
