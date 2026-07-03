// crates/game_bevy/src/data/user_prefs.rs
//! Persistent user setup preferences (outside assets/ to avoid hot-reload loops).

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use bevy::prelude::*;
use game_data::CompiledIslandGeneration;
use serde::{Deserialize, Serialize};
use shared::StableId;
use tracing::{info, warn};

/// User-editable setup state persisted between sessions.
///
/// Older saved files may contain a `use_expanded_profile` key from the
/// removed op-based expanded_slice worlds; serde ignores unknown fields, so
/// those files still load.
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct UserSetupPrefs {
    pub world_id: String,
    pub seed: u64,
    #[serde(default)]
    pub island_overrides: BTreeMap<String, f32>,
    #[serde(default)]
    pub preview_color_mode: String,
    #[serde(default = "default_preview_resolution")]
    pub preview_resolution: u32,
}

fn default_preview_resolution() -> u32 {
    256
}

impl Default for UserSetupPrefs {
    fn default() -> Self {
        Self {
            world_id: "world.island_testbed".to_string(),
            seed: 48_129,
            island_overrides: BTreeMap::new(),
            preview_color_mode: "elevation".to_string(),
            preview_resolution: 256,
        }
    }
}

impl UserSetupPrefs {
    pub fn world_stable_id(&self) -> StableId {
        StableId::new(&self.world_id)
    }

    pub fn apply_overrides(&self, base: &CompiledIslandGeneration) -> CompiledIslandGeneration {
        let mut merged = base.clone();
        for (key, value) in &self.island_overrides {
            merged.set_param(key, *value);
        }
        merged.seed = self.seed;
        merged
    }
}

pub fn user_data_root() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        return cwd.join("user_data");
    }
    PathBuf::from("user_data")
}

pub fn user_prefs_path() -> PathBuf {
    user_data_root().join("user_prefs.yaml")
}

pub fn load_user_prefs() -> UserSetupPrefs {
    let path = user_prefs_path();
    if !path.is_file() {
        return UserSetupPrefs::default();
    }
    match fs::read_to_string(&path) {
        Ok(text) => serde_yaml::from_str(&text).unwrap_or_else(|error| {
            warn!(%error, path = %path.display(), "failed to parse user prefs; using defaults");
            UserSetupPrefs::default()
        }),
        Err(error) => {
            warn!(%error, path = %path.display(), "failed to read user prefs");
            UserSetupPrefs::default()
        }
    }
}

pub fn save_user_prefs(prefs: &UserSetupPrefs) -> Result<(), String> {
    let dir = user_data_root();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = user_prefs_path();
    let text = serde_yaml::to_string(prefs).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| e.to_string())?;
    info!(path = %path.display(), "saved user setup preferences");
    Ok(())
}