use super::collision::resolve_collision_distance;
use super::components::{self, wrap_angle};
use super::{desired_camera_position, smooth_angle};
use bevy::prelude::*;

#[test]
fn wrap_angle_crosses_pi_boundary() {
    use std::f32::consts::PI;
    let wrapped = wrap_angle(PI + 0.1);
    assert!(wrapped < 0.0);
    assert!((wrap_angle(-PI - 0.1) - (PI - 0.1)).abs() < 1e-5);
}

#[test]
fn smooth_angle_takes_shortest_path() {
    use std::f32::consts::PI;
    let result = smooth_angle(PI - 0.2, -PI + 0.2, 100.0, 0.016);
    assert!((result - (PI - 0.2 + 0.4 * (1.0 - (-100.0 * 0.016_f32).exp()))).abs() < 0.05);
}

#[test]
fn desired_camera_position_behind_focus() {
    let focus = Vec3::ZERO;
    let pos = desired_camera_position(focus, 0.0, 0.49, 8.0, 0.0);
    assert!(pos.z > 0.0);
    assert!(pos.y > 0.0);
    assert!((pos.distance(focus) - 8.0).abs() < 0.01);
}

#[test]
fn resolve_collision_distance_contracts_faster_than_release() {
    let dt = 0.016;
    let contracted = resolve_collision_distance(8.0, 8.0, 3.0, 40.0, 8.0, dt);
    let expanded = resolve_collision_distance(3.0, 8.0, 8.0, 40.0, 8.0, dt);
    assert!(8.0 - contracted > expanded - 3.0);
}

#[test]
fn intent_yaw_combines_character_and_offset() {
    let camera = components::MmoCamera {
        target: Entity::PLACEHOLDER,
        player: Entity::PLACEHOLDER,
        character_yaw: 1.0,
        orbit_yaw_offset: 0.5,
        pitch: 0.0,
        distance: 8.0,
        current_yaw: 0.0,
        current_pitch: 0.0,
        current_distance: 8.0,
        current_focus: Vec3::ZERO,
        collision_limited_distance: 8.0,
        recenter_active: false,
        focus_height: 1.4,
        focus_offset_x: 0.0,
        focus_offset_z: 0.0,
        shoulder_offset: 0.0,
    };
    assert!((camera.intent_yaw() - 1.5).abs() < 1e-5);
}
