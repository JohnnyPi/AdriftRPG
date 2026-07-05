//! Milestone B integration tests (Phases 8–10).

use game_data::{load_worldgen_bundle, resolve_world_bundle};
use terrain_generation::{
    AtlasWorldProvider, CompileOptions, FieldKey, WorldDensityProvider, WorldXZ,
    compile_world_from_bundle,
};

fn assets_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/worldgen")
}

#[test]
fn compile_milestone_b_world_has_climate_and_hydrology_fields() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.small", &bundle).expect("resolve world recipe");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile world");

    assert!(
        compiled
            .atlas
            .fields
            .get_scalar(FieldKey::Temperature)
            .is_some()
    );
    assert!(
        compiled
            .atlas
            .fields
            .get_scalar(FieldKey::Rainfall)
            .is_some()
    );
    assert!(
        compiled
            .atlas
            .fields
            .get_scalar(FieldKey::ErodedElevation)
            .is_some()
    );
    assert!(
        compiled
            .atlas
            .fields
            .get_scalar(FieldKey::FlowAccumulation)
            .is_some()
    );
    assert!(compiled.atlas.graphs.hydrology.is_some());
}

#[test]
fn climate_higher_elevation_is_cooler() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.small", &bundle).expect("resolve world recipe");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile world");
    let temp = compiled
        .atlas
        .fields
        .get_scalar(FieldKey::Temperature)
        .unwrap();
    let elev = compiled
        .atlas
        .fields
        .get_scalar(FieldKey::ErodedElevation)
        .unwrap();

    let (min_elev, max_elev) = elev.min_max();
    let mut low_temp = f32::MAX;
    let mut high_temp = f32::MIN;
    for z in 0..elev.descriptor.height {
        for x in 0..elev.descriptor.width {
            let e = elev.get(x, z);
            let t = temp.get(x, z);
            if (e - min_elev).abs() < 1.0 {
                low_temp = low_temp.min(t);
            }
            if (e - max_elev).abs() < 5.0 {
                high_temp = high_temp.max(t);
            }
        }
    }
    assert!(
        high_temp < low_temp,
        "summit should be cooler than lowlands"
    );
}

#[test]
fn compilation_milestone_b_is_deterministic() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve testbed");
    let options = CompileOptions::default();
    let a = compile_world_from_bundle(&resolved, &options).expect("compile a");
    let b = compile_world_from_bundle(&resolved, &options).expect("compile b");
    let elev_a = a
        .atlas
        .fields
        .get_scalar(FieldKey::ErodedElevation)
        .unwrap();
    let elev_b = b
        .atlas
        .fields
        .get_scalar(FieldKey::ErodedElevation)
        .unwrap();
    assert_eq!(elev_a.values, elev_b.values);
}

#[test]
fn provider_exposes_climate_and_river() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve testbed");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile testbed");
    let provider = AtlasWorldProvider::from_compiled(&compiled);
    let column = provider.sample_column(WorldXZ::new(0.0, 0.0));
    assert!(column.temperature > 0.0);
    assert!(column.humidity > 0.0);
    assert!(provider.hydrology_graph().is_some());
}

#[test]
fn testbed_compiles_with_primary_river() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve testbed");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile testbed");
    let hydro = compiled.atlas.graphs.hydrology.as_ref().unwrap();
    assert!(
        hydro.primary_river.is_some(),
        "testbed should produce a primary river"
    );
}
