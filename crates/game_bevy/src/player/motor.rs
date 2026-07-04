// crates/game_bevy/src/player/motor.rs
//! Formal movement intent separated from physical resolution (VS2 §9.1).

use bevy::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum MovementSpeed {
    #[default]
    Walk,
    Run,
}

#[derive(Component, Debug, Default)]
pub struct MovementIntent {
    pub direction: Vec2,
    pub requested_speed: MovementSpeed,
    pub jump_pressed: bool,
    pub jump_held: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LocomotionState {
    #[default]
    Grounded,
    Airborne,
    Sliding,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FacingMode {
    Movement,
    /// Explicit camera-facing mode (VS2 §9.7); distinct from Movement + steering.
    #[allow(dead_code)]
    Camera,
    /// Fixed heading for scripted actors (VS2 §9.7).
    #[allow(dead_code)]
    LockedYaw(f32),
}

impl Default for FacingMode {
    fn default() -> Self {
        Self::Movement
    }
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerFacingMode(pub FacingMode);

#[derive(Component, Debug, Default)]
pub struct CharacterMotorState {
    pub velocity: Vec3,
    pub grounded: bool,
    pub ground_normal: Vec3,
    pub current_slope: f32,
    pub locomotion_state: LocomotionState,
}

/// Resolve desired facing yaw from mode and input (VS2 §9.7).
pub fn resolve_facing_yaw(
    mode: FacingMode,
    steering_camera: bool,
    camera_yaw: f32,
    movement_dir: Vec2,
) -> f32 {
    match mode {
        FacingMode::Camera => camera_yaw,
        FacingMode::Movement if steering_camera => camera_yaw,
        FacingMode::LockedYaw(yaw) => yaw,
        FacingMode::Movement => {
            if movement_dir.length_squared() > 0.001 {
                movement_dir.x.atan2(movement_dir.y)
            } else {
                camera_yaw
            }
        }
    }
}

/// Classify locomotion from grounded state and slope (VS2 §9.1).
pub fn classify_locomotion(
    grounded: bool,
    slope_deg: f32,
    max_walkable_slope_deg: f32,
) -> LocomotionState {
    if !grounded {
        LocomotionState::Airborne
    } else if slope_deg >= max_walkable_slope_deg * 0.92 {
        LocomotionState::Sliding
    } else {
        LocomotionState::Grounded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::FRAC_PI_4;

    #[test]
    fn camera_mode_follows_camera_while_steering() {
        let yaw = resolve_facing_yaw(FacingMode::Camera, true, 1.2, Vec2::ZERO);
        assert!((yaw - 1.2).abs() < 1e-5);
    }

    #[test]
    fn locked_yaw_mode_returns_fixed_heading() {
        let yaw = resolve_facing_yaw(FacingMode::LockedYaw(0.75), false, 0.0, Vec2::ZERO);
        assert!((yaw - 0.75).abs() < 1e-5);
    }

    #[test]
    fn movement_mode_faces_travel_direction() {
        let yaw = resolve_facing_yaw(FacingMode::Movement, false, 0.0, Vec2::new(1.0, 1.0));
        assert!((yaw - FRAC_PI_4).abs() < 0.01);
    }

    #[test]
    fn steep_slope_becomes_sliding() {
        assert_eq!(
            classify_locomotion(true, 50.0, 47.0),
            LocomotionState::Sliding
        );
    }
}
