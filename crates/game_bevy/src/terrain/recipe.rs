use game_data::{
    ConfigRegistry, CompiledWater, CompiledWorld, TerrainOperationDefinition,
};
use sha2::{Digest, Sha256};
use terrain_generation::{
    default_vertical_slice_recipe, CombineOp, RecipeDensitySource, RecipeOp, TerrainRecipe,
};

pub fn build_density_source(registry: &ConfigRegistry, seed_override: Option<u64>) -> RecipeDensitySource {
    let world = registry.active_world().expect("world");
    let water = registry.water.get(&world.water).expect("water");
    RecipeDensitySource::new(compile_terrain_recipe(
        registry,
        world,
        water,
        seed_override,
    ))
}

pub fn terrain_recipe_hash(registry: &ConfigRegistry, seed_override: Option<u64>) -> String {
    let world = registry.active_world().expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let recipe = compile_terrain_recipe(registry, world, water, seed_override);
    let mut hasher = Sha256::new();
    hasher.update(recipe.seed.to_le_bytes());
    hasher.update(recipe.sea_level.to_le_bytes());
    hasher.update(recipe.spawn_x.to_le_bytes());
    hasher.update(recipe.spawn_z.to_le_bytes());
    hasher.update((recipe.ops.len() as u32).to_le_bytes());
    for op in &recipe.ops {
        hasher.update(format!("{op:?}").as_bytes());
    }
    hex::encode(hasher.finalize())
}

pub fn compile_terrain_recipe(
    registry: &ConfigRegistry,
    world: &CompiledWorld,
    water: &CompiledWater,
    seed_override: Option<u64>,
) -> TerrainRecipe {
    let terrain = registry
        .terrain
        .get(&world.terrain)
        .expect("terrain definition");

    let mut ops = Vec::new();
    for op_def in &terrain.operations {
        ops.push(convert_op(op_def));
    }
    for include in &terrain.includes {
        if let Some(cave) = registry.caves.get(include) {
            for op_def in &cave.operations {
                ops.push(convert_op(op_def));
            }
        }
    }

    if ops.is_empty() {
        return default_vertical_slice_recipe(
            seed_override.unwrap_or(world.seed),
            water.sea_level_m,
        );
    }

    let (spawn_x, spawn_z) = terrain
        .spawn
        .map(|s| (s[0], s[2]))
        .unwrap_or((-30.0, -25.0));

    TerrainRecipe {
        seed: seed_override.unwrap_or(world.seed),
        sea_level: water.sea_level_m,
        spawn_x,
        spawn_z,
        ops,
    }
}

fn convert_op(def: &TerrainOperationDefinition) -> RecipeOp {
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
            combine: parse_combine(combine),
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
            combine: parse_combine(combine),
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

fn parse_combine(value: &str) -> CombineOp {
    match value.to_ascii_lowercase().as_str() {
        "subtract" => CombineOp::Subtract,
        _ => CombineOp::Union,
    }
}
