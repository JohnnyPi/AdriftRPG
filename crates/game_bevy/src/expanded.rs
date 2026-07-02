// crates/game_bevy/src/expanded.rs
//! VS2 §20 expanded vertical slice plugin entry point.

use bevy::prelude::*;

use crate::plugin::configure_vertical_slice_app;

/// VS2-named plugin graph alias (§20).
pub struct ExpandedVerticalSlicePlugin;

impl Plugin for ExpandedVerticalSlicePlugin {
    fn build(&self, app: &mut App) {
        configure_vertical_slice_app(app, "RPG Adrift — Expanded Vertical Slice");
    }
}
