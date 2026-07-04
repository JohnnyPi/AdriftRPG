// crates/terrain_meshing/tests/integration.rs
use terrain_generation::{
    RecipeDensitySource, default_vertical_slice_recipe, generate_padded_samples,
    iter_world_chunk_coords,
};
use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMesher};
use voxel_core::{CHUNK_CELLS, ChunkCoord, MaterialId};

fn test_density_source(seed: u64) -> RecipeDensitySource {
    RecipeDensitySource::new(default_vertical_slice_recipe(seed, 2.0))
}

#[test]
fn integration_generate_and_mesh_all_chunks() {
    let source = test_density_source(48129);
    let extent = [6i32, 3, 6];
    let mesher = SurfaceNetsMesher;
    let mut meshed = 0usize;
    for coord in iter_world_chunk_coords(extent) {
        let samples = generate_padded_samples(&source, coord, MaterialId(0));
        let input = ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
            cell_stride: 1,
            surface_resolver: None,
        };
        let mesh = mesher.build_mesh(&input).expect("mesh");
        if !mesh.positions.is_empty() {
            meshed += 1;
        }
    }
    assert!(meshed > 10, "expected visible terrain chunks, got {meshed}");
}

#[test]
fn cave_region_produces_hollow_mesh() {
    let source = test_density_source(48129);
    let coord = ChunkCoord::new(1, 0, 0);
    let samples = generate_padded_samples(&source, coord, MaterialId(0));
    let mesh = SurfaceNetsMesher
        .build_mesh(&ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
            cell_stride: 1,
            surface_resolver: None,
        })
        .expect("mesh");
    assert!(
        !mesh.positions.is_empty(),
        "cave chunk should have surface geometry"
    );
    assert_eq!(mesh.material_vertices.len(), mesh.positions.len());
}

#[test]
fn overhang_region_produces_geometry() {
    let source = test_density_source(48129);
    let coord = ChunkCoord::new(1, 1, 0);
    let samples = generate_padded_samples(&source, coord, MaterialId(0));
    let mesh = SurfaceNetsMesher
        .build_mesh(&ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
            cell_stride: 1,
            surface_resolver: None,
        })
        .expect("mesh");
    assert!(
        !mesh.positions.is_empty(),
        "overhang chunk should have geometry"
    );
}

#[test]
fn neighbor_chunk_boundary_vertices_align() {
    let source = test_density_source(48129);
    let mesher = SurfaceNetsMesher;
    let left_samples = generate_padded_samples(&source, ChunkCoord::new(0, 1, 0), MaterialId(0));
    let right_samples = generate_padded_samples(&source, ChunkCoord::new(1, 1, 0), MaterialId(0));
    let left = mesher
        .build_mesh(&ChunkMeshingInput {
            samples: &left_samples,
            chunk_cells: CHUNK_CELLS,
            cell_stride: 1,
            surface_resolver: None,
        })
        .expect("left");
    let right = mesher
        .build_mesh(&ChunkMeshingInput {
            samples: &right_samples,
            chunk_cells: CHUNK_CELLS,
            cell_stride: 1,
            surface_resolver: None,
        })
        .expect("right");

    let boundary_x = CHUNK_CELLS as f32;
    let mut left_boundary = Vec::new();
    for pos in &left.positions {
        if pos[0] >= boundary_x - 0.5 {
            left_boundary.push(*pos);
        }
    }
    assert!(
        !left_boundary.is_empty(),
        "expected vertices near +X chunk face"
    );
    let mut matched = 0usize;
    for lpos in &left_boundary {
        for rpos in &right.positions {
            if rpos[0] > 0.5 {
                continue;
            }
            if (lpos[1] - rpos[1]).abs() < 0.25 && (lpos[2] - rpos[2]).abs() < 0.25 {
                matched += 1;
                break;
            }
        }
    }
    assert!(
        matched > 0,
        "expected seam vertex correspondence, matched {matched} of {}",
        left_boundary.len()
    );
}

#[test]
fn all_adjacent_chunk_pairs_have_boundary_vertex_correspondence() {
    let source = test_density_source(48129);
    let mesher = SurfaceNetsMesher;
    let extent = [6i32, 3, 6];
    let coords: Vec<_> = iter_world_chunk_coords(extent).collect();
    let mut checked = 0usize;
    let mut matched_pairs = 0usize;

    for a in &coords {
        for b in &coords {
            if a.x + 1 == b.x && a.y == b.y && a.z == b.z {
                if assert_chunk_x_seam(&source, &mesher, *a, *b) {
                    matched_pairs += 1;
                }
                checked += 1;
            }
        }
    }

    assert!(
        matched_pairs > 0,
        "expected at least one seam pair with matching boundary verts (checked {checked})"
    );
}

/// Returns true when the pair was checked and passed; false when skipped (no surface on seam).
fn assert_chunk_x_seam(
    source: &RecipeDensitySource,
    mesher: &SurfaceNetsMesher,
    left: ChunkCoord,
    right: ChunkCoord,
) -> bool {
    let left_samples = generate_padded_samples(source, left, MaterialId(0));
    let right_samples = generate_padded_samples(source, right, MaterialId(0));
    let left = mesher
        .build_mesh(&ChunkMeshingInput {
            samples: &left_samples,
            chunk_cells: CHUNK_CELLS,
            cell_stride: 1,
            surface_resolver: None,
        })
        .expect("left mesh");
    let right = mesher
        .build_mesh(&ChunkMeshingInput {
            samples: &right_samples,
            chunk_cells: CHUNK_CELLS,
            cell_stride: 1,
            surface_resolver: None,
        })
        .expect("right mesh");

    if left.indices.is_empty() || right.indices.is_empty() {
        return false;
    }

    if left.positions.is_empty() || right.positions.is_empty() {
        return false;
    }

    let boundary_x = CHUNK_CELLS as f32;
    let mut left_boundary = Vec::new();
    for pos in &left.positions {
        if pos[0] >= boundary_x - 0.5 {
            left_boundary.push(*pos);
        }
    }
    if left_boundary.is_empty() {
        return false;
    }

    let mut matched = 0usize;
    for lpos in &left_boundary {
        for rpos in &right.positions {
            if rpos[0] > 0.5 {
                continue;
            }
            if (lpos[1] - rpos[1]).abs() < 0.35 && (lpos[2] - rpos[2]).abs() < 0.35 {
                matched += 1;
                break;
            }
        }
    }

    let match_ratio = matched as f32 / left_boundary.len() as f32;
    assert!(
        match_ratio > 0.5,
        "chunk seam {:?}/{:?} matched only {matched}/{} boundary verts",
        left,
        right,
        left_boundary.len()
    );
    true
}
