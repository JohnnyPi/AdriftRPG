// crates/procedural_textures/src/recipe.rs
use blake3::Hasher;

use crate::error::TextureGenerationError;
use crate::generators::{
    CobblestoneConfig, CobblestoneGenerator, GroundConfig, GroundGenerator, RockConfig,
    RockGenerator, SandConfig, SandGenerator,
};
use crate::maps::GeneratedPbrMaps;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum TextureRecipe {
    Rock(RockConfig),
    Ground(GroundConfig),
    Sand(SandConfig),
    Cobblestone(CobblestoneConfig),
    Graph(crate::texture_graph::TextureGraphRecipe),
}

pub fn texture_recipe_from_yaml_value(value: &serde_yaml::Value) -> Result<TextureRecipe, TextureGenerationError> {
    let map = value.as_mapping().ok_or_else(|| {
        TextureGenerationError::InvalidConfig("generator must be a mapping".to_owned())
    })?;
    if map.len() != 1 {
        return Err(TextureGenerationError::InvalidConfig(
            "generator must have exactly one variant key".to_owned(),
        ));
    }
    let (key, config_value) = map.iter().next().expect("len checked");
    let variant = key.as_str().ok_or_else(|| {
        TextureGenerationError::InvalidConfig("generator key must be a string".to_owned())
    })?;
    match variant {
        "Rock" => Ok(TextureRecipe::Rock(serde_yaml::from_value(config_value.clone()).map_err(|e| {
            TextureGenerationError::InvalidConfig(format!("Rock config: {e}"))
        })?)),
        "Ground" => Ok(TextureRecipe::Ground(serde_yaml::from_value(config_value.clone()).map_err(
            |e| TextureGenerationError::InvalidConfig(format!("Ground config: {e}")),
        )?)),
        "Sand" => Ok(TextureRecipe::Sand(serde_yaml::from_value(config_value.clone()).map_err(
            |e| TextureGenerationError::InvalidConfig(format!("Sand config: {e}")),
        )?)),
        "Cobblestone" => Ok(TextureRecipe::Cobblestone(
            serde_yaml::from_value(config_value.clone()).map_err(|e| {
                TextureGenerationError::InvalidConfig(format!("Cobblestone config: {e}"))
            })?,
        )),
        "Graph" => Ok(TextureRecipe::Graph(
            crate::texture_graph::TextureGraphRecipe::from_yaml_value(config_value, 0)?,
        )),
        other => Err(TextureGenerationError::InvalidConfig(format!(
            "unknown generator variant '{other}'"
        ))),
    }
}

/// Parse a texture recipe from a catalog `generator:` block or standalone graph YAML.
pub fn texture_recipe_from_definition(
    generator: Option<&serde_yaml::Value>,
    graph: Option<&serde_yaml::Value>,
    seed: u32,
) -> Result<Option<TextureRecipe>, TextureGenerationError> {
    if let Some(value) = generator {
        return Ok(Some(texture_recipe_from_yaml_value(value)?));
    }
    if let Some(value) = graph {
        return Ok(Some(TextureRecipe::Graph(
            crate::texture_graph::TextureGraphRecipe::from_yaml_value(value, seed)?,
        )));
    }
    Ok(None)
}

impl TextureRecipe {
    pub fn generate(
        &self,
        width: u32,
        height: u32,
    ) -> Result<GeneratedPbrMaps, TextureGenerationError> {
        match self {
            Self::Rock(config) => RockGenerator::new(config.clone()).generate(width, height),
            Self::Ground(config) => GroundGenerator::new(config.clone()).generate(width, height),
            Self::Sand(config) => SandGenerator::new(config.clone()).generate(width, height),
            Self::Cobblestone(config) => {
                CobblestoneGenerator::new(config.clone()).generate(width, height)
            }
            Self::Graph(config) => config.generate(width, height),
        }
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        let json = serde_json::to_string(self).unwrap_or_else(|err| {
            format!("TextureRecipe::fingerprint serialize error: {err}")
        });
        *Hasher::new().update(json.as_bytes()).finalize().as_bytes()
    }
}

pub trait ProceduralTextureGenerator: Send + Sync + 'static {
    fn generate(
        &self,
        width: u32,
        height: u32,
    ) -> Result<GeneratedPbrMaps, TextureGenerationError>;

    fn fingerprint(&self) -> [u8; 32];
}
