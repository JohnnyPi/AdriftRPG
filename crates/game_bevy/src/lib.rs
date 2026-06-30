//! Bevy integration layer for the vertical slice.

mod camera;
mod data;
mod debug_tools;
mod environment;
mod expanded;
mod interaction;
mod performance;
mod physics;
mod player;
mod plugin;
mod scene;
mod state;
mod structures;
mod terrain;
mod ui;
mod vegetation;
mod water;
mod world;

pub use expanded::ExpandedVerticalSlicePlugin;
pub use performance::{PerformanceReport, PerformanceValidationPlugin};
pub use plugin::VerticalSlicePlugin;
pub use state::AppState;

use bevy::prelude::*;

/// Run the vertical slice application.
pub fn run() {
    App::new().add_plugins(VerticalSlicePlugin).run();
}
