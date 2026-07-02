// crates/procedural_textures/src/generators/cobblestone.rs
use crate::error::TextureGenerationError;
use crate::generators::rock::{RockConfig, RockGenerator};
use crate::maps::GeneratedPbrMaps;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CobblestoneConfig {
    pub seed: u32,
    pub scale: f32,
    pub octaves: u32,
    pub color_light: [f32; 3],
    pub color_dark: [f32; 3],
    pub normal_strength: f32,
    #[serde(default = "default_roughness")]
    pub roughness: f32,
}

fn default_roughness() -> f32 {
    0.88
}

impl Default for CobblestoneConfig {
    fn default() -> Self {
        Self {
            seed: 4001,
            scale: 5.0,
            octaves: 5,
            color_light: [0.42, 0.40, 0.36],
            color_dark: [0.22, 0.20, 0.18],
            normal_strength: 4.0,
            roughness: 0.88,
        }
    }
}

pub struct CobblestoneGenerator {
    config: CobblestoneConfig,
}

impl CobblestoneGenerator {
    pub fn new(config: CobblestoneConfig) -> Self {
        Self { config }
    }

    pub fn generate(
        &self,
        width: u32,
        height: u32,
    ) -> Result<GeneratedPbrMaps, TextureGenerationError> {
        RockGenerator::new(RockConfig {
            seed: self.config.seed,
            scale: self.config.scale,
            octaves: self.config.octaves,
            attenuation: 2.5,
            color_light: self.config.color_light,
            color_dark: self.config.color_dark,
            normal_strength: self.config.normal_strength,
            roughness: self.config.roughness,
            metallic: 0.0,
        })
        .generate(width, height)
    }
}
