use voxel_core::CHUNK_CELLS;

use crate::{ChunkMeshingInput, MeshingError, TerrainMeshData, TerrainMesher};

/// Surface Nets mesher for signed-density terrain.
#[derive(Clone, Copy, Debug, Default)]
pub struct SurfaceNetsMesher;

impl TerrainMesher for SurfaceNetsMesher {
    fn build_mesh(&self, input: &ChunkMeshingInput<'_>) -> Result<TerrainMeshData, MeshingError> {
        build_surface_nets(input)
    }
}

fn build_surface_nets(input: &ChunkMeshingInput<'_>) -> Result<TerrainMeshData, MeshingError> {
    let cells = input.chunk_cells;
    if cells != CHUNK_CELLS {
        return Err(MeshingError::Failed(format!(
            "unsupported chunk_cells {cells}, expected {CHUNK_CELLS}"
        )));
    }

    let padded = cells + 3;
    let expected = padded * padded * padded;
    if input.samples.len() != expected {
        return Err(MeshingError::Failed(format!(
            "expected {expected} padded samples, got {}",
            input.samples.len()
        )));
    }

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut materials = Vec::new();
    let mut material_ids = Vec::new();
    let mut material_weights = Vec::new();
    let mut indices = Vec::new();
    // Halo cells (-1..=cells) are required so boundary quads can reference neighbors.
    let halo = cells + 2;
    let mut cell_verts = vec![None; halo * halo * halo];

    for z in -1..=cells as i32 {
        for y in -1..=cells as i32 {
            for x in -1..=cells as i32 {
                let corners = corner_densities(input, x, y, z, padded);
                if !cell_has_surface(&corners) {
                    continue;
                }
                let pos = cell_vertex_position(&corners, x as f32, y as f32, z as f32);
                let normal = estimate_normal(input, pos, padded);
                let idx = positions.len() as u32;
                let (ids, weights) = cell_material_blend(input, x, y, z, padded);
                positions.push(pos);
                normals.push(normal);
                materials.push(ids[0]);
                material_ids.push(ids);
                material_weights.push(weights);
                cell_verts[halo_cell_index(x, y, z, cells)] = Some(idx);
            }
        }
    }

    emit_face_quads(input, &cell_verts, cells, padded, &mut indices);
    orient_triangles_toward_air(input, &positions, &mut indices, padded);

    Ok(TerrainMeshData {
        positions,
        normals,
        indices,
        materials,
        material_ids,
        material_weights,
    })
}

fn halo_cell_index(x: i32, y: i32, z: i32, cells: usize) -> usize {
    let stride = cells + 2;
    (x + 1) as usize + (y + 1) as usize * stride + (z + 1) as usize * stride * stride
}

fn sample_at(input: &ChunkMeshingInput<'_>, x: i32, y: i32, z: i32, padded: usize) -> f32 {
    let idx = (x + 1) as usize + (y + 1) as usize * padded + (z + 1) as usize * padded * padded;
    input.samples[idx].density
}

fn sample_material(input: &ChunkMeshingInput<'_>, x: i32, y: i32, z: i32, padded: usize) -> u16 {
    let idx = (x + 1) as usize + (y + 1) as usize * padded + (z + 1) as usize * padded * padded;
    input.samples[idx].material.0
}

fn corner_densities(
    input: &ChunkMeshingInput<'_>,
    x: i32,
    y: i32,
    z: i32,
    padded: usize,
) -> [f32; 8] {
    [
        sample_at(input, x, y, z, padded),
        sample_at(input, x + 1, y, z, padded),
        sample_at(input, x, y + 1, z, padded),
        sample_at(input, x + 1, y + 1, z, padded),
        sample_at(input, x, y, z + 1, padded),
        sample_at(input, x + 1, y, z + 1, padded),
        sample_at(input, x, y + 1, z + 1, padded),
        sample_at(input, x + 1, y + 1, z + 1, padded),
    ]
}

fn cell_has_surface(corners: &[f32; 8]) -> bool {
    let mut has_solid = false;
    let mut has_air = false;
    for &d in corners {
        if d <= 0.0 {
            has_solid = true;
        }
        if d > 0.0 {
            has_air = true;
        }
    }
    has_solid && has_air
}

