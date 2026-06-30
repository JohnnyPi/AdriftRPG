//! Composable 2D terrain fields and masks (VS2 §5).

use crate::noise::ValueNoise;

#[derive(Clone, Debug)]
pub struct TerrainMask {
    pub resolution: (u32, u32),
    pub origin: [f32; 2],
    pub spacing: f32,
    pub samples: Vec<f32>,
}

impl TerrainMask {
    pub fn sample_bilinear(&self, x: f32, z: f32) -> f32 {
        let lx = (x - self.origin[0]) / self.spacing;
        let lz = (z - self.origin[1]) / self.spacing;
        if lx < 0.0 || lz < 0.0 {
            return 0.0;
        }
        let w = self.resolution.0 as f32;
        let h = self.resolution.1 as f32;
        if lx >= w - 1.0 || lz >= h - 1.0 {
            return 0.0;
        }
        let x0 = lx.floor() as u32;
        let z0 = lz.floor() as u32;
        let fx = lx - x0 as f32;
        let fz = lz - z0 as f32;
        let i = |cx: u32, cz: u32| {
            self.samples[(cz * self.resolution.0 + cx) as usize]
        };
        let a = i(x0, z0);
        let b = i(x0 + 1, z0);
        let c = i(x0, z0 + 1);
        let d = i(x0 + 1, z0 + 1);
        let ab = a + (b - a) * fx;
        let cd = c + (d - c) * fx;
        ab + (cd - ab) * fz
    }
}

#[derive(Clone, Debug, Default)]
pub struct FieldStackParams {
    pub ridge_amplitude: f32,
    pub valley_depth: f32,
    pub coast_blend: f32,
}

pub fn coastal_height(x: f32, z: f32, sea_level: f32, noise: &ValueNoise) -> f32 {
    let coast_dist = (x * 0.5 + z * 0.3).abs() / 128.0;
    let base = sea_level + 4.0 + (1.0 - coast_dist.min(1.0)) * 10.0;
    base + noise.fbm_2d(x * 0.02, z * 0.02, 3) * 3.0
}

pub fn ridge_field(x: f32, z: f32, origin: [f32; 2], scale: [f32; 2], amplitude: f32) -> f32 {
    let dx = (x - origin[0]) / scale[0];
    let dz = (z - origin[1]) / scale[1];
    let r2 = dx * dx + dz * dz;
    if r2 > 1.0 {
        return 0.0;
    }
    let t = 1.0 - r2;
    amplitude * t * t
}

pub fn valley_field(x: f32, z: f32, origin: [f32; 2], scale: [f32; 2], depth: f32) -> f32 {
    let dx = (x - origin[0]) / scale[0];
    let dz = (z - origin[1]) / scale[1];
    let r2 = dx * dx + dz * dz;
    if r2 > 1.0 {
        return 0.0;
    }
  -depth * (1.0 - r2).sqrt()
}

pub fn stack_surface_height(
    x: f32,
    z: f32,
    sea_level: f32,
    seed: u64,
    params: &FieldStackParams,
) -> f32 {
    let noise = ValueNoise::new(seed);
    let mut h = coastal_height(x, z, sea_level, &noise);
    h += ridge_field(x, z, [180.0, 196.0], [48.0, 56.0], 14.0 * params.ridge_amplitude);
    h += valley_field(x, z, [128.0, 140.0], [80.0, 120.0], 6.0 * params.valley_depth);
    h
}

pub fn build_coast_mask(width: u32, height: u32, origin: [f32; 2], spacing: f32) -> TerrainMask {
    let mut samples = Vec::with_capacity((width * height) as usize);
    for z in 0..height {
        for x in 0..width {
            let wx = origin[0] + x as f32 * spacing;
            let wz = origin[1] + z as f32 * spacing;
            let cx = origin[0] + (width.saturating_sub(1)) as f32 * spacing * 0.5;
            let cz = origin[1] + (height.saturating_sub(1)) as f32 * spacing * 0.5;
            let max_r = ((width.max(height).saturating_sub(1)) as f32 * spacing * 0.5).max(spacing);
            let dist = ((wx - cx).powi(2) + (wz - cz).powi(2)).sqrt();
            let coast = 1.0 - (dist / max_r).min(1.0);
            samples.push(coast.max(0.0));
        }
    }
    TerrainMask {
        resolution: (width, height),
        origin,
        spacing,
        samples,
    }
}
