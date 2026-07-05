//! Convert atlas hydrology and coast products into runtime water body definitions.

use crate::coast::count_lagoon_components;
use crate::contract::coordinates::WorldXZ;
use crate::fields::key::FieldKey as Fk;
use crate::fields::scalar::ScalarField;
use crate::hydrology::graph::{HydrologyGraph, LakeBasin, WaterfallCandidate};
use crate::world::atlas::WorldAtlas;

#[derive(Clone, Debug, Default)]
pub struct LakeBody {
    pub id: String,
    pub surface_elevation_m: f32,
    pub vertices_xz: Vec<[f32; 2]>,
    pub centroid_xz: [f32; 2],
}

#[derive(Clone, Debug, Default)]
pub struct LagoonBody {
    pub id: String,
    pub surface_elevation_m: f32,
    pub vertices_xz: Vec<[f32; 2]>,
    pub centroid_xz: [f32; 2],
    pub salinity: f32,
}

#[derive(Clone, Debug, Default)]
pub struct WaterfallBody {
    pub id: String,
    pub position_xz: [f32; 2],
    pub drop_m: f32,
    pub surface_elevation_m: f32,
}

#[derive(Clone, Debug, Default)]
pub struct WetlandBody {
    pub id: String,
    pub surface_elevation_m: f32,
    pub vertices_xz: Vec<[f32; 2]>,
}

#[derive(Clone, Debug, Default)]
pub struct CavePoolBody {
    pub id: String,
    pub center_xz: [f32; 2],
    pub elevation_m: f32,
    pub radius_m: f32,
}

#[derive(Clone, Debug, Default)]
pub struct CompiledHydrologyProducts {
    pub sea_level_m: f32,
    pub lakes: Vec<LakeBody>,
    pub lagoons: Vec<LagoonBody>,
    pub waterfalls: Vec<WaterfallBody>,
    pub wetlands: Vec<WetlandBody>,
    pub cave_pools: Vec<CavePoolBody>,
}

pub fn realize_hydrology_from_atlas(
    atlas: &WorldAtlas,
    cave_pool_nodes: &[(f64, f64, f64, f32)],
) -> CompiledHydrologyProducts {
    let sea_level_m = atlas.metadata.extent.sea_level_m;
    let mut products = CompiledHydrologyProducts {
        sea_level_m,
        ..Default::default()
    };

    if let Some(graph) = atlas.graphs.hydrology.as_ref() {
        products.lakes = lakes_from_graph(atlas, graph);
        products.waterfalls = waterfalls_from_graph(atlas, graph);
        products.wetlands = wetlands_from_graph(atlas, graph);
    }

    if let Some(lagoon_field) = atlas.fields.get_scalar(Fk::LagoonSuitability) {
        products.lagoons = lagoons_from_field(atlas, lagoon_field.as_ref(), 0.4);
    }

    for (i, (x, y, z, radius)) in cave_pool_nodes.iter().enumerate() {
        products.cave_pools.push(CavePoolBody {
            id: format!("cave_pool.{i}"),
            center_xz: [*x as f32, *z as f32],
            elevation_m: *y as f32,
            radius_m: *radius,
        });
    }

    products
}

fn lakes_from_graph(atlas: &WorldAtlas, graph: &HydrologyGraph) -> Vec<LakeBody> {
    let cell = atlas.control_descriptor.cell_size_m as f32;
    let origin_x = atlas.control_descriptor.origin_x() as f32;
    let origin_z = atlas.control_descriptor.origin_z() as f32;
    graph
        .lakes
        .iter()
        .enumerate()
        .map(|(i, basin)| lake_from_basin(basin, i, cell, origin_x, origin_z))
        .collect()
}

fn lake_from_basin(
    basin: &LakeBasin,
    index: usize,
    cell: f32,
    origin_x: f32,
    origin_z: f32,
) -> LakeBody {
    let mut vertices = Vec::new();
    let mut cx = 0.0f32;
    let mut cz = 0.0f32;
    for (x, z) in &basin.cells {
        let wx = origin_x + *x as f32 * cell + cell * 0.5;
        let wz = origin_z + *z as f32 * cell + cell * 0.5;
        vertices.push([wx, wz]);
        cx += wx;
        cz += wz;
    }
    let n = basin.cells.len().max(1) as f32;
    LakeBody {
        id: format!("lake.{index}"),
        surface_elevation_m: basin.surface_elevation_m,
        vertices_xz: convex_hull_or_fan(&vertices),
        centroid_xz: [cx / n, cz / n],
    }
}

fn waterfalls_from_graph(atlas: &WorldAtlas, graph: &HydrologyGraph) -> Vec<WaterfallBody> {
    let cell = atlas.control_descriptor.cell_size_m as f32;
    let origin_x = atlas.control_descriptor.origin_x() as f32;
    let origin_z = atlas.control_descriptor.origin_z() as f32;
    graph
        .waterfalls
        .iter()
        .enumerate()
        .map(|(i, wf)| waterfall_from_candidate(wf, i, cell, origin_x, origin_z, atlas))
        .collect()
}

