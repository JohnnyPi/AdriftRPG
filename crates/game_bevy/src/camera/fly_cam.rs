// crates/game_bevy/src/camera/fly_cam.rs
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;

use crate::data::ConfigRegistryResource;
use crate::ui::CameraTweaks;

use super::components::{CameraInputState, MainGameCamera, MmoCamera};
use super::{camera_view_direction, clamp_visual_delta, components::wrap_angle, desired_camera_position};

#[derive(Resource, Debug, Default)]
pub struct FlyCamState {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub initialized: bool,
}

pub fn fly_cam_active(tweaks: &CameraTweaks) -> bool {
    tweaks.fly_cam
}

pub fn capture_fly_cam_from_orbit(
    camera_tweaks: Res<CameraTweaks>,
    mut fly: ResMut<FlyCamState>,
    mut last_active: Local<bool>,
    cameras: Query<&MmoCamera>,
    camera_transforms: Query<&Transform, With<MainGameCamera>>,
) {
    let active = fly_cam_active(&camera_tweaks);
    if active && !*last_active {
        fly.initialized = false;
    }
    if !active {
        fly.initialized = false;
        *last_active = active;
        return;
    }
    if fly.initialized {
        *last_active = active;
        return;
    }

    if let Ok(transform) = camera_transforms.single() {
        fly.position = transform.translation;
    }
    if let Ok(camera) = cameras.single() {
        fly.yaw = camera.current_yaw;
        fly.pitch = camera.current_pitch;
        fly.position = desired_camera_position(
            camera.current_focus,
            camera.current_yaw,
            camera.current_pitch,
            camera.collision_limited_distance,
            camera.shoulder_offset,
        );
    }
    fly.initialized = true;
    *last_active = active;
}

pub fn update_fly_cam(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    input_state: Res<CameraInputState>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    registry: Res<ConfigRegistryResource>,
    camera_tweaks: Res<CameraTweaks>,
    mut fly: ResMut<FlyCamState>,
) {
    if !fly_cam_active(&camera_tweaks) || !fly.initialized {
        return;
    }

    let Some(config) = registry.0.active_camera().ok() else {
        return;
    };
    let dt = clamp_visual_delta(time.delta_secs());

    if input_state.left_look {
        let delta = mouse_motion.delta;
        fly.yaw = wrap_angle(fly.yaw - delta.x * config.mouse_sensitivity_x);
        let pitch_sign = if config.invert_y { -1.0 } else { 1.0 };
        fly.pitch = (fly.pitch + delta.y * config.mouse_sensitivity_y * pitch_sign)
            .clamp(-1.45, 1.45);
    }

    let view_forward = camera_view_direction(fly.yaw, fly.pitch);
    let planar_right = Vec3::new(fly.yaw.cos(), 0.0, -fly.yaw.sin());
    let mut move_dir = Vec3::ZERO;
    if keyboard.pressed(KeyCode::KeyW) {
        move_dir += view_forward;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        move_dir -= view_forward;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        move_dir += planar_right;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        move_dir -= planar_right;
    }
    if keyboard.pressed(KeyCode::Space) {
        move_dir += Vec3::Y;
    }
    if keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight) {
        move_dir -= Vec3::Y;
    }

    if move_dir.length_squared() > f32::EPSILON {
        let speed = camera_tweaks.fly_cam_speed_mps
            * if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
                3.0
            } else {
                1.0
            };
        fly.position += move_dir.normalize() * speed * dt;
    }
}
