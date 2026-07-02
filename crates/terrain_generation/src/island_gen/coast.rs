// crates/terrain_generation/src/island_gen/coast.rs
//! Cliff and beach classification (VS3 §8–9).

use crate::field2d::{smoothstep, Field2D};
use crate::island_gen::params::IslandGenParams;
use crate::noise::ValueNoise;

const SALT_BERM_HEIGHT: u64 = 0xBEAC_0001_0001;

fn seeded_unit(params: &IslandGenParams, wx: f32, wz: f32, salt: u64) -> f32 {
    ValueNoise::new(params.seed.wrapping_add(salt)).sample(wx * 0.0025, 0.0, wz * 0.0025)
}

fn range_mix(min: f32, max: f32, t: f32) -> f32 {
    min + (max - min) * t.clamp(0.0, 1.0)
}

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
            let wx = origin[0] + x as f32 * spacing;
            let wz = origin[1] + z as f32 * spacing;
            let coast = coast_distance.get(x, z);
            let berm = range_mix(
                params.beaches.berm_height_min_m,
                params.beaches.berm_height_max_m,
                seeded_unit(params, wx, wz, SALT_BERM_HEIGHT),
            );
            let target = sea + berm - coast * 0.08;
            let idx = elevation.index(x, z);
            let h = &mut elevation.samples[idx];
            *h = *h + (target - *h) * (mask * 0.6);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::island_gen::bathymetry::compute_coast_distance;

    fn circular_mask(radius_cells: u32, spacing: f32) -> Field2D<f32> {
        let diameter = radius_cells * 2 + 1;
        let origin = [
            -(radius_cells as f32) * spacing,
            -(radius_cells as f32) * spacing,
        ];
        let mut mask = Field2D::<f32>::new(diameter, diameter, origin, spacing);
        let cx = radius_cells as f32;
        let cz = radius_cells as f32;
        for z in 0..diameter {
            for x in 0..diameter {
                let dx = x as f32 - cx;
                let dz = z as f32 - cz;
                let d = (dx * dx + dz * dz).sqrt() * spacing;
                let land = if d < radius_cells as f32 * spacing * 0.95 {
                    1.0
                } else if d > radius_cells as f32 * spacing * 1.05 {
                    0.0
                } else {
                    1.0 - (d / (radius_cells as f32 * spacing) - 0.95) / 0.1
                };
                mask.set(x, z, land.clamp(0.0, 1.0));
            }
        }
        mask
    }

    #[test]
    fn cliff_mask_nonzero_on_steep_coastal_ring() {
        use crate::island_gen::params::IslandGenParams;

        let spacing = 4.0;
        let radius_cells = 18;
        let mask = circular_mask(radius_cells, spacing);
        let coast_distance = compute_coast_distance(&mask, spacing);
        let mut elevation = Field2D::<f32>::new(
            mask.width,
            mask.height,
            mask.origin,
            spacing,
        );
        let cx = radius_cells;
        let sea = 0.0f32;
        for z in 0..mask.height {
            for x in 0..mask.width {
                let coast = coast_distance.get(x, z);
                let h = if mask.get(x, z) > 0.5 {
                    sea + 40.0 - coast * 2.5
                } else {
                    sea - 5.0
                };
                elevation.set(x, z, h);
            }
        }
        let slope = crate::island_gen::carving::compute_slope(&elevation);
        let sediment = Field2D::<f32>::new(mask.width, mask.height, mask.origin, spacing);
        let params = IslandGenParams::default();
        let (cliff, _) = classify_coast(
            &elevation,
            &slope,
            &coast_distance,
            &mask,
            &sediment,
            &params,
        );
        let mut max_cliff = 0.0f32;
        for r in 2..8 {
            max_cliff = max_cliff.max(cliff.get(cx + r, cx));
        }
        assert!(
            max_cliff > 0.1,
            "steep coastal ring should produce nonzero cliff mask, got {max_cliff}"
        );
    }
}
