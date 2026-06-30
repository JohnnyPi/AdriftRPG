use game_data::load_registry_from_directory;
use std::path::PathBuf;
use terrain_generation::{
    default_vertical_slice_recipe, RecipeDensitySource, RecipeOp, TerrainRecipe,
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

    let (sx, sy, sz) = source.spawn_position();
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
            let surface = source.surface_height_at(x, z);
            let headroom = source.density_at(x, surface + 2.0, z);
            assert!(
                headroom > 0.0,
                "route blocked at ({x},{z}): surface={surface} headroom density={headroom}"
            );
        }
    }
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
