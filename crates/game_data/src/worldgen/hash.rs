//! Canonical recipe content hashing.

use super::compile::CompiledWorldRecipe;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RecipeHash(pub [u8; 32]);

impl RecipeHash {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn hex(&self) -> String {
        hex::encode(self.0)
    }
}

pub fn recipe_content_hash(recipe: &CompiledWorldRecipe) -> RecipeHash {
    let mut copy = recipe.clone();
    copy.recipe_hash = RecipeHash::from_bytes([0u8; 32]);
    let json = serde_json::to_vec(&copy).expect("compiled recipe serializes");
    RecipeHash::from_bytes(*blake3::hash(&json).as_bytes())
}
