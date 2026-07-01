//! Typed 2D scalar grids for island-scale fields.

use crate::resolution::grid_dims;

#[derive(Clone, Copy, Debug)]
pub struct FieldTier {
    pub spacing_m: f32,
    pub origin: [f32; 2],
    pub width: u32,
    pub height: u32,
}

impl<T: Copy> Field2D<T> {
    pub fn tier(&self) -> FieldTier {
        FieldTier {
            spacing_m: self.spacing,
            origin: self.origin,
            width: self.width,
            height: self.height,
        }
    }

    pub fn from_extent(extent_m: f32, origin: [f32; 2], spacing_m: f32) -> Self
    where
        T: Default,
    {
        let (width, height) = grid_dims(extent_m, spacing_m);
        Self::new(width, height, origin, spacing_m)
    }
}

#[derive(Clone, Debug)]
pub struct Field2D<T: Copy> {
    pub width: u32,
    pub height: u32,
    pub origin: [f32; 2],
    pub spacing: f32,
    pub samples: Vec<T>,
}

impl<T: Copy + Default> Field2D<T> {
    pub fn new(width: u32, height: u32, origin: [f32; 2], spacing: f32) -> Self {
        Self {
            width,
            height,
            origin,
            spacing,
            samples: vec![T::default(); (width * height) as usize],
        }
    }

    pub fn index(&self, x: u32, z: u32) -> usize {
        (z * self.width + x) as usize
    }

    pub fn get(&self, x: u32, z: u32) -> T {
        self.samples[self.index(x, z)]
    }

    pub fn set(&mut self, x: u32, z: u32, value: T) {
        let i = self.index(x, z);
        self.samples[i] = value;
    }

    pub fn world_to_grid(&self, wx: f32, wz: f32) -> (f32, f32) {
        (
            (wx - self.origin[0]) / self.spacing,
            (wz - self.origin[1]) / self.spacing,
        )
    }
}

impl Field2D<f32> {
    pub fn sample_bilinear(&self, wx: f32, wz: f32) -> f32 {
        let (lx, lz) = self.world_to_grid(wx, wz);
        if lx < 0.0 || lz < 0.0 {
            return 0.0;
        }
        let w = self.width as f32;
        let h = self.height as f32;
        if lx >= w - 1.0 || lz >= h - 1.0 {
            return 0.0;
        }
        let x0 = lx.floor() as u32;
        let z0 = lz.floor() as u32;
        let fx = lx - x0 as f32;
        let fz = lz - z0 as f32;
        let i = |cx: u32, cz: u32| self.get(cx, cz);
        let a = i(x0, z0);
        let b = i(x0 + 1, z0);
        let c = i(x0, z0 + 1);
        let d = i(x0 + 1, z0 + 1);
        let ab = a + (b - a) * fx;
        let cd = c + (d - c) * fx;
        ab + (cd - ab) * fz
    }

    pub fn for_each_world<F: FnMut(f32, f32, &mut f32)>(&mut self, mut f: F) {
        for z in 0..self.height {
            for x in 0..self.width {
                let wx = self.origin[0] + x as f32 * self.spacing;
                let wz = self.origin[1] + z as f32 * self.spacing;
                let idx = self.index(x, z);
                let sample = &mut self.samples[idx];
                f(wx, wz, sample);
            }
        }
    }

    /// Upsample preserving peak heights within each coarse cell (for elevation, not masks).
    pub fn upsample_preserving_peaks(&self, target_spacing: f32) -> Self {
        if (self.spacing - target_spacing).abs() < f32::EPSILON {
            return self.clone();
        }
        let ratio = (self.spacing / target_spacing).round().max(1.0) as u32;
        let extent_x = (self.width.saturating_sub(1)) as f32 * self.spacing;
        let extent_z = (self.height.saturating_sub(1)) as f32 * self.spacing;
        let width = (extent_x / target_spacing).floor() as u32 + 1;
        let height = (extent_z / target_spacing).floor() as u32 + 1;
        let mut out = Field2D::new(width, height, self.origin, target_spacing);
        for z in 0..height {
            for x in 0..width {
                let wx = self.origin[0] + x as f32 * target_spacing;
                let wz = self.origin[1] + z as f32 * target_spacing;
                let (lx, lz) = self.world_to_grid(wx, wz);
                let cx = lx.floor() as i32;
                let cz = lz.floor() as i32;
                let mut peak = f32::MIN;
                for dz in 0..=ratio {
                    for dx in 0..=ratio {
                        let sx = cx + dx as i32 - (ratio as i32 / 2);
                        let sz = cz + dz as i32 - (ratio as i32 / 2);
                        if sx >= 0
                            && sz >= 0
                            && sx < self.width as i32
                            && sz < self.height as i32
                        {
                            peak = peak.max(self.get(sx as u32, sz as u32));
                        }
                    }
                }
                if peak > f32::MIN / 2.0 {
                    out.set(x, z, peak);
                } else {
                    out.set(x, z, self.sample_bilinear(wx, wz));
                }
            }
        }
        out
    }

