//! Sediment pickup, transport, and deposition.

use game_data::CompiledErosionRecipe;

use crate::fields::scalar::ScalarField;
use crate::hydrology::fill::D8_NEIGHBORS;

pub fn transport_and_deposit_sediment(
    elevation: &mut ScalarField,
    sediment: &mut ScalarField,
    accumulation: &ScalarField,
    direction: &crate::fields::typed::CategoricalField<u8>,
    slope: &ScalarField,
    land_mask: &ScalarField,
    recipe: &CompiledErosionRecipe,
) {
    let sed = &recipe.sediment;
    let w = elevation.descriptor.width;
    let h = elevation.descriptor.height;

    for z in 0..h {
        for x in 0..w {
            if land_mask.get(x, z) < 0.3 {
                continue;
            }
            let acc = accumulation.get(x, z);
            let sl = slope.get(x, z);
            let capacity = acc.powf(0.5) * sed.capacity_factor * (1.0 - sl / 45.0).max(0.1);
            let current = sediment.get(x, z);
            if current < capacity {
                let pickup = (capacity - current) * sed.pickup_rate;
                sediment.set(x, z, current + pickup);
                elevation.set(x, z, elevation.get(x, z) - pickup * 0.3);
            }
        }
    }

    let mut deposits = vec![0.0f32; elevation.values.len()];
    for z in 0..h {
        for x in 0..w {
            if land_mask.get(x, z) < 0.3 {
                continue;
            }
            let dir = direction.get(x, z);
            if dir == 255 {
                continue;
            }
            let load = sediment.get(x, z);
            if load < 0.01 {
                continue;
            }
            let (dx, dz) = D8_NEIGHBORS[dir as usize];
            let nx = x as i32 + dx;
            let nz = z as i32 + dz;
            if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                continue;
            }
            let transport = load * sed.transport_rate;
            let i = sediment.index(x, z);
            let ni = sediment.index(nx as u32, nz as u32);
            deposits[i] -= transport;
            deposits[ni] += transport * (1.0 - sed.deposition_rate);
        }
    }

    for z in 0..h {
        for x in 0..w {
            if land_mask.get(x, z) < 0.3 {
                continue;
            }
            let i = sediment.index(x, z);
            let sl = slope.get(x, z);
            if sl < 3.0 && accumulation.get(x, z) > 5.0 {
                let deposit = deposits[i].abs() * sed.deposition_rate;
                sediment.set(x, z, sediment.get(x, z) + deposit);
                elevation.set(x, z, elevation.get(x, z) + deposit * 0.2);
            } else {
                sediment.set(x, z, (sediment.get(x, z) + deposits[i]).max(0.0));
            }
        }
    }
}
