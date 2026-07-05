//! D8 flow direction and accumulation on filled elevation.

use crate::fields::scalar::ScalarField;
use crate::fields::typed::CategoricalField;

use super::fill::D8_NEIGHBORS;

pub struct FlowResult {
    pub direction: CategoricalField<u8>,
    pub accumulation: ScalarField,
}

pub fn compute_flow(
    filled: &ScalarField,
    land_mask: &ScalarField,
    runoff: &ScalarField,
) -> FlowResult {
    let w = filled.descriptor.width;
    let h = filled.descriptor.height;
    let mut direction = CategoricalField::zeros(filled.descriptor.clone());
    let mut accumulation = ScalarField::zeros(filled.descriptor.clone());

    let mut cells: Vec<(f32, u32, u32)> = Vec::new();
    for z in 0..h {
        for x in 0..w {
            if land_mask.get(x, z) < 0.2 {
                continue;
            }
            cells.push((filled.get(x, z), x, z));
            accumulation.set(x, z, runoff.get(x, z).max(0.01));
        }
    }
    cells.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(&b.2))
    });

    for (elev, x, z) in &cells {
        let mut best_dir = 255u8;
        let mut best_gradient = 0.0f32;
        for (dir, (dx, dz)) in D8_NEIGHBORS.iter().enumerate() {
            let nx = *x as i32 + dx;
            let nz = *z as i32 + dz;
            if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                continue;
            }
            let neighbor = filled.get(nx as u32, nz as u32);
            let drop = *elev - neighbor;
            if drop <= 0.0 {
                continue;
            }
            let gradient = drop
                / if *dx != 0 && *dz != 0 {
                    std::f32::consts::SQRT_2
                } else {
                    1.0
                };
            if gradient > best_gradient {
                best_gradient = gradient;
                best_dir = dir as u8;
            }
        }
        direction.set(*x, *z, best_dir);
    }

    for (_, x, z) in &cells {
        let dir = direction.get(*x, *z);
        if dir == 255 {
            continue;
        }
        let (dx, dz) = D8_NEIGHBORS[dir as usize];
        let nx = *x as i32 + dx;
        let nz = *z as i32 + dz;
        if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
            continue;
        }
        let acc = accumulation.get(*x, *z);
        let downstream = accumulation.get(nx as u32, nz as u32);
        accumulation.set(nx as u32, nz as u32, downstream + acc);
    }

    FlowResult {
        direction,
        accumulation,
    }
}

pub fn compute_slope(elevation: &ScalarField) -> ScalarField {
    let mut slope = ScalarField::zeros(elevation.descriptor.clone());
    let cell = elevation.descriptor.cell_size_m as f32;
    let w = elevation.descriptor.width;
    let h = elevation.descriptor.height;
    for z in 1..h - 1 {
        for x in 1..w - 1 {
            let e0 = elevation.get(x, z);
            let ex = elevation.get(x + 1, z);
            let ez = elevation.get(x, z + 1);
            let dx = (ex - e0) / cell;
            let dz = (ez - e0) / cell;
            slope.set(x, z, (dx * dx + dz * dz).sqrt().atan().to_degrees());
        }
    }
    slope
}

pub fn extract_river_mask(
    accumulation: &ScalarField,
    land_mask: &ScalarField,
    stream_threshold: f32,
    permanent_river_threshold: f32,
) -> ScalarField {
    let mut river = ScalarField::zeros(accumulation.descriptor.clone());
    for z in 0..accumulation.descriptor.height {
        for x in 0..accumulation.descriptor.width {
            if land_mask.get(x, z) < 0.3 {
                continue;
            }
            let acc = accumulation.get(x, z);
            let threshold = if acc >= permanent_river_threshold {
                1.0
            } else if acc >= stream_threshold {
                0.5
            } else {
                0.0
            };
            river.set(x, z, threshold);
        }
    }
    river
}
