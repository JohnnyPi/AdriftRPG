use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowFocused};

use super::components::CameraInputState;

/// Stub until UI/menu systems exist.
pub fn should_capture_input() -> bool {
    true
}

pub fn update_cursor_capture(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut input_state: ResMut<CameraInputState>,
    mut window_focused: MessageReader<WindowFocused>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    for event in window_focused.read() {
        if !event.focused {
            input_state.left_look = false;
            input_state.right_steer = false;
            input_state.cursor_captured = false;
        }
    }

    if !should_capture_input() {
        input_state.left_look = false;
        input_state.right_steer = false;
        input_state.cursor_captured = false;
    } else {
        input_state.left_look = mouse.pressed(MouseButton::Left);
        input_state.right_steer = mouse.pressed(MouseButton::Right);

        if keyboard.just_pressed(KeyCode::Escape) {
            input_state.left_look = false;
            input_state.right_steer = false;
            input_state.cursor_captured = false;
        } else {
            input_state.cursor_captured = input_state.rotating_camera();
        }
    }

    let Ok(mut cursor) = windows.single_mut() else {
        return;
    };

    cursor.visible = !input_state.cursor_captured;
    cursor.grab_mode = if input_state.cursor_captured {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
}
