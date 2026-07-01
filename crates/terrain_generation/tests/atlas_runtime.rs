use game_data::load_registry_from_directory;
use std::path::PathBuf;
use terrain_generation::{
    build_island_atlas, generate_padded_samples, iter_world_chunk_coords, RecipeDensitySource,
    TerrainRecipe,
};
use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMesher};
use voxel_core::{MaterialId, WorldCell, CHUNK_CELLS};

fn workspace_assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .canonicalize()
        .expect("assets")
}

fn build_atlas_source(seed: u64, world_id: &str) -> RecipeDensitySource {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let world = registry
        .world_by_id(&shared::StableId::new(world_id))
        .expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let terrain = registry.terrain.get(&world.terrain).expect("terrain");
    let base = registry
        .island_generation_for_world(world)
        .expect("island gen");
    let mut merged = base.clone();
    merged.seed = seed;

    let mut params = terrain_generation::IslandGenParams::default();
    params.seed = seed;
    params.ocean_extent_m = world.ocean_extent_m.unwrap_or(256.0);
    params.island.playable_diameter_m = merged.island.playable_diameter_m;
    params.island.maximum_height_m = merged.island.maximum_height_m;
    params.island.sea_level_m = water.sea_level_m;
    params.island.lobe_count = merged.island.lobe_count;
    params.island.warp_frequency = merged.island.warp_frequency;
    params.island.warp_amplitude = merged.island.warp_amplitude;
    params.volcano.center = [
        merged.volcano.center[0] - world.coord_offset[0],
        merged.volcano.center[1] - world.coord_offset[2],
    ];
    params.volcano.shield_radius_m = merged.volcano.shield_radius_m;
    params.volcano.shield_exponent = merged.volcano.shield_exponent;
    params.volcano.shield_height_m = merged.volcano.shield_height_m;
    params.volcano.summit_radius_m = merged.volcano.summit_radius_m;
    params.volcano.summit_exponent = merged.volcano.summit_exponent;
    params.volcano.summit_height_m = merged.volcano.summit_height_m;
    params.volcano.caldera_radius_m = merged.volcano.caldera_radius_m;
    params.volcano.caldera_depth_m = merged.volcano.caldera_depth_m;
    params.volcano.caldera_rim_height_m = merged.volcano.caldera_rim_height_m;
    params.volcano.radial_ridge_count = merged.volcano.radial_ridge_count;
    params.volcano.collapse_direction_deg = merged.volcano.collapse_direction_deg;
    params.volcano.collapse_depth_m = merged.volcano.collapse_depth_m;

    let atlas = build_island_atlas(&params);
    let spawn = terrain.spawn.unwrap_or([-30.0, 0.0, -25.0]);
    let mut ops = Vec::new();
    for op in &terrain.operations {
        ops.push(convert_op(op));
    }
    for include in &terrain.includes {
        if let Some(cave) = registry.caves.get(include) {
            for op in &cave.operations {
                ops.push(convert_op(op));
            }
        }
    }
    let recipe = TerrainRecipe {
        seed,
        sea_level: water.sea_level_m,
        spawn_x: spawn[0],
        spawn_z: spawn[2],
        coord_offset: world.coord_offset,
        ops,
    };
    RecipeDensitySource::new(recipe).with_atlas(atlas)
}

