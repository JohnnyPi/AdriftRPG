//! Compiled world manifest for reproducibility and caching.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::metadata::{RecipeHash, WorldExtent};
use super::version::GeneratorVersion;
use crate::compiler::report::PassReport;
use crate::fields::descriptor::FieldDescriptor;
use crate::fields::key::FieldKey;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldManifest {
    pub world_id: String,
    pub recipe_id: String,
    pub recipe_hash: RecipeHash,
    pub generator_version: GeneratorVersion,
    pub seed: u64,
    pub extent: WorldExtent,
    pub sea_level_m: f32,
    pub field_descriptors: BTreeMap<FieldKey, FieldDescriptor>,
    pub pass_reports: Vec<PassReport>,
}

impl WorldManifest {
    pub fn manifest_hash(&self) -> RecipeHash {
        let json = serde_json::to_vec(self).expect("manifest serializes");
        RecipeHash::from_bytes(*blake3::hash(&json).as_bytes())
    }
}
