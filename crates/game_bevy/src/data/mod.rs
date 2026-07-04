// crates/game_bevy/src/data/mod.rs
mod hot_reload;
mod plugin;
mod user_prefs;
mod watcher;

pub use hot_reload::VisualConfigHotReloadPlugin;
pub use plugin::{
    ConfigRegistryResource, DataAssetPlugin, assets_root, debounce_duration, is_yaml_path,
};
pub use user_prefs::{UserSetupPrefs, load_user_prefs, sanitize_user_prefs, save_user_prefs};
