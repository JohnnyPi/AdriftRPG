use game_data::load_registry_from_directory;
use std::path::PathBuf;
use terrain_generation::{WorldSetupError, resolve_island_atlas};

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .canonicalize()
        .expect("assets")
}

#[test]
#[ignore = "legacy island_gen; superseded by worldgen tier worlds (docs/worlds/)"]
fn resolve_island_atlas_bad_baked_path_returns_err() {
    let registry = load_registry_from_directory(assets()).expect("registry");
    let world_id = shared::StableId::new("world.small");
    let mut world = registry.world_by_id(&world_id).expect("world").clone();
    let island = registry
        .island_generation_for_world(&world)
        .expect("island gen")
        .clone();

    world.island_atlas_baked = Some("terrain/baked/missing.seed999.atlas".into());

    let result = resolve_island_atlas(&island, &world, world.seed, 0.0, Some(assets().as_path()));

    assert!(
        matches!(result, Err(WorldSetupError::AtlasLoad { .. })),
        "expected atlas load error, got {result:?}"
    );
}
