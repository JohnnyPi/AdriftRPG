// crates/game_bevy/src/data/plugin.rs
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use game_data::{load_registry_from_directory, ConfigRegistry};
use shared::DataError;
use tracing::{error, info, warn};

use crate::data::sanitize_user_prefs;
use crate::data::watcher::YamlWatcher;
use crate::state::AppState;

#[derive(Resource, Clone, Debug)]
pub struct ConfigLoadStatus {
    pub assets_root: PathBuf,
    pub last_success: Option<Instant>,
    pub last_error: Option<String>,
    pub reload_count: u32,
}

#[derive(Resource, Clone, Debug)]
pub struct ConfigRegistryResource(pub ConfigRegistry);

pub struct DataAssetPlugin;

impl Plugin for DataAssetPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ConfigLoadStatus {
            assets_root: assets_root(),
            last_success: None,
            last_error: None,
            reload_count: 0,
        })
        .add_systems(Startup, initial_load)
        .add_systems(Update, poll_yaml_watcher.run_if(in_state(AppState::Running)));
    }
}

fn initial_load(
    mut commands: Commands,
    mut status: ResMut<ConfigLoadStatus>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let mut prefs = crate::data::load_user_prefs();
    match load_registry_from_directory(&status.assets_root) {
        Ok(registry) => {
            sanitize_user_prefs(&mut prefs, &registry);
            commands.insert_resource(prefs);
            info!(
                registry_hash = %registry.hash,
                world = %registry.app.world,
                "loaded configuration registry"
            );
            status.last_success = Some(Instant::now());
            status.last_error = None;
            commands.insert_resource(ConfigRegistryResource(registry));
            commands.insert_resource(YamlWatcher::new(&status.assets_root));
            next_state.set(AppState::MainMenu);
        }
        Err(error) => {
            let message = format_load_error(&error);
            error!(%message, "failed to load configuration");
            status.last_error = Some(message.clone());
            panic!("configuration load failed:\n{message}");
        }
    }
}

fn poll_yaml_watcher(
    mut commands: Commands,
    mut status: ResMut<ConfigLoadStatus>,
    watcher: Res<YamlWatcher>,
    current: Res<ConfigRegistryResource>,
) {
    if !watcher.drain_pending() {
        return;
    }

    match load_registry_from_directory(&status.assets_root) {
        Ok(registry) => {
            let previous_hash = current.0.hash.clone();
            status.last_success = Some(Instant::now());
            status.last_error = None;
            status.reload_count += 1;
            info!(
                registry_hash = %registry.hash,
                previous_hash = %previous_hash,
                reload_count = status.reload_count,
                "reloaded configuration registry"
            );
            commands.insert_resource(ConfigRegistryResource(registry));
        }
        Err(error) => {
            let message = format_load_error(&error);
            warn!(%message, "configuration reload rejected; retaining last valid registry");
            status.last_error = Some(message);
        }
    }
}

pub fn format_load_error(error: &DataError) -> String {
    match error {
        DataError::Parse { path, message } => {
            format!("{path}\n  parse error: {message}")
        }
        DataError::Io { path, source } => {
            format!("{path}\n  io error: {source}")
        }
        DataError::DuplicateId {
            id,
            first_path,
            duplicate_path,
        } => format!(
            "duplicate id `{id}`\n  first: {first_path}\n  duplicate: {duplicate_path}"
        ),
        DataError::UnsupportedSchemaVersion {
            id,
            found,
            expected,
        } => format!(
            "definition `{id}` has schema_version {found}; expected {expected}"
        ),
        DataError::UnknownReference { reference, context } => {
            format!("unknown reference `{reference}` in {context}")
        }
        DataError::InvalidValue { context, message } => {
            format!("invalid value in {context}: {message}")
        }
        DataError::ValidationFailed { count, details } => {
            format!("{count} validation error(s):\n{details}")
        }
    }
}

pub fn assets_root() -> PathBuf {
    if let Some(env) = std::env::var_os("RPG_ADRIFT_ASSETS") {
        return PathBuf::from(env);
    }

    let mut search_roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        search_roots.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            search_roots.push(parent.to_path_buf());
        }
    }

    for mut dir in search_roots {
        for _ in 0..6 {
            let candidate = dir.join("assets");
            if candidate.join("config").is_dir() {
                return std::fs::canonicalize(&candidate).unwrap_or(candidate);
            }
            if !dir.pop() {
                break;
            }
        }
    }

    PathBuf::from("assets")
}

pub fn debounce_duration() -> Duration {
    Duration::from_millis(250)
}

pub fn is_yaml_path(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "yaml")
}
