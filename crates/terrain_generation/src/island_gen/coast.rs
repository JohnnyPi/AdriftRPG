//! Cliff and beach classification (VS3 §8–9).

use crate::field2d::{smoothstep, Field2D};
use crate::island_gen::params::IslandGenParams;

pub fn classify_coast(
    elevation: &Field2D<f32>,
    slope: &Field2D<f32>,
    coast_distance: &Field2D<f32>,
    island_mask: &Field2D<f32>,
    sediment: &Field2D<f32>,
    params: &IslandGenParams,
) -> (Field2D<f32>, Field2D<f32>) {
    let mut cliff = Field2D::<f32>::new(
        elevation.width,
        elevation.height,
        elevation.origin,
        elevation.spacing,
    );
    let mut beach = Field2D::<f32>::new(
        elevation.width,
        elevation.height,
        elevation.origin,
        elevation.spacing,
    );

    for z in 0..elevation.height {
        for x in 0..elevation.width {
            if island_mask.get(x, z) < 0.4 {
                continue;
            }
            let sl = slope.get(x, z);
            let coast = coast_distance.get(x, z);
            let sed = sediment.get(x, z);
            let slope_score = smoothstep(25.0, 45.0, sl);
            let sediment_score = 1.0 - smoothstep(0.2, 0.8, sed);
            let cliff_score = slope_score * sediment_score * smoothstep(2.0, 30.0, coast);
            cliff.set(x, z, cliff_score);

            let low_slope = 1.0 - smoothstep(5.0, params.beaches.maximum_slope_deg, sl);
            let near_coast = 1.0 - smoothstep(0.0, params.beaches.width_max_m, coast);
            let beach_score = low_slope * near_coast * sed.max(0.2) * (1.0 - cliff_score * 0.8);
            beach.set(x, z, beach_score.clamp(0.0, 1.0));
        }
    }
    (cliff, beach)
}

pub fn apply_beach_profiles(
    elevation: &mut Field2D<f32>,
    beach_mask: &Field2D<f32>,
    coast_distance: &Field2D<f32>,
    params: &IslandGenParams,
) {
    let sea = params.island.sea_level_m;
    let width = elevation.width;
    let height = elevation.height;
    let origin = elevation.origin;
    let spacing = elevation.spacing;
    for z in 0..height {
        for x in 0..width {
            let mask = beach_mask.get(x, z);
            if mask < 0.15 {
                continue;
            }
            let coast = coast_distance.get(x, z);
            let berm = params.beaches.berm_height_min_m
                + (params.beaches.berm_height_max_m - params.beaches.berm_height_min_m) * 0.5;
            let target = sea + berm - coast * 0.08;
            let idx = elevation.index(x, z);
            let h = &mut elevation.samples[idx];
            *h = *h + (target - *h) * (mask * 0.6);
            let _ = (origin, spacing);
        }
    }
}
