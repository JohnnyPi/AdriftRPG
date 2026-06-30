use bevy::prelude::*;

#[derive(Component, Debug)]
pub struct CameraFollowTarget;

#[derive(Component, Debug)]
pub struct CameraRig;

#[derive(Component, Debug)]
pub struct CameraPivot;

#[derive(Component, Debug)]
pub struct MainGameCamera;

/// Gameplay camera controller state — desired values updated by input, current values smoothed for rendering.
#[derive(Component, Debug, Clone)]
pub struct MmoCamera {
    pub target: Entity,
    pub player: Entity,

    /// Character facing (updated on right-mouse steering).
    pub character_yaw: f32,
    /// Free-look offset from character facing (updated on left-mouse drag).
    pub orbit_yaw_offset: f32,

    /// Desired orbit pitch (elevation radians) and distance.
    pub pitch: f32,
    pub distance: f32,

    /// Smoothed values used for rendering.
    pub current_yaw: f32,
    pub current_pitch: f32,
    pub current_distance: f32,
    pub current_focus: Vec3,

    /// Collision-adjusted distance (smoothed separately from zoom).
    pub collision_limited_distance: f32,

    /// When true, orbit_yaw_offset is smoothed toward zero (Home recenter).
    pub recenter_active: bool,

    pub focus_height: f32,
    pub focus_offset_x: f32,
    pub focus_offset_z: f32,
    pub shoulder_offset: f32,
}

impl MmoCamera {
    /// Yaw used for movement and steering — not the smoothed render yaw.
    pub fn intent_yaw(&self) -> f32 {
        wrap_angle(self.character_yaw + self.orbit_yaw_offset)
    }
}

#[derive(Resource, Debug, Default)]
pub struct CameraInputState {
    pub left_look: bool,
    pub right_steer: bool,
    pub cursor_captured: bool,
    pub autorun: bool,
}

impl CameraInputState {
    pub fn rotating_camera(&self) -> bool {
        self.left_look || self.right_steer
    }

    pub fn steering_character(&self) -> bool {
        self.right_steer
    }

    pub fn two_button_forward(&self) -> bool {
        self.left_look && self.right_steer
    }
}

#[derive(Component, Debug)]
pub struct CharacterFacing {
    pub yaw: f32,
    pub desired_yaw: f32,
    pub turn_speed: f32,
}

/// Interpolation snapshot for fixed-timestep player rendering.
#[derive(Component, Debug, Default)]
pub struct PlayerInterpolation {
    pub previous: Vec3,
    pub current: Vec3,
}

impl PlayerInterpolation {
    pub fn interpolated(&self, alpha: f32) -> Vec3 {
        self.previous.lerp(self.current, alpha)
    }
}

/// Runtime debug snapshot updated each frame when camera debug is enabled.
#[derive(Resource, Debug, Default, Clone)]
pub struct CameraDebugSnapshot {
    pub focus: Vec3,
    pub desired_position: Vec3,
    pub final_position: Vec3,
    pub hit_position: Option<Vec3>,
    pub intent_yaw: f32,
    pub character_yaw: f32,
    pub desired_distance: f32,
    pub collision_limited_distance: f32,
    pub left_look: bool,
    pub right_steer: bool,
}

pub fn wrap_angle(angle: f32) -> f32 {
    use std::f32::consts::PI;
    (angle + PI).rem_euclid(2.0 * PI) - PI
}
