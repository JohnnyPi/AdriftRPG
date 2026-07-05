//! Milestone A integration and acceptance tests.

use game_data::{load_worldgen_bundle, recipe_content_hash, resolve_world_bundle};
use terrain_generation::{
    AtlasWorldProvider, CompileOptions, WorldDensityProvider, WorldPosition, WorldXZ,
    compile_world_from_bundle, derive_seed,
};

fn assets_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/worldgen")
}

#[test]
fn worldgen_yaml_fixture_resolves_and_hashes() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.small", &bundle).expect("resolve world recipe");
    let h1 = recipe_content_hash(&resolved.recipe);
    let h2 = recipe_content_hash(&resolved.recipe);
    assert_eq!(h1.0, h2.0);
    assert_eq!(resolved.recipe.islands.len(), 1);
}

#[test]
fn compile_small_world() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.small", &bundle).expect("resolve world recipe");
    let options = CompileOptions::default();
    let compiled = compile_world_from_bundle(&resolved, &options).expect("compile world");
    assert_eq!(compiled.manifest.pass_reports.len(), 18);
    assert!(
        compiled
            .atlas
            .fields
            .get_scalar(terrain_generation::FieldKey::FinalElevation)
            .is_some()
    );
}

#[test]
fn compilation_is_deterministic() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.small", &bundle).expect("resolve world recipe");
    let options = CompileOptions::default();
    let a = compile_world_from_bundle(&resolved, &options).expect("compile a");
    let b = compile_world_from_bundle(&resolved, &options).expect("compile b");
    let elev_a = a
        .atlas
        .fields
        .get_scalar(terrain_generation::FieldKey::FinalElevation)
        .unwrap();
    let elev_b = b
        .atlas
        .fields
        .get_scalar(terrain_generation::FieldKey::FinalElevation)
        .unwrap();
    assert_eq!(elev_a.values, elev_b.values);
}

#[test]
fn world_density_provider_samples_without_yaml() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.small", &bundle).expect("resolve world recipe");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile world");
    let provider = AtlasWorldProvider::from_compiled(&compiled);
    let surface = provider.sample_surface(WorldXZ::new(0.0, 0.0));
    assert!(surface.elevation_m > -5000.0);
    let density = provider.sample_density(WorldPosition::new(0.0, surface.elevation_m as f64, 0.0));
    assert!(density.abs() < 50.0);
}

#[test]
fn seed_derivation_is_stable() {
    let a = derive_seed(99, "test", None, 1);
    let b = derive_seed(99, "test", None, 1);
    assert_eq!(a, b);
}

#[test]
fn golden_world_asymmetric_caldera_compiles() {
    let mut bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    if let Some(island) = bundle.islands.get_mut("island.volcanic_small") {
        if let game_data::IslandPlacementSource::SingleCentered(ref mut s) = island.placement {
            s.volcano.caldera_radius_m = 520.0;
            s.volcano.caldera_depth_m = 140.0;
            if let game_data::FootprintSource::WarpedEllipse(ref mut f) = s.footprint {
                f.major_radius_m = 1800.0;
                f.minor_radius_m = 1400.0;
                f.warp_amplitude_m = 650.0;
            }
        }
    }
    let resolved = resolve_world_bundle("world.small", &bundle).expect("resolve caldera world");
    let compiled = compile_world_from_bundle(&resolved, &CompileOptions::default())
        .expect("compile caldera world");
    let elev = compiled
        .atlas
        .fields
        .get_scalar(terrain_generation::FieldKey::FinalElevation)
        .unwrap();
    let (min, max) = elev.min_max();
    assert!(max > 150.0);
    assert!(min < -50.0);
}

#[test]
fn golden_world_broad_old_island_compiles() {
    let mut bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    if let Some(island) = bundle.islands.get_mut("island.volcanic_small") {
        if let game_data::IslandPlacementSource::SingleCentered(ref mut s) = island.placement {
            s.age_myr = 8.0;
            if let game_data::FootprintSource::WarpedEllipse(ref mut f) = s.footprint {
                f.major_radius_m = 3200.0;
                f.minor_radius_m = 2800.0;
            }
            s.volcano.shield_radius_m = 3500.0;
            s.volcano.peak_height_m = 900.0;
        }
    }
    let resolved = resolve_world_bundle("world.small", &bundle).expect("resolve broad world");
    let compiled = compile_world_from_bundle(&resolved, &CompileOptions::default())
        .expect("compile broad world");
    let influence = compiled
        .atlas
        .fields
        .get_scalar(terrain_generation::FieldKey::IslandInfluence)
        .unwrap();
    let land_fraction = influence.values.iter().filter(|&&v| v > 0.5).count() as f32
        / influence.values.len() as f32;
    assert!(land_fraction > 0.02);
    assert!(land_fraction < 0.45);
}

#[test]
fn golden_world_extreme_narrow_island_compiles() {
    let mut bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    if let Some(island) = bundle.islands.get_mut("island.volcanic_small") {
        if let game_data::IslandPlacementSource::SingleCentered(ref mut s) = island.placement {
            if let game_data::FootprintSource::WarpedEllipse(ref mut f) = s.footprint {
                f.major_radius_m = 2200.0;
                f.minor_radius_m = 600.0;
                f.warp_amplitude_m = 900.0;
            }
            s.volcano.shield_radius_m = 1800.0;
        }
    }
    let resolved = resolve_world_bundle("world.small", &bundle).expect("resolve narrow world");
    let compiled = compile_world_from_bundle(&resolved, &CompileOptions::default())
        .expect("compile narrow world");
    let elev = compiled
        .atlas
        .fields
        .get_scalar(terrain_generation::FieldKey::FinalElevation)
        .unwrap();
    let (min, max) = elev.min_max();
    assert!(max > 100.0);
    assert!(min < -200.0);
}

#[test]
fn golden_world_tiny_shield_compiles() {
    let mut bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    if let Some(world) = bundle.worlds.get_mut("world.small") {
        world.seed = 1001;
    }
    if let Some(island) = bundle.islands.get_mut("island.volcanic_small") {
        if let game_data::IslandPlacementSource::SingleCentered(ref mut s) = island.placement {
            if let game_data::FootprintSource::WarpedEllipse(ref mut f) = s.footprint {
                f.major_radius_m = 1200.0;
                f.minor_radius_m = 900.0;
            }
            s.volcano.shield_radius_m = 1500.0;
        }
    }
    let resolved = resolve_world_bundle("world.small", &bundle).expect("resolve tiny world");
    let compiled = compile_world_from_bundle(&resolved, &CompileOptions::default())
        .expect("compile tiny world");
    let elev = compiled
        .atlas
        .fields
        .get_scalar(terrain_generation::FieldKey::FinalElevation)
        .unwrap();
    let (min, max) = elev.min_max();
    assert!(max > 200.0);
    assert!(min < -100.0);
}

#[test]
fn compile_smoke_world() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve smoke world");
    let compiled = compile_world_from_bundle(&resolved, &CompileOptions::default())
        .expect("compile smoke world");
    assert!(
        compiled
            .atlas
            .fields
            .get_scalar(terrain_generation::FieldKey::FinalElevation)
            .is_some()
    );
}

#[test]
fn compile_medium_world() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.medium", &bundle).expect("resolve medium world");
    let compiled = compile_world_from_bundle(&resolved, &CompileOptions::default())
        .expect("compile medium world");
    assert!(
        compiled
            .atlas
            .fields
            .get_scalar(terrain_generation::FieldKey::FinalElevation)
            .is_some()
    );
}
