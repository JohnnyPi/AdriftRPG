mod collision;
mod components;
mod debug;
mod follow;
mod input;
mod orbit;
mod environment;
mod plugin;
mod spawn;
mod transform;

#[cfg(test)]
mod tests;

pub use components::{
    CameraDebugSnapshot, CameraFollowTarget, CameraInputState, CameraPivot, CameraRig,
    CharacterFacing, MainGameCamera, MmoCamera, PlayerInterpolation,
};
pub use plugin::ThirdPersonCameraPlugin;
pub use spawn::spawn_game_camera;

use bevy::prelude::*;

pub(crate) fn exp_smoothing_factor(sharpness: f32, delta_seconds: f32) -> f32 {
    1.0 - (-sharpness * delta_seconds).exp()
}

pub(crate) fn smooth_vec3(
    current: Vec3,
    target: Vec3,
    sharpness: f32,
    delta_seconds: f32,
) -> Vec3 {
    current.lerp(target, exp_smoothing_factor(sharpness, delta_seconds))
}

pub(crate) fn smooth_scalar(
    current: f32,
    target: f32,
    sharpness: f32,
    delta_seconds: f32,
) -> f32 {
    current + (target - current) * exp_smoothing_factor(sharpness, delta_seconds)
}

pub(crate) fn smooth_angle(
    current: f32,
    target: f32,
    sharpness: f32,
    delta_seconds: f32,
) -> f32 {
    let difference = components::wrap_angle(target - current);
    current + difference * exp_smoothing_factor(sharpness, delta_seconds)
}

pub(crate) fn desired_camera_position(
    focus: Vec3,
    yaw: f32,
    pitch: f32,
    distance: f32,
    shoulder_offset: f32,
) -> Vec3 {
    // `pitch` is positive elevation above the horizontal plane behind the character.
    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-pitch);
    let backward = rotation * Vec3::Z;
    let right = rotation * Vec3::X;
    focus + backward * distance + right * shoulder_offset
}

pub(crate) fn camera_planar_basis(yaw: f32) -> (Vec3, Vec3) {
    let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(forward.z, 0.0, -forward.x);
    (forward.normalize_or_zero(), right.normalize_or_zero())
}

pub(crate) fn camera_view_direction(yaw: f32, pitch: f32) -> Vec3 {
    // Camera sits behind the focus along the "backward" vector; view direction is the opposite.
    let backward = Vec3::new(
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        yaw.cos() * pitch.cos(),
    );
    (-backward).normalize_or_zero()
}

pub(crate) fn camera_forward_xz(yaw: f32) -> Vec3 {
    camera_planar_basis(yaw).0
}

pub(crate) fn camera_right_xz(yaw: f32) -> Vec3 {
    camera_planar_basis(yaw).1
}

pub(crate) fn clamp_visual_delta(dt: f32) -> f32 {
    dt.min(0.05)
}

pub(crate) fn parse_recenter_key(key: &str) -> KeyCode {
    match key.to_ascii_lowercase().as_str() {
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        _ => KeyCode::Home,
    }
}
