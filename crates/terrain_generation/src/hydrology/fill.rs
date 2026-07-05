//! Priority-flood depression filling for drainage routing.

use std::collections::BinaryHeap;

use crate::fields::scalar::ScalarField;

const D8_OFFSETS: [(i32, i32); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

const FLOOD_EPSILON_FACTOR: f32 = 1e-4;

#[derive(Clone, Copy, PartialEq, Eq)]
struct FloodCell {
    elevation_bits: u32,
    x: u32,
    z: u32,
}

impl PartialOrd for FloodCell {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FloodCell {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .elevation_bits
            .cmp(&self.elevation_bits)
            .then_with(|| self.x.cmp(&other.x))
            .then_with(|| self.z.cmp(&other.z))
    }
}

fn float_bits(e: f32) -> u32 {
    e.to_bits()
}

pub fn priority_flood(elevation: &ScalarField) -> ScalarField {
    let mut filled = elevation.clone();
    let w = elevation.descriptor.width;
    let h = elevation.descriptor.height;
    let mut heap = BinaryHeap::new();
    let mut visited = vec![false; (w * h) as usize];

    for x in 0..w {
        for z in [0, h - 1] {
            let i = elevation.index(x, z);
            visited[i] = true;
            heap.push(FloodCell {
                elevation_bits: float_bits(elevation.get(x, z)),
                x,
                z,
            });
        }
    }
    for z in 1..h - 1 {
        for x in [0, w - 1] {
            let i = elevation.index(x, z);
            visited[i] = true;
            heap.push(FloodCell {
                elevation_bits: float_bits(elevation.get(x, z)),
                x,
                z,
            });
        }
    }

    let eps = elevation.descriptor.cell_size_m as f32 * FLOOD_EPSILON_FACTOR;

    while let Some(cell) = heap.pop() {
        let cell_elev = f32::from_bits(cell.elevation_bits);
        for (dx, dz) in D8_OFFSETS {
            let nx = cell.x as i32 + dx;
            let nz = cell.z as i32 + dz;
            if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                continue;
            }
            let (nx, nz) = (nx as u32, nz as u32);
            let ni = elevation.index(nx, nz);
            if visited[ni] {
                continue;
            }
            visited[ni] = true;
            let elev = elevation.get(nx, nz);
            let new_elev = elev.max(cell_elev + eps);
            filled.set(nx, nz, new_elev);
            heap.push(FloodCell {
                elevation_bits: float_bits(new_elev),
                x: nx,
                z: nz,
            });
        }
    }
    filled
}

pub const D8_NEIGHBORS: [(i32, i32); 8] = D8_OFFSETS;
