use crate::error::TextureGenerationError;
use crate::generators::rock::build_maps_from_height;
use crate::maps::linear_to_srgb_u8;
use crate::noise::SeamlessNoise;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct GroundConfig {
    pub seed: u32,
    pub macro_scale: f32,
    pub macro_octaves: u32,
    pub micro_scale: f32,
    pub micro_octaves: u32,
    pub micro_weight: f32,
    pub color_dry: [f32; 3],
    pub color_moist: [f32; 3],
    pub normal_strength: f32,
    #[serde(default = "default_roughness")]
    pub roughness: f32,
}

fn default_roughness() -> f32 {
    0.92
}

impl Default for GroundConfig {
    fn default() -> Self {
        Self {
            seed: 2001,
            macro_scale: 2.2,
            macro_octaves: 5,
            micro_scale: 10.0,
            micro_octaves: 4,
            micro_weight: 0.38,
            color_dry: [0.48, 0.17, 0.07],
            color_moist: [0.19, 0.055, 0.025],
            normal_strength: 2.0,
            roughness: 0.92,
        }
    }
}

pub struct GroundGenerator {
    config: GroundConfig,
}

impl GroundGenerator {
    pub fn new(config: GroundConfig) -> Self {
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
                let macro_n = noise.fbm(
                    u * self.config.macro_scale,
                    v * self.config.macro_scale,
                    self.config.macro_octaves,
                    2.0,
                    0.5,
                );
                let micro_n = noise.fbm(
                    u * self.config.micro_scale,
                    v * self.config.micro_scale,
                    self.config.micro_octaves,
                    2.2,
                    0.45,
                );
                let height_val =
                    macro_n * (1.0 - self.config.micro_weight) + micro_n * self.config.micro_weight;
                let moisture = noise.fbm(u * 3.0 + 0.31, v * 3.0 + 0.17, 3, 2.0, 0.5);
                let idx = y * w + x;
                height_field[idx] = height_val;

                let r = self.config.color_dry[0]
                    + (self.config.color_moist[0] - self.config.color_dry[0]) * moisture;
                let g = self.config.color_dry[1]
                    + (self.config.color_moist[1] - self.config.color_dry[1]) * moisture;
                let b = self.config.color_dry[2]
                    + (self.config.color_moist[2] - self.config.color_dry[2]) * moisture;
                let ai = idx * 4;
                albedo[ai] = linear_to_srgb_u8(r);
                albedo[ai + 1] = linear_to_srgb_u8(g);
                albedo[ai + 2] = linear_to_srgb_u8(b);
                albedo[ai + 3] = 255;
                ao[idx] = (100.0 + macro_n * 155.0) as u8;
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

use crate::maps::GeneratedPbrMaps;
