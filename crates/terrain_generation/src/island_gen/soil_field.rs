// crates/terrain_generation/src/island_gen/soil_field.rs
//! Soil depth from slope, sediment, and relief (VS3 §15 adjunct).

use crate::field2d::{Field2D, smoothstep};
use crate::island_gen::params::IslandGenParams;

pub fn compute_soil_depth(
    elevation: &Field2D<f32>,
    slope: &Field2D<f32>,
    sediment: &Field2D<f32>,
    island_mask: &Field2D<f32>,
    params: &IslandGenParams,
) -> Field2D<f32> {
    let mut soil = Field2D::<f32>::new(
        elevation.width,
        elevation.height,
        elevation.origin,
        elevation.spacing,
    );
    let sea = params.island.sea_level_m;
    let base = 1.8f32;
    let slope_penalty = 1.6f32;
    let sediment_gain = 0.35f32;
    let max_soil = 2.5f32;
    let summit_start = params.island.maximum_height_m * 0.65;

    for z in 0..elevation.height {
        for x in 0..elevation.width {
            if island_mask.get(x, z) < 0.3 {
                continue;
            }
            let relief = (elevation.get(x, z) - sea).max(0.0);
            let slope_norm = (slope.get(x, z) / 45.0).clamp(0.0, 1.0);
            let sediment_term = sediment.get(x, z) * sediment_gain;
            let summit_thin = smoothstep(summit_start, summit_start + 50.0, relief);
            let depth = (base - slope_penalty * slope_norm + sediment_term) * (1.0 - summit_thin);
            soil.set(x, z, depth.clamp(0.0, max_soil));
        }
    }
    soil
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soil_is_thicker_on_flats_than_steep_slopes() {
        let mut elevation = Field2D::<f32>::new(3, 3, [0.0, 0.0], 4.0);
        let mut slope = Field2D::<f32>::new(3, 3, [0.0, 0.0], 4.0);
        let mut sediment = Field2D::<f32>::new(3, 3, [0.0, 0.0], 4.0);
        let mut mask = Field2D::<f32>::new(3, 3, [0.0, 0.0], 4.0);
        for z in 0..3 {
            for x in 0..3 {
                elevation.set(x, z, 20.0);
                sediment.set(x, z, 0.1);
                mask.set(x, z, 1.0);
            }
        }
        slope.set(1, 1, 5.0);
        slope.set(0, 1, 40.0);
        let params = IslandGenParams::default();
        let soil = compute_soil_depth(&elevation, &slope, &sediment, &mask, &params);
        let mut values = Vec::new();
        for z in 0..3 {
            for x in 0..3 {
                values.push(soil.get(x, z));
            }
        }
        let variance: f32 = {
            let mean = values.iter().sum::<f32>() / values.len() as f32;
            values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / values.len() as f32
        };
        assert!(variance > 0.0, "soil depth should vary across the grid");
        assert!(
            soil.get(1, 1) > soil.get(0, 1),
            "flat cell soil {} should exceed steep cell {}",
            soil.get(1, 1),
            soil.get(0, 1)
        );
    }
}
