use game_data::load_registry_from_directory;
use std::path::PathBuf;
use terrain_generation::{
    default_vertical_slice_recipe, CoastModifierKind, RecipeDensitySource, RecipeOp, TerrainRecipe,
};
use voxel_core::CHUNK_CELLS;

fn workspace_assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .canonicalize()
        .expect("assets")
}

fn load_yaml_recipe() -> TerrainRecipe {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let world = registry.active_world().expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let terrain = registry.terrain.get(&world.terrain).expect("terrain");
    let mut ops: Vec<RecipeOp> = Vec::new();
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
    let spawn = terrain.spawn.unwrap_or([-30.0, 0.0, -25.0]);
    TerrainRecipe {
        seed: world.seed,
        sea_level: water.sea_level_m,
        spawn_x: spawn[0],
        spawn_z: spawn[2],
        coord_offset: world.coord_offset,
        ops,
    }
}

fn convert_op(def: &game_data::TerrainOperationDefinition) -> RecipeOp {
    use game_data::TerrainOperationDefinition;
    use terrain_generation::CombineOp;
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
            kind: parse_coast_kind(kind),
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
            combine: if combine.eq_ignore_ascii_case("subtract") {
                CombineOp::Subtract
            } else {
                CombineOp::Union
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
            combine: if combine.eq_ignore_ascii_case("subtract") {
                CombineOp::Subtract
            } else {
                CombineOp::Union
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

fn load_expanded_recipe() -> TerrainRecipe {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let world = registry
        .world_by_id(&shared::StableId::new("world.expanded_slice"))
        .expect("expanded world");
    let water = registry.water.get(&world.water).expect("water");
    let terrain = registry.terrain.get(&world.terrain).expect("terrain");
    let mut ops: Vec<RecipeOp> = Vec::new();
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
    let spawn = terrain.spawn.unwrap_or([48.0, 0.0, 185.0]);
    TerrainRecipe {
        seed: world.seed,
        sea_level: water.sea_level_m,
        spawn_x: spawn[0],
        spawn_z: spawn[2],
        coord_offset: world.coord_offset,
        ops,
    }
}

fn parse_coast_kind(value: &str) -> CoastModifierKind {
    match value.to_ascii_lowercase().as_str() {
        "harbor" => CoastModifierKind::Harbor,
        "cliff_shelf" | "cliff" => CoastModifierKind::CliffShelf,
        _ => CoastModifierKind::Cove,
    }
}

fn recipe_xz(source: &RecipeDensitySource, rx: f32, rz: f32) -> (f32, f32) {
    (
        rx - source.recipe().coord_offset[0],
        rz - source.recipe().coord_offset[2],
    )
}

fn assert_route_traversable(source: &RecipeDensitySource, waypoints: &[[f32; 2]]) {
    for window in waypoints.windows(2) {
        let x0 = window[0][0];
        let z0 = window[0][1];
        let x1 = window[1][0];
        let z1 = window[1][1];
        let steps = 16;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let x = x0 + (x1 - x0) * t;
            let z = z0 + (z1 - z0) * t;
            let surface = source.surface_height_at_recipe(x, z);
            let headroom = source.density_at_recipe(x, surface + 2.0, z);
            assert!(
                headroom > 0.0,
                "route blocked at ({x},{z}): surface={surface} headroom density={headroom}"
            );
        }
    }
}

#[test]
fn yaml_recipe_matches_default_vertical_slice() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let world = registry.active_world().expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let yaml = RecipeDensitySource::new(load_yaml_recipe());
    let default = RecipeDensitySource::new(default_vertical_slice_recipe(
        world.seed,
        water.sea_level_m,
    ));

    for x in (-20..40).step_by(4) {
        for z in (-20..40).step_by(4) {
            for y in (-5..25).step_by(3) {
                let a = yaml.density_at(x as f32, y as f32, z as f32);
                let b = default.density_at(x as f32, y as f32, z as f32);
                assert!(
                    (a - b).abs() < 0.001,
                    "density mismatch at ({x},{y},{z}): yaml={a} default={b}"
                );
            }
        }
    }
}

#[test]
fn route_landmarks_exist() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let world = registry.active_world().expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let source = RecipeDensitySource::new(load_yaml_recipe());

    let (_sx, sy, _sz) = source.spawn_position();
    assert!(sy > water.sea_level_m, "spawn should be above sea level");

    let ridge_density = source.density_at(35.0, 18.0, 15.0);
    assert!(ridge_density <= 0.0, "ridge landmark should be solid");

    let cave_air = source.density_at(26.0, 0.0, 12.0);
    assert!(cave_air > 0.0, "cave chamber should be air");
}

