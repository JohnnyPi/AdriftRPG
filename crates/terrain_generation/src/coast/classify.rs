//! Beach and cliff classification.

use game_data::CompiledCoastRecipe;

use crate::fields::scalar::ScalarField;

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 >= edge1 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub struct CoastMasks {
    pub beach: ScalarField,
    pub cliff: ScalarField,
}

pub fn classify_coast_masks(
    elevation: &ScalarField,
    slope: &ScalarField,
    coast_distance: &ScalarField,
    land_mask: &ScalarField,
    sediment: &ScalarField,
    wave_exposure: &ScalarField,
    recipe: &CompiledCoastRecipe,
) -> CoastMasks {
    let desc = elevation.descriptor.clone();
    let mut cliff = ScalarField::zeros(desc.clone());
    let mut beach = ScalarField::zeros(desc);

    for z in 0..elevation.descriptor.height {
        for x in 0..elevation.descriptor.width {
            if land_mask.get(x, z) < 0.4 {
                continue;
            }
            let sl = slope.get(x, z);
            let coast = coast_distance.get(x, z).max(0.0);
            let sed = sediment.get(x, z);
            let exposure = wave_exposure.get(x, z);

            let slope_score = smoothstep(recipe.cliff_min_slope_deg, 45.0, sl);
            let sediment_score = 1.0 - smoothstep(0.2, 0.8, sed);
            let exposure_score = smoothstep(recipe.cliff_min_exposure, 1.0, exposure);
            let cliff_score =
                slope_score * sediment_score * exposure_score * smoothstep(2.0, 30.0, coast);
            cliff.set(x, z, cliff_score);

            let low_slope = 1.0 - smoothstep(5.0, recipe.beach_max_slope_deg, sl);
            let near_coast = 1.0 - smoothstep(0.0, recipe.beach_width_max_m, coast);
            let moderate_exposure = 1.0 - smoothstep(0.65, 1.0, exposure);
            let beach_score = low_slope
                * near_coast
                * moderate_exposure
                * sed.max(0.15)
                * (1.0 - cliff_score * 0.85);
            beach.set(x, z, beach_score.clamp(0.0, 1.0));
        }
    }

    CoastMasks { beach, cliff }
}
