//! Lake, wetland, waterfall, and river graph extraction.

use game_data::CompiledHydrologyRecipe;

use crate::fields::scalar::ScalarField;
use crate::fields::typed::CategoricalField;
use crate::water_body::{RiverControlPoint, RiverSpline};

use super::fill::D8_NEIGHBORS;
use super::graph::{HydrologyGraph, LakeBasin, WaterfallCandidate, WetlandRegion};

pub fn build_hydrology_graph(
    filled: &ScalarField,
    accumulation: &ScalarField,
    direction: &CategoricalField<u8>,
    land_mask: &ScalarField,
    humidity: &ScalarField,
    slope: &ScalarField,
    recipe: &CompiledHydrologyRecipe,
    sea_level: f32,
) -> HydrologyGraph {
    let w = filled.descriptor.width;
    let h = filled.descriptor.height;
    let cell = filled.descriptor.cell_size_m as f32;

    let lakes = extract_lakes(filled, accumulation, land_mask, recipe);
    let wetlands = extract_wetlands(land_mask, humidity, slope, recipe);
    let waterfalls = detect_waterfalls(filled, accumulation, direction, land_mask, recipe);
    let primary_river = trace_primary_river(
        filled,
        accumulation,
        direction,
        land_mask,
        recipe,
        sea_level,
        cell,
    );

    let mut nodes = Vec::new();
    for z in 0..h {
        for x in 0..w {
            if land_mask.get(x, z) < 0.2 {
                continue;
            }
            let dir = direction.get(x, z);
            let downstream = if dir == 255 {
                None
            } else {
                let (dx, dz) = D8_NEIGHBORS[dir as usize];
                let nx = x as i32 + dx;
                let nz = z as i32 + dz;
                if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                    None
                } else {
                    Some(nodes.len() + 1)
                }
            };
            nodes.push(super::graph::HydroNode {
                cell_x: x,
                cell_z: z,
                downstream,
                drainage_area: accumulation.get(x, z),
                discharge: accumulation.get(x, z) * recipe.rainfall_weight,
                stream_order: stream_order_at(accumulation, x, z, recipe.permanent_river_threshold),
                sediment: 0.0,
            });
        }
    }

    HydrologyGraph {
        nodes,
        lakes,
        wetlands,
        waterfalls,
        primary_river,
    }
}

fn stream_order_at(accumulation: &ScalarField, x: u32, z: u32, permanent_threshold: f32) -> u8 {
    let acc = accumulation.get(x, z);
    if acc < permanent_threshold * 0.25 {
        return 0;
    }
    if acc < permanent_threshold {
        return 1;
    }
    if acc < permanent_threshold * 4.0 {
        return 2;
    }
    if acc < permanent_threshold * 16.0 {
        return 3;
    }
    4
}

fn extract_lakes(
    filled: &ScalarField,
    accumulation: &ScalarField,
    land_mask: &ScalarField,
    recipe: &CompiledHydrologyRecipe,
) -> Vec<LakeBasin> {
    let w = filled.descriptor.width;
    let h = filled.descriptor.height;
    let mut visited = vec![false; (w * h) as usize];
    let mut lakes = Vec::new();

    for z in 0..h {
        for x in 0..w {
            let i = filled.index(x, z);
            if visited[i] || land_mask.get(x, z) < 0.3 {
                continue;
            }
            if accumulation.get(x, z) >= recipe.stream_threshold {
                continue;
            }
            let elev = filled.get(x, z);
            let mut basin = Vec::new();
            let mut queue = std::collections::VecDeque::from([(x, z)]);
            visited[i] = true;
            while let Some((cx, cz)) = queue.pop_front() {
                basin.push((cx, cz));
                for (dx, dz) in [(0i32, 1), (0, -1), (1, 0), (-1, 0)] {
                    let nx = cx as i32 + dx;
                    let nz = cz as i32 + dz;
                    if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                        continue;
                    }
                    let (nx, nz) = (nx as u32, nz as u32);
                    let ni = filled.index(nx, nz);
                    if visited[ni] || land_mask.get(nx, nz) < 0.3 {
                        continue;
                    }
                    if (filled.get(nx, nz) - elev).abs() > 2.0 {
                        continue;
                    }
                    if accumulation.get(nx, nz) >= recipe.stream_threshold {
                        continue;
                    }
                    visited[ni] = true;
                    queue.push_back((nx, nz));
                }
            }
            if basin.len() as u32 >= recipe.lake_min_area_cells {
                let area = basin.len() as u32;
                lakes.push(LakeBasin {
                    cells: basin,
                    surface_elevation_m: elev,
                    area_cells: area,
                });
            }
        }
    }
    lakes
}

fn extract_wetlands(
    land_mask: &ScalarField,
    humidity: &ScalarField,
    slope: &ScalarField,
    recipe: &CompiledHydrologyRecipe,
) -> Vec<WetlandRegion> {
    let w = land_mask.descriptor.width;
    let h = land_mask.descriptor.height;
    let mut visited = vec![false; (w * h) as usize];
    let mut wetlands = Vec::new();

    for z in 0..h {
        for x in 0..w {
            let i = land_mask.index(x, z);
            if visited[i] || land_mask.get(x, z) < 0.3 {
                continue;
            }
            if humidity.get(x, z) < recipe.wetland_moisture_threshold || slope.get(x, z) > 8.0 {
                continue;
            }
            let moisture = humidity.get(x, z);
            let mut cells = Vec::new();
            let mut queue = std::collections::VecDeque::from([(x, z)]);
            visited[i] = true;
            while let Some((cx, cz)) = queue.pop_front() {
                cells.push((cx, cz));
                for (dx, dz) in [(0i32, 1), (0, -1), (1, 0), (-1, 0)] {
                    let nx = cx as i32 + dx;
                    let nz = cz as i32 + dz;
                    if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                        continue;
                    }
                    let (nx, nz) = (nx as u32, nz as u32);
                    let ni = land_mask.index(nx, nz);
                    if visited[ni] || land_mask.get(nx, nz) < 0.3 {
                        continue;
                    }
                    if humidity.get(nx, nz) < recipe.wetland_moisture_threshold * 0.9
                        || slope.get(nx, nz) > 10.0
                    {
                        continue;
                    }
                    visited[ni] = true;
                    queue.push_back((nx, nz));
                }
            }
            if cells.len() >= 4 {
                wetlands.push(WetlandRegion { cells, moisture });
            }
        }
    }
    wetlands
}