fn edge_vertex(corners: &[f32; 8], edge: usize, ox: f32, oy: f32, oz: f32) -> Option<[f32; 3]> {
    const EDGES: [(usize, usize, f32, f32, f32, f32, f32, f32); 12] = [
        (0, 1, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0),
        (2, 3, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0),
        (4, 5, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0),
        (6, 7, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0),
        (0, 2, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0),
        (1, 3, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0),
        (4, 6, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0),
        (5, 7, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0),
        (0, 4, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0),
        (1, 5, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0),
        (2, 6, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0),
        (3, 7, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0),
    ];
    let (a, b, lx0, ly0, lz0, lx1, ly1, lz1) = EDGES[edge];
    let da = corners[a];
    let db = corners[b];
    if (da <= 0.0) == (db <= 0.0) {
        return None;
    }
    let t = da / (da - db);
    Some([
        ox + lx0 + (lx1 - lx0) * t,
        oy + ly0 + (ly1 - ly0) * t,
        oz + lz0 + (lz1 - lz0) * t,
    ])
}

fn cell_vertex_position(corners: &[f32; 8], ox: f32, oy: f32, oz: f32) -> [f32; 3] {
    let mut sum = [0.0f32; 3];
    let mut count = 0u32;
    for edge in 0..12 {
        if let Some(v) = edge_vertex(corners, edge, ox, oy, oz) {
            sum[0] += v[0];
            sum[1] += v[1];
            sum[2] += v[2];
            count += 1;
        }
    }
    if count == 0 {
        [ox + 0.5, oy + 0.5, oz + 0.5]
    } else {
        let n = count as f32;
        [sum[0] / n, sum[1] / n, sum[2] / n]
    }
}

fn estimate_normal(input: &ChunkMeshingInput<'_>, pos: [f32; 3], padded: usize) -> [f32; 3] {
    let eps = 0.5;
    let px = pos[0];
    let py = pos[1];
    let pz = pos[2];
    let dx = sample_at(input, (px + eps) as i32, py as i32, pz as i32, padded)
        - sample_at(input, (px - eps) as i32, py as i32, pz as i32, padded);
    let dy = sample_at(input, px as i32, (py + eps) as i32, pz as i32, padded)
        - sample_at(input, px as i32, (py - eps) as i32, pz as i32, padded);
    let dz = sample_at(input, px as i32, py as i32, (pz + eps) as i32, padded)
        - sample_at(input, px as i32, py as i32, (pz - eps) as i32, padded);
    normalize([dx, dy, dz])
}

