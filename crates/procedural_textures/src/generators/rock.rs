use crate::error::TextureGenerationError;
use crate::maps::{encode_height_u8, linear_to_srgb_u8, pack_ormh, GeneratedPbrMaps};
use crate::noise::SeamlessNoise;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RockConfig {
    pub seed: u32,
    pub scale: f32,
    pub octaves: u32,
    pub attenuation: f32,
    pub color_light: [f32; 3],
    pub color_dark: [f32; 3],
    pub normal_strength: f32,
    #[serde(default = "default_roughness")]
    pub roughness: f32,
    #[serde(default)]
    pub metallic: f32,
}

fn default_roughness() -> f32 {
    0.85
}

impl Default for RockConfig {
    fn default() -> Self {
        Self {
            seed: 1001,
            scale: 3.0,
            octaves: 7,
            attenuation: 2.0,
            color_light: [0.25, 0.22, 0.18],
            color_dark: [0.07, 0.06, 0.055],
            normal_strength: 3.7,
            roughness: 0.85,
            metallic: 0.0,
        }
    }
}

pub struct RockGenerator {
    config: RockConfig,
}

impl RockGenerator {
    pub fn new(config: RockConfig) -> Self {
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
                let ridged = noise.ridged(
                    u * self.config.scale,
                    v * self.config.scale,
                    self.config.octaves,
                    self.config.attenuation,
                    0.5,
                );
                let detail = noise.fbm(
                    u * self.config.scale * 4.0,
                    v * self.config.scale * 4.0,
                    4,
                    2.0,
                    0.5,
                );
                let height_val = ridged * 0.75 + detail * 0.25;
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
                ao[idx] = (128.0 + detail * 127.0) as u8;
            }
        }

        build_maps_from_height(
            width,
            height,
            &height_field,
            albedo,
            ao,
            self.config.roughness,
            self.config.metallic,
            self.config.normal_strength,
        )
    }
}

pub(crate) fn build_maps_from_height(
    width: u32,
    height: u32,
    height_field: &[f32],
    albedo: Vec<u8>,
    ao: Vec<u8>,
    roughness: f32,
    metallic: f32,
    normal_strength: f32,
) -> Result<GeneratedPbrMaps, TextureGenerationError> {
    let w = width as usize;
    let h = height as usize;
    let count = w * h;
    let (min_h, max_h) = height_field
        .iter()
        .fold((f32::MAX, f32::MIN), |(mn, mx), v| (mn.min(*v), mx.max(*v)));

    let normal = crate::normal::normals_from_height_field(
        width,
        height,
        height_field,
        normal_strength,
    );
    let height_u8 = encode_height_u8(height_field, min_h, max_h);
    let roughness_u8 = vec![(roughness.clamp(0.0, 1.0) * 255.0).round() as u8; count];
    let metallic_u8 = vec![(metallic.clamp(0.0, 1.0) * 255.0).round() as u8; count];
    let ormh = pack_ormh(&ao, &roughness_u8, &metallic_u8, &height_u8);

    Ok(GeneratedPbrMaps {
        width,
        height,
        albedo_rgba8: albedo,
        normal_rgba8: normal,
        ormh_rgba8: ormh,
        emissive_rgba8: None,
        mip_level_count: 1,
    })
}
