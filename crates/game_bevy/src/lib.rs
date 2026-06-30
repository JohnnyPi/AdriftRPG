//! Bevy integration layer for the vertical slice.

mod camera;
mod data;
mod debug_tools;
mod environment;
mod interaction;
mod physics;
mod player;
mod plugin;
mod scene;
mod state;
mod terrain;
mod ui;
mod vegetation;
mod water;

pub use plugin::VerticalSlicePlugin;
pub use state::AppState;

use bevy::prelude::*;

/// Run the vertical slice application.
pub fn run() {
    App::new().add_plugins(VerticalSlicePlugin).run();
}
