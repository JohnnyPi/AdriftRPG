//! Priority-flood, D8 flow, accumulation (VS3 §5).

use std::collections::BinaryHeap;

use crate::field2d::Field2D;
use crate::island_gen::params::IslandGenParams;
use crate::water_body::{RiverControlPoint, RiverSpline};

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

#[derive(Clone, Copy, PartialEq)]
struct FloodCell {
    elevation: f32,
    x: u32,
    z: u32,
}

impl Eq for FloodCell {}

impl PartialOrd for FloodCell {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FloodCell {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.elevation
            .partial_cmp(&other.elevation)
            .unwrap_or(std::cmp::Ordering::Equal)
            .reverse()
    }
}

pub fn priority_flood(elevation: &Field2D<f32>) -> Field2D<f32> {
    let mut filled = elevation.clone();
    let w = elevation.width;
    let h = elevation.height;
    let mut heap = BinaryHeap::new();
    let mut visited = vec![false; (w * h) as usize];

    for x in 0..w {
        for z in [0, h - 1] {
            let i = elevation.index(x, z);
            visited[i] = true;
            heap.push(FloodCell {
                elevation: elevation.get(x, z),
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
                elevation: elevation.get(x, z),
                x,
                z,
            });
        }
    }

    while let Some(cell) = heap.pop() {
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
            let new_elev = elev.max(cell.elevation);
            filled.set(nx, nz, new_elev);
            heap.push(FloodCell {
                elevation: new_elev,
                x: nx,
                z: nz,
            });
        }
    }
    filled
}

pub fn compute_flow(
    filled: &Field2D<f32>,
    island_mask: &Field2D<f32>,
    params: &IslandGenParams,
) -> (Field2D<u8>, Field2D<f32>) {
    let w = filled.width;
    let h = filled.height;
    let mut direction = Field2D::<u8>::new(w, h, filled.origin, filled.spacing);
    let mut accumulation = Field2D::<f32>::new(w, h, filled.origin, filled.spacing);

    let mut cells: Vec<(f32, u32, u32)> = Vec::new();
    for z in 0..h {
        for x in 0..w {
            if island_mask.get(x, z) < 0.2 {
                continue;
            }
            cells.push((filled.get(x, z), x, z));
            accumulation.set(x, z, params.hydrology.rainfall_base);
        }
    }
    cells.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    for (elev, x, z) in &cells {
        let mut best_dir = 255u8;
        let mut best_drop = 0.0f32;
        for (dir, (dx, dz)) in D8_OFFSETS.iter().enumerate() {
            let nx = *x as i32 + dx;
            let nz = *z as i32 + dz;
            if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                continue;
            }
            let neighbor = filled.get(nx as u32, nz as u32);
            let drop = *elev - neighbor;
            if drop > best_drop {
                best_drop = drop;
                best_dir = dir as u8;
            }
        }
        direction.set(*x, *z, best_dir);
    }

    for (_, x, z) in cells.iter().rev() {
        let dir = direction.get(*x, *z);
        if dir == 255 {
            continue;
        }
        let (dx, dz) = D8_OFFSETS[dir as usize];
        let nx = *x as i32 + dx;
        let nz = *z as i32 + dz;
        if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
            continue;
        }
        let acc = accumulation.get(*x, *z);
        let downstream = accumulation.get(nx as u32, nz as u32);
        accumulation.set(nx as u32, nz as u32, downstream + acc);
    }

    (direction, accumulation)
}

pub fn extract_river_mask(
    accumulation: &Field2D<f32>,
    island_mask: &Field2D<f32>,
    params: &IslandGenParams,
) -> Field2D<f32> {
    let mut river = Field2D::<f32>::new(
        accumulation.width,
        accumulation.height,
        accumulation.origin,
        accumulation.spacing,
    );
    for z in 0..accumulation.height {
        for x in 0..accumulation.width {
            if island_mask.get(x, z) < 0.3 {
                continue;
            }
            let acc = accumulation.get(x, z);
            let threshold = if acc >= params.hydrology.permanent_river_threshold {
                1.0
            } else if acc >= params.hydrology.stream_threshold {
                0.5
            } else {
                0.0
            };
            river.set(x, z, threshold);
        }
    }
    river
}

