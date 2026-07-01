use game_data::load_registry_from_directory;
use std::path::PathBuf;
use terrain_generation::{build_island_atlas, iter_world_chunk_coords, RecipeDensitySource};
use terrain_meshing::TerrainMesher;

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .canonicalize()
        .expect("assets")
}

#[test]
fn vs3_island_has_peak_and_chunk_coverage() {
    let registry = load_registry_from_directory(assets()).expect("registry");
    let world = registry
        .world_by_id(&shared::StableId::new("world.vs3_island"))
        .expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let island = registry
        .island_generation_for_world(world)
        .expect("island gen");

    let mut params = terrain_generation::IslandGenParams::default();
    params.seed = world.seed;
    params.ocean_extent_m = world.ocean_extent_m.unwrap_or(288.0);
    params.resolution = terrain_generation::GenerationResolution::for_extent(params.ocean_extent_m);
    if let Some(ref res) = world.resolution {
        params.resolution.regional_m = res.regional_m.unwrap_or(params.resolution.regional_m);
        params.resolution.local_m = res.local_m.unwrap_or(params.resolution.local_m);
        params.resolution.voxel_m = res.voxel_m.unwrap_or(1.0);
    }
    params.island.playable_diameter_m = island.island.playable_diameter_m;
    params.island.maximum_height_m = island.island.maximum_height_m;
    params.island.sea_level_m = water.sea_level_m;
    params.volcano.center = [
        island.volcano.center[0] - world.coord_offset[0],
        island.volcano.center[1] - world.coord_offset[2],
    ];
    params.volcano.shield_radius_m = island.volcano.shield_radius_m;
    params.volcano.shield_height_m = island.volcano.shield_height_m;
    params.volcano.summit_height_m = island.volcano.summit_height_m;
    params.volcano.shield_exponent = island.volcano.shield_exponent;
    params.volcano.summit_exponent = island.volcano.summit_exponent;
    params.volcano.summit_radius_m = island.volcano.summit_radius_m;
    params.volcano.caldera_radius_m = island.volcano.caldera_radius_m;
    params.volcano.caldera_depth_m = island.volcano.caldera_depth_m;
    params.surface_noise = terrain_generation::SurfaceNoiseParams {
        regional_amplitude_m: island.surface_noise.regional_amplitude_m,
        local_amplitude_m: island.surface_noise.local_amplitude_m,
        voxel_amplitude_m: island.surface_noise.voxel_amplitude_m,
    };

    let atlas = build_island_atlas(&params);
    let recipe = terrain_generation::TerrainRecipe {
        seed: world.seed,
        sea_level: water.sea_level_m,
        spawn_x: 70.0,
        spawn_z: 160.0,
        coord_offset: world.coord_offset,
        ops: vec![],
    };
    let source = RecipeDensitySource::new(recipe).with_atlas(atlas);

    let mut max_h = f32::MIN;
    let mut min_land_h = f32::MAX;
    let mut land_samples = 0u32;
    let mut alpine_candidates = 0u32;
    let mut rocky_candidates = 0u32;
    let mut lowland_candidates = 0u32;

    for cx in -128..128 {
        for cz in -128..128 {
            let h = source.terrain_surface_height_at(cx as f32, cz as f32);
            if h > water.sea_level_m + 0.5 {
                land_samples += 1;
                max_h = max_h.max(h);
                min_land_h = min_land_h.min(h);
                let elev = h - water.sea_level_m;
                if elev >= 28.0 {
                    alpine_candidates += 1;
                } else if elev >= 12.0 {
                    rocky_candidates += 1;
                } else {
                    lowland_candidates += 1;
                }
            }
        }
    }

    eprintln!(
        "resolution regional={} local={} max_h={max_h:.1} min_land={min_land_h:.1} land_samples={land_samples} alpine={alpine_candidates} rocky={rocky_candidates} lowland={lowland_candidates}",
        params.resolution.regional_m,
        params.resolution.local_m
    );

    assert!(
        max_h > water.sea_level_m + 60.0,
        "VS3 peak too low: {max_h:.1} m (need alpine biomes)"
    );
    assert!(
        lowland_candidates > 100,
        "VS3 missing lowland elevations: {lowland_candidates} samples"
    );
    assert!(
        alpine_candidates > 10,
        "VS3 missing alpine elevations: {alpine_candidates} samples"
    );

    let extent = [
        world.world_extent_chunks[0] as i32,
        world.world_extent_chunks[1] as i32,
        world.world_extent_chunks[2] as i32,
    ];
    let mut meshed = 0usize;
    for coord in iter_world_chunk_coords(extent) {
        let samples = terrain_generation::generate_padded_samples(&source, coord, voxel_core::MaterialId(0));
        let mesh = terrain_meshing::SurfaceNetsMesher
            .build_mesh(&terrain_meshing::ChunkMeshingInput {
                samples: &samples,
                chunk_cells: voxel_core::CHUNK_CELLS,
                surface_resolver: None,
            })
            .expect("mesh");
        if !mesh.positions.is_empty() {
            meshed += 1;
        }
    }
    eprintln!("meshed_chunks={meshed}/128");
    assert!(meshed > 30, "too few meshed chunks: {meshed}");
}
