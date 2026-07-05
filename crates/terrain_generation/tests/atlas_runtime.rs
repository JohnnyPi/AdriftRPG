// crates/terrain_generation/tests/atlas_runtime.rs
use game_data::load_registry_from_directory;
use std::path::PathBuf;
use terrain_generation::{
    RecipeDensitySource, RiverCarveContext, RiverGenConfig, WorldVolumeBounds, atlas_content_hash,
    build_atlas_density_source, build_island_atlas, compile_terrain_recipe,
    generate_padded_samples, generate_river_spline, island_params_from_compiled,
    iter_world_chunk_coords, load_baked_atlas, resolve_baked_atlas_path,
};
use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMesher};
use voxel_core::{CHUNK_CELLS, MaterialId, WorldCell};

fn workspace_assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .canonicalize()
        .expect("assets")
}

fn island_large_world_id() -> shared::StableId {
    shared::StableId::new("world.island_large")
}

fn testbed_world_id() -> shared::StableId {
    shared::StableId::new("world.island_testbed")
}

fn build_authored_testbed_density_source(
    registry: &game_data::ConfigRegistry,
) -> RecipeDensitySource {
    let world = registry.world_by_id(&testbed_world_id()).expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let recipe = compile_terrain_recipe(registry, world, water, Some(world.seed)).expect("recipe");
    let bounds = WorldVolumeBounds::from_compiled_world(world);
    let mut source = RecipeDensitySource::new(recipe.clone()).with_world_bounds(bounds);
    let river_config = RiverGenConfig {
        seed: recipe.seed,
        surface_recipe: Some(recipe),
        source_center: [210.0, 324.0],
        source_radius_m: 48.0,
        grid_spacing_m: 2.0,
        mouth_width_m: 8.0,
        source_width_m: 2.0,
        source_depth_m: 0.5,
        mouth_depth_m: 1.8,
        bank_width_m: 3.5,
        minimum_depth_m: 0.25,
        ..RiverGenConfig::default()
    };
    if let Some(spline) = generate_river_spline(&river_config, water.sea_level_m) {
        source = source.with_river_carve(RiverCarveContext {
            spline,
            bank_width_m: river_config.bank_width_m,
        });
    }
    source
}

#[test]
#[ignore = "legacy island_gen; superseded by worldgen tier worlds (docs/worlds/)"]
fn atlas_harness_matches_runtime_island_params() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let world = registry
        .world_by_id(&island_large_world_id())
        .expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let base = registry
        .island_generation_for_world(world)
        .expect("island gen");
    let seed = world.seed;
    let mut merged = base.clone();
    merged.seed = seed;

    let harness =
        island_params_from_compiled(&merged, world, seed, water.sea_level_m).expect("params");
    let runtime =
        island_params_from_compiled(&merged, world, seed, water.sea_level_m).expect("params");
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
    assert_eq!(
        harness.coast.shelf_width_min_m,
        runtime.coast.shelf_width_min_m
    );
    assert_eq!(harness.beaches.width_max_m, runtime.beaches.width_max_m);
    assert_eq!(
        harness.caves.chamber_count_max,
        runtime.caves.chamber_count_max
    );
    assert_eq!(harness.resolution.regional_m, runtime.resolution.regional_m);
}

#[test]
#[ignore = "legacy island_gen; superseded by worldgen tier worlds (docs/worlds/)"]
fn testbed_spawn_chunk_has_mesh() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let source = build_authored_testbed_density_source(&registry);
    let (sx, sy, sz, report) = source.resolve_player_spawn(2.0, 48.0);
    assert!(report.passed, "spawn failed: {:?}", report.messages);

    let wx = sx;
    let wz = sz;
    let terrain_y = source.terrain_surface_height_at(wx, wz);
    assert!(
        terrain_y > source.recipe().sea_level + 0.1,
        "resolved spawn should be on land (y={terrain_y}, spawn=({sx:.1},{sy:.1},{sz:.1}))"
    );

    let spawn_chunk =
        WorldCell::new(sx.floor() as i32, sy.floor() as i32, sz.floor() as i32).chunk_coord();
    let samples = generate_padded_samples(&source, spawn_chunk, MaterialId(0));
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
        "spawn chunk {:?} at ({sx:.1},{sy:.1},{sz:.1}) should mesh (terrain_y={terrain_y})",
        spawn_chunk
    );
}

#[test]
#[ignore = "legacy island_gen; superseded by worldgen tier worlds (docs/worlds/)"]
fn testbed_meshes_surface_chunks() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let source = build_authored_testbed_density_source(&registry);
    let world = registry.world_by_id(&testbed_world_id()).expect("world");
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
                cell_stride: 1,
                surface_resolver: None,
            })
            .expect("mesh");
        if !mesh.positions.is_empty() {
            meshed += 1;
        }
    }
    assert!(
        meshed > 200,
        "expected most surface terrain chunks to mesh for authored testbed, got {meshed}"
    );
    eprintln!("island_testbed_authored meshed_chunks={meshed}");
}

