// crates/game_bevy/src/camera/orbit.rs
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use crate::data::ConfigRegistryResource;
use crate::ui::CameraTweaks;

use super::components::{CameraInputState, MmoCamera};
use super::fly_cam::fly_cam_active;
use super::{parse_recenter_key, components::wrap_angle};

pub fn read_camera_orbit_input(
    registry: Res<ConfigRegistryResource>,
    camera_tweaks: Res<CameraTweaks>,
    input_state: Res<CameraInputState>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut cameras: Query<&mut MmoCamera>,
) {
    if fly_cam_active(&camera_tweaks) {
        return;
    }
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };

    let Some(config) = registry.0.active_camera().ok() else {
        return;
    };
    let delta = mouse_motion.delta;

    if input_state.rotating_camera() {
        if input_state.right_steer {
            camera.character_yaw =
                wrap_angle(camera.character_yaw - delta.x * config.mouse_sensitivity_x);
            camera.orbit_yaw_offset = 0.0;
            camera.recenter_active = false;
        } else if input_state.left_look {
            camera.orbit_yaw_offset =
                wrap_angle(camera.orbit_yaw_offset + delta.x * config.mouse_sensitivity_x);
            camera.recenter_active = false;
        }

        let pitch_sign = if config.invert_y { -1.0 } else { 1.0 };
        camera.pitch = (camera.pitch
            + delta.y * config.mouse_sensitivity_y * pitch_sign)
            .clamp(config.pitch_minimum_rad, config.pitch_maximum_rad);
    }

    let recenter_key = parse_recenter_key(&config.recenter_key);
    if keyboard.just_pressed(recenter_key) {
        camera.recenter_active = true;
    }
}

pub fn read_camera_zoom_input(
    registry: Res<ConfigRegistryResource>,
    mut mouse_scroll: MessageReader<MouseWheel>,
    mut cameras: Query<&mut MmoCamera>,
) {
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };

    let Some(config) = registry.0.active_camera().ok() else {
        return;
    };
    let mut scroll_delta = 0.0f32;
    for event in mouse_scroll.read() {
        scroll_delta += match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y * 0.01,
        };
    }

    if scroll_delta.abs() <= f32::EPSILON {
        return;
    }

    camera.distance = (camera.distance - scroll_delta * config.zoom_speed)
        .clamp(config.distance_minimum_m, config.distance_maximum_m);
}
