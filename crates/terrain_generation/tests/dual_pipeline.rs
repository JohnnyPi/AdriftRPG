//! Worldgen smoke test: compiled smoke world produces rivers and land.

use game_data::{load_worldgen_bundle, resolve_world_bundle};
use terrain_generation::{CompileOptions, FieldKey, compile_world_from_bundle};

fn assets_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/worldgen")
}

#[test]
fn smoke_worldgen_produces_rivers_and_land() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve");
    let compiled = compile_world_from_bundle(&resolved, &CompileOptions::default())
        .expect("compile worldgen smoke");

    let elev = compiled
        .atlas
        .fields
        .get_scalar(FieldKey::ErodedElevation)
        .unwrap();
    let (min_e, max_e) = elev.min_max();
    assert!(
        max_e > 25.0,
        "worldgen peak should be above 25m, got {max_e}"
    );
    assert!(min_e < 5.0, "worldgen should have below-sea samples");

    let hydro = compiled.atlas.graphs.hydrology.as_ref().unwrap();
    assert!(
        hydro.primary_river.is_some(),
        "worldgen smoke should trace a primary river"
    );

    let worldgen_land = compiled
        .atlas
        .fields
        .get_scalar(FieldKey::LandMask)
        .unwrap();
    let land_fraction = worldgen_land.values.iter().filter(|&&v| v > 0.5).count() as f32
        / worldgen_land.values.len() as f32;
    assert!(land_fraction > 0.02 && land_fraction < 0.5);
}
