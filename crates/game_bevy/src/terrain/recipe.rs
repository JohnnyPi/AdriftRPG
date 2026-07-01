use game_data::{
    CompiledIslandGeneration, CompiledRiver, CompiledWater, CompiledWorld, ConfigRegistry,
    TerrainOperationDefinition,
};
use sha2::{Digest, Sha256};
use shared::StableId;
use terrain_generation::{
    CoastModifierKind, CombineOp, RecipeDensitySource, RecipeOp, RiverCarveContext, RiverGenConfig,
    TerrainRecipe, build_island_atlas, default_vertical_slice_recipe, generate_river_spline,
};

use super::island_params::island_params_from_compiled;
use crate::data::UserSetupPrefs;

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

pub fn build_density_source_from_prefs(
    registry: &ConfigRegistry,
    prefs: &UserSetupPrefs,
    field_stack: terrain_generation::FieldStackParams,
) -> RecipeDensitySource {
    let world_id = prefs.world_stable_id();
    let seed = Some(prefs.seed);
    let world = registry
        .world_by_id(&world_id)
        .or_else(|_| registry.active_world())
        .expect("world");
    let has_island_gen = registry.island_generation_for_world(world).is_some();
    let mut source = if has_island_gen {
        let water = registry.water.get(&world.water).expect("water");
        let recipe = compile_terrain_recipe(registry, world, water, seed);
        RecipeDensitySource::new(recipe)
    } else {
        build_density_source(registry, Some(&world_id), seed, field_stack)
    };
    if let Some(base) = registry.island_generation_for_world(world) {
        let merged = prefs.apply_overrides(base);
        let water = registry.water.get(&world.water).expect("water");
        let params = island_params_from_compiled(&merged, world, prefs.seed, water.sea_level_m);
        let atlas = build_island_atlas(&params);
        source = source.with_atlas(atlas);
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
    if let Some(island_gen) = registry.island_generation_for_world(world) {
        append_generated_island_caves(&mut ops, island_gen, seed_override.unwrap_or(world.seed));
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

fn append_generated_island_caves(
    ops: &mut Vec<RecipeOp>,
    island_gen: &CompiledIslandGeneration,
    seed: u64,
) {
    let caves = &island_gen.caves;
    if caves.chamber_count_max == 0 || caves.passage_radius_max_m <= 0.0 {
        return;
    }

    let count =
        ((caves.chamber_count_min + caves.chamber_count_max).max(2) / 2).clamp(2, 8) as usize;
    let base_angle = island_gen.volcano.collapse_direction_deg.to_radians();
    let base_radius = island_gen.volcano.shield_radius_m * 0.38;
    let radius_span = island_gen.volcano.shield_radius_m * 0.18;
    let min_passage = caves.passage_radius_min_m.max(0.6);
    let max_passage = caves.passage_radius_max_m.max(min_passage);
    let mut previous = None;

    for index in 0..count {
        let t = if count == 1 {
            0.5
        } else {
            index as f32 / (count - 1) as f32
        };
        let angle_jitter = hash_unit(seed, index as u32) - 0.5;
        let angle = base_angle + (t - 0.5) * 0.7 + angle_jitter * 0.2;
        let radial = base_radius + radius_span * t;
        let chamber_radius = lerp(
            min_passage,
            max_passage,
            0.35 + 0.5 * hash_unit(seed ^ 0xA5A5_A5A5, index as u32),
        );
        let center = [
            island_gen.volcano.center[0] + radial * angle.cos(),
            cave_center_height(island_gen, t),
            island_gen.volcano.center[1] + radial * angle.sin(),
        ];
        ops.push(RecipeOp::Ellipsoid {
            center,
            radii: [chamber_radius * 1.7, chamber_radius, chamber_radius * 1.5],
            peak_noise: None,
            combine: CombineOp::Subtract,
        });
        if let Some(previous_center) = previous {
            ops.push(RecipeOp::Capsule {
                start: previous_center,
                end: center,
                radius: chamber_radius.min(max_passage) * 0.72,
                combine: CombineOp::Subtract,
            });
        }
        previous = Some(center);
    }

    if caves.overhang_enabled {
        let mouth_radius = min_passage * 1.15;
        let mouth = [
            island_gen.volcano.center[0] + (base_radius + radius_span * 1.15) * base_angle.cos(),
            (island_gen.island.sea_level_m + caves.minimum_cover_m + mouth_radius)
                .min(cave_center_height(island_gen, 0.1)),
            island_gen.volcano.center[1] + (base_radius + radius_span * 1.15) * base_angle.sin(),
        ];
        if let Some(last_center) = previous {
            ops.push(RecipeOp::Capsule {
                start: last_center,
                end: mouth,
                radius: mouth_radius,
                combine: CombineOp::Subtract,
            });
        }
    }
}

fn cave_center_height(island_gen: &CompiledIslandGeneration, t: f32) -> f32 {
    let caves = &island_gen.caves;
    let base = island_gen.island.sea_level_m + caves.minimum_cover_m + 6.0;
    let depth_span = caves.maximum_depth_m * (0.18 + 0.18 * t);
    let ceiling_limit = island_gen.island.maximum_height_m * 0.28;
    (base + depth_span).min(ceiling_limit.max(base + 2.0))
}

fn hash_unit(seed: u64, index: u32) -> f32 {
    let mut value = seed ^ ((index as u64 + 1) * 0x9E37_79B9_7F4A_7C15);
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    ((value >> 40) as u32) as f32 / u32::MAX as f32
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
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
            kind: parse_coast_modifier_kind(kind),
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

fn parse_combine(value: &str) -> CombineOp {
    match value.to_ascii_lowercase().as_str() {
        "subtract" => CombineOp::Subtract,
        _ => CombineOp::Union,
    }
}

fn parse_coast_modifier_kind(value: &str) -> CoastModifierKind {
    match value.to_ascii_lowercase().as_str() {
        "harbor" => CoastModifierKind::Harbor,
        "cliff_shelf" | "cliff" => CoastModifierKind::CliffShelf,
        _ => CoastModifierKind::Cove,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_data::load_registry_from_directory;
    use std::path::PathBuf;

    fn workspace_assets() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .canonicalize()
            .expect("assets")
    }

    #[test]
    fn generated_vs3_caves_respond_to_authored_parameters() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry");
        let world = registry
            .world_by_id(&StableId::new("world.vs3_island"))
            .expect("world");
        let base = registry
            .island_generation_for_world(world)
            .expect("island")
            .clone();

        let mut low = base.clone();
        low.caves.chamber_count_min = 2;
        low.caves.chamber_count_max = 2;
        low.caves.overhang_enabled = false;

        let mut high = base.clone();
        high.caves.chamber_count_min = 6;
        high.caves.chamber_count_max = 8;
        high.caves.overhang_enabled = true;

        let mut low_ops = Vec::new();
        append_generated_island_caves(&mut low_ops, &low, world.seed);
        let mut high_ops = Vec::new();
        append_generated_island_caves(&mut high_ops, &high, world.seed);

        assert!(high_ops.len() > low_ops.len());
        assert!(
            high_ops
                .iter()
                .any(|op| matches!(op, RecipeOp::Capsule { .. }))
        );
        assert!(
            low_ops
                .iter()
                .filter(|op| matches!(op, RecipeOp::Ellipsoid { .. }))
                .count()
                <= 2
        );
    }
}
