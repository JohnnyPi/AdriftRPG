//! Milestone D integration tests — volumetric caves and water realization.

use game_data::{load_worldgen_bundle, resolve_world_bundle};
use terrain_generation::{
    CaveNodeKind, CompileOptions, FieldKey, VolumetricWorldProvider, WorldDensityProvider, WorldXZ,
    compile_world_from_bundle,
};

fn assets_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/worldgen")
}

#[test]
fn smoke_world_compiles_with_cave_pass() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile");
    assert_eq!(compiled.manifest.pass_reports.len(), 18);
    assert!(compiled.atlas.graphs.cave_systems.is_some());
    assert!(
        compiled
            .atlas
            .fields
            .get_scalar(FieldKey::LavaTubeSuitability)
            .is_some()
    );
}

#[test]
fn cave_system_count_meets_validation() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile");
    let caves = compiled
        .atlas
        .graphs
        .cave_systems
        .as_ref()
        .expect("cave systems");
    assert!(caves.system_count() >= 1);
    assert!(caves.traversable_system_count() >= 1);
}

#[test]
fn medium_produces_multiple_cave_systems() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load");
    let resolved = resolve_world_bundle("world.medium", &bundle).expect("resolve");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile");
    let caves = compiled
        .atlas
        .graphs
        .cave_systems
        .as_ref()
        .expect("cave systems");
    assert!(caves.system_count() >= 2, "expected >=2 cave systems");
}

#[test]
fn cave_deterministic_across_runs() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve");
    let options = CompileOptions::default();
    let a = compile_world_from_bundle(&resolved, &options).expect("compile a");
    let b = compile_world_from_bundle(&resolved, &options).expect("compile b");
    let ca = a.atlas.graphs.cave_systems.as_ref().unwrap();
    let cb = b.atlas.graphs.cave_systems.as_ref().unwrap();
    assert_eq!(ca.system_count(), cb.system_count());
    assert_eq!(ca.systems[0].nodes.len(), cb.systems[0].nodes.len());
}

#[test]
fn worldgen_density_has_interior_void() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile");
    let provider = VolumetricWorldProvider::from_compiled(&compiled);
    let caves = compiled.atlas.graphs.cave_systems.as_ref().unwrap();
    let system = &caves.systems[0];
    let sample_node = system
        .nodes
        .iter()
        .find(|n| !matches!(n.kind, CaveNodeKind::Entrance))
        .unwrap_or(&system.nodes[0]);
    let pos = sample_node.position;
    let density = provider.sample_density(pos);
    assert!(
        density > 0.0,
        "expected air inside cave chamber, got {density}"
    );
}

#[test]
fn compiler_lakes_produce_water_bodies() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile");
    let products = compiled
        .atlas
        .graphs
        .hydrology_products
        .as_ref()
        .expect("hydrology products");
    let graph = compiled
        .atlas
        .graphs
        .hydrology
        .as_ref()
        .expect("hydrology graph");
    assert_eq!(products.lakes.len(), graph.lakes.len());
}

#[test]
fn carved_elevation_field_exists() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile");
    assert!(
        compiled
            .atlas
            .fields
            .get_scalar(FieldKey::CarvedElevation)
            .is_some()
    );
}

#[test]
fn volumetric_provider_samples_surface() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile");
    let provider = VolumetricWorldProvider::from_compiled(&compiled);
    let surface = provider.sample_surface(WorldXZ::new(0.0, 0.0));
    assert!(surface.elevation_m.is_finite());
}

#[test]
#[ignore = "world.large compile is a manual perf benchmark; run with --ignored"]
fn large_world_compiles_with_validation() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load");
    let resolved = resolve_world_bundle("world.large", &bundle).expect("resolve");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile");
    assert_eq!(compiled.manifest.pass_reports.len(), 18);
}