#[test]
#[ignore = "legacy island_gen; superseded by worldgen tier worlds (docs/worlds/)"]
fn testbed_world_edge_columns_mesh_seabed() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let source = build_authored_testbed_density_source(&registry);
    let world = registry.world_by_id(&testbed_world_id()).expect("world");
    let (mins, maxs) = world.axis_bounds_m();
    let edge_x = 0.0;
    let edge_z = mins[2] + 24.0;
    let surface_y = source.terrain_surface_height_at(edge_x, edge_z);
    assert!(
        surface_y > mins[1] + 10.0 && surface_y < maxs[1] - 4.0,
        "world-edge seabed should stay inside the chunk volume (y={surface_y}, bounds [{}, {}))",
        mins[1],
        maxs[1]
    );

    let cell_y = surface_y.floor() as i32;
    let coord = WorldCell::new(edge_x.floor() as i32, cell_y, edge_z.floor() as i32).chunk_coord();
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
        "world-edge column ({edge_x:.0}, {edge_z:.0}) should produce seabed geometry"
    );
}

#[test]
#[ignore = "legacy island_gen; superseded by worldgen tier worlds (docs/worlds/)"]
fn golden_atlas_hash_matches_live_generation() {
    let assets = workspace_assets();
    let registry = load_registry_from_directory(&assets).expect("registry");
    let world = registry
        .world_by_id(&island_large_world_id())
        .expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let base = registry
        .island_generation_for_world(world)
        .expect("island gen");
    let seed = world.seed;
    let mut merged = base.clone();
    merged.seed = seed;
    let params =
        island_params_from_compiled(&merged, world, seed, water.sea_level_m).expect("params");
    let live = build_island_atlas(&params);
    let baked_rel = "terrain/baked/island_large.seed48130.atlas";
    let baked_path = resolve_baked_atlas_path(&assets, baked_rel);
    let loaded = load_baked_atlas(&baked_path, Some(world.id.as_str()), Some(seed))
        .expect("load golden atlas");
    assert_eq!(
        atlas_content_hash(&live),
        atlas_content_hash(&loaded),
        "committed golden atlas must match live procedural generation"
    );
}

#[test]
#[ignore = "legacy island_gen; superseded by worldgen tier worlds (docs/worlds/)"]
fn golden_spawn_height_stable() {
    let assets = workspace_assets();
    let registry = load_registry_from_directory(&assets).expect("registry");
    let world_id = island_large_world_id();
    let world = registry.world_by_id(&world_id).expect("world");
    let procedural = build_atlas_density_source(&registry, &world_id, world.seed, None, None);
    let baked = build_atlas_density_source(&registry, &world_id, world.seed, Some(&assets), None);
    let (px, py, pz, _) = procedural.resolve_player_spawn(2.0, 48.0);
    let (bx, by, bz, _) = baked.resolve_player_spawn(2.0, 48.0);
    assert!((px - bx).abs() < 0.01, "spawn X drift {px} vs {bx}");
    assert!((py - by).abs() < 0.01, "spawn Y drift {py} vs {by}");
    assert!((pz - bz).abs() < 0.01, "spawn Z drift {pz} vs {bz}");
}

#[test]
#[ignore = "legacy island_gen; superseded by worldgen tier worlds (docs/worlds/)"]
fn golden_mesh_vertex_count_band() {
    let assets = workspace_assets();
    let registry = load_registry_from_directory(&assets).expect("registry");
    let world_id = island_large_world_id();
    let world = registry.world_by_id(&world_id).expect("world");
    let procedural = build_atlas_density_source(&registry, &world_id, world.seed, None, None);
    let baked = build_atlas_density_source(&registry, &world_id, world.seed, Some(&assets), None);
    let (sx, sy, sz, _) = procedural.resolve_player_spawn(2.0, 48.0);
    let spawn_chunk =
        WorldCell::new(sx.floor() as i32, sy.floor() as i32, sz.floor() as i32).chunk_coord();

    let mesh_count = |source: &terrain_generation::RecipeDensitySource| {
        let samples = generate_padded_samples(source, spawn_chunk, MaterialId(0));
        SurfaceNetsMesher
            .build_mesh(&ChunkMeshingInput {
                samples: &samples,
                chunk_cells: CHUNK_CELLS,
                cell_stride: 1,
                surface_resolver: None,
            })
            .expect("mesh")
            .positions
            .len()
    };

    let proc_verts = mesh_count(&procedural);
    let baked_verts = mesh_count(&baked);
    let delta = (proc_verts as i64 - baked_verts as i64).unsigned_abs();
    assert!(
        delta <= 4,
        "spawn chunk vertex count drift too large: procedural={proc_verts} baked={baked_verts}"
    );
}

#[test]
#[ignore = "legacy island_gen; superseded by worldgen tier worlds (docs/worlds/)"]
fn baked_atlas_rejects_wrong_seed() {
    let assets = workspace_assets();
    let registry = load_registry_from_directory(&assets).expect("registry");
    let world = registry
        .world_by_id(&island_large_world_id())
        .expect("world");
    let baked_rel = "terrain/baked/island_large.seed48130.atlas";
    let baked_path = resolve_baked_atlas_path(&assets, baked_rel);
    let wrong_seed = world.seed.wrapping_add(1);
    assert!(
        load_baked_atlas(&baked_path, Some(world.id.as_str()), Some(wrong_seed)).is_err(),
        "golden atlas load must reject a mismatched seed"
    );
    let atlas = load_baked_atlas(&baked_path, Some(world.id.as_str()), Some(world.seed))
        .expect("load golden atlas");
    assert_eq!(atlas.seed, world.seed);
}
