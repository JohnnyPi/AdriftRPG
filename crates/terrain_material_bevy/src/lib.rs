mod arrays;
mod bake;
mod material;
mod plugin;

pub use arrays::{
    TerrainArrayHandles, create_array_image, create_placeholder_array_images, upload_texture_arrays,
};
pub use bake::{
    TerrainProceduralMaterialState, bake_cpu_arrays, build_material_from_arrays, cache_path_for,
    load_recipes_from_yaml, recipe_fingerprint_for, recipes_for_world, write_cache,
};
pub use material::{
    TerrainLayerScales, TerrainMaterialSettings, TerrainPbrMaterial, layer_scales_from_recipes,
};
pub use plugin::{
    FallbackTerrainMaterialSet, PendingTextureBake, ProceduralMaterialRecipeOverride,
    ProceduralMaterialYamlPath, ProceduralTerrainMaterialPlugin,
};