pub fn trace_primary_river(
    filled: &Field2D<f32>,
    accumulation: &Field2D<f32>,
    direction: &Field2D<u8>,
    island_mask: &Field2D<f32>,
    params: &IslandGenParams,
    sea_level: f32,
) -> Option<RiverSpline> {
    let mut best_source = None;
    let mut best_score = f32::MIN;
    for z in 0..filled.height {
        for x in 0..filled.width {
            if island_mask.get(x, z) < 0.4 {
                continue;
            }
            let acc = accumulation.get(x, z);
            let elev = filled.get(x, z);
            if acc < params.hydrology.stream_threshold && elev <= sea_level + 4.0 {
                continue;
            }
            let score = acc + elev.max(sea_level) * 2.0;
            if score > best_score {
                best_score = score;
                let wx = filled.origin[0] + x as f32 * filled.spacing;
                let wz = filled.origin[1] + z as f32 * filled.spacing;
                best_source = Some((x, z, wx, wz));
            }
        }
    }
    let (sx, sz, mut wx, mut wz) = best_source?;

    let mut path = vec![(wx, wz)];
    let mut visited = std::collections::HashSet::new();
    visited.insert((sx, sz));
    let mut cells = vec![(sx, sz)];

    let mut cx = sx;
    let mut cz = sz;
    for _ in 0..500 {
        let dir = direction.get(cx, cz);
        if dir == 255 {
            break;
        }
        let (dx, dz) = D8_OFFSETS[dir as usize];
        let nx = cx as i32 + dx;
        let nz = cz as i32 + dz;
        if nx < 0 || nz < 0 || nx >= filled.width as i32 || nz >= filled.height as i32 {
            break;
        }
        let next = (nx as u32, nz as u32);
        if visited.contains(&next) {
            break;
        }
        visited.insert(next);
        cx = next.0;
        cz = next.1;
        cells.push(next);
        wx = filled.origin[0] + cx as f32 * filled.spacing;
        wz = filled.origin[1] + cz as f32 * filled.spacing;
        path.push((wx, wz));
        if island_mask.get(cx, cz) < 0.1 || filled.get(cx, cz) <= sea_level + 0.25 {
            break;
        }
    }

    if path.len() < 4 {
        return None;
    }
    let total_len: f32 = path
        .windows(2)
        .map(|w| {
            let dx = w[1].0 - w[0].0;
            let dz = w[1].1 - w[0].1;
            (dx * dx + dz * dz).sqrt()
        })
        .sum();
    if total_len < params.hydrology.minimum_stream_length_m {
        return None;
    }

    let n = path.len();
    let mut points = Vec::new();
    let source_acc = accumulation.get(sx, sz).max(params.hydrology.rainfall_base);
    for (i, ((x, z), (gx, gz))) in path.iter().zip(cells.iter()).enumerate() {
        let t = i as f32 / (n - 1) as f32;
        let acc = (accumulation.get(*gx, *gz) / source_acc).clamp(0.0, 1.0);
        let width = 1.8 + (6.5 - 1.8) * acc.max(t * 0.35);
        let depth = 0.4 + (1.6 - 0.4) * acc.max(t * 0.5);
        let terrain_height = filled.get(*gx, *gz);
        let water_elevation = terrain_height.max(sea_level) - depth * 0.25;
        let bed_elevation = (water_elevation - depth).max(sea_level - depth * 0.25);
        points.push(RiverControlPoint {
            position_xz: [*x, *z],
            bed_elevation,
            water_elevation,
            width,
            depth,
            discharge: accumulation
                .get(*gx, *gz)
                .max(params.hydrology.rainfall_base),
        });
    }
    Some(RiverSpline { points })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traced_river_follows_flow_direction_field() {
        let mut filled = Field2D::<f32>::new(5, 5, [0.0, 0.0], 1.0);
        let mut accumulation = Field2D::<f32>::new(5, 5, [0.0, 0.0], 1.0);
        let mut direction = Field2D::<u8>::new(5, 5, [0.0, 0.0], 1.0);
        let mut mask = Field2D::<f32>::new(5, 5, [0.0, 0.0], 1.0);

        for z in 0..5 {
            for x in 0..5 {
                filled.set(x, z, 20.0 - z as f32 - x as f32 * 0.1);
                accumulation.set(x, z, 1.0);
                direction.set(x, z, 255);
                mask.set(x, z, 1.0);
            }
        }
        accumulation.set(2, 0, 80.0);
        accumulation.set(2, 1, 70.0);
        accumulation.set(2, 2, 60.0);
        accumulation.set(2, 3, 50.0);
        direction.set(2, 0, 4);
        direction.set(2, 1, 4);
        direction.set(2, 2, 4);
        direction.set(2, 3, 4);
        mask.set(2, 4, 0.0);
        filled.set(2, 4, 0.0);

        let mut params = IslandGenParams::default();
        params.hydrology.stream_threshold = 10.0;
        params.hydrology.minimum_stream_length_m = 3.0;
        let river = trace_primary_river(&filled, &accumulation, &direction, &mask, &params, 0.0)
            .expect("river");
        let xs: Vec<_> = river
            .points
            .iter()
            .map(|point| point.position_xz[0])
            .collect();
        assert_eq!(xs, vec![2.0, 2.0, 2.0, 2.0, 2.0]);
        assert!(
            river.points[0].discharge >= river.points[3].discharge,
            "river discharge should be sourced from flow accumulation"
        );
    }
}