#[test]
fn route_beach_to_cave_is_traversable() {
    let source = RecipeDensitySource::new(load_yaml_recipe());
    let waypoints = [
        (-30.0, -25.0),
        (-10.0, -10.0),
        (10.0, 0.0),
        (26.0, 12.0),
    ];

    for window in waypoints.windows(2) {
        let (x0, z0) = window[0];
        let (x1, z1) = window[1];
        let steps = 16;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let x = x0 + (x1 - x0) * t;
            let z = z0 + (z1 - z0) * t;
            let surface = source.surface_height_at_recipe(x, z);
            let headroom = source.density_at_recipe(x, surface + 2.0, z);
            assert!(
                headroom > 0.0,
                "route blocked at ({x},{z}): surface={surface} headroom density={headroom}"
            );
        }
    }
}

/// Land route from beach through overhang to cave entrance must have continuous floor support.
#[test]
fn overhang_cave_route_has_floor_support() {
    let source = RecipeDensitySource::new(load_yaml_recipe());
    let waypoints = [
        (-30.0, -25.0),
        (-10.0, -10.0),
        (5.0, 0.0),
        (15.0, 6.0),
        (20.0, 10.0),
        (24.0, 10.0),
        (28.0, 8.0),
        (30.0, 6.0),
    ];

    for window in waypoints.windows(2) {
        let (x0, z0) = window[0];
        let (x1, z1) = window[1];
        let steps = 20;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let x = x0 + (x1 - x0) * t;
            let z = z0 + (z1 - z0) * t;
            let floor = source
                .walkable_floor_at(x, z, 32.0)
                .unwrap_or_else(|| panic!("no walkable floor at ({x},{z})"));
            assert!(
                source.has_support_below(x, floor, z, 2.5),
                "floor at ({x},{z}) y={floor} lacks solid support within 2.5m below"
            );
            if x >= 20.0 && x <= 32.0 && z >= 5.0 && z <= 13.0 {
                let clearance = source.clearance_above_floor(x, floor, z);
                assert!(
                    clearance >= 2.0,
                    "insufficient headroom at ({x},{z}) floor_y={floor} clearance={clearance}"
                );
            }
        }
    }
}

/// Critical overhang / entrance columns must not be open shafts to the world bottom.
#[test]
fn overhang_entrance_columns_are_not_voids() {
    let source = RecipeDensitySource::new(load_yaml_recipe());
    let probes = [
        (28.0, 8.0, "under overhang center"),
        (26.0, 10.0, "cave entrance"),
        (24.0, 10.0, "approach"),
        (30.0, 6.0, "cave mouth"),
    ];

    for (x, z, label) in probes {
        assert!(
            !source.column_is_void(x, z, 0.0, 8.0),
            "{label} at ({x},{z}) is void from y=0..8"
        );
        let floor = source
            .walkable_floor_with_clearance(x, z, 20.0, 2.0)
            .unwrap_or_else(|| panic!("{label} at ({x},{z}) has no walkable floor"));
        assert!(
            floor < 12.0,
            "{label} at ({x},{z}) floor too high at y={floor}"
        );
    }
}

