// crates/terrain_meshing/src/surface_nets.rs
use terrain_surface::{ChunkSlotRemapper, MaterialVertex};
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
    let mut material_vertices = Vec::new();
    let mut indices = Vec::new();
    let mut fallback_remapper = ChunkSlotRemapper::new();
    // Halo cells (-1..cells) hold min-face vertices; max-face edges belong to +neighbors.
    let halo = cells + 2;
    let mut cell_verts = vec![None; halo * halo * halo];

    let stride = input.cell_stride.max(1) as i32;
    let step = stride as usize;

    for z in (-1..cells as i32).step_by(step) {
        for y in (-1..cells as i32).step_by(step) {
            for x in (-1..cells as i32).step_by(step) {
                let corners = corner_densities(input, x, y, z, padded);
                if !cell_has_surface(&corners) {
                    continue;
                }
                let pos = cell_vertex_position(&corners, x as f32, y as f32, z as f32);
                let normal = estimate_normal(input, pos, padded);
                let idx = positions.len() as u32;
                let vertex = if let Some(resolver) = input.surface_resolver {
                    resolver.vertex_blend(pos, normal)
                } else {
                    let (ids, weights) = cell_material_blend(input, x, y, z, padded);
                    let globals = [ids[0] as u32, ids[1] as u32, ids[2] as u32, ids[3] as u32];
                    terrain_surface::remap_blend_to_local_slots(
                        globals,
                        weights,
                        &mut fallback_remapper,
                    )
                };
                positions.push(pos);
                normals.push(normal);
                materials.push(dominant_local_slot(vertex));
                material_vertices.push(vertex);
                cell_verts[halo_cell_index(x, y, z, cells)] = Some(idx);
            }
        }
    }

    emit_face_quads(input, &cell_verts, cells, padded, &mut indices);

    let chunk_palette = if let Some(resolver) = input.surface_resolver {
        resolver.chunk_palette()
    } else {
        fallback_remapper.finish()
    };

    Ok(TerrainMeshData {
        positions,
        normals,
        indices,
        materials,
        material_vertices,
        chunk_palette,
    })
}

fn dominant_local_slot(vertex: MaterialVertex) -> u16 {
    vertex
        .local_indices
        .iter()
        .zip(vertex.weights.iter())
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| *i as u16)
        .unwrap_or(0)
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
    let cells = input.chunk_cells as i32;
    let clamp = |c: i32| c.clamp(-1, cells + 1);
    let xi = pos[0].round() as i32;
    let yi = pos[1].round() as i32;
    let zi = pos[2].round() as i32;
    let dx = sample_at(input, clamp(xi + 1), yi, zi, padded)
        - sample_at(input, clamp(xi - 1), yi, zi, padded);
    let dy = sample_at(input, xi, clamp(yi + 1), zi, padded)
        - sample_at(input, xi, clamp(yi - 1), zi, padded);
    let dz = sample_at(input, xi, yi, clamp(zi + 1), padded)
        - sample_at(input, xi, yi, clamp(zi - 1), padded);
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

    let mut ranked = [(0u16, 0f32); 8];
    let mut count = 0usize;
    'outer: for mat in samples {
        for entry in &mut ranked[..count] {
            if entry.0 == mat {
                entry.1 += 1.0;
                continue 'outer;
            }
        }
        if count < ranked.len() {
            ranked[count] = (mat, 1.0);
            count += 1;
        }
    }

    ranked[..count].sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut ids = [0u16; 4];
    let mut w = [0.0f32; 4];
    let mut total = 0.0f32;
    for (i, (mat, weight)) in ranked[..count].iter().copied().take(4).enumerate() {
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

fn edge_crosses(
    input: &ChunkMeshingInput<'_>,
    ax: i32,
    ay: i32,
    az: i32,
    bx: i32,
    by: i32,
    bz: i32,
    padded: usize,
) -> bool {
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

fn emit_face_quads(
    input: &ChunkMeshingInput<'_>,
    cell_verts: &[Option<u32>],
    cells: usize,
    padded: usize,
    indices: &mut Vec<u32>,
) {
    // X-aligned grid edges
    for z in 0..cells as i32 {
        for y in 0..cells as i32 {
            for x in 0..cells as i32 {
                if !edge_crosses(input, x, y, z, x + 1, y, z, padded) {
                    continue;
                }
                let da = sample_at(input, x, y, z, padded);
                let reversed = da <= 0.0;
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
    for z in 0..cells as i32 {
        for y in 0..cells as i32 {
            for x in 0..cells as i32 {
                if !edge_crosses(input, x, y, z, x, y + 1, z, padded) {
                    continue;
                }
                let da = sample_at(input, x, y, z, padded);
                let reversed = da <= 0.0;
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
        for y in 0..cells as i32 {
            for x in 0..cells as i32 {
                if !edge_crosses(input, x, y, z, x, y, z + 1, padded) {
                    continue;
                }
                let da = sample_at(input, x, y, z, padded);
                let reversed = da <= 0.0;
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
