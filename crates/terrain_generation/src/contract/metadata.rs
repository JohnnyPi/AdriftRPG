//! World metadata shared between compiler and runtime.

use serde::{Deserialize, Serialize};

use super::version::{GENERATOR_VERSION, GeneratorVersion};

/// Stable recipe content hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RecipeHash(pub [u8; 32]);

impl RecipeHash {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn hex(&self) -> String {
        hex::encode(self.0)
    }
}

/// Finite world extent in meters. Origin at center.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldExtent {
    pub width_m: f64,
    pub depth_m: f64,
    pub vertical_min_m: f64,
    pub vertical_max_m: f64,
    pub sea_level_m: f32,
}

impl WorldExtent {
    pub fn half_width(&self) -> f64 {
        self.width_m * 0.5
    }

    pub fn half_depth(&self) -> f64 {
        self.depth_m * 0.5
    }
}

/// Runtime-visible world metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldMetadata {
    pub world_id: String,
    pub recipe_id: String,
    pub recipe_hash: RecipeHash,
    pub generator_version: GeneratorVersion,
    pub seed: u64,
    pub extent: WorldExtent,
}

impl WorldMetadata {
    pub fn new(
        world_id: impl Into<String>,
        recipe_id: impl Into<String>,
        recipe_hash: RecipeHash,
        seed: u64,
        extent: WorldExtent,
    ) -> Self {
        Self {
            world_id: world_id.into(),
            recipe_id: recipe_id.into(),
            recipe_hash,
            generator_version: GENERATOR_VERSION,
            seed,
            extent,
        }
    }
}
