use game_data::{
    CompiledRiver, CompiledWater, CompiledWorld, ConfigRegistry, TerrainOperationDefinition,
};
use shared::StableId;
use sha2::{Digest, Sha256};
use terrain_generation::{
    default_vertical_slice_recipe, generate_river_spline, CombineOp, RecipeDensitySource,
    RecipeOp, RiverCarveContext, RiverGenConfig, TerrainRecipe,
};

pub fn build_density_source(
    registry: &ConfigRegistry,
    world_id: Option<&StableId>,
    seed_override: Option<u64>,
    field_stack: terrain_generation::FieldStackParams,
) -> RecipeDensitySource {
    let world = registry.effective_world(world_id).expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let recipe = compile_terrain_recipe(registry, world, water, seed_override);
    let mut source = RecipeDensitySource::new(recipe);
    if let Some(ctx) = build_river_carve(registry, world, seed_override, field_stack) {
        source = source.with_river_carve(ctx);
    }
    source
}

pub fn terrain_recipe_hash(
    registry: &ConfigRegistry,
    world_id: Option<&StableId>,
    seed_override: Option<u64>,
) -> String {
    let world = registry.effective_world(world_id).expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let recipe = compile_terrain_recipe(registry, world, water, seed_override);
    let mut hasher = Sha256::new();
    if let Some(id) = world_id {
        hasher.update(id.as_str().as_bytes());
    }
    hasher.update(recipe.seed.to_le_bytes());
    hasher.update(recipe.sea_level.to_le_bytes());
    hasher.update(recipe.spawn_x.to_le_bytes());
    hasher.update(recipe.spawn_z.to_le_bytes());
    hasher.update((recipe.ops.len() as u32).to_le_bytes());
    for op in &recipe.ops {
        hasher.update(format!("{op:?}").as_bytes());
    }
    if let Some(river) = registry.demo_river() {
        hasher.update(river.id.as_str().as_bytes());
        hasher.update(river.bank_width_m.to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

pub fn river_gen_config(
    river: &CompiledRiver,
    seed: u64,
    field_stack: terrain_generation::FieldStackParams,
) -> RiverGenConfig {
    RiverGenConfig {
        source_center: river.source_region_center,
        source_radius_m: river.source_region_radius_m,
        grid_spacing_m: river.grid_spacing_m,
        mouth_width_m: river.mouth_width_m,
        source_width_m: river.source_width_m,
        source_depth_m: river.source_depth_m,
        mouth_depth_m: river.mouth_depth_m,
        bank_width_m: river.bank_width_m,
        minimum_depth_m: river.minimum_depth_m,
        depression_repair_radius_cells: river.depression_repair_radius_cells,
        maximum_breach_depth_m: river.maximum_breach_depth_m,
        seed,
        field_stack,
    }
}

pub fn build_river_carve(
    registry: &ConfigRegistry,
    world: &CompiledWorld,
    seed_override: Option<u64>,
    field_stack: terrain_generation::FieldStackParams,
) -> Option<RiverCarveContext> {
    let river_def = registry.demo_river()?;
    let water = registry.water.get(&world.water)?;
    let seed = seed_override.unwrap_or(world.seed);
    let config = river_gen_config(river_def, seed, field_stack);
    let spline = generate_river_spline(&config, water.sea_level_m)?;
    Some(RiverCarveContext {
        spline,
        bank_width_m: river_def.bank_width_m,
    })
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
        coord_offset: world.coord_offset,
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
        TerrainOperationDefinition::IslandMask {
            center,
            radius_m,
            falloff_m,
            ocean_floor_y,
        } => RecipeOp::IslandMask {
            center: *center,
            radius_m: *radius_m,
            falloff_m: *falloff_m,
            ocean_floor_y: *ocean_floor_y,
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

fn parse_combine(value: &str) -> CombineOp {
    match value.to_ascii_lowercase().as_str() {
        "subtract" => CombineOp::Subtract,
        _ => CombineOp::Union,
    }
}
