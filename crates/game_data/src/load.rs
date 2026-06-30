use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use shared::{DataError, DataResult, StableId};
use walkdir::WalkDir;

use crate::definitions::*;
use crate::registry::ConfigRegistry;

#[derive(Clone, Debug)]
pub struct LoadedFile {
    pub path: PathBuf,
    pub definition: RawDefinition,
}

pub fn load_registry_from_directory(assets_root: impl AsRef<Path>) -> DataResult<ConfigRegistry> {
    let assets_root = assets_root.as_ref();
    let files = load_yaml_files(assets_root)?;
    ConfigRegistry::from_loaded_files(&files)
}

fn load_yaml_files(assets_root: &Path) -> DataResult<Vec<LoadedFile>> {
    if !assets_root.is_dir() {
        return Err(DataError::Io {
            path: assets_root.display().to_string(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "assets directory not found",
            ),
        });
    }

    let mut loaded = Vec::new();
    let mut seen_ids: HashMap<StableId, String> = HashMap::new();

    for entry in WalkDir::new(assets_root)
        .follow_links(true)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "yaml"))
    {
        let path = entry.path().to_path_buf();
        let text = fs::read_to_string(&path).map_err(|source| DataError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let text = strip_utf8_bom(&text);

        let definition = parse_yaml_file(&path, text)?;
        definition.validate_header()?;

        let id = definition.id().clone();
        if let Some(first_path) = seen_ids.get(&id) {
            return Err(DataError::DuplicateId {
                id,
                first_path: first_path.clone(),
                duplicate_path: path.display().to_string(),
            });
        }
        seen_ids.insert(id, path.display().to_string());
        loaded.push(LoadedFile { path, definition });
    }

    loaded.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(loaded)
}

fn parse_yaml_file(path: &Path, text: &str) -> DataResult<RawDefinition> {
    let value: serde_yaml::Value = serde_yaml::from_str(text).map_err(|error| DataError::Parse {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;

    let id = value
        .get("id")
        .and_then(|id| id.as_str())
        .ok_or_else(|| DataError::Parse {
            path: path.display().to_string(),
            message: "missing required field `id`".to_string(),
        })?;

    let relative = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");

    let definition = if relative.contains("/config/app") || id.starts_with("app.") {
        RawDefinition::App(deserialize(path, text)?)
    } else if id.starts_with("performance.") {
        RawDefinition::Performance(deserialize(path, text)?)
    } else if id.starts_with("player.") {
        RawDefinition::Player(deserialize(path, text)?)
    } else if id.starts_with("camera.") {
        RawDefinition::Camera(deserialize(path, text)?)
    } else if id.starts_with("lighting.") {
        RawDefinition::Lighting(deserialize(path, text)?)
    } else if id.starts_with("water.") {
        RawDefinition::Water(deserialize(path, text)?)
    } else if id.starts_with("world.") {
        RawDefinition::World(deserialize(path, text)?)
    } else if id.starts_with("terrain.") {
        RawDefinition::TerrainGeneration(deserialize(path, text)?)
    } else if id.starts_with("biomes.") {
        RawDefinition::Biomes(deserialize(path, text)?)
    } else if id.starts_with("materials.") {
        RawDefinition::TerrainMaterials(deserialize(path, text)?)
    } else if id.starts_with("vegetation.") {
        RawDefinition::Vegetation(deserialize(path, text)?)
    } else if id.starts_with("cave.") {
        RawDefinition::Cave(deserialize(path, text)?)
    } else if id.starts_with("debug.") {
        RawDefinition::Debug(deserialize(path, text)?)
    } else if id.starts_with("options.") {
        RawDefinition::Options(deserialize(path, text)?)
    } else if id.starts_with("physics.") {
        RawDefinition::Physics(deserialize(path, text)?)
    } else if id.starts_with("river.") {
        RawDefinition::River(deserialize(path, text)?)
    } else if id.starts_with("hydrology.") {
        RawDefinition::Hydrology(deserialize(path, text)?)
    } else if id.starts_with("waterbody.") {
        RawDefinition::WaterBodyMaterial(deserialize(path, text)?)
    } else if id.starts_with("atmosphere.") {
        RawDefinition::Atmosphere(deserialize(path, text)?)
    } else if id.starts_with("fog.") {
        RawDefinition::Fog(deserialize(path, text)?)
    } else if id.starts_with("sky.") {
        RawDefinition::Sky(deserialize(path, text)?)
    } else if id.starts_with("landmarks.") {
        RawDefinition::Landmarks(deserialize(path, text)?)
    } else if id.starts_with("routes.") {
        RawDefinition::Routes(deserialize(path, text)?)
    } else if id.starts_with("structure.") {
        RawDefinition::Structure(deserialize(path, text)?)
    } else {
        return Err(DataError::Parse {
            path: path.display().to_string(),
            message: format!("unable to classify definition with id `{id}`"),
        });
    };

    Ok(definition)
}

fn deserialize<T: serde::de::DeserializeOwned>(path: &Path, text: &str) -> DataResult<T> {
    serde_yaml::from_str(text).map_err(|error| DataError::Parse {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn strip_utf8_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_assets() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .canonicalize()
            .expect("workspace assets directory")
    }

    #[test]
    fn loads_workspace_assets() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry loads");
        assert_eq!(registry.app.id.as_str(), "app.vertical_slice");
    }
}
