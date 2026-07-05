//! Load worldgen YAML assets from directory tree.

use std::collections::BTreeMap;
use std::path::Path;

use super::definitions::*;

#[derive(Debug, thiserror::Error)]
pub enum WorldgenLoadError {
    #[error("io error at {path}: {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("parse error in {path}: {message}")]
    Parse { path: String, message: String },
}

pub fn load_worldgen_bundle(root: &Path) -> Result<WorldgenSourceBundle, WorldgenLoadError> {
    let mut bundle = WorldgenSourceBundle::default();
    load_dir(&root.join("worlds"), &mut bundle.worlds)?;
    load_dir(&root.join("boundaries"), &mut bundle.boundaries)?;
    load_dir(&root.join("islands"), &mut bundle.islands)?;
    load_dir(&root.join("geology"), &mut bundle.geology)?;
    load_dir(&root.join("refinement"), &mut bundle.refinement)?;
    load_dir(&root.join("climate"), &mut bundle.climate)?;
    load_dir(&root.join("hydrology"), &mut bundle.hydrology)?;
    load_dir(&root.join("erosion"), &mut bundle.erosion)?;
    load_dir(&root.join("coasts"), &mut bundle.coasts)?;
    load_dir(&root.join("biomes"), &mut bundle.biomes)?;
    load_dir(&root.join("strata"), &mut bundle.strata)?;
    load_dir(&root.join("caves"), &mut bundle.caves)?;
    load_dir(&root.join("validation"), &mut bundle.validation)?;
    Ok(bundle)
}

fn load_dir<T>(dir: &Path, map: &mut BTreeMap<String, T>) -> Result<(), WorldgenLoadError>
where
    T: serde::de::DeserializeOwned + HasWorldgenId,
{
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir).map_err(|source| WorldgenLoadError::Io {
        path: dir.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| WorldgenLoadError::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|source| WorldgenLoadError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let text = strip_utf8_bom(&text);
        let value: T = serde_yaml::from_str(text).map_err(|e| WorldgenLoadError::Parse {
            path: path.display().to_string(),
            message: e.to_string(),
        })?;
        map.insert(value.worldgen_id().to_string(), value);
    }
    Ok(())
}

fn strip_utf8_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

pub trait HasWorldgenId {
    fn worldgen_id(&self) -> &str;
}

impl HasWorldgenId for WorldRecipeSource {
    fn worldgen_id(&self) -> &str {
        &self.id
    }
}
impl HasWorldgenId for BoundaryRecipeSource {
    fn worldgen_id(&self) -> &str {
        &self.id
    }
}
impl HasWorldgenId for IslandRecipeSource {
    fn worldgen_id(&self) -> &str {
        &self.id
    }
}
impl HasWorldgenId for GeologyRecipeSource {
    fn worldgen_id(&self) -> &str {
        &self.id
    }
}
impl HasWorldgenId for RefinementRecipeSource {
    fn worldgen_id(&self) -> &str {
        &self.id
    }
}
impl HasWorldgenId for ClimateRecipeSource {
    fn worldgen_id(&self) -> &str {
        &self.id
    }
}
impl HasWorldgenId for HydrologyRecipeSource {
    fn worldgen_id(&self) -> &str {
        &self.id
    }
}
impl HasWorldgenId for ErosionRecipeSource {
    fn worldgen_id(&self) -> &str {
        &self.id
    }
}
impl HasWorldgenId for CoastRecipeSource {
    fn worldgen_id(&self) -> &str {
        &self.id
    }
}
impl HasWorldgenId for BiomeRecipeSource {
    fn worldgen_id(&self) -> &str {
        &self.id
    }
}
impl HasWorldgenId for StrataRecipeSource {
    fn worldgen_id(&self) -> &str {
        &self.id
    }
}
impl HasWorldgenId for CavesRecipeSource {
    fn worldgen_id(&self) -> &str {
        &self.id
    }
}
impl HasWorldgenId for ValidationRecipeSource {
    fn worldgen_id(&self) -> &str {
        &self.id
    }
}
