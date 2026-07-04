// crates/game_data/tests/validation.rs
use game_data::load_registry_from_directory;
use shared::DataError;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn workspace_assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .canonicalize()
        .expect("workspace assets directory")
}

#[test]
fn rejects_unknown_fields() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("bad.yaml");
    fs::write(
        &path,
        r#"schema_version: 1
id: player.default
unknown_field: true
capsule:
  radius_m: 0.38
  half_height_m: 0.72
movement:
  walk_speed_mps: 4.8
  run_speed_mps: 7.5
  acceleration_mps2: 26.0
  deceleration_mps2: 32.0
  rotation_speed_deg_per_s: 720.0
  maximum_walkable_slope_deg: 47.0
  step_height_m: 0.45
  ground_snap_m: 0.28
  jump_height_m: 1.15
gravity_mps2: 18.0
"#,
    )
    .expect("write yaml");

    let error = load_registry_from_directory(dir.path()).unwrap_err();
    assert!(matches!(error, DataError::Parse { .. }));
}

#[test]
fn rejects_duplicate_ids() {
    let dir = tempdir().expect("tempdir");
    let content = r#"schema_version: 1
id: player.default
capsule:
  radius_m: 0.38
  half_height_m: 0.72
movement:
  walk_speed_mps: 4.8
  run_speed_mps: 7.5
  acceleration_mps2: 26.0
  deceleration_mps2: 32.0
  rotation_speed_deg_per_s: 720.0
  maximum_walkable_slope_deg: 47.0
  step_height_m: 0.45
  ground_snap_m: 0.28
  jump_height_m: 1.15
gravity_mps2: 18.0
"#;
    fs::write(dir.path().join("a.yaml"), content).expect("write a");
    fs::write(dir.path().join("b.yaml"), content).expect("write b");

    let error = load_registry_from_directory(dir.path()).unwrap_err();
    assert!(matches!(error, DataError::DuplicateId { .. }));
}

#[test]
fn rejects_invalid_camera_range() {
    let dir = tempdir().expect("tempdir");
    fs::write(
        dir.path().join("camera.yaml"),
        r#"schema_version: 1
id: camera.mmo_default
orbit:
  default_distance: 8.0
  minimum_distance: 10.0
  maximum_distance: 16.0
  default_pitch_degrees: -28.0
  minimum_pitch_degrees: -65.0
  maximum_pitch_degrees: -8.0
  mouse_sensitivity_x: 0.0035
  mouse_sensitivity_y: 0.0030
  invert_y: false
  zoom_speed: 1.0
follow:
  focus_height: 1.4
  focus_offset_x: 0.0
  focus_offset_z: 0.0
  shoulder_offset: 0.0
  follow_sharpness: 18.0
  rotation_sharpness: 24.0
  zoom_sharpness: 20.0
collision:
  radius: 0.25
  margin: 0.10
  inward_sharpness: 40.0
  outward_sharpness: 8.0
controls:
  both_buttons_move_forward: true
  recenter_key: home
"#,
    )
    .expect("write camera");

    let error = load_registry_from_directory(dir.path()).unwrap_err();
    assert!(matches!(error, DataError::ValidationFailed { .. }));
}

#[test]
fn workspace_registry_resolves_active_profiles() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    assert_eq!(registry.active_world().expect("world").seed, 48129);
    assert_eq!(
        registry.active_player().expect("player").walk_speed_mps,
        4.8
    );
    assert_eq!(
        registry.active_camera().expect("camera").distance_default_m,
        8.0
    );
}

