// crates/procedural_textures/src/generators/sand.rs
use crate::error::TextureGenerationError;
use crate::generators::rock::build_maps_from_height;
use crate::maps::{GeneratedPbrMaps, linear_to_srgb_u8};
use crate::noise::SeamlessNoise;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SandConfig {
    pub seed: u32,
    pub ripple_scale: f32,
    pub grain_scale: f32,
    pub color_light: [f32; 3],
    pub color_dark: [f32; 3],
    pub normal_strength: f32,
    #[serde(default = "default_roughness")]
    pub roughness: f32,
}

fn default_roughness() -> f32 {
    0.95
}

impl Default for SandConfig {
    fn default() -> Self {
        Self {
            seed: 3001,
            ripple_scale: 8.0,
            grain_scale: 32.0,
            color_light: [0.92, 0.86, 0.72],
            color_dark: [0.72, 0.62, 0.48],
            normal_strength: 1.5,
            roughness: 0.95,
        }
    }
}

pub struct SandGenerator {
    config: SandConfig,
}

impl SandGenerator {
    pub fn new(config: SandConfig) -> Self {
        Self { config }
    }

    pub fn generate(
        &self,
        width: u32,
        height: u32,
    ) -> Result<GeneratedPbrMaps, TextureGenerationError> {
        if width == 0 || height == 0 {
            return Err(TextureGenerationError::ZeroDimension);
        }
        let noise = SeamlessNoise::new(self.config.seed);
        let w = width as usize;
        let h = height as usize;
        let count = w * h;
        let mut height_field = vec![0.0f32; count];
        let mut albedo = vec![0u8; count * 4];
        let mut ao = vec![0u8; count];

        for y in 0..h {
            for x in 0..w {
                let u = x as f32 / w as f32;
                let v = y as f32 / h as f32;
                let ripple = (v * self.config.ripple_scale * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                let grain = noise.fbm(
                    u * self.config.grain_scale,
                    v * self.config.grain_scale,
                    4,
                    2.0,
                    0.5,
                );
                let height_val = ripple * 0.6 + grain * 0.4;
                let idx = y * w + x;
                height_field[idx] = height_val;

                let t = height_val.clamp(0.0, 1.0);
                let r = self.config.color_dark[0]
                    + (self.config.color_light[0] - self.config.color_dark[0]) * t;
                let g = self.config.color_dark[1]
                    + (self.config.color_light[1] - self.config.color_dark[1]) * t;
                let b = self.config.color_dark[2]
                    + (self.config.color_light[2] - self.config.color_dark[2]) * t;
                let ai = idx * 4;
                albedo[ai] = linear_to_srgb_u8(r);
                albedo[ai + 1] = linear_to_srgb_u8(g);
                albedo[ai + 2] = linear_to_srgb_u8(b);
                albedo[ai + 3] = 255;
                ao[idx] = 255;
            }
        }

        build_maps_from_height(
            width,
            height,
            &height_field,
            albedo,
            ao,
            self.config.roughness,
            0.0,
            self.config.normal_strength,
        )
    }
}
