//! Signed coast-distance field via BFS from the sea-level coastline.

use crate::fields::scalar::ScalarField;

/// Positive inland, zero at coastline, negative offshore.
pub fn compute_signed_coast_distance(base: &ScalarField, sea_level_m: f32) -> ScalarField {
    let desc = base.descriptor.clone();
    let w = desc.width;
    let h = desc.height;
    let cell = desc.cell_size_m as f32;
    let mut result = ScalarField::zeros(desc);
    let mut land_dist = vec![f32::MAX; (w * h) as usize];
    let mut ocean_dist = vec![f32::MAX; (w * h) as usize];

    let mut land_queue = std::collections::VecDeque::new();
    let mut ocean_queue = std::collections::VecDeque::new();

    for z in 0..h {
        for x in 0..w {
            let i = base.index(x, z);
            let elev = base.get(x, z);
            if elev >= sea_level_m {
                land_dist[i] = 0.0;
                land_queue.push_back((x, z));
            } else {
                ocean_dist[i] = 0.0;
                ocean_queue.push_back((x, z));
            }
        }
    }

    const NEIGHBORS: [(i32, i32, f32); 8] = [
        (0, -1, 1.0),
        (1, 0, 1.0),
        (0, 1, 1.0),
        (-1, 0, 1.0),
        (1, -1, std::f32::consts::SQRT_2),
        (1, 1, std::f32::consts::SQRT_2),
        (-1, 1, std::f32::consts::SQRT_2),
        (-1, -1, std::f32::consts::SQRT_2),
    ];

    while let Some((x, z)) = land_queue.pop_front() {
        let i = base.index(x, z);
        let d = land_dist[i];
        for (dx, dz, step) in NEIGHBORS {
            let nx = x as i32 + dx;
            let nz = z as i32 + dz;
            if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                continue;
            }
            let (nx, nz) = (nx as u32, nz as u32);
            if base.get(nx, nz) < sea_level_m {
                continue;
            }
            let ni = base.index(nx, nz);
            let nd = d + step * cell;
            if nd < land_dist[ni] {
                land_dist[ni] = nd;
                land_queue.push_back((nx, nz));
            }
        }
    }

    while let Some((x, z)) = ocean_queue.pop_front() {
        let i = base.index(x, z);
        let d = ocean_dist[i];
        for (dx, dz, step) in NEIGHBORS {
            let nx = x as i32 + dx;
            let nz = z as i32 + dz;
            if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                continue;
            }
            let (nx, nz) = (nx as u32, nz as u32);
            if base.get(nx, nz) >= sea_level_m {
                continue;
            }
            let ni = base.index(nx, nz);
            let nd = d + step * cell;
            if nd < ocean_dist[ni] {
                ocean_dist[ni] = nd;
                ocean_queue.push_back((nx, nz));
            }
        }
    }

    for z in 0..h {
        for x in 0..w {
            let i = base.index(x, z);
            let signed = if base.get(x, z) >= sea_level_m {
                land_dist[i]
            } else {
                -ocean_dist[i]
            };
            result.set(x, z, signed);
        }
    }
    result
}

pub fn compute_land_mask(base: &ScalarField, sea_level_m: f32) -> ScalarField {
    let mut mask = ScalarField::zeros(base.descriptor.clone());
    for z in 0..mask.descriptor.height {
        for x in 0..mask.descriptor.width {
            let v = if base.get(x, z) >= sea_level_m {
                1.0
            } else {
                0.0
            };
            mask.set(x, z, v);
        }
    }
    mask
}