    /// Resample to a new spacing aligned on the same origin (bilinear upsample / box downsample).
    pub fn resample_to_spacing(&self, target_spacing: f32) -> Self {
        if (self.spacing - target_spacing).abs() < f32::EPSILON {
            return self.clone();
        }
        let extent_x = (self.width.saturating_sub(1)) as f32 * self.spacing;
        let extent_z = (self.height.saturating_sub(1)) as f32 * self.spacing;
        let width = (extent_x / target_spacing).floor() as u32 + 1;
        let height = (extent_z / target_spacing).floor() as u32 + 1;
        let mut out = Field2D::new(width, height, self.origin, target_spacing);
        for z in 0..height {
            for x in 0..width {
                let wx = self.origin[0] + x as f32 * target_spacing;
                let wz = self.origin[1] + z as f32 * target_spacing;
                out.set(x, z, self.sample_bilinear(wx, wz));
            }
        }
        out
    }
}

/// Upsample `detail` to `base` spacing and add in world space.
pub fn add_residual(base: &Field2D<f32>, detail: &Field2D<f32>) -> Field2D<f32> {
    let aligned = if (base.spacing - detail.spacing).abs() < f32::EPSILON
        && base.origin == detail.origin
        && base.width == detail.width
        && base.height == detail.height
    {
        detail.clone()
    } else {
        detail.resample_to_spacing(base.spacing)
    };
    let mut out = base.clone();
    for (a, b) in out.samples.iter_mut().zip(aligned.samples.iter()) {
        *a += *b;
    }
    out
}

/// Compute `local_absolute - upsampled(regional)` as a local-tier residual field.
pub fn residual_from_absolute(regional: &Field2D<f32>, local_absolute: &Field2D<f32>) -> Field2D<f32> {
    let mut residual = local_absolute.clone();
    for z in 0..residual.height {
        for x in 0..residual.width {
            let wx = residual.origin[0] + x as f32 * residual.spacing;
            let wz = residual.origin[1] + z as f32 * residual.spacing;
            let base = regional.sample_bilinear(wx, wz);
            residual.set(x, z, residual.get(x, z) - base);
        }
    }
    residual
}

impl Field2D<u8> {
    pub fn sample_nearest(&self, wx: f32, wz: f32) -> u8 {
        let (lx, lz) = self.world_to_grid(wx, wz);
        let x = lx.round().clamp(0.0, (self.width - 1) as f32) as u32;
        let z = lz.round().clamp(0.0, (self.height - 1) as f32) as u32;
        self.get(x, z)
    }
}

pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < f32::EPSILON {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn smooth_max(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (a - b) / k.max(0.001)).clamp(0.0, 1.0);
    b + (a - b) * h + k * h * (1.0 - h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_to_same_spacing_is_identity() {
        let mut field = Field2D::new(4, 4, [0.0, 0.0], 8.0);
        field.set(1, 1, 3.5);
        let copy = field.resample_to_spacing(8.0);
        assert_eq!(copy.get(1, 1), 3.5);
    }

    #[test]
    fn resample_deterministic_across_calls() {
        let mut field = Field2D::new(5, 5, [-10.0, -10.0], 16.0);
        field.for_each_world(|wx, wz, v| *v = wx + wz);
        let a = field.resample_to_spacing(4.0);
        let b = field.resample_to_spacing(4.0);
        assert_eq!(a.samples, b.samples);
    }

    #[test]
    fn residual_from_absolute_zeros_when_matching() {
        let mut regional = Field2D::from_extent(32.0, [-16.0, -16.0], 16.0);
        regional.for_each_world(|_, _, v| *v = 10.0);
        let local = regional.resample_to_spacing(4.0);
        let residual = residual_from_absolute(&regional, &local);
        for sample in &residual.samples {
            assert!(sample.abs() < 0.01);
        }
    }
}
