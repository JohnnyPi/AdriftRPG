// crates/game_bevy/src/ui/mod.rs
mod hud;
mod main_menu;
mod options_panel;
mod overlay_camera;
mod setup_options;
mod tweaks;

pub use hud::HudPlugin;
pub use main_menu::MainMenuPlugin;
pub use overlay_camera::{
    configure_ui_overlay_for_game, sync_camera_viewports_to_window, UiOverlayPlugin,
};
pub use options_panel::{OptionsPanelPlugin, OptionsPanelState};
pub use setup_options::SetupOptionsPlugin;
pub use tweaks::{
    ambient_brightness_for_elevation, sun_angles_from_time_of_day, sun_illuminance_for_elevation,
    AtmosphereTweaks, CameraTweaks, EcologyTweaks, LightingTweaks, MovementTweaks, PhysicsTweaks,
    RiverTweaks, TerrainTweaks, WaterPhysicsTweaks, WaterTweaks, WorldTweaks,
};
