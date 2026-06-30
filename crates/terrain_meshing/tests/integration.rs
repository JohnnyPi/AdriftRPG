use terrain_generation::{iter_world_chunk_coords, VerticalSliceDensitySource};
use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMesher};
use voxel_core::{ChunkCoord, MaterialId, CHUNK_CELLS};

#[test]
fn integration_generate_and_mesh_all_chunks() {
    let source = VerticalSliceDensitySource::new(48129, 2.0);
    let extent = [6i32, 3, 6];
    let mesher = SurfaceNetsMesher;
    let mut meshed = 0usize;
    for coord in iter_world_chunk_coords(extent) {
        let samples = terrain_generation::generate_padded_samples(&source, coord, MaterialId(0));
        let input = ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
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
    let source = VerticalSliceDensitySource::new(48129, 2.0);
    let coord = ChunkCoord::new(1, 0, 0);
    let samples = terrain_generation::generate_padded_samples(&source, coord, MaterialId(0));
    let mesh = SurfaceNetsMesher
        .build_mesh(&ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
        })
        .expect("mesh");
    assert!(!mesh.positions.is_empty(), "cave chunk should have surface geometry");
    assert_eq!(mesh.material_ids.len(), mesh.positions.len());
}

#[test]
fn overhang_region_produces_geometry() {
    let source = VerticalSliceDensitySource::new(48129, 2.0);
    let coord = ChunkCoord::new(1, 1, 0);
    let samples = terrain_generation::generate_padded_samples(&source, coord, MaterialId(0));
    let mesh = SurfaceNetsMesher
        .build_mesh(&ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
        })
        .expect("mesh");
    assert!(!mesh.positions.is_empty(), "overhang chunk should have geometry");
}

#[test]
fn neighbor_chunk_boundary_vertices_align() {
    let source = VerticalSliceDensitySource::new(48129, 2.0);
    let mesher = SurfaceNetsMesher;
    let left_samples =
        terrain_generation::generate_padded_samples(&source, ChunkCoord::new(0, 1, 0), MaterialId(0));
    let right_samples =
        terrain_generation::generate_padded_samples(&source, ChunkCoord::new(1, 1, 0), MaterialId(0));
    let left = mesher
        .build_mesh(&ChunkMeshingInput {
            samples: &left_samples,
            chunk_cells: CHUNK_CELLS,
        })
        .expect("left");
    let right = mesher
        .build_mesh(&ChunkMeshingInput {
            samples: &right_samples,
            chunk_cells: CHUNK_CELLS,
        })
        .expect("right");

    let boundary_x = CHUNK_CELLS as f32;
    let mut left_boundary = Vec::new();
    for pos in &left.positions {
        if pos[0] >= boundary_x - 0.5 {
            left_boundary.push(*pos);
        }
    }
    assert!(!left_boundary.is_empty(), "expected vertices near +X chunk face");
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