/// Outdoor columns outside declared caves must have bedrock within the foundation depth.
#[test]
fn outdoor_columns_have_foundation_bedrock() {
    use terrain_generation::{outside_declared_cavities, coastal_surface_height, FOUNDATION_DEPTH_M};

    let source = RecipeDensitySource::new(load_yaml_recipe());
    let recipe = source.recipe();
    let mut violations = 0usize;

    for x in (-40..40).step_by(2) {
        for z in (-40..40).step_by(2) {
            let xf = x as f32;
            let zf = z as f32;
            let surface = coastal_surface_height(recipe, xf, zf);
            if surface < recipe.sea_level + 1.0 {
                continue;
            }
            let bedrock_y = surface - FOUNDATION_DEPTH_M;
            if !outside_declared_cavities(recipe, xf, bedrock_y, zf) {
                continue;
            }
            let density = source.density_at(xf, bedrock_y, zf);
            if density > 0.0 {
                violations += 1;
                if violations <= 5 {
                    eprintln!("missing bedrock at ({xf},{zf}) y={bedrock_y:.1}");
                }
            }
        }
    }

    assert_eq!(
        violations, 0,
        "found {violations} outdoor columns missing foundation bedrock"
    );
}

/// No shallow outdoor void shafts from bedrock to walk height in the landmark corridor.
#[test]
fn no_shallow_outdoor_void_shafts_in_landmark_corridor() {
    use terrain_generation::outside_declared_cavities;

    let source = RecipeDensitySource::new(load_yaml_recipe());
    let recipe = source.recipe();
    let mut violations = 0usize;

    for x in 180..=320 {
        for z in 40..=130 {
            let xf = x as f32 / 10.0;
            let zf = z as f32 / 10.0;
            if !outside_declared_cavities(recipe, xf, 3.0, zf) {
                continue;
            }
            if source.column_is_void(xf, zf, 0.0, 6.0) {
                violations += 1;
                if violations <= 5 {
                    eprintln!("outdoor void shaft at ({xf},{zf}) from y=0..6");
                }
            }
        }
    }

    assert_eq!(
        violations, 0,
        "found {violations} shallow outdoor void shafts in landmark corridor"
    );
}

/// Hillside-to-overhang transition must not leave unsupported shelves (>3m air under foot).
#[test]
fn no_large_floor_gaps_on_land_route() {
    let source = RecipeDensitySource::new(load_yaml_recipe());
    let mut violations = 0usize;
    for x in 180..=320 {
        for z in 50..=120 {
            let xf = x as f32 / 10.0;
            let zf = z as f32 / 10.0;
            if let Some(floor) = source.walkable_floor_at(xf, zf, 24.0) {
                if floor > 5.0 && floor < 22.0 && !source.has_support_below(xf, floor, zf, 3.0) {
                    violations += 1;
                    if violations <= 5 {
                        eprintln!("unsupported floor at ({xf},{zf}) y={floor}");
                    }
                }
            }
        }
    }
    assert!(
        violations == 0,
        "found {violations} columns with >3m unsupported floor in overhang approach region"
    );
}

#[test]
fn no_floating_voxel_islands_in_probe_grid() {
    let source = RecipeDensitySource::new(load_yaml_recipe());
    let mut floaters = 0usize;
    for x in (0..80).step_by(2) {
        for z in (0..80).step_by(2) {
            for y in 1..30 {
                let yf = y as f32;
                let below = source.density_at(x as f32, yf - 1.0, z as f32);
                let here = source.density_at(x as f32, yf, z as f32);
                let above = source.density_at(x as f32, yf + 1.0, z as f32);
                if here <= 0.0 && below > 0.0 && above > 0.0 {
                    floaters += 1;
                }
            }
        }
    }
    assert!(
        floaters < 15,
        "expected almost no floating solid voxels, found {floaters}"
    );
}

#[test]
fn chunk_border_sample_indices_are_consistent() {
    let source = RecipeDensitySource::new(load_yaml_recipe());
    let coord = voxel_core::ChunkCoord::new(1, 1, 0);
    let samples =
        terrain_generation::generate_padded_samples(&source, coord, voxel_core::MaterialId(0));
    let cells = CHUNK_CELLS as usize;
    let stride = cells + 3;
    let idx = |x: i32, y: i32, z: i32| -> usize {
        (z + 1) as usize * stride * stride + (y + 1) as usize * stride + (x + 1) as usize
    };
    let boundary = idx(cells as i32, 8, 8);
    assert!(samples[boundary].density.is_finite());
}