#[test]
fn biome_rules_accept_stub_profile_fields() {
    // Fully self-contained fixture: world-specific defs (world, biomes,
    // terrain, materials) are inlined with `*.test` ids instead of copying
    // asset files from removed worlds. Only world-agnostic config files
    // (player/camera/performance/water/lighting) are copied from assets.
    let dir = tempdir().expect("tempdir");
    fs::write(
        dir.path().join("app.yaml"),
        r#"schema_version: 1
id: app.test
world: world.test
player: player.default
camera: camera.mmo_default
performance: performance.rtx3070_60fps
"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("world.yaml"),
        r#"schema_version: 1
id: world.test
seed: 1
voxel:
  cell_size_m: 1.0
chunks:
  cells: [16, 16, 16]
  world_extent: [6, 3, 6]
terrain: terrain.test
biomes: biomes.test
materials: materials.test
water: water.tropical_shallow
lighting: lighting.late_morning
"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("biomes.yaml"),
        r#"schema_version: 1
id: biomes.test
rules:
  - id: grassland
    material_id: 0
    color: [0.3, 0.5, 0.2]
    vegetation_profile_id: vegetation.test
    ambient_audio_profile_id: audio.coastal_day
    weather_profile_id: weather.clear
    spawn_profile_id: spawn.coastal_wildlife
    gameplay_tags: [coastal, traversable]
    tint: [1.0, 1.0, 1.0]
    roughness_modifier: 0.05
    wetness_modifier: 0.1
"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("terrain.yaml"),
        r#"schema_version: 1
id: terrain.test
description: Empty terrain scaffold for biome stub validation
spawn: [0.0, 0.0, 0.0]
includes: []
operations: []
"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("materials.yaml"),
        r#"schema_version: 2
id: materials.test
description: Single-layer material scaffold for biome stub validation
materials:
  - key: grass
    id: 0
    name: grass
    albedo: [0.34, 0.52, 0.28]
    triplanar_scale: 0.5
    roughness: 0.9
layers: [grass]
"#,
    )
    .unwrap();
    fs::write(
        dir.path().join("surface.yaml"),
        r#"schema_version: 1
id: surface.test
description: Minimal surface scaffold for biome stub validation
classifiers:
  - id: land_default
    blend:
      - { material: grass, weight: 1.0 }
gates: []
"#,
    )
    .unwrap();
    for name in [
        "player.yaml",
        "camera.yaml",
        "performance.yaml",
        "water.yaml",
        "lighting.yaml",
    ] {
        let src = workspace_assets().join("config").join(name);
        if src.exists() {
            fs::copy(&src, dir.path().join(name)).unwrap();
        }
    }
    let render_profile =
        workspace_assets().join("terrain_materials/render_profiles/terrain_high.render.yaml");
    if render_profile.exists() {
        fs::copy(&render_profile, dir.path().join("terrain_high.render.yaml")).unwrap();
    }

    let registry = load_registry_from_directory(dir.path()).expect("registry with biome stubs");
    let biomes = registry
        .biomes
        .get(&shared::StableId::new("biomes.test"))
        .unwrap();
    assert_eq!(biomes.rules[0].gameplay_tags.len(), 2);
    assert!(biomes.rules[0].vegetation_profile_id.is_some());
}

#[test]
fn island_worlds_load_with_scale_appropriate_biomes() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let testbed = registry
        .world_by_id(&shared::StableId::new("world.island_testbed"))
        .expect("testbed");
    let large = registry
        .world_by_id(&shared::StableId::new("world.island_large"))
        .expect("large");
    assert_eq!(testbed.biomes.as_str(), "biomes.expanded_slice");
    assert_eq!(large.biomes.as_str(), "biomes.island_large");
    assert_eq!(large.surface.as_str(), "surface.island_large");
    assert!(testbed.island_gen.is_some());
    assert!(large.island_gen.is_some());
    assert!(!testbed.hydrology_bodies.is_empty());
}

