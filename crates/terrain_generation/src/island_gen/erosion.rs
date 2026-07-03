// crates/terrain_generation/src/island_gen/erosion.rs
//! Stream-power and thermal erosion (VS3 §7).

use crate::field2d::Field2D;
use crate::island_gen::carving::compute_slope;
use crate::island_gen::params::IslandGenParams;

pub fn apply_stream_power_erosion(
    elevation: &mut Field2D<f32>,
    accumulation: &Field2D<f32>,
    island_mask: &Field2D<f32>,
    params: &IslandGenParams,
) {
    let er = &params.erosion;
    let mut slope = compute_slope(elevation);
    for _ in 0..er.stream_power_iterations {
        let mut deltas = vec![0.0f32; elevation.samples.len()];
        for z in 1..elevation.height - 1 {
            for x in 1..elevation.width - 1 {
                if island_mask.get(x, z) < 0.3 {
                    continue;
                }
                let discharge = accumulation.get(x, z).max(0.1);
                let sl = slope.get(x, z).to_radians().tan().max(0.0);
                let erosion = discharge.powf(er.m) * sl.powf(er.n) * er.stream_power_erodibility;
                let step = erosion.min(er.maximum_step_m);
                deltas[elevation.index(x, z)] -= step;
            }
        }
        for (i, delta) in deltas.into_iter().enumerate() {
            if delta != 0.0 {
                elevation.samples[i] += delta;
            }
        }
        slope = compute_slope(elevation);
    }
}

pub fn apply_thermal_erosion(
    elevation: &mut Field2D<f32>,
    island_mask: &Field2D<f32>,
    params: &IslandGenParams,
) {
    let talus_angle = params.erosion.thermal_talus_deg.to_radians().tan();
    let rate = params.erosion.thermal_transfer_rate;
    for _ in 0..params.erosion.thermal_iterations {
        let mut transfers = vec![0.0f32; elevation.samples.len()];
        for z in 1..elevation.height - 1 {
            for x in 1..elevation.width - 1 {
                if island_mask.get(x, z) < 0.3 {
                    continue;
                }
                let h = elevation.get(x, z);
                for (dx, dz) in [(1i32, 0), (-1, 0), (0, 1), (0, -1)] {
                    let nx = x as i32 + dx;
                    let nz = z as i32 + dz;
                    let nh = elevation.get(nx as u32, nz as u32);
                    let diff = h - nh;
                    let allowed = elevation.spacing * talus_angle;
                    if diff > allowed {
                        let excess = diff - allowed;
                        let transfer = excess * rate * 0.25;
                        let i = elevation.index(x, z);
                        let ni = elevation.index(nx as u32, nz as u32);
                        transfers[i] -= transfer;
                        transfers[ni] += transfer;
                    }
                }
            }
        }
        for (i, t) in transfers.into_iter().enumerate() {
            elevation.samples[i] += t;
        }
    }
}