fn detect_waterfalls(
    filled: &ScalarField,
    accumulation: &ScalarField,
    direction: &CategoricalField<u8>,
    land_mask: &ScalarField,
    recipe: &CompiledHydrologyRecipe,
) -> Vec<WaterfallCandidate> {
    let w = filled.descriptor.width;
    let h = filled.descriptor.height;
    let mut waterfalls = Vec::new();
    for z in 0..h {
        for x in 0..w {
            if land_mask.get(x, z) < 0.3 {
                continue;
            }
            let dir = direction.get(x, z);
            if dir == 255 {
                continue;
            }
            let acc = accumulation.get(x, z);
            if acc < recipe.permanent_river_threshold {
                continue;
            }
            let (dx, dz) = D8_NEIGHBORS[dir as usize];
            let nx = x as i32 + dx;
            let nz = z as i32 + dz;
            if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                continue;
            }
            let drop = filled.get(x, z) - filled.get(nx as u32, nz as u32);
            if drop >= recipe.waterfall_min_drop_m && acc >= recipe.waterfall_min_discharge {
                waterfalls.push(WaterfallCandidate {
                    from: (x, z),
                    to: (nx as u32, nz as u32),
                    drop_m: drop,
                    discharge: acc,
                });
            }
        }
    }
    waterfalls
}

pub fn trace_primary_river(
    filled: &ScalarField,
    accumulation: &ScalarField,
    direction: &CategoricalField<u8>,
    land_mask: &ScalarField,
    recipe: &CompiledHydrologyRecipe,
    sea_level: f32,
    cell_size_m: f32,
) -> Option<RiverSpline> {
    let desc = &filled.descriptor;
    let origin_x = desc.origin_x() as f32;
    let origin_z = desc.origin_z() as f32;

    let mut best_source = None;
    let mut best_score = f32::MIN;
    for z in 0..desc.height {
        for x in 0..desc.width {
            if land_mask.get(x, z) < 0.4 {
                continue;
            }
            let acc = accumulation.get(x, z);
            let elev = filled.get(x, z);
            if acc < recipe.stream_threshold && elev <= sea_level + 4.0 {
                continue;
            }
            let score = acc.ln().max(0.0) * 8.0 + (elev - sea_level).max(0.0) * 1.5;
            if score > best_score {
                best_score = score;
                let wx = origin_x + x as f32 * cell_size_m;
                let wz = origin_z + z as f32 * cell_size_m;
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
        let (dx, dz) = D8_NEIGHBORS[dir as usize];
        let nx = cx as i32 + dx;
        let nz = cz as i32 + dz;
        if nx < 0 || nz < 0 || nx >= desc.width as i32 || nz >= desc.height as i32 {
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
        wx = origin_x + cx as f32 * cell_size_m;
        wz = origin_z + cz as f32 * cell_size_m;
        path.push((wx, wz));
        if land_mask.get(cx, cz) < 0.1 || filled.get(cx, cz) <= sea_level + 0.25 {
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
    if total_len < recipe.minimum_stream_length_m {
        return None;
    }

    let n = path.len();
    let mut points = Vec::new();
    let source_acc = accumulation.get(sx, sz).max(0.01);
    let mut max_water_elev = f32::MAX;
    for (i, ((x, z), (gx, gz))) in path.iter().zip(cells.iter()).enumerate() {
        let t = i as f32 / (n - 1) as f32;
        let acc = (accumulation.get(*gx, *gz) / source_acc).clamp(0.0, 1.0);
        let width = 1.8 + (6.5 - 1.8) * acc.max(t * 0.35);
        let depth = 0.4 + (1.6 - 0.4) * acc.max(t * 0.5);
        let terrain_height = filled.get(*gx, *gz);
        let mut water_elevation = terrain_height.max(sea_level) - depth * 0.25;
        water_elevation = water_elevation.min(max_water_elev);
        max_water_elev = water_elevation;
        let bed_elevation = (water_elevation - depth).max(sea_level - depth * 0.25);
        points.push(RiverControlPoint {
            position_xz: [*x, *z],
            bed_elevation,
            water_elevation,
            width,
            depth,
            discharge: accumulation.get(*gx, *gz).max(0.01),
        });
    }
    Some(RiverSpline { points })
}

pub fn compute_lake_mask(
    graph: &HydrologyGraph,
    descriptor: crate::fields::descriptor::FieldDescriptor,
) -> ScalarField {
    let mut mask = ScalarField::zeros(descriptor);
    for lake in &graph.lakes {
        for &(x, z) in &lake.cells {
            mask.set(x, z, 1.0);
        }
    }
    mask
}

pub fn compute_wetland_mask(
    graph: &HydrologyGraph,
    descriptor: crate::fields::descriptor::FieldDescriptor,
) -> ScalarField {
    let mut mask = ScalarField::zeros(descriptor);
    for wetland in &graph.wetlands {
        for &(x, z) in &wetland.cells {
            mask.set(x, z, wetland.moisture);
        }
    }
    mask
}
