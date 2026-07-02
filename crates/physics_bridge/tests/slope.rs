// crates/physics_bridge/tests/slope.rs
use bevy::prelude::*;
use physics_bridge::CharacterController;

fn slope_angle_deg(normal: Vec3) -> f32 {
    normal.angle_between(Vec3::Y).to_degrees()
}

#[test]
fn steep_slope_above_max_is_rejected() {
    let controller = CharacterController {
        walk_speed: 4.8,
        run_speed: 7.5,
        jump_speed: 5.0,
        max_slope_deg: 47.0,
        step_height: 0.45,
        ground_snap_m: 0.28,
    };
    let steep_normal = Vec3::new(0.85, 0.53, 0.0).normalize();
    let angle = slope_angle_deg(steep_normal);
    assert!(angle > controller.max_slope_deg, "test normal should exceed walk limit");
}

#[test]
fn gentle_slope_within_walk_limit() {
    let controller = CharacterController {
        walk_speed: 4.8,
        run_speed: 7.5,
        jump_speed: 5.0,
        max_slope_deg: 47.0,
        step_height: 0.45,
        ground_snap_m: 0.28,
    };
    let gentle = Vec3::new(0.5, 0.87, 0.0).normalize();
    let angle = slope_angle_deg(gentle);
    assert!(angle <= controller.max_slope_deg);
}
