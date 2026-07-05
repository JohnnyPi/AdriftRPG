//! Milestone C integration tests (Phases 11–13).

use game_data::{load_worldgen_bundle, resolve_world_bundle};
use terrain_generation::{
    AtlasWorldProvider, CompileOptions, FieldKey, WorldDensityProvider, WorldXZ,
    compile_world_from_bundle,
};

fn assets_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/worldgen")
}

#[test]
fn compile_milestone_c_world_has_coastal_biome_and_strata_fields() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.small", &bundle).expect("resolve world recipe");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile world");

    assert_eq!(compiled.manifest.pass_reports.len(), 18);
    assert!(
        compiled
            .atlas
            .fields
            .get_scalar(FieldKey::ReefSuitability)
            .is_some()
    );
    assert!(
        compiled
            .atlas
            .fields
            .get_scalar(FieldKey::SoilDepth)
            .is_some()
    );
    assert!(
        compiled
            .atlas
            .fields
            .get_categorical(FieldKey::PrimaryBiome)
            .is_some()
    );
    assert!(
        compiled
            .atlas
            .fields
            .get_scalar(FieldKey::RegolithDepth)
            .is_some()
    );
    assert!(compiled.atlas.graphs.biome.is_some());
}

#[test]
fn provider_column_uses_soil_depth_not_hardcoded() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve world recipe");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile world");
    let provider = AtlasWorldProvider::from_compiled(&compiled);
    let column = provider.sample_column(WorldXZ::new(0.0, 0.0));
    if column.surface.land_mask > 0.5 {
        assert!(
            column.soil_depth_m > 0.0,
            "land column should have positive soil depth from atlas"
        );
        assert_ne!(
            column.regolith_depth_m, 1.5,
            "regolith depth should come from strata pass, not legacy hardcoded 1.5"
        );
    }
}

#[test]
fn windward_forest_exceeds_leeward_on_trade_wind_world() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.small", &bundle).expect("resolve world recipe");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile world");
    let biome_report = compiled
        .manifest
        .pass_reports
        .iter()
        .find(|r| format!("{:?}", r.pass).contains("Biome"))
        .expect("biome pass report");
    let windward = biome_report
        .metrics
        .get("windward_forest_mean")
        .copied()
        .unwrap_or(0.0);
    let leeward = biome_report
        .metrics
        .get("leeward_forest_mean")
        .copied()
        .unwrap_or(0.0);
    assert!(
        windward >= leeward,
        "windward forest mean {windward} should be >= leeward {leeward}"
    );
}

#[test]
fn small_world_land_cells_have_multiple_primary_biomes() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.small", &bundle).expect("resolve world recipe");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile world");
    let provider = AtlasWorldProvider::from_compiled(&compiled);
    let mut seen = std::collections::BTreeSet::new();
    let metadata = provider.world_metadata();
    let half = metadata.extent.width_m.min(metadata.extent.depth_m) as f64 * 0.25;
    for ix in 0..32 {
        for iz in 0..32 {
            let wx = (ix as f64 / 31.0 - 0.5) * half * 2.0;
            let wz = (iz as f64 / 31.0 - 0.5) * half * 2.0;
            let column = provider.sample_column(WorldXZ::new(wx, wz));
            if column.surface.land_mask > 0.5 {
                seen.insert(column.primary_biome);
            }
        }
    }
    assert!(
        seen.len() >= 4,
        "expected at least 4 land primary biomes, got {seen:?}"
    );
}

#[test]
fn provider_exposes_biome_blend_samples() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve world recipe");
    let compiled =
        compile_world_from_bundle(&resolved, &CompileOptions::default()).expect("compile world");
    let provider = AtlasWorldProvider::from_compiled(&compiled);
    let blend = provider.sample_biome_blend(WorldXZ::new(0.0, 0.0));
    assert!(blend.is_some());
}
#[test]
fn milestone_c_compilation_is_deterministic() {
    let bundle = load_worldgen_bundle(&assets_root()).expect("load worldgen assets");
    let resolved = resolve_world_bundle("world.smoke", &bundle).expect("resolve world recipe");
    let options = CompileOptions::default();
    let a = compile_world_from_bundle(&resolved, &options).expect("compile a");
    let b = compile_world_from_bundle(&resolved, &options).expect("compile b");
    let reef_a = a
        .atlas
        .fields
        .get_scalar(FieldKey::ReefSuitability)
        .unwrap();
    let reef_b = b
        .atlas
        .fields
        .get_scalar(FieldKey::ReefSuitability)
        .unwrap();
    assert_eq!(reef_a.values, reef_b.values);
}