fn cell_material_blend(
    input: &ChunkMeshingInput<'_>,
    x: i32,
    y: i32,
    z: i32,
    padded: usize,
) -> ([u16; 4], [f32; 4]) {
    let samples = [
        sample_material(input, x, y, z, padded),
        sample_material(input, x + 1, y, z, padded),
        sample_material(input, x, y + 1, z, padded),
        sample_material(input, x, y, z + 1, padded),
        sample_material(input, x + 1, y + 1, z, padded),
        sample_material(input, x, y + 1, z + 1, padded),
        sample_material(input, x + 1, y, z + 1, padded),
        sample_material(input, x + 1, y + 1, z + 1, padded),
    ];

    let mut weights = std::collections::HashMap::<u16, f32>::new();
    for mat in samples {
        *weights.entry(mat).or_insert(0.0) += 1.0;
    }

    let mut ranked: Vec<(u16, f32)> = weights.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut ids = [0u16; 4];
    let mut w = [0.0f32; 4];
    let mut total = 0.0f32;
    for (i, (mat, weight)) in ranked.into_iter().take(4).enumerate() {
        ids[i] = mat;
        w[i] = weight;
        total += weight;
    }
    if total > f32::EPSILON {
        for weight in &mut w {
            *weight /= total;
        }
    } else {
        w[0] = 1.0;
    }
    (ids, w)
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len <= f32::EPSILON {
        [0.0, 1.0, 0.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

fn edge_crosses(input: &ChunkMeshingInput<'_>, ax: i32, ay: i32, az: i32, bx: i32, by: i32, bz: i32, padded: usize) -> bool {
    let da = sample_at(input, ax, ay, az, padded);
    let db = sample_at(input, bx, by, bz, padded);
    (da <= 0.0) != (db <= 0.0)
}

fn maybe_quad(
    cell_verts: &[Option<u32>],
    cells: usize,
    indices: &mut Vec<u32>,
    c0: (i32, i32, i32),
    c1: (i32, i32, i32),
    c2: (i32, i32, i32),
    c3: (i32, i32, i32),
    reversed: bool,
) {
    let verts = [c0, c1, c2, c3].map(|(x, y, z)| {
        if x < -1 || y < -1 || z < -1 || x > cells as i32 || y > cells as i32 || z > cells as i32 {
            None
        } else {
            cell_verts[halo_cell_index(x, y, z, cells)]
        }
    });
    if let [Some(v0), Some(v1), Some(v2), Some(v3)] = verts {
        if reversed {
            indices.extend([v0, v2, v1, v0, v3, v2]);
        } else {
            indices.extend([v0, v1, v2, v0, v2, v3]);
        }
    }
}

fn orient_triangles_toward_air(
    input: &ChunkMeshingInput<'_>,
    positions: &[[f32; 3]],
    indices: &mut [u32],
    padded: usize,
) {
    for tri in indices.chunks_mut(3) {
        let p0 = positions[tri[0] as usize];
        let p1 = positions[tri[1] as usize];
        let p2 = positions[tri[2] as usize];
        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let normal = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let center = [
            (p0[0] + p1[0] + p2[0]) / 3.0,
            (p0[1] + p1[1] + p2[1]) / 3.0,
            (p0[2] + p1[2] + p2[2]) / 3.0,
        ];
        let gradient = estimate_normal(input, center, padded);
        let facing_air = normal[0] * gradient[0] + normal[1] * gradient[1] + normal[2] * gradient[2];
        if facing_air < 0.0 {
            tri.swap(1, 2);
        }
    }
}

fn emit_face_quads(
    input: &ChunkMeshingInput<'_>,
    cell_verts: &[Option<u32>],
    cells: usize,
    padded: usize,
    indices: &mut Vec<u32>,
) {
    // X-aligned grid edges
    for z in 0..=cells as i32 {
        for y in 0..=cells as i32 {
            for x in 0..cells as i32 {
                if !edge_crosses(input, x, y, z, x + 1, y, z, padded) {
                    continue;
                }
                let da = sample_at(input, x, y, z, padded);
                let reversed = da >= 0.0;
                maybe_quad(
                    cell_verts,
                    cells,
                    indices,
                    (x, y, z - 1),
                    (x, y - 1, z - 1),
                    (x, y - 1, z),
                    (x, y, z),
                    reversed,
                );
            }
        }
    }
    // Y-aligned grid edges
    for z in 0..=cells as i32 {
        for y in 0..cells as i32 {
            for x in 0..=cells as i32 {
                if !edge_crosses(input, x, y, z, x, y + 1, z, padded) {
                    continue;
                }
                let da = sample_at(input, x, y, z, padded);
                let reversed = da >= 0.0;
                maybe_quad(
                    cell_verts,
                    cells,
                    indices,
                    (x, y, z - 1),
                    (x, y, z),
                    (x - 1, y, z),
                    (x - 1, y, z - 1),
                    reversed,
                );
            }
        }
    }
    // Z-aligned grid edges
    for z in 0..cells as i32 {
        for y in 0..=cells as i32 {
            for x in 0..=cells as i32 {
                if !edge_crosses(input, x, y, z, x, y, z + 1, padded) {
                    continue;
                }
                let da = sample_at(input, x, y, z, padded);
                let reversed = da >= 0.0;
                maybe_quad(
                    cell_verts,
                    cells,
                    indices,
                    (x, y, z),
                    (x, y - 1, z),
                    (x - 1, y - 1, z),
                    (x - 1, y, z),
                    reversed,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ChunkMeshingInput;
    use voxel_core::{MaterialId, TerrainSample};

    fn padded_from_density<F>(cells: usize, mut f: F) -> Vec<TerrainSample>
    where
        F: FnMut(i32, i32, i32) -> f32,
    {
        let padded = cells + 3;
        let mut samples = Vec::with_capacity(padded * padded * padded);
        for z in -1..=(cells as i32 + 1) {
            for y in -1..=(cells as i32 + 1) {
                for x in -1..=(cells as i32 + 1) {
                    samples.push(TerrainSample {
                        density: f(x, y, z),
                        material: MaterialId(1),
                    });
                }
            }
        }
        samples
    }

    #[test]
    fn empty_air_chunk_produces_no_geometry() {
        let samples = padded_from_density(CHUNK_CELLS, |_, _, _| 1.0);
        let input = ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
        };
        let mesh = SurfaceNetsMesher.build_mesh(&input).unwrap();
        assert!(mesh.positions.is_empty());
    }

    #[test]
    fn solid_chunk_produces_no_geometry() {
        let samples = padded_from_density(CHUNK_CELLS, |_, _, _| -1.0);
        let input = ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
        };
        let mesh = SurfaceNetsMesher.build_mesh(&input).unwrap();
        assert!(mesh.positions.is_empty());
    }

    #[test]
    fn plane_triangles_face_air_side() {
        let samples = padded_from_density(CHUNK_CELLS, |_, y, _| y as f32 - 8.0);
        let input = ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
        };
        let mesh = SurfaceNetsMesher.build_mesh(&input).unwrap();
        assert!(!mesh.indices.is_empty());

        let mut upward = 0usize;
        let mut downward = 0usize;
        for tri in mesh.indices.chunks_exact(3) {
            let p0 = mesh.positions[tri[0] as usize];
            let p1 = mesh.positions[tri[1] as usize];
            let p2 = mesh.positions[tri[2] as usize];
            let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let ny = e1[2] * e2[0] - e1[0] * e2[2];
            let _ = e1[1] * e2[2] - e1[2] * e2[1];
            let _ = e1[0] * e2[1] - e1[1] * e2[0];
            if ny > 0.0 {
                upward += 1;
            } else if ny < 0.0 {
                downward += 1;
            }
        }
        assert!(
            upward > downward,
            "expected upward-facing triangles (up={upward}, down={downward})"
        );
        assert_eq!(downward, 0, "back-facing triangles indicate winding errors");
    }

    #[test]
    fn plane_produces_geometry() {
        let samples = padded_from_density(CHUNK_CELLS, |_, y, _| y as f32 - 8.0);
        let input = ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
        };
        let mesh = SurfaceNetsMesher.build_mesh(&input).unwrap();
        assert!(!mesh.positions.is_empty());
        assert!(!mesh.indices.is_empty());
    }

    #[test]
    fn sphere_triangles_face_outward() {
        let samples = padded_from_density(CHUNK_CELLS, |x, y, z| {
            let dx = x as f32 - 8.0;
            let dy = y as f32 - 8.0;
            let dz = z as f32 - 8.0;
            (dx * dx + dy * dy + dz * dz).sqrt() - 5.0
        });
        let input = ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
        };
        let mesh = SurfaceNetsMesher.build_mesh(&input).unwrap();
        let center = [8.0f32, 8.0, 8.0];
        let mut outward = 0usize;
        let mut inward = 0usize;
        for tri in mesh.indices.chunks_exact(3) {
            let p0 = mesh.positions[tri[0] as usize];
            let p1 = mesh.positions[tri[1] as usize];
            let p2 = mesh.positions[tri[2] as usize];
            let cx = (p0[0] + p1[0] + p2[0]) / 3.0;
            let cy = (p0[1] + p1[1] + p2[1]) / 3.0;
            let cz = (p0[2] + p1[2] + p2[2]) / 3.0;
            let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let nx = e1[1] * e2[2] - e1[2] * e2[1];
            let ny = e1[2] * e2[0] - e1[0] * e2[2];
            let nz = e1[0] * e2[1] - e1[1] * e2[0];
            let dot = nx * (cx - center[0]) + ny * (cy - center[1]) + nz * (cz - center[2]);
            if dot > 0.0 {
                outward += 1;
            } else if dot < 0.0 {
                inward += 1;
            }
        }
        assert!(
            outward > inward,
            "expected outward-facing sphere triangles (out={outward}, in={inward})"
        );
        assert_eq!(inward, 0, "inward-facing sphere triangles indicate inverted winding");
    }

    #[test]
    fn sphere_produces_geometry() {
        let samples = padded_from_density(CHUNK_CELLS, |x, y, z| {
            let dx = x as f32 - 8.0;
            let dy = y as f32 - 8.0;
            let dz = z as f32 - 8.0;
            (dx * dx + dy * dy + dz * dz).sqrt() - 5.0
        });
        let input = ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
        };
        let mesh = SurfaceNetsMesher.build_mesh(&input).unwrap();
        assert!(!mesh.positions.is_empty());
    }
}
