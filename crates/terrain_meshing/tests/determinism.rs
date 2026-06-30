use terrain_generation::VerticalSliceDensitySource;
use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMesher};
use voxel_core::{ChunkCoord, MaterialId, CHUNK_CELLS};

fn mesh_topology_hash(source: &VerticalSliceDensitySource, coord: ChunkCoord) -> u64 {
    use std::hash::{Hash, Hasher};
    let samples = terrain_generation::generate_padded_samples(source, coord, MaterialId(0));
    let mesh = SurfaceNetsMesher
        .build_mesh(&ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
        })
        .expect("mesh");
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for p in &mesh.positions {
        ((p[0] * 1000.0).round() as i32).hash(&mut hasher);
        ((p[1] * 1000.0).round() as i32).hash(&mut hasher);
        ((p[2] * 1000.0).round() as i32).hash(&mut hasher);
    }
    mesh.indices.len().hash(&mut hasher);
    hasher.finish()
}

#[test]
fn determinism_same_seed_same_mesh_topology() {
    let a = VerticalSliceDensitySource::new(48129, 2.0);
    let b = VerticalSliceDensitySource::new(48129, 2.0);
    let coord = ChunkCoord::new(0, 1, 0);
    assert_eq!(
        mesh_topology_hash(&a, coord),
        mesh_topology_hash(&b, coord),
        "mesh topology should be deterministic for same seed"
    );
}

#[test]
fn different_seeds_may_differ() {
    let a = VerticalSliceDensitySource::new(48129, 2.0);
    let b = VerticalSliceDensitySource::new(48130, 2.0);
    let coord = ChunkCoord::new(0, 1, 0);
    let ha = mesh_topology_hash(&a, coord);
    let hb = mesh_topology_hash(&b, coord);
    assert_ne!(ha, hb, "different seeds should produce different topology hashes");
}
