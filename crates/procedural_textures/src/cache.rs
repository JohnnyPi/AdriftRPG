// crates/procedural_textures/src/cache.rs
//! Per-texture bake cache keyed by recipe content hash.

use std::path::{Path, PathBuf};

use crate::maps::GeneratedPbrMaps;
use crate::material_recipe::TerrainMaterialRecipe;
use crate::texture_graph::GENERATOR_VERSION;

const TEXTURE_CACHE_DIR: &str = "target/terrain_material_cache/textures";

pub fn texture_cache_key(recipe: &TerrainMaterialRecipe) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&GENERATOR_VERSION.to_le_bytes());
    hasher.update(&recipe.resolution.to_le_bytes());
    hasher.update(recipe.id.as_bytes());
    hasher.update(&recipe.generator.fingerprint());
    hasher.update(&recipe.meters_per_repeat.to_le_bytes());
    hasher.update(&recipe.tint[0].to_le_bytes());
    hasher.update(&recipe.tint[1].to_le_bytes());
    hasher.update(&recipe.tint[2].to_le_bytes());
    *hasher.finalize().as_bytes()
}

pub fn texture_cache_path(key: [u8; 32]) -> PathBuf {
    cache_root().join(format!("{}.bin", hex::encode(key)))
}

pub fn try_load_texture_cache(key: [u8; 32]) -> Option<GeneratedPbrMaps> {
    let path = texture_cache_path(key);
    let bytes = std::fs::read(path).ok()?;
    bincode::deserialize(&bytes).ok()
}

pub fn write_texture_cache(key: [u8; 32], maps: &GeneratedPbrMaps) {
    let _ = std::fs::create_dir_all(cache_root());
    let path = texture_cache_path(key);
    if let Ok(bytes) = bincode::serialize(maps) {
        let _ = std::fs::write(path, bytes);
    }
}

pub fn invalidate_texture_cache(recipe: &TerrainMaterialRecipe) {
    let path = texture_cache_path(texture_cache_key(recipe));
    let _ = std::fs::remove_file(path);
}

pub fn bake_recipe_with_cache(
    recipe: &TerrainMaterialRecipe,
) -> Result<GeneratedPbrMaps, crate::error::TextureGenerationError> {
    let key = texture_cache_key(recipe);
    if let Some(cached) = try_load_texture_cache(key) {
        return Ok(cached);
    }
    let maps = recipe
        .generator
        .generate(recipe.resolution, recipe.resolution)?;
    write_texture_cache(key, &maps);
    Ok(maps)
}

pub fn cache_root() -> &'static Path {
    Path::new(TEXTURE_CACHE_DIR)
}
