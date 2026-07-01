mod arrays;
mod bake;
mod material;
mod plugin;

pub use arrays::{create_array_image, create_placeholder_array_images, upload_texture_arrays, TerrainArrayHandles};
pub use bake::{
    bake_cpu_arrays, build_material_from_arrays, cache_path_for, load_recipes_from_yaml,
    recipe_fingerprint_for, recipes_for_world, write_cache, TerrainProceduralMaterialState,
};
pub use material::{
    layer_scales_from_recipes, TerrainLayerScales, TerrainMaterialSettings, TerrainPbrMaterial,
};
pub use plugin::{FallbackTerrainMaterialSet, PendingTextureBake, ProceduralMaterialYamlPath, ProceduralTerrainMaterialPlugin};
