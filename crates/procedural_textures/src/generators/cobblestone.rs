// crates/procedural_textures/src/generators/cobblestone.rs
use crate::error::TextureGenerationError;
use crate::generators::rock::build_maps_from_height_spatial;
use crate::maps::{GeneratedPbrMaps, linear_to_srgb_u8};
use crate::noise::SeamlessNoise;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CobblestoneConfig {
    pub seed: u32,
    pub scale: f32,
    pub octaves: u32,
    pub color_light: [f32; 3],
    pub color_dark: [f32; 3],
    pub normal_strength: f32,
    #[serde(default = "default_roughness")]
    pub roughness: f32,
}

fn default_roughness() -> f32 {
    0.88
}

impl Default for CobblestoneConfig {
    fn default() -> Self {
        Self {
            seed: 4001,
            scale: 5.0,
            octaves: 5,
            color_light: [0.42, 0.40, 0.36],
            color_dark: [0.22, 0.20, 0.18],
            normal_strength: 4.0,
            roughness: 0.88,
        }
    }
}

pub struct CobblestoneGenerator {
    config: CobblestoneConfig,
}

impl CobblestoneGenerator {
    pub fn new(config: CobblestoneConfig) -> Self {
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
        let mut roughness_field = vec![0.0f32; count];

        for y in 0..h {
            for x in 0..w {
                let u = x as f32 / w as f32;
                let v = y as f32 / h as f32;
                let (cell_u, cell_v, cell_id) = worley_cell(u * self.config.scale, v * self.config.scale, self.config.seed);
                let edge = (cell_u - 0.5).abs().max((cell_v - 0.5).abs()) * 2.0;
                let mortar = smoothstep(0.72, 0.92, edge);
                let pebble = noise.fbm(
                    u * self.config.scale * 4.0 + cell_id as f32 * 0.17,
                    v * self.config.scale * 4.0 + cell_id as f32 * 0.23,
                    self.config.octaves.min(4),
                    2.0,
                    0.5,
                );
                let height_val = pebble * (1.0 - mortar * 0.85) + mortar * 0.08;
                let idx = y * w + x;
                height_field[idx] = height_val;

                let mortar_color = [0.12, 0.11, 0.10];
                let r = self.config.color_dark[0]
                    + (self.config.color_light[0] - self.config.color_dark[0]) * pebble;
                let g = self.config.color_dark[1]
                    + (self.config.color_light[1] - self.config.color_dark[1]) * pebble;
                let b = self.config.color_dark[2]
                    + (self.config.color_light[2] - self.config.color_dark[2]) * pebble;
                let lr = r * (1.0 - mortar) + mortar_color[0] * mortar;
                let lg = g * (1.0 - mortar) + mortar_color[1] * mortar;
                let lb = b * (1.0 - mortar) + mortar_color[2] * mortar;
                let ai = idx * 4;
                albedo[ai] = linear_to_srgb_u8(lr);
                albedo[ai + 1] = linear_to_srgb_u8(lg);
                albedo[ai + 2] = linear_to_srgb_u8(lb);
                albedo[ai + 3] = 255;
                ao[idx] = (64.0 + pebble * 160.0 - mortar * 80.0).clamp(0.0, 255.0) as u8;
                roughness_field[idx] =
                    self.config.roughness * (1.0 - mortar * 0.15) + mortar * 0.98;
            }
        }

        build_maps_from_height_spatial(
            width,
            height,
            &height_field,
            albedo,
            &ao,
            &roughness_field,
            0.0,
            self.config.normal_strength,
        )
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn worley_cell(u: f32, v: f32, seed: u32) -> (f32, f32, u32) {
    let x0 = u.floor() as i32;
    let y0 = v.floor() as i32;
    let fx = u - x0 as f32;
    let fy = v - y0 as f32;

    let mut best_dist = f32::MAX;
    let mut best_u = 0.5f32;
    let mut best_v = 0.5f32;
    let mut best_id = 0u32;

    for dy in -1..=1 {
        for dx in -1..=1 {
            let cx = x0 + dx;
            let cy = y0 + dy;
            let id = cell_hash(cx, cy, seed);
            let jx = cell_jitter(id, 0);
            let jy = cell_jitter(id, 1);
            let px = dx as f32 + jx;
            let py = dy as f32 + jy;
            let dist = (fx - px).hypot(fy - py);
            if dist < best_dist {
                best_dist = dist;
                best_u = fx - px + 0.5;
                best_v = fy - py + 0.5;
                best_id = id;
            }
        }
    }
    (best_u, best_v, best_id)
}

fn cell_hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = seed as u64 ^ (x as u64).wrapping_mul(0x9e37_79b9);
    h ^= (y as u64).wrapping_mul(0xbf58_476d);
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afed_558c_c2fa);
    (h & 0xffff) as u32
}

fn cell_jitter(id: u32, channel: u32) -> f32 {
    let h = id.wrapping_mul(1664525).wrapping_add(1013904223 + channel);
    (h as f32 / u32::MAX as f32) * 0.8 + 0.1
}
