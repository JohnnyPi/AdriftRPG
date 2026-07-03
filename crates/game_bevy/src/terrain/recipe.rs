// crates/game_bevy/src/terrain/recipe.rs
use game_data::ConfigRegistry;
use serde::Serialize;
use sha2::{Digest, Sha256};
use shared::StableId;
use terrain_generation::{
    build_atlas_density_source_for_world, compile_terrain_recipe,
    compile_terrain_recipe_with_island, generate_river_spline, RecipeDensitySource,
    RiverCarveContext, RiverGenConfig, WorldVolumeBounds,
};

use crate::data::{assets_root, UserSetupPrefs};

pub fn build_density_source(
    registry: &ConfigRegistry,
    world_id: Option<&StableId>,
    seed_override: Option<u64>,
    field_stack: terrain_generation::FieldStackParams,
) -> RecipeDensitySource {
    build_density_source_with_assets(registry, world_id, seed_override, field_stack, None)
}

pub fn build_density_source_with_assets(
    registry: &ConfigRegistry,
    world_id: Option<&StableId>,
    seed_override: Option<u64>,
    field_stack: terrain_generation::FieldStackParams,
    assets_root: Option<&std::path::Path>,
) -> RecipeDensitySource {
    let world = registry.effective_world(world_id).expect("world");
    if registry.island_generation_for_world(world).is_some() {
        let seed = seed_override.unwrap_or(world.seed);
        build_atlas_density_source_for_world(registry, world, seed, assets_root, None)
    } else {
        build_legacy_density_source(registry, world, seed_override, field_stack)
    }
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
            build_atlas_density_source_for_world(
                registry,
                world,
                prefs.seed,
                Some(assets_root().as_path()),
                Some(&merged),
            )
        }
        None => build_density_source_with_assets(
            registry,
            Some(&world_id),
            Some(prefs.seed),
            field_stack,
            Some(assets_root().as_path()),
        ),
    }
}

/// Non-island worlds: op-based recipe density without an atlas.
fn build_legacy_density_source(
    registry: &ConfigRegistry,
    world: &game_data::CompiledWorld,
    seed_override: Option<u64>,
    field_stack: terrain_generation::FieldStackParams,
) -> RecipeDensitySource {
    let water = registry.water.get(&world.water).expect("water");
    let recipe = compile_terrain_recipe(registry, world, water, seed_override);
    let bounds = WorldVolumeBounds::from_compiled_world(world);
    let mut source = RecipeDensitySource::new(recipe.clone())
        .with_field_stack(field_stack)
        .with_world_bounds(bounds);

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
    let island_generation = world.island_gen.as_ref().and_then(|id| {
        registry.island_gen.get(id).map(|base| {
            if let Some(prefs) = prefs {
                prefs.apply_overrides(base)
            } else {
                base.clone()
            }
        })
    });
    let recipe = compile_terrain_recipe_with_island(
        registry,
        world,
        water,
        Some(seed),
        island_generation.as_ref(),
    );
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
    let effective_field_stack = if world.island_gen.is_some() {
        None
    } else {
        field_stack.cloned()
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
        field_stack: effective_field_stack,
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
    fn authored_testbed_terrain_compiles_with_river_carve() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry");
        let world = registry
            .world_by_id(&StableId::new("world.island_testbed"))
            .expect("world");
        assert!(
            world.island_gen.is_none(),
            "testbed must be an op-based authored world"
        );
        let source = build_density_source(&registry, Some(&StableId::new("world.island_testbed")), None, terrain_generation::FieldStackParams::default());
        assert!(
            source.river_carve().is_some(),
            "authored testbed should generate a carved primary river"
        );
        assert!(source.atlas().is_none());
    }

    #[test]
    fn island_large_world_passes_budget_validation() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry");
        let world = registry
            .world_by_id(&StableId::new("world.island_large"))
            .expect("world");
        let base = registry
            .island_generation_for_world(world)
            .expect("island");
        let water = registry.water.get(&world.water).expect("water");
        let messages =
            terrain_generation::validate_island_world_budget(base, world, water.sea_level_m);
        assert!(
            messages.is_empty(),
            "shipped island_large island/world YAML must satisfy the world budget:\n  - {}",
            messages.join("\n  - ")
        );
    }

    #[test]
    fn terrain_recipe_hash_changes_with_field_stack() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry");
        let Some((world_id, _)) = registry
            .worlds
            .iter()
            .find(|(_, world)| world.island_gen.is_none())
        else {
            return;
        };
        let default_stack = terrain_generation::FieldStackParams::default();
        let mut tweaked = default_stack.clone();
        tweaked.ridge_amplitude = default_stack.ridge_amplitude + 1.0;
        let hash_a = terrain_recipe_hash(
            &registry,
            Some(world_id),
            None,
            None,
            Some(&default_stack),
        );
        let hash_b = terrain_recipe_hash(
            &registry,
            Some(world_id),
            None,
            None,
            Some(&tweaked),
        );
        assert_ne!(hash_a, hash_b);
    }
}
