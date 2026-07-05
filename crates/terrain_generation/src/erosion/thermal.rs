//! Thermal talus erosion.

use game_data::CompiledErosionRecipe;

use crate::fields::scalar::ScalarField;

pub fn apply_thermal_erosion(
    elevation: &mut ScalarField,
    hardness: &ScalarField,
    land_mask: &ScalarField,
    recipe: &CompiledErosionRecipe,
) {
    let th = &recipe.thermal;
    let base_talus = th.talus_deg.to_radians().tan();
    for _ in 0..th.iterations_per_cycle {
        let mut transfers = vec![0.0f32; elevation.values.len()];
        for z in 1..elevation.descriptor.height - 1 {
            for x in 1..elevation.descriptor.width - 1 {
                if land_mask.get(x, z) < 0.3 {
                    continue;
                }
                let hard = hardness.get(x, z).clamp(0.1, 1.0);
                let talus_angle = base_talus * (1.0 + hard * 0.4);
                let allowed = elevation.descriptor.cell_size_m as f32 * talus_angle;
                let h = elevation.get(x, z);
                for (dx, dz) in [(1i32, 0), (-1, 0), (0, 1), (0, -1)] {
                    let nx = x as i32 + dx;
                    let nz = z as i32 + dz;
                    let nh = elevation.get(nx as u32, nz as u32);
                    let diff = h - nh;
                    if diff > allowed {
                        let excess = diff - allowed;
                        let transfer = excess * th.transfer_rate * 0.25;
                        let i = elevation.index(x, z);
                        let ni = elevation.index(nx as u32, nz as u32);
                        transfers[i] -= transfer;
                        transfers[ni] += transfer;
                    }
                }
            }
        }
        for (i, t) in transfers.into_iter().enumerate() {
            elevation.values[i] += t;
        }
    }
}
