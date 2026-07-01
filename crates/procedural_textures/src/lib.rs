//! CPU procedural PBR texture generation (Symbios-style, Bevy-independent).

mod arrays;
mod error;
mod generators;
mod maps;
mod material_recipe;
mod normal;
mod noise;
mod recipe;

pub use arrays::{arrays_fingerprint, build_cpu_arrays, CpuTextureArrays};
pub use error::TextureGenerationError;
pub use generators::{
    CobblestoneConfig, CobblestoneGenerator, GroundConfig, GroundGenerator, RockConfig,
    RockGenerator, SandConfig, SandGenerator,
};
pub use maps::{GeneratedPbrMaps, encode_height_u8, pack_ormh};
pub use material_recipe::{
    default_island_recipes, document_fingerprint, strip_utf8_bom, ProceduralMaterialsDocument,
    TerrainMaterialIdName, TerrainMaterialRecipe,
};
pub use recipe::{texture_recipe_from_yaml_value, ProceduralTextureGenerator, TextureRecipe};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rock_generates_valid_buffers() {
        let maps = TextureRecipe::Rock(RockConfig::default())
            .generate(64, 64)
            .expect("generate");
        assert_eq!(maps.albedo_rgba8.len(), 64 * 64 * 4);
        assert_eq!(maps.normal_rgba8.len(), 64 * 64 * 4);
        assert_eq!(maps.ormh_rgba8.len(), 64 * 64 * 4);
    }

    #[test]
    fn generated_texture_edges_are_seamless() {
        let maps = TextureRecipe::Ground(GroundConfig::default())
            .generate(128, 128)
            .expect("generate");
        let w = 128usize;
        let h = 128usize;
        let tolerance = 8u8;

        for y in 0..h {
            let left = maps.albedo_rgba8[(y * w) * 4];
            let right = maps.albedo_rgba8[(y * w + w - 1) * 4];
            assert!(
                left.abs_diff(right) <= tolerance,
                "horizontal seam at y={y}: {left} vs {right}"
            );
        }
        for x in 0..w {
            let top = maps.albedo_rgba8[x * 4];
            let bottom = maps.albedo_rgba8[((h - 1) * w + x) * 4];
            assert!(
                top.abs_diff(bottom) <= tolerance,
                "vertical seam at x={x}: {top} vs {bottom}"
            );
        }
    }

    #[test]
    fn build_cpu_arrays_from_default_island() {
        let recipes = default_island_recipes();
        let arrays = build_cpu_arrays(&recipes).expect("arrays");
        assert_eq!(arrays.layers, 8);
        assert_eq!(
            arrays.albedo.len(),
            512 * 512 * 4 * 8
        );
    }

    #[test]
    fn load_procedural_island_yaml() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/procedural/terrain/procedural_island.yaml");
        let text = std::fs::read_to_string(&path).expect("read yaml");
        let doc: ProceduralMaterialsDocument = serde_yaml::from_str(&text).expect("parse yaml");
        assert_eq!(doc.materials.len(), 8);
    }
}
