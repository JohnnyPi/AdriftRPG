//! Release profiling harness for vertical slice terrain pipeline (§24) and FPS budgets.

use std::time::Instant;

use terrain_generation::{default_vertical_slice_recipe, iter_world_chunk_coords, RecipeDensitySource};
use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMesher};
use voxel_core::{MaterialId, CHUNK_CELLS};

/// VS1 §24 terrain lifecycle budget (planning limits, per-chunk averages).
const MAX_DENSITY_MS_PER_CHUNK: f32 = 2.5;
const MAX_MESH_MS_PER_CHUNK: f32 = 4.0;

fn profile_world(label: &str, source: RecipeDensitySource, extent: [i32; 3]) -> bool {
    let mesher = SurfaceNetsMesher;

    let density_start = Instant::now();
    for coord in iter_world_chunk_coords(extent) {
        let _ = terrain_generation::generate_padded_samples(&source, coord, MaterialId(0));
    }
    let density_ms = density_start.elapsed().as_secs_f32() * 1000.0;

    let mesh_start = Instant::now();
    let mut mesh_count = 0usize;
    for coord in iter_world_chunk_coords(extent) {
        let samples = terrain_generation::generate_padded_samples(&source, coord, MaterialId(0));
        let mesh = mesher
            .build_mesh(&ChunkMeshingInput {
                samples: &samples,
                chunk_cells: CHUNK_CELLS,
            })
            .expect("mesh");
        if !mesh.positions.is_empty() {
            mesh_count += 1;
        }
    }
    let mesh_ms = mesh_start.elapsed().as_secs_f32() * 1000.0;

    let chunk_count: usize = extent.iter().map(|e| (*e as usize) * 2 + 1).product();
    let density_per_chunk = density_ms / chunk_count.max(1) as f32;
    let mesh_per_chunk = mesh_ms / chunk_count.max(1) as f32;
    let density_ok = density_per_chunk <= MAX_DENSITY_MS_PER_CHUNK;
    let mesh_ok = mesh_per_chunk <= MAX_MESH_MS_PER_CHUNK;

    println!("{label}");
    println!("  Density generation ({chunk_count} chunks): {density_ms:.1} ms ({density_per_chunk:.2} ms/chunk) [{density_ok}]");
    println!("  Mesh generation ({mesh_count} non-empty): {mesh_ms:.1} ms ({mesh_per_chunk:.2} ms/chunk) [{mesh_ok}]");

    density_ok && mesh_ok
}

fn main() {
    let compact_ok = profile_world(
        "Vertical slice profile — compact (seed 48129)",
        RecipeDensitySource::new(default_vertical_slice_recipe(48129, 2.0)),
        [6, 3, 6],
    );
    let expanded_ok = profile_world(
        "Vertical slice profile — expanded_slice (seed 48129)",
        RecipeDensitySource::new(default_vertical_slice_recipe(48129, 2.0)),
        [8, 4, 8],
    );

    println!();
    println!("Release FPS validation (VS1 §30):");
    println!("  Target: 60 FPS (16.67 ms/frame) @ 2560x1440");
    println!("  Run: RPG_ADRIFT_FPS_BENCHMARK=30 cargo run --release --bin rpg_adrift");
    println!("  Terrain pipeline budgets: {}", if compact_ok && expanded_ok { "PASS" } else { "REVIEW" });

    if !compact_ok || !expanded_ok {
        std::process::exit(1);
    }
}
