// crates/procedural_textures/src/arrays.rs
use crate::cache::bake_recipe_with_cache;
use crate::error::TextureGenerationError;
use crate::material_recipe::{order_recipes_for_palette, TerrainMaterialRecipe};
use crate::maps::GeneratedPbrMaps;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CpuTextureArrays {
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub albedo: Vec<u8>,
    pub normal: Vec<u8>,
    pub ormh: Vec<u8>,
}

pub fn build_cpu_arrays(
    recipes: &[TerrainMaterialRecipe],
) -> Result<CpuTextureArrays, TextureGenerationError> {
    build_cpu_arrays_from_ordered(recipes)
}

/// Build texture arrays in palette layer order.
pub fn build_cpu_arrays_for_palette(
    layer_order: &[impl AsRef<str>],
    recipes: &[TerrainMaterialRecipe],
) -> Result<CpuTextureArrays, TextureGenerationError> {
    let ordered = order_recipes_for_palette(layer_order, recipes)?;
    build_cpu_arrays_from_ordered(&ordered)
}

/// Build texture arrays in canonical legacy core-layer order regardless of YAML file order.
pub fn build_cpu_arrays_in_core_order(
    recipes: &[TerrainMaterialRecipe],
) -> Result<CpuTextureArrays, TextureGenerationError> {
    use crate::material_recipe::order_recipes_for_core_layers;
    let ordered = order_recipes_for_core_layers(recipes)?;
    build_cpu_arrays_from_ordered(&ordered)
}

fn build_cpu_arrays_from_ordered(
    recipes: &[TerrainMaterialRecipe],
) -> Result<CpuTextureArrays, TextureGenerationError> {
    let first = recipes.first().ok_or_else(|| {
        TextureGenerationError::InvalidConfig("no terrain material recipes".to_owned())
    })?;

    let width = first.resolution;
    let height = first.resolution;
    let pixels_per_layer = width as usize * height as usize;
    let bytes_per_layer = pixels_per_layer * 4;

    let mut albedo = Vec::with_capacity(bytes_per_layer * recipes.len());
    let mut normal = Vec::with_capacity(bytes_per_layer * recipes.len());
    let mut ormh = Vec::with_capacity(bytes_per_layer * recipes.len());

    for recipe in recipes {
        if recipe.resolution != width {
            return Err(TextureGenerationError::InvalidConfig(format!(
                "{:?} uses resolution {}, expected {}",
                recipe.id, recipe.resolution, width,
            )));
        }

        let mut maps = bake_recipe_with_cache(recipe)?;
        apply_tint(&mut maps, recipe.tint);
        validate_map_lengths(&maps)?;
        albedo.extend_from_slice(&maps.albedo_rgba8);
        normal.extend_from_slice(&maps.normal_rgba8);
        ormh.extend_from_slice(&maps.ormh_rgba8);
    }

    Ok(CpuTextureArrays {
        width,
        height,
        layers: recipes.len() as u32,
        albedo,
        normal,
        ormh,
    })
}

fn apply_tint(maps: &mut GeneratedPbrMaps, tint: [f32; 3]) {
    if tint == [1.0, 1.0, 1.0] {
        return;
    }
    for chunk in maps.albedo_rgba8.chunks_exact_mut(4) {
        chunk[0] = ((chunk[0] as f32 / 255.0 * tint[0]).clamp(0.0, 1.0) * 255.0) as u8;
        chunk[1] = ((chunk[1] as f32 / 255.0 * tint[1]).clamp(0.0, 1.0) * 255.0) as u8;
        chunk[2] = ((chunk[2] as f32 / 255.0 * tint[2]).clamp(0.0, 1.0) * 255.0) as u8;
    }
}

fn validate_map_lengths(maps: &GeneratedPbrMaps) -> Result<(), TextureGenerationError> {
    let expected = maps.width as usize * maps.height as usize * 4;
    for buffer in [
        &maps.albedo_rgba8,
        &maps.normal_rgba8,
        &maps.ormh_rgba8,
    ] {
        if buffer.len() != expected {
            return Err(TextureGenerationError::InvalidBufferLength);
        }
    }
    Ok(())
}

pub fn arrays_fingerprint(arrays: &CpuTextureArrays, recipe_hash: [u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&recipe_hash);
    hasher.update(&arrays.width.to_le_bytes());
    hasher.update(&arrays.height.to_le_bytes());
    hasher.update(&arrays.layers.to_le_bytes());
    *hasher.finalize().as_bytes()
}
