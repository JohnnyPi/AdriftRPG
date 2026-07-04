// crates/game_bevy/src/camera/follow.rs
use bevy::prelude::*;

use crate::data::ConfigRegistryResource;
use crate::player::Player;
use crate::ui::CameraTweaks;

use super::components::{CameraFollowTarget, MmoCamera, PlayerInterpolation};
use super::fly_cam::fly_cam_active;
use super::{clamp_visual_delta, components::wrap_angle, smooth_angle, smooth_scalar, smooth_vec3};

pub fn update_camera_focus(
    time: Res<Time>,
    fixed_time: Res<Time<Fixed>>,
    registry: Res<ConfigRegistryResource>,
    camera_tweaks: Res<CameraTweaks>,
    players: Query<&PlayerInterpolation, With<Player>>,
    follow_targets: Query<Entity, With<CameraFollowTarget>>,
    mut cameras: Query<&mut MmoCamera>,
) {
    if fly_cam_active(&camera_tweaks) {
        return;
    }
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };

    if !follow_targets.contains(camera.target) {
        return;
    }

    let Ok(interpolation) = players.get(camera.player) else {
        return;
    };

    let Some(config) = registry.0.active_camera().ok() else {
        return;
    };
    let dt = clamp_visual_delta(time.delta_secs());
    let alpha = fixed_time.overstep_fraction();

    let player_pos = interpolation.interpolated(alpha);
    let desired_focus = player_pos
        + Vec3::new(
            camera.focus_offset_x,
            camera.focus_height,
            camera.focus_offset_z,
        );

    camera.current_focus = smooth_vec3(
        camera.current_focus,
        desired_focus,
        config.follow_sharpness,
        dt,
    );

    if camera.recenter_active {
        camera.orbit_yaw_offset =
            smooth_angle(camera.orbit_yaw_offset, 0.0, config.rotation_sharpness, dt);
        if camera.orbit_yaw_offset.abs() < 0.001 {
            camera.orbit_yaw_offset = 0.0;
            camera.recenter_active = false;
        }
    }

    let desired_total_yaw = wrap_angle(camera.character_yaw + camera.orbit_yaw_offset);
    camera.current_yaw = smooth_angle(
        camera.current_yaw,
        desired_total_yaw,
        config.rotation_sharpness,
        dt,
    );

    camera.current_pitch = smooth_scalar(
        camera.current_pitch,
        camera.pitch,
        config.rotation_sharpness,
        dt,
    );

    camera.current_distance = smooth_scalar(
        camera.current_distance,
        camera.distance,
        config.zoom_sharpness,
        dt,
    );
}
