//! Fluvial stream-power erosion.

use game_data::CompiledErosionRecipe;

use crate::fields::scalar::ScalarField;
use crate::hydrology::routing::compute_slope;

pub fn apply_stream_power_erosion(
    elevation: &mut ScalarField,
    accumulation: &ScalarField,
    erodibility: &ScalarField,
    value_constraint: &ScalarField,
    land_mask: &ScalarField,
    recipe: &CompiledErosionRecipe,
) {
    let sp = &recipe.stream_power;
    let mut slope = compute_slope(elevation);
    for _ in 0..sp.iterations_per_cycle {
        let mut deltas = vec![0.0f32; elevation.values.len()];
        for z in 1..elevation.descriptor.height - 1 {
            for x in 1..elevation.descriptor.width - 1 {
                if land_mask.get(x, z) < 0.3 {
                    continue;
                }
                let constraint = 1.0 - value_constraint.get(x, z).clamp(0.0, 1.0);
                if constraint < 0.05 {
                    continue;
                }
                let discharge = accumulation.get(x, z).max(0.1);
                let sl = slope.get(x, z).to_radians().tan().max(0.0);
                let erod = erodibility.get(x, z).clamp(0.05, 1.0);
                let erosion =
                    discharge.powf(sp.m) * sl.powf(sp.n) * sp.erodibility * erod * constraint;
                let step = erosion.min(sp.maximum_step_m);
                deltas[elevation.index(x, z)] -= step;
            }
        }
        for (i, delta) in deltas.into_iter().enumerate() {
            if delta != 0.0 {
                elevation.values[i] += delta;
            }
        }
        slope = compute_slope(elevation);
    }
}