/// Shared chunk faces must sample identical density values (mesh seam prerequisite).
#[test]
fn chunk_face_density_is_continuous() {
    let source = RecipeDensitySource::new(load_yaml_recipe());
    let cells = CHUNK_CELLS as i32;
    let pairs = [
        (voxel_core::ChunkCoord::new(0, 1, 0), voxel_core::ChunkCoord::new(1, 1, 0), 0i32),
        (voxel_core::ChunkCoord::new(1, 0, 0), voxel_core::ChunkCoord::new(1, 1, 0), 1i32),
        (voxel_core::ChunkCoord::new(1, 1, 0), voxel_core::ChunkCoord::new(1, 1, 1), 2i32),
    ];

    for (a, b, axis) in pairs {
        let (a_origin, b_origin) = (
            voxel_core::TerrainChunk::new(a).sample_origin(),
            voxel_core::TerrainChunk::new(b).sample_origin(),
        );
        for u in 0..=cells {
            for v in 0..=cells {
                for y in 0..=cells {
                    let (ax, ay, az, bx, by, bz) = match axis {
                        0 => (
                            a_origin.0 + cells,
                            a_origin.1 + y,
                            a_origin.2 + u,
                            b_origin.0,
                            b_origin.1 + y,
                            b_origin.2 + u,
                        ),
                        1 => (
                            a_origin.0 + u,
                            a_origin.1 + cells,
                            a_origin.2 + v,
                            b_origin.0 + u,
                            b_origin.1,
                            b_origin.2 + v,
                        ),
                        _ => (
                            a_origin.0 + u,
                            a_origin.1 + y,
                            a_origin.2 + cells,
                            b_origin.0 + u,
                            b_origin.1 + y,
                            b_origin.2,
                        ),
                    };
                    let da = source.density_at(ax as f32, ay as f32, az as f32);
                    let db = source.density_at(bx as f32, by as f32, bz as f32);
                    assert!(
                        (da - db).abs() < 0.0001,
                        "density seam mismatch on axis {axis} at ({ax},{ay},{az}) vs ({bx},{by},{bz}): {da} != {db}"
                    );
                }
            }
        }
    }
}

#[test]
fn expanded_recipe_includes_island_mask() {
    let recipe = load_expanded_recipe();
    assert!(
        recipe.ops.iter().any(|op| matches!(op, RecipeOp::IslandMask { .. })),
        "expected island_mask op, got {} ops",
        recipe.ops.len()
    );
    let source = RecipeDensitySource::new(recipe);
    let offshore = source.surface_height_at_recipe(240.0, 60.0);
    assert!(
        offshore < 2.0,
        "offshore seabed should be below sea level, got {offshore}"
    );
}

#[test]
fn expanded_island_peak_above_50m() {
    let source = RecipeDensitySource::new(load_expanded_recipe());
    let peak = source.surface_height_at_recipe(188.0, 178.0);
    assert!(
        peak > 50.0,
        "summit should exceed 50m, got {peak}"
    );
}

#[test]
fn expanded_trench_below_minus_20m() {
    let recipe = load_expanded_recipe();
    let ocean_floor_y = recipe
        .ops
        .iter()
        .find_map(|op| {
            if let RecipeOp::IslandMask { ocean_floor_y, .. } = op {
                Some(*ocean_floor_y)
            } else {
                None
            }
        })
        .expect("island mask");
    assert!(
        ocean_floor_y <= -20.0,
        "authored ocean floor should be below -20m, got {ocean_floor_y}"
    );
    let source = RecipeDensitySource::new(recipe);
    let deep_ocean = source.surface_height_at_recipe(220.0, 45.0);
    assert!(
        deep_ocean < 2.0,
        "offshore floor should be below sea level, got {deep_ocean}"
    );
}

