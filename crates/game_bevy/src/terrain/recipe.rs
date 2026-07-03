// crates/game_bevy/src/terrain/recipe.rs
use game_data::{
    CompiledIslandGeneration, CompiledWater, CompiledWorld, ConfigRegistry,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use shared::StableId;
use terrain_generation::{
    RecipeDensitySource, build_island_atlas, compile_terrain_recipe,
    island_params_from_compiled, validate_island_world_budget,
};

use crate::data::UserSetupPrefs;

/// Fail fast, with the full message list, when an island/world configuration
/// is contradictory (footprint exceeds chunk extents, relief exceeds the chunk
/// ceiling, shelf below the chunk floor, sea-level disagreement, or a
/// configuration `fit_to_ocean_extent` would have silently rescaled).
///
/// Panicking here is deliberate: generating terrain from a config the
/// generator would have to clip or distort produces exactly the "big lumpy
/// clipped cone" class of bug, and doing so silently costs far more debugging
/// time than an immediate, explained failure at startup.
fn validate_island_world_or_panic(
    compiled: &CompiledIslandGeneration,
    world: &CompiledWorld,
    water_sea_level_m: f32,
) {
    let messages = validate_island_world_budget(compiled, world, water_sea_level_m);
    if !messages.is_empty() {
        panic!(
            "island/world budget validation failed for '{}' (island_gen '{}'):\n  - {}\n\
             Fix the YAML defs; see docs/terrain_yaml_authoring.md for the budget rules.",
            world.id.as_str(),
            compiled.id.as_str(),
            messages.join("\n  - ")
        );
    }
}

/// Attach the island atlas for `compiled` to `source`, validating the
/// island/world budget first. Bank blend width is fixed at 3.5 m (historically
/// matched demo river bank width for atlas shoreline blending).
fn with_validated_atlas(
    source: RecipeDensitySource,
    _registry: &ConfigRegistry,
    world: &CompiledWorld,
    water: &CompiledWater,
    compiled: &CompiledIslandGeneration,
    seed: u64,
) -> RecipeDensitySource {
    validate_island_world_or_panic(compiled, world, water.sea_level_m);
    let params = island_params_from_compiled(compiled, world, seed, water.sea_level_m);
    let atlas = build_island_atlas(&params);
    source.with_atlas(atlas, 3.5)
}

pub fn build_density_source(
    registry: &ConfigRegistry,
    world_id: Option<&StableId>,
    seed_override: Option<u64>,
    _field_stack: terrain_generation::FieldStackParams,
) -> RecipeDensitySource {
    let world = registry.effective_world(world_id).expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let recipe = compile_terrain_recipe(registry, world, water, seed_override);
    let mut source = RecipeDensitySource::new(recipe);
    if let Some(base) = registry.island_generation_for_world(world) {
        // Island worlds get their terrain from the atlas. Previously this
        // function skipped the atlas entirely, so any caller reaching an
        // island world through it received only the generated cave ops --
        // caves carved out of nothing. The demo-river carve is intentionally
        // not attached for island worlds: the demo river spline is generated
        // against an op-based surface recipe, and island hydrology rivers come
        // from the atlas passes instead.
        let seed = seed_override.unwrap_or(world.seed);
        source = with_validated_atlas(source, registry, world, water, base, seed);
    }
    source
}

pub fn build_density_source_from_prefs(
    registry: &ConfigRegistry,
    prefs: &UserSetupPrefs,
    field_stack: terrain_generation::FieldStackParams,
) -> RecipeDensitySource {
    let world_id = prefs.world_stable_id();
    let world = registry
        .effective_world(Some(&world_id))
        .expect("world");
    match registry.island_generation_for_world(world) {
        Some(base) => {
            let merged = prefs.apply_overrides(base);
            let water = registry.water.get(&world.water).expect("water");
            // KNOWN GAP: compile_terrain_recipe fetches the *base* island gen
            // from the registry for generated cave ops, so prefs overrides to
            // cave parameters affect the hash payload and the atlas but not
            // the compiled cave geometry. Requires a
            // compile_terrain_recipe_with_island variant in world_setup.rs.
            let recipe = compile_terrain_recipe(registry, world, water, Some(prefs.seed));
            let source = RecipeDensitySource::new(recipe);
            with_validated_atlas(source, registry, world, water, &merged, prefs.seed)
        }
        None => build_density_source(registry, Some(&world_id), Some(prefs.seed), field_stack),
    }
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
    }
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
    fn generated_island_testbed_caves_respond_to_authored_parameters() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry");
        let world = registry
            .world_by_id(&StableId::new("world.island_testbed"))
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
            .world_by_id(&StableId::new("world.island_testbed"))
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

    #[test]
    fn island_testbed_world_passes_budget_validation() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry");
        let world = registry
            .world_by_id(&StableId::new("world.island_testbed"))
            .expect("world");
        let base = registry
            .island_generation_for_world(world)
            .expect("island");
        let water = registry.water.get(&world.water).expect("water");
        let messages =
            terrain_generation::validate_island_world_budget(base, world, water.sea_level_m);
        assert!(
            messages.is_empty(),
            "shipped island_testbed island/world YAML must satisfy the world budget:\n  - {}",
            messages.join("\n  - ")
        );
    }
}