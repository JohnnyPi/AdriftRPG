use bevy::prelude::*;

use super::components::{CameraDebugSnapshot, MainGameCamera, MmoCamera};
use super::desired_camera_position;

pub fn apply_camera_transform(
    mut debug: ResMut<CameraDebugSnapshot>,
    mut cameras: Query<(&mut Transform, &MmoCamera), With<MainGameCamera>>,
) {
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
