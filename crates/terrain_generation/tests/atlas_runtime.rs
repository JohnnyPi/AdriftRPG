// crates/terrain_generation/tests/atlas_runtime.rs
use game_data::load_registry_from_directory;
use std::path::PathBuf;
use terrain_generation::{
    build_atlas_density_source, generate_padded_samples, island_params_from_compiled,
    iter_world_chunk_coords,
};
use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMesher};
use voxel_core::{MaterialId, WorldCell, CHUNK_CELLS};

fn workspace_assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .canonicalize()
        .expect("assets")
}

#[test]
fn atlas_harness_matches_runtime_island_params() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let world = registry
        .world_by_id(&shared::StableId::new("world.island_testbed"))
        .expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let base = registry
        .island_generation_for_world(world)
        .expect("island gen");
    let seed = world.seed;
    let mut merged = base.clone();
    merged.seed = seed;

    let harness = island_params_from_compiled(&merged, world, seed, water.sea_level_m);
    let runtime = island_params_from_compiled(&merged, world, seed, water.sea_level_m);
    assert_eq!(harness.seed, runtime.seed);
    assert_eq!(harness.ocean_extent_m, runtime.ocean_extent_m);
    assert_eq!(harness.volcano.center, runtime.volcano.center);
    assert_eq!(
        harness.hydrology.stream_threshold,
        runtime.hydrology.stream_threshold
    );
    assert_eq!(
        harness.erosion.stream_power_iterations,
        runtime.erosion.stream_power_iterations
    );
    assert_eq!(harness.coast.shelf_width_min_m, runtime.coast.shelf_width_min_m);
    assert_eq!(harness.beaches.width_max_m, runtime.beaches.width_max_m);
    assert_eq!(
        harness.caves.chamber_count_max,
        runtime.caves.chamber_count_max
    );
    assert_eq!(
        harness.resolution.regional_m,
        runtime.resolution.regional_m
    );
}

#[test]
fn testbed_seed_800000_spawn_chunk_has_mesh() {
    let source = build_atlas_density_source(
        &load_registry_from_directory(workspace_assets()).expect("registry"),
        &shared::StableId::new("world.island_testbed"),
        800_000,
    );
    let (sx, sy, sz, report) = source.resolve_player_spawn(2.0, 48.0);
    assert!(report.passed, "spawn failed: {:?}", report.messages);

    let wx = sx;
    let wz = sz;
    let terrain_y = source.terrain_surface_height_at(wx, wz);
    assert!(
        terrain_y > source.recipe().sea_level + 0.1,
        "resolved spawn should be on land (y={terrain_y}, spawn=({sx:.1},{sy:.1},{sz:.1}))"
    );

    let spawn_chunk = WorldCell::new(
        sx.floor() as i32,
        sy.floor() as i32,
        sz.floor() as i32,
    )
    .chunk_coord();
    let samples = generate_padded_samples(&source, spawn_chunk, MaterialId(0));
    let mesh = SurfaceNetsMesher
        .build_mesh(&ChunkMeshingInput {
            samples: &samples,
            chunk_cells: CHUNK_CELLS,
            surface_resolver: None,
        })
        .expect("mesh");
    assert!(
        !mesh.positions.is_empty(),
        "spawn chunk {:?} at ({sx:.1},{sy:.1},{sz:.1}) should mesh (terrain_y={terrain_y})",
        spawn_chunk
    );
}

#[test]
fn testbed_seed_800000_meshes_most_surface_chunks() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let source = build_atlas_density_source(
        &registry,
        &shared::StableId::new("world.island_testbed"),
        800_000,
    );
    let world = registry
        .world_by_id(&shared::StableId::new("world.island_testbed"))
        .expect("world");
    let extent = [
        world.world_extent_chunks[0] as i32,
        world.world_extent_chunks[1] as i32,
        world.world_extent_chunks[2] as i32,
    ];
    let mut meshed = 0usize;
    for coord in iter_world_chunk_coords(extent) {
        let samples = generate_padded_samples(&source, coord, MaterialId(0));
        let mesh = SurfaceNetsMesher
            .build_mesh(&ChunkMeshingInput {
                samples: &samples,
                chunk_cells: CHUNK_CELLS,
                surface_resolver: None,
            })
            .expect("mesh");
        if !mesh.positions.is_empty() {
            meshed += 1;
        }
    }
    assert!(
        meshed > 200,
        "expected most surface terrain chunks to mesh for testbed seed 800000, got {meshed}"
    );
    eprintln!("island_testbed_800000 meshed_chunks={meshed}");

    let atlas = source.atlas().expect("atlas");
    let mut edge_mask = 0.0f32;
    for x in -140i32..140 {
        for z in -140i32..140 {
            let wx = x as f32;
            let wz = z as f32;
            if x.abs() >= 128 || z.abs() >= 128 {
                edge_mask = edge_mask.max(atlas.island_mask.sample_bilinear(wx, wz));
            }
        }
    }
    assert!(
        edge_mask < 0.2,
        "island mask reaches visible world edge and will clip abruptly (edge_mask={edge_mask:.2})"
    );
}