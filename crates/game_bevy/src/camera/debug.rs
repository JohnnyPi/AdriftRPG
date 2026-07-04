// crates/game_bevy/src/camera/debug.rs
use bevy::prelude::*;

use crate::debug_tools::DebugOverlayState;

use super::components::{CameraDebugSnapshot, CameraInputState};
use super::{camera_forward_xz, camera_planar_basis};

pub fn draw_camera_debug(
    debug_overlay: Res<DebugOverlayState>,
    input_state: Res<CameraInputState>,
    snapshot: Res<CameraDebugSnapshot>,
    mut gizmos: Gizmos,
) {
    if !debug_overlay.debug_panel {
        return;
    }

    let focus = snapshot.focus;
    let desired = snapshot.desired_position;
    let final_pos = snapshot.final_position;

    gizmos.sphere(focus, 0.12, Color::srgb(0.2, 0.9, 0.3));
    gizmos.line(focus, desired, Color::srgb(0.2, 0.5, 0.95));

    if let Some(hit) = snapshot.hit_position {
        gizmos.sphere(hit, 0.1, Color::srgb(0.95, 0.2, 0.2));
    }

    gizmos.sphere(final_pos, 0.1, Color::srgb(0.95, 0.9, 0.2));

    let (forward, _) = camera_planar_basis(snapshot.intent_yaw);
    gizmos.line(focus, focus + forward * 2.0, Color::srgb(0.95, 0.95, 0.95));

    let character_forward = camera_forward_xz(snapshot.character_yaw);
    gizmos.line(
        focus,
        focus + character_forward * 1.8,
        Color::srgb(0.7, 0.3, 0.95),
    );

    let _ = input_state;
}
