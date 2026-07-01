use std::path::PathBuf;

use bevy::prelude::*;
use procedural_textures::{
    ProceduralMaterialsDocument, TerrainMaterialRecipe, arrays_fingerprint, build_cpu_arrays,
    default_island_recipes, document_fingerprint,
};

use crate::arrays::{TerrainArrayHandles, upload_texture_arrays};
use crate::material::{
    TerrainLayerScales, TerrainMaterialSettings, TerrainPbrMaterial, layer_scales_from_recipes,
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

pub fn load_recipes_from_yaml(path: &PathBuf) -> Option<Vec<TerrainMaterialRecipe>> {
    let text = std::fs::read_to_string(path).ok()?;
    let text = procedural_textures::strip_utf8_bom(&text);
    let doc: ProceduralMaterialsDocument = serde_yaml::from_str(text).ok()?;
    Some(doc.materials)
}

pub fn recipes_for_world(yaml_path: Option<&PathBuf>) -> Vec<TerrainMaterialRecipe> {
    if let Some(path) = yaml_path {
        if let Some(recipes) = load_recipes_from_yaml(path) {
            return recipes;
        }
    }
    default_island_recipes()
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

pub fn bake_cpu_arrays(recipes: &[TerrainMaterialRecipe]) -> procedural_textures::CpuTextureArrays {
    build_cpu_arrays(recipes).expect("bake cpu arrays")
}

pub fn build_material_from_arrays(
    arrays: &procedural_textures::CpuTextureArrays,
    recipes: &[TerrainMaterialRecipe],
    images: &mut Assets<Image>,
    materials: &mut Assets<TerrainPbrMaterial>,
) -> Handle<TerrainPbrMaterial> {
    let handles = upload_texture_arrays(arrays, images);
    let layer_scales = layer_scales_from_recipes(recipes);
    let normal_strength = if recipes.is_empty() {
        0.0
    } else {
        recipes
            .iter()
            .map(|recipe| recipe.normal_strength)
            .sum::<f32>()
            / recipes.len() as f32
    };
    materials.add(TerrainPbrMaterial {
        albedo_array: handles.albedo.clone(),
        normal_array: handles.normal.clone(),
        ormh_array: handles.ormh.clone(),
        settings: TerrainMaterialSettings {
            triplanar_sharpness: 4.0,
            global_texture_scale: 1.0,
            normal_strength,
            height_blend_strength: 0.0,
            layer_count: arrays.layers,
            debug_mode: 0,
            _padding: Vec2::ZERO,
        },
        layer_scales,
    })
}

pub fn fingerprint_matches_baked(
    arrays: &procedural_textures::CpuTextureArrays,
    recipe_hash: [u8; 32],
) -> [u8; 32] {
    arrays_fingerprint(arrays, recipe_hash)
}
