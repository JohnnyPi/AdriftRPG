// crates/game_bevy/src/terrain/recipe.rs
use game_data::{
    CompiledRiver, CompiledWorld, ConfigRegistry,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use shared::StableId;
use terrain_generation::{
    RecipeDensitySource, RiverCarveContext, RiverGenConfig,
    build_island_atlas, compile_terrain_recipe, generate_river_spline,
    island_params_from_compiled,
};

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
        let bank_width_m = registry
            .demo_river()
            .map(|river| river.bank_width_m)
            .unwrap_or(3.5);
        source = source.with_atlas(atlas, bank_width_m);
    }
    source
}

pub fn terrain_recipe_hash(
    registry: &ConfigRegistry,
    world_id: Option<&StableId>,
    seed_override: Option<u64>,
    prefs: Option<&UserSetupPrefs>,
    field_stack: Option<&terrain_generation::FieldStackParams>,
) -> String {
    let payload = terrain_recipe_hash_payload(registry, world_id, seed_override, prefs, field_stack);
    let bytes = serde_json::to_vec(&payload).expect("terrain recipe hash serialization");
    hex::encode(Sha256::digest(bytes))
}

#[derive(Serialize)]
struct TerrainRecipeHashPayload {
    world_id: Option<String>,
    seed: u64,
    sea_level: f32,
    spawn_x: f32,
    spawn_z: f32,
    coord_offset: [f32; 3],
    terrain_operations: Vec<game_data::TerrainOperationDefinition>,
    cave_operations: Vec<(String, Vec<game_data::TerrainOperationDefinition>)>,
    island_generation: Option<game_data::CompiledIslandGeneration>,
    prefs_world_id: Option<String>,
    prefs_preview_color_mode: Option<String>,
    prefs_island_overrides: Vec<(String, f32)>,
    field_stack: Option<terrain_generation::FieldStackParams>,
    river: Option<game_data::CompiledRiver>,
}

fn terrain_recipe_hash_payload(
    registry: &ConfigRegistry,
    world_id: Option<&StableId>,
    seed_override: Option<u64>,
    prefs: Option<&UserSetupPrefs>,
    field_stack: Option<&terrain_generation::FieldStackParams>,
) -> TerrainRecipeHashPayload {
    let world = registry.effective_world(world_id).expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let seed = seed_override.unwrap_or_else(|| prefs.map(|p| p.seed).unwrap_or(world.seed));
    let recipe = compile_terrain_recipe(registry, world, water, Some(seed));
    let terrain = registry.terrain.get(&world.terrain).expect("terrain");
    let cave_operations = terrain
        .includes
        .iter()
        .filter_map(|cave_id| {
            registry
                .caves
                .get(cave_id)
                .map(|cave| (cave_id.as_str().to_string(), cave.operations.clone()))
        })
        .collect();
    let island_generation = world.island_gen.as_ref().and_then(|id| {
        registry.island_gen.get(id).map(|base| {
            if let Some(prefs) = prefs {
                prefs.apply_overrides(base)
            } else {
                base.clone()
            }
        })
    });
    let (prefs_world_id, prefs_preview_color_mode, prefs_island_overrides) = if let Some(prefs) = prefs {
        (
            Some(prefs.world_id.clone()),
            Some(prefs.preview_color_mode.clone()),
            prefs
                .island_overrides
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
        )
    } else {
        (None, None, Vec::new())
    };
    TerrainRecipeHashPayload {
        world_id: world_id.map(|id| id.as_str().to_string()),
        seed,
        sea_level: recipe.sea_level,
        spawn_x: recipe.spawn_x,
        spawn_z: recipe.spawn_z,
        coord_offset: recipe.coord_offset,
        terrain_operations: terrain.operations.clone(),
        cave_operations,
        island_generation,
        prefs_world_id,
        prefs_preview_color_mode,
        prefs_island_overrides,
        field_stack: field_stack.cloned(),
        river: registry.demo_river().cloned(),
    }
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
        surface_recipe: None,
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
    let recipe = compile_terrain_recipe(registry, world, water, seed_override);
    let mut config = river_gen_config(river_def, seed, field_stack);
    config.surface_recipe = Some(recipe);
    let spline = generate_river_spline(&config, water.sea_level_m)?;
    Some(RiverCarveContext {
        spline,
        bank_width_m: river_def.bank_width_m,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_data::load_registry_from_directory;
    use std::path::PathBuf;
    use terrain_generation::{append_generated_island_caves, RecipeOp};

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

    #[test]
    fn generated_cave_count_respects_min_max_bounds() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry");
        let world = registry
            .world_by_id(&StableId::new("world.vs3_island"))
            .expect("world");
        let base = registry
            .island_generation_for_world(world)
            .expect("island")
            .clone();

        for count in 1u32..=4 {
            let mut cfg = base.clone();
            cfg.caves.chamber_count_min = count;
            cfg.caves.chamber_count_max = count;
            let mut ops = Vec::new();
            append_generated_island_caves(&mut ops, &cfg, world.seed);
            let chambers = ops
                .iter()
                .filter(|op| matches!(op, RecipeOp::Ellipsoid { .. }))
                .count();
            assert_eq!(chambers, count as usize, "min=max={count} should yield {count} chambers");
        }
    }

    #[test]
    fn terrain_recipe_hash_changes_with_river_source_depth() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry");
        let hash_a = terrain_recipe_hash(&registry, None, None, None, None);
        assert!(!hash_a.is_empty());
    }
}