#[test]
fn rejects_invalid_combine_op() {
    use game_data::{
        RawDefinition, TerrainGenerationDefinition, TerrainOperationDefinition,
        validate_definitions,
    };
    use shared::{DefinitionHeader, StableId};

    let report = validate_definitions(&[RawDefinition::TerrainGeneration(
        TerrainGenerationDefinition {
            header: DefinitionHeader {
                schema_version: 1,
                id: StableId::new("terrain.bad_combine"),
            },
            description: String::new(),
            spawn: None,
            includes: vec![],
            operations: vec![TerrainOperationDefinition::Ellipsoid {
                center: [0.0, 0.0, 0.0],
                radii: [1.0, 1.0, 1.0],
                peak_noise: None,
                combine: "subtractt".to_string(),
            }],
        },
    )]);

    assert!(!report.is_ok());
    let err = report.into_result().unwrap_err().to_string();
    assert!(err.contains("combine must be 'union' or 'subtract'"));
}

#[test]
fn rejects_non_standard_chunk_cells() {
    use game_data::{
        RawDefinition, WorldChunksDefinition, WorldDefinition, WorldVoxelDefinition,
        validate_definitions,
    };
    use shared::{DefinitionHeader, StableId};

    let report = validate_definitions(&[RawDefinition::World(WorldDefinition {
        header: DefinitionHeader {
            schema_version: 1,
            id: StableId::new("world.bad_chunks"),
        },
        seed: 1,
        voxel: WorldVoxelDefinition { cell_size_m: 1.0 },
        chunks: WorldChunksDefinition {
            cells: [32, 16, 16],
            world_extent: [6, 3, 6],
            residency: Default::default(),
            lod: Default::default(),
            staging: Default::default(),
        },
        terrain: StableId::new("terrain.test"),
        biomes: StableId::new("biomes.test"),
        materials: StableId::new("materials.test"),
        surface: None,
        water: StableId::new("water.test"),
        lighting: StableId::new("lighting.test"),
        sky: None,
        landmarks: None,
        structures: vec![],
        ocean_extent_m: None,
        coord_offset: None,
        island_gen: None,
        resolution: None,
        island_atlas_baked: None,
        hydrology_bodies: Vec::new(),
        material_catalog: None,
        vegetation: None,
        weather: None,
    })]);
}

#[test]
fn expanded_slice_materials_v2_loads_with_layer_order() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let materials = registry
        .materials
        .get(&shared::StableId::new("materials.expanded_slice"))
        .expect("expanded slice materials");
    assert!(materials.layer_order.len() >= 9);
    assert!(
        materials
            .layer_for_key(&shared::StableId::new("flowstone"))
            .is_some()
    );
    assert!(
        materials
            .layer_for_key(&shared::StableId::new("limestone"))
            .is_some()
    );
}

#[test]
fn rejects_duplicate_material_layers() {
    use game_data::{
        RawDefinition, TerrainMaterialEntryDefinition, TerrainMaterialsDefinition,
        validate_definitions,
    };
    use shared::{DefinitionHeader, StableId};

    let report = validate_definitions(&[RawDefinition::TerrainMaterials(
        TerrainMaterialsDefinition {
            header: DefinitionHeader {
                schema_version: 2,
                id: StableId::new("materials.bad_layers"),
            },
            description: String::new(),
            materials: vec![
                TerrainMaterialEntryDefinition {
                    key: Some(StableId::new("grass")),
                    id: None,
                    name: "grass".to_string(),
                    albedo: [0.3, 0.5, 0.2],
                    triplanar_scale: 0.5,
                    roughness: 0.9,
                    generator: None,
                    texture: None,
                    surface: None,
                    rendering: None,
                    responses: None,
                },
                TerrainMaterialEntryDefinition {
                    key: Some(StableId::new("sand")),
                    id: None,
                    name: "sand".to_string(),
                    albedo: [0.8, 0.7, 0.5],
                    triplanar_scale: 0.5,
                    roughness: 0.9,
                    generator: None,
                    texture: None,
                    surface: None,
                    rendering: None,
                    responses: None,
                },
            ],
            layers: vec![StableId::new("grass"), StableId::new("grass")],
        },
    )]);

    assert!(!report.is_ok());
    let err = report.into_result().unwrap_err().to_string();
    assert!(err.contains("duplicate layer key"));
}
