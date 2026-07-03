// crates/game_bevy/src/camera/transform.rs
use bevy::prelude::*;

use super::components::{CameraDebugSnapshot, MainGameCamera, MmoCamera};
use super::desired_camera_position;
use super::fly_cam::{fly_cam_active, FlyCamState};
use crate::ui::CameraTweaks;

pub fn apply_camera_transform(
    camera_tweaks: Res<CameraTweaks>,
    fly: Res<FlyCamState>,
    mut debug: ResMut<CameraDebugSnapshot>,
    mut cameras: Query<(&mut Transform, &MmoCamera), With<MainGameCamera>>,
) {
    if fly_cam_active(&camera_tweaks) && fly.initialized {
        for (mut transform, _camera) in &mut cameras {
            let look_target = fly.position + super::camera_view_direction(fly.yaw, fly.pitch);
            transform.translation = fly.position;
            transform.look_at(look_target, Vec3::Y);
            debug.final_position = fly.position;
        }
        return;
    }

    for (mut transform, camera) in &mut cameras {
        let focus = camera.current_focus;
        let position = desired_camera_position(
            focus,
            camera.current_yaw,
            camera.current_pitch,
            camera.collision_limited_distance,
            camera.shoulder_offset,
        );

        transform.translation = position;
        transform.look_at(focus, Vec3::Y);

        debug.final_position = position;
    }
}
