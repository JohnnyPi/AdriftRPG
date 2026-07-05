// crates/terrain_material_bevy/src/bake.rs
use std::path::PathBuf;

use bevy::prelude::*;
use procedural_textures::{
    ProceduralMaterialsDocument, TerrainMaterialRecipe, default_island_recipes,
    document_fingerprint,
};

use crate::arrays::{TerrainArrayHandles, upload_texture_arrays};
use crate::material::{
    TerrainLayerScales, TerrainMaterialSettings, TerrainPbrMaterial, default_chunk_slots,
    layer_scales_from_recipes,
};

const CACHE_DIR: &str = "target/terrain_material_cache";

#[derive(Resource, Clone)]
pub struct TerrainProceduralMaterialState {
    pub material: Handle<TerrainPbrMaterial>,
    pub arrays: TerrainArrayHandles,
    pub layer_scales: TerrainLayerScales,
    pub ready: bool,
    pub recipe_fingerprint: [u8; 32],
}

impl Default for TerrainProceduralMaterialState {
    fn default() -> Self {
        Self {
            material: Handle::default(),
            arrays: TerrainArrayHandles {
                albedo: Handle::default(),
                normal: Handle::default(),
                ormh: Handle::default(),
            },
            layer_scales: TerrainLayerScales::default(),
            ready: false,
            recipe_fingerprint: [0; 32],
        }
    }
}

pub fn load_recipes_from_yaml(path: &PathBuf) -> Option<(Vec<TerrainMaterialRecipe>, Vec<String>)> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            warn!(
                "failed to read procedural materials YAML at {}: {err}",
                path.display()
            );
            return None;
        }
    };
    let text = procedural_textures::strip_utf8_bom(&text);
    let doc: ProceduralMaterialsDocument = match serde_yaml::from_str(text) {
        Ok(doc) => doc,
        Err(err) => {
            warn!(
                "failed to parse procedural materials YAML at {}: {err}",
                path.display()
            );
            return None;
        }
    };
    match procedural_textures::order_recipes_for_document(&doc) {
        Ok(recipes) => Some((recipes, procedural_textures::document_layer_order(&doc))),
        Err(err) => {
            warn!(
                "failed to order procedural materials from {}: {err}",
                path.display()
            );
            None
        }
    }
}

pub fn recipes_for_world(yaml_path: Option<&PathBuf>) -> (Vec<TerrainMaterialRecipe>, Vec<String>) {
    if let Some(path) = yaml_path {
        if let Some((recipes, order)) = load_recipes_from_yaml(path) {
            return (recipes, order);
        }
        warn!(
            "falling back to built-in island recipes after YAML load failure: {}",
            path.display()
        );
    }
    let recipes = default_island_recipes();
    let order = recipes.iter().map(|r| r.id.clone()).collect();
    (recipes, order)
}

pub fn recipe_fingerprint_for(recipes: &[TerrainMaterialRecipe]) -> [u8; 32] {
    let doc = ProceduralMaterialsDocument {
        schema_version: 1,
        id: "runtime".to_string(),
        description: String::new(),
        materials: recipes.to_vec(),
    };
    document_fingerprint(&doc)
}

pub fn cache_path_for(fingerprint: [u8; 32]) -> PathBuf {
    PathBuf::from(CACHE_DIR).join(format!("{}.bin", hex::encode(fingerprint)))
}

pub fn try_load_cache(fingerprint: [u8; 32]) -> Option<procedural_textures::CpuTextureArrays> {
    let path = cache_path_for(fingerprint);
    let bytes = std::fs::read(path).ok()?;
    bincode::deserialize(&bytes).ok()
}

pub fn write_cache(fingerprint: [u8; 32], arrays: &procedural_textures::CpuTextureArrays) {
    let path = cache_path_for(fingerprint);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = bincode::serialize(arrays) {
        let _ = std::fs::write(path, bytes);
    }
}

pub fn bake_cpu_arrays(
    layer_order: &[String],
    recipes: &[TerrainMaterialRecipe],
) -> procedural_textures::CpuTextureArrays {
    procedural_textures::build_cpu_arrays_for_palette(layer_order, recipes)
        .expect("bake cpu arrays")
}

pub fn ordered_recipes_for_palette(
    layer_order: &[String],
    recipes: &[TerrainMaterialRecipe],
) -> Vec<TerrainMaterialRecipe> {
    procedural_textures::order_recipes_for_palette(layer_order, recipes)
        .expect("order recipes for palette")
}

pub fn build_material_from_arrays(
    arrays: &procedural_textures::CpuTextureArrays,
    recipes: &[TerrainMaterialRecipe],
    images: &mut Assets<Image>,
    materials: &mut Assets<TerrainPbrMaterial>,
) -> Handle<TerrainPbrMaterial> {
    let handles = upload_texture_arrays(arrays, images);
    let layer_scales = layer_scales_from_recipes(recipes);
    let normal_strength = recipes
        .iter()
        .map(|recipe| recipe.normal_strength)
        .fold(0.0f32, f32::max);
    materials.add(TerrainPbrMaterial {
        albedo_array: handles.albedo.clone(),
        normal_array: handles.normal.clone(),
        ormh_array: handles.ormh.clone(),
        settings: TerrainMaterialSettings {
            triplanar_sharpness: 4.0,
            global_texture_scale: 1.0,
            normal_strength,
            height_blend_strength: 2.0,
            layer_count: arrays.layers,
            debug_mode: 0,
            macro_variation_scale: 42.0,
            macro_variation_strength: 0.10,
            global_wetness: 0.12,
            global_moss: 0.08,
        },
        layer_scales,
        chunk_slots: default_chunk_slots(),
    })
}
