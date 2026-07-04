// crates/terrain_generation/tests/sampling_parity.rs
use game_data::load_registry_from_directory;
use std::path::PathBuf;
use terrain_generation::build_atlas_density_source_for_world;

fn assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .canonicalize()
        .expect("assets")
}

#[test]
fn island_testbed_column_and_distance_sources_match_atlas() {
    let registry = load_registry_from_directory(assets()).expect("registry");
    let world = registry
        .world_by_id(&shared::StableId::new("world.island_testbed"))
        .expect("world");
    let source = build_atlas_density_source_for_world(&registry, world, world.seed, None, None);
    let atlas = source.atlas().expect("atlas");

    for (wx, wz) in [(0.0, 0.0), (48.0, -32.0), (-64.0, 80.0), (120.0, 120.0)] {
        let column_h = source.column_surface_height_at(wx, wz);
        let atlas_h = atlas.surface_height_at(wx, wz);
        assert!(
            (column_h - atlas_h).abs() < 0.01,
            "column height mismatch at ({wx}, {wz}): column={column_h} atlas={atlas_h}"
        );

        let coast = source.distance_to_water_m(wx, wz);
        let atlas_coast = atlas.sample_coast_distance(wx, wz);
        assert!(
            (coast - atlas_coast).abs() < 0.01,
            "coast distance mismatch at ({wx}, {wz}): recipe path={coast} atlas={atlas_coast}"
        );
    }
}

#[test]
fn expanded_slice_alpine_threshold_is_28m() {
    let low = terrain_surface::compute_soft_biome_weights(&terrain_surface::EnvironmentSample {
        elevation: 20.0,
        slope_degrees: 20.0,
        moisture: 0.4,
        effective_moisture: 0.4,
        transition_noise: 0.5,
        temperature: 0.5,
        distance_to_water: 80.0,
        distance_to_river: 100.0,
        cave_depth: 0.0,
        world_y: 22.0,
    })
    .alpine;
    let high = terrain_surface::compute_soft_biome_weights(&terrain_surface::EnvironmentSample {
        elevation: 35.0,
        slope_degrees: 20.0,
        moisture: 0.4,
        effective_moisture: 0.4,
        transition_noise: 0.5,
        temperature: 0.5,
        distance_to_water: 80.0,
        distance_to_river: 100.0,
        cave_depth: 0.0,
        world_y: 37.0,
    })
    .alpine;
    assert!(
        high > low + 0.05,
        "alpine weight should increase above the 28 m band (low={low}, high={high})"
    );
}
