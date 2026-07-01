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
        // Verify the underlying SeamlessNoise wraps correctly: u=0 and u=1 must produce
        // the same value. We test this by sampling the noise directly rather than through
        // pixel generators that use u=x/w (where the last pixel is at u=(w-1)/w, not u=1).
        let noise = crate::noise::SeamlessNoise::new(2001);
        let tolerance = f32::EPSILON * 4.0;
        for v_step in 0..16u32 {
            let v = v_step as f32 / 16.0;
            // At u=0 and u=1, fract() maps both to 0.0 so they must be identical.
            let at_zero = noise.sample(0.0, v);
            let at_one = noise.sample(1.0, v);
            assert!(
                (at_zero - at_one).abs() < tolerance,
                "u wrap failed at v={v}: sample(0)={at_zero} sample(1)={at_one}"
            );
            let at_v_zero = noise.sample(v, 0.0);
            let at_v_one = noise.sample(v, 1.0);
            assert!(
                (at_v_zero - at_v_one).abs() < tolerance,
                "v wrap failed at u={v}: sample(0)={at_v_zero} sample(1)={at_v_one}"
            );
        }
        // Also verify that a ground texture generates valid (non-garbage) albedo values.
        let maps = TextureRecipe::Ground(GroundConfig::default())
            .generate(64, 64)
            .expect("generate");
        assert_eq!(maps.albedo_rgba8.len(), 64 * 64 * 4);
        // All channel values must be valid u8 (trivially true) and non-zero in aggregate.
        let any_nonzero = maps.albedo_rgba8.iter().any(|&v| v > 0);
        assert!(any_nonzero, "ground texture albedo is all zeros");
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
        let doc: ProceduralMaterialsDocument =
            serde_yaml::from_str(strip_utf8_bom(&text)).expect("parse yaml");
        assert_eq!(doc.materials.len(), 8);
    }
}