fn convert_op(def: &game_data::TerrainOperationDefinition) -> terrain_generation::RecipeOp {
    use game_data::TerrainOperationDefinition;
    use terrain_generation::{CoastModifierKind, CombineOp, RecipeOp};
    match def {
        TerrainOperationDefinition::CoastalSurface {
            origin,
            scale,
            base_height,
            height_range,
            ridge_origin,
            ridge_scale,
            ridge_amplitude,
            detail_frequency,
            detail_amplitude,
            detail_octaves,
            regional_frequency,
            regional_amplitude,
            local_frequency,
            local_amplitude,
            ridged_amplitude,
            domain_warp,
        } => RecipeOp::CoastalSurface {
            origin: *origin,
            scale: *scale,
            base_height: *base_height,
            height_range: *height_range,
            ridge_origin: *ridge_origin,
            ridge_scale: *ridge_scale,
            ridge_amplitude: *ridge_amplitude,
            detail_frequency: *detail_frequency,
            detail_amplitude: *detail_amplitude,
            detail_octaves: *detail_octaves,
            regional_frequency: *regional_frequency,
            regional_amplitude: *regional_amplitude,
            local_frequency: *local_frequency,
            local_amplitude: *local_amplitude,
            ridged_amplitude: *ridged_amplitude,
            domain_warp: *domain_warp,
        },
        TerrainOperationDefinition::ValleyBasin {
            origin,
            scale,
            depth_m,
        } => RecipeOp::ValleyBasin {
            origin: *origin,
            scale: *scale,
            depth_m: *depth_m,
        },
        TerrainOperationDefinition::CoastModifier {
            kind,
            center,
            radius_m,
            depth_m,
            min_land_factor,
            max_land_factor,
        } => RecipeOp::CoastModifier {
            kind: match kind.to_ascii_lowercase().as_str() {
                "harbor" => CoastModifierKind::Harbor,
                "cliff_shelf" | "cliff" => CoastModifierKind::CliffShelf,
                _ => CoastModifierKind::Cove,
            },
            center: *center,
            radius_m: *radius_m,
            depth_m: *depth_m,
            min_land_factor: *min_land_factor,
            max_land_factor: *max_land_factor,
        },
        TerrainOperationDefinition::Ellipsoid {
            center,
            radii,
            peak_noise,
            combine,
        } => RecipeOp::Ellipsoid {
            center: *center,
            radii: *radii,
            peak_noise: peak_noise.map(|p| (p[0], p[1])),
            combine: match combine.to_ascii_lowercase().as_str() {
                "subtract" => CombineOp::Subtract,
                _ => CombineOp::Union,
            },
        },
        TerrainOperationDefinition::Capsule {
            start,
            end,
            radius,
            combine,
        } => RecipeOp::Capsule {
            start: *start,
            end: *end,
            radius: *radius,
            combine: match combine.to_ascii_lowercase().as_str() {
                "subtract" => CombineOp::Subtract,
                _ => CombineOp::Union,
            },
        },
        TerrainOperationDefinition::NoisePerturb {
            scale,
            amplitude,
            density_min,
            density_max,
        } => RecipeOp::NoisePerturb {
            scale: *scale,
            amplitude: *amplitude,
            density_min: *density_min,
            density_max: *density_max,
        },
        TerrainOperationDefinition::IslandMask {
            center,
            radius_m,
            falloff_m,
            ocean_floor_y,
            domain_warp,
        } => RecipeOp::IslandMask {
            center: *center,
            radius_m: *radius_m,
            falloff_m: *falloff_m,
            ocean_floor_y: *ocean_floor_y,
            domain_warp: *domain_warp,
        },
        TerrainOperationDefinition::OceanFloor {
            origin,
            scale,
            base_depth_m,
            variation_m,
            detail_frequency,
            detail_octaves,
        } => RecipeOp::OceanFloor {
            origin: *origin,
            scale: *scale,
            base_depth_m: *base_depth_m,
            variation_m: *variation_m,
            detail_frequency: *detail_frequency,
            detail_octaves: *detail_octaves,
        },
        TerrainOperationDefinition::MountainPeak {
            center,
            base_elevation_m,
            base_radius_m,
            peak_height_m,
            steepness,
            peak_noise,
        } => RecipeOp::MountainPeak {
            center: *center,
            base_elevation_m: *base_elevation_m,
            base_radius_m: *base_radius_m,
            peak_height_m: *peak_height_m,
            steepness: *steepness,
            peak_noise: peak_noise.map(|p| (p[0], p[1])),
        },
        TerrainOperationDefinition::UnderwaterTrench { points, width_m } => {
            RecipeOp::UnderwaterTrench {
                points: points.clone(),
                width_m: *width_m,
            }
        }
    }
}

#[test]
fn expanded_hd_seed_800000_spawn_chunk_has_mesh() {
    let source = build_atlas_source(800_000, "world.expanded_slice_hd");
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
fn expanded_hd_seed_800000_meshes_most_surface_chunks() {
    let source = build_atlas_source(800_000, "world.expanded_slice_hd");
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let world = registry
        .world_by_id(&shared::StableId::new("world.expanded_slice_hd"))
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
        "expected most surface terrain chunks to mesh for expanded HD seed 800000, got {meshed}"
    );
    eprintln!("expanded_hd_800000 meshed_chunks={meshed}");

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