fn waterfall_from_candidate(
    wf: &WaterfallCandidate,
    index: usize,
    cell: f32,
    origin_x: f32,
    origin_z: f32,
    atlas: &WorldAtlas,
) -> WaterfallBody {
    let wx = origin_x + wf.from.0 as f32 * cell + cell * 0.5;
    let wz = origin_z + wf.from.1 as f32 * cell + cell * 0.5;
    let elev = atlas
        .fields
        .get_scalar(Fk::FilledElevation)
        .or_else(|| atlas.fields.get_scalar(Fk::CoastalElevation))
        .map(|f| f.sample_at_world(WorldXZ::new(wx as f64, wz as f64)))
        .unwrap_or(atlas.metadata.extent.sea_level_m);
    WaterfallBody {
        id: format!("waterfall.{index}"),
        position_xz: [wx, wz],
        drop_m: wf.drop_m,
        surface_elevation_m: elev,
    }
}

fn wetlands_from_graph(atlas: &WorldAtlas, graph: &HydrologyGraph) -> Vec<WetlandBody> {
    let cell = atlas.control_descriptor.cell_size_m as f32;
    let origin_x = atlas.control_descriptor.origin_x() as f32;
    let origin_z = atlas.control_descriptor.origin_z() as f32;
    graph
        .wetlands
        .iter()
        .enumerate()
        .map(|(i, region)| {
            let mut vertices = Vec::new();
            for (x, z) in &region.cells {
                vertices.push([
                    origin_x + *x as f32 * cell + cell * 0.5,
                    origin_z + *z as f32 * cell + cell * 0.5,
                ]);
            }
            WetlandBody {
                id: format!("wetland.{i}"),
                surface_elevation_m: atlas.metadata.extent.sea_level_m + 0.2,
                vertices_xz: convex_hull_or_fan(&vertices),
            }
        })
        .collect()
}

fn lagoons_from_field(atlas: &WorldAtlas, lagoon: &ScalarField, threshold: f32) -> Vec<LagoonBody> {
    let cell = lagoon.descriptor.cell_size_m as f32;
    let origin_x = lagoon.descriptor.origin_x() as f32;
    let origin_z = lagoon.descriptor.origin_z() as f32;
    let w = lagoon.descriptor.width;
    let h = lagoon.descriptor.height;
    let mut visited = vec![false; (w * h) as usize];
    let mut lagoons = Vec::new();
    let mut index = 0usize;
    for z in 0..h {
        for x in 0..w {
            let idx = (z * w + x) as usize;
            if visited[idx] || lagoon.get(x, z) < threshold {
                continue;
            }
            let mut cells = Vec::new();
            let mut stack = vec![(x, z)];
            visited[idx] = true;
            while let Some((cx, cz)) = stack.pop() {
                cells.push((cx, cz));
                for (nx, nz) in [
                    (cx.wrapping_sub(1), cz),
                    (cx + 1, cz),
                    (cx, cz.wrapping_sub(1)),
                    (cx, cz + 1),
                ] {
                    if nx >= w || nz >= h {
                        continue;
                    }
                    let nidx = (nz * w + nx) as usize;
                    if visited[nidx] || lagoon.get(nx, nz) < threshold {
                        continue;
                    }
                    visited[nidx] = true;
                    stack.push((nx, nz));
                }
            }
            if cells.len() < 4 {
                continue;
            }
            let mut vertices = Vec::new();
            let mut cx_sum = 0.0f32;
            let mut cz_sum = 0.0f32;
            for (gx, gz) in &cells {
                let wx = origin_x + *gx as f32 * cell + cell * 0.5;
                let wz = origin_z + *gz as f32 * cell + cell * 0.5;
                vertices.push([wx, wz]);
                cx_sum += wx;
                cz_sum += wz;
            }
            let n = cells.len() as f32;
            let centroid = [cx_sum / n, cz_sum / n];
            let surface = atlas
                .fields
                .get_scalar(Fk::FilledElevation)
                .map(|f| f.sample_at_world(WorldXZ::new(centroid[0] as f64, centroid[1] as f64)))
                .unwrap_or(atlas.metadata.extent.sea_level_m);
            lagoons.push(LagoonBody {
                id: format!("lagoon.{index}"),
                surface_elevation_m: surface.min(atlas.metadata.extent.sea_level_m + 2.0),
                vertices_xz: convex_hull_or_fan(&vertices),
                centroid_xz: centroid,
                salinity: 0.85,
            });
            index += 1;
        }
    }
    let _ = count_lagoon_components(lagoon, threshold);
    lagoons
}

fn convex_hull_or_fan(vertices: &[[f32; 2]]) -> Vec<[f32; 2]> {
    if vertices.len() <= 3 {
        return vertices.to_vec();
    }
    let mut cx = 0.0f32;
    let mut cz = 0.0f32;
    for v in vertices {
        cx += v[0];
        cz += v[1];
    }
    let n = vertices.len() as f32;
    let center = [cx / n, cz / n];
    let mut ordered: Vec<_> = vertices
        .iter()
        .map(|v| {
            let angle = (v[1] - center[1]).atan2(v[0] - center[0]);
            (angle, *v)
        })
        .collect();
    ordered.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    ordered.into_iter().map(|(_, v)| v).collect()
}
