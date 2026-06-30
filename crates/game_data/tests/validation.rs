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
