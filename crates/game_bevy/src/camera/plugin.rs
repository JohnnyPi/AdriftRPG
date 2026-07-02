// crates/game_bevy/src/camera/plugin.rs
use bevy::prelude::*;
use bevy::input::InputSystems;

use crate::state::AppState;

use super::environment;
use super::collision::{resolve_camera_collision, update_camera_debug_input};
use super::components::{CameraDebugSnapshot, CameraInputState};
use super::debug::draw_camera_debug;
use super::follow::update_camera_focus;
use super::input::update_cursor_capture;
use super::orbit::{read_camera_orbit_input, read_camera_zoom_input};
use super::transform::apply_camera_transform;

pub struct ThirdPersonCameraPlugin;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CameraSystemSet {
    Follow,
    Collision,
    Transform,
}

impl Plugin for ThirdPersonCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraInputState>()
            .init_resource::<CameraDebugSnapshot>()
            .init_resource::<environment::CameraEnvironment>()
            .configure_sets(
                Update,
                (
                    CameraSystemSet::Follow,
                    CameraSystemSet::Collision,
                    CameraSystemSet::Transform,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(
                PreUpdate,
                (update_cursor_capture, read_camera_orbit_input)
                    .chain()
                    .after(InputSystems)
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                environment::update_camera_environment
                    .in_set(CameraSystemSet::Follow)
                    .after(update_camera_focus),
            )
            .add_systems(
                Update,
                read_camera_zoom_input
                    .in_set(CameraSystemSet::Follow)
                    .before(update_camera_focus)
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                update_camera_focus.in_set(CameraSystemSet::Follow),
            )
            .add_systems(
                Update,
                (
                    update_camera_debug_input,
                    resolve_camera_collision,
                )
                    .chain()
                    .in_set(CameraSystemSet::Collision),
            )
            .add_systems(
                Update,
                apply_camera_transform.in_set(CameraSystemSet::Transform),
            )
            .add_systems(
                Update,
                draw_camera_debug
                    .after(CameraSystemSet::Transform)
                    .run_if(in_state(AppState::Running)),
            );
    }
}