#[test]
fn expanded_offshore_is_submerged() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let world = registry
        .world_by_id(&shared::StableId::new("world.expanded_slice"))
        .expect("expanded");
    let water = registry.water.get(&world.water).expect("water");
    let source = RecipeDensitySource::new(load_expanded_recipe());
    let offshore = source.surface_height_at_recipe(240.0, 60.0);
    assert!(
        offshore < water.sea_level_m,
        "offshore surface should be below sea level, got {offshore}"
    );
}

#[test]
fn expanded_fort_pad_has_floor_support() {
    let source = RecipeDensitySource::new(load_expanded_recipe());
    let (wx, wz) = recipe_xz(&source, 48.0, 185.0);
    let floor = source
        .walkable_floor_at(wx, wz, 32.0)
        .expect("fort pad should have walkable floor");
    assert!(
        source.has_support_below(wx, floor, wz, 2.5),
        "fort pad lacks support"
    );
}

#[test]
fn expanded_inland_has_no_shallow_void_shafts() {
    use terrain_generation::outside_declared_cavities;

    let source = RecipeDensitySource::new(load_expanded_recipe());
    let recipe = source.recipe();
    let noise = terrain_generation::ValueNoise::new(recipe.seed);
    let mut violations = 0usize;

    for rx in (60..=190).step_by(4) {
        for rz in (60..=190).step_by(4) {
            let rxf = rx as f32;
            let rzf = rz as f32;
            let land = terrain_generation::island_land_factor_warped(recipe, rxf, rzf, &noise);
            if land < 0.5 {
                continue;
            }
            if !outside_declared_cavities(recipe, rxf, 3.0, rzf) {
                continue;
            }
            let (wx, wz) = recipe_xz(&source, rxf, rzf);
            if source.column_is_void(wx, wz, 0.0, 6.0) {
                violations += 1;
                if violations <= 5 {
                    eprintln!("outdoor void shaft at recipe ({rxf},{rzf})");
                }
            }
        }
    }

    assert_eq!(
        violations, 0,
        "found {violations} shallow outdoor void shafts on expanded island inland"
    );
}

#[test]
fn expanded_cave_entrance_is_traversable() {
    let source = RecipeDensitySource::new(load_expanded_recipe());
    let probes = [
        (56.0, 116.0, "cave entrance center"),
        (58.0, 114.0, "cave mouth"),
        (54.0, 118.0, "approach"),
    ];

    for (rx, rz, label) in probes {
        let (x, z) = recipe_xz(&source, rx, rz);
        assert!(
            !source.column_is_void(x, z, 0.0, 8.0),
            "{label} at recipe ({rx},{rz}) is void from y=0..8"
        );
        let floor = source
            .walkable_floor_with_clearance(x, z, 20.0, 2.0)
            .unwrap_or_else(|| panic!("{label} at recipe ({rx},{rz}) has no walkable floor"));
        assert!(
            floor < 14.0,
            "{label} at recipe ({rx},{rz}) floor too high at y={floor}"
        );
        assert!(
            source.has_support_below(x, floor, z, 3.0),
            "{label} at recipe ({rx},{rz}) lacks support below floor y={floor}"
        );
    }
}

#[test]
fn expanded_orphan_subtract_probes_have_support() {
    let source = RecipeDensitySource::new(load_expanded_recipe());
    for (rx, rz) in [(82.0, 196.0), (56.0, 116.0)] {
        let (x, z) = recipe_xz(&source, rx, rz);
        if let Some(floor) = source.walkable_floor_at(x, z, 32.0) {
            assert!(
                source.has_support_below(x, floor, z, 3.0),
                "unsupported floor at recipe ({rx},{rz}) y={floor}"
            );
        }
    }
}

#[test]
fn expanded_routes_from_yaml_are_traversable() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let routes = registry
        .routes
        .get(&shared::StableId::new("routes.expanded_slice"))
        .expect("expanded routes");
    let source = RecipeDensitySource::new(load_expanded_recipe());
    for route in &routes.routes {
        assert_route_traversable(&source, &route.waypoints);
    }
}
