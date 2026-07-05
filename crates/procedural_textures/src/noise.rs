// crates/procedural_textures/src/noise.rs
use std::f32::consts::{FRAC_1_SQRT_2, TAU};

/// Toroidal seamless gradient (Perlin-style) noise for tileable textures.
///
/// Uses per-lattice gradient vectors with a quintic fade curve, remapped to
/// `[0, 1]`. Gradient noise avoids both the axis-aligned value grid and the
/// C2 interpolation creases that value-noise produced in derived normal maps.
#[derive(Clone, Copy, Debug)]
pub struct SeamlessNoise {
    seed: u64,
}

impl SeamlessNoise {
    pub fn new(seed: u32) -> Self {
        Self { seed: seed as u64 }
    }

    fn hash(x: i32, y: i32, seed: u64) -> u32 {
        let mut h = seed
            .wrapping_mul(0x517c_c1b7_2722_0a95)
            .wrapping_add((x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15))
            .wrapping_add((y as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9));
        h ^= h >> 30;
        h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h ^= h >> 27;
        h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
        h ^= h >> 31;
        h as u32
    }

    fn gradient(x: i32, y: i32, seed: u64) -> (f32, f32) {
        let h = Self::hash(x, y, seed);
        let angle = (h as f32 / u32::MAX as f32) * TAU;
        (angle.cos(), angle.sin())
    }

    fn fade(t: f32) -> f32 {
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + t * (b - a)
    }

    /// Sample with toroidal wrap on both axes (period = 1.0 in uv space).
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let x = u.rem_euclid(1.0) * 256.0;
        let y = v.rem_euclid(1.0) * 256.0;
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let x1 = x0 + 1;
        let y1 = y0 + 1;
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;

        let (gx0, gy0) = (x0 & 255, y0 & 255);
        let (gx1, gy1) = (x1 & 255, y1 & 255);

        let g00 = Self::gradient(gx0, gy0, self.seed);
        let g10 = Self::gradient(gx1, gy0, self.seed);
        let g01 = Self::gradient(gx0, gy1, self.seed);
        let g11 = Self::gradient(gx1, gy1, self.seed);

        let d00 = g00.0 * fx + g00.1 * fy;
        let d10 = g10.0 * (fx - 1.0) + g10.1 * fy;
        let d01 = g01.0 * fx + g01.1 * (fy - 1.0);
        let d11 = g11.0 * (fx - 1.0) + g11.1 * (fy - 1.0);

        let tx = Self::fade(fx);
        let ty = Self::fade(fy);
        let a = Self::lerp(d00, d10, tx);
        let b = Self::lerp(d01, d11, tx);
        let n = Self::lerp(a, b, ty);

        (n * FRAC_1_SQRT_2 + 0.5).clamp(0.0, 1.0)
    }

    pub fn fbm(&self, u: f32, v: f32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut amp = 0.5;
        let mut freq = 1.0;
        let mut sum = 0.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            sum += amp * self.sample(u * freq, v * freq);
            norm += amp;
            amp *= gain;
            freq *= lacunarity;
        }
        if norm > f32::EPSILON { sum / norm } else { 0.0 }
    }

    pub fn ridged(&self, u: f32, v: f32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut amp = 0.5;
        let mut freq = 1.0;
        let mut sum = 0.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            let sample = self.sample(u * freq, v * freq);
            let n = 1.0 - (sample * 2.0 - 1.0).abs();
            sum += amp * n * n;
            norm += amp;
            amp *= gain;
            freq *= lacunarity;
        }
        if norm > f32::EPSILON { sum / norm } else { 0.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ridged_does_not_clamp_half_domain_to_zero() {
        let noise = SeamlessNoise::new(1001);
        let side = 256;
        let mut exact_zeros = 0usize;
        for y in 0..side {
            for x in 0..side {
                let u = x as f32 / side as f32;
                let v = y as f32 / side as f32;
                let value = noise.ridged(u * 3.0, v * 3.0, 7, 2.0, 0.5);
                if value == 0.0 {
                    exact_zeros += 1;
                }
            }
        }
        let zero_fraction = exact_zeros as f32 / (side * side) as f32;
        assert!(
            zero_fraction < 0.05,
            "ridged noise clamped {zero_fraction:.1}% of samples to exactly zero"
        );
    }

    #[test]
    fn samples_stay_in_unit_range() {
        let noise = SeamlessNoise::new(1234);
        let side = 128;
        for y in 0..side {
            for x in 0..side {
                let u = x as f32 / side as f32;
                let v = y as f32 / side as f32;
                let s = noise.sample(u * 5.0, v * 5.0);
                assert!((0.0..=1.0).contains(&s), "sample {s} out of range");
            }
        }
    }

    #[test]
    fn sample_has_unit_period() {
        let noise = SeamlessNoise::new(2024);
        for &(u, v) in &[(0.13, 0.42), (0.77, 0.05), (0.5, 0.9)] {
            assert!(
                (noise.sample(u, v) - noise.sample(u + 1.0, v)).abs() < 1e-4,
                "u seam at ({u},{v})"
            );
            assert!(
                (noise.sample(u, v) - noise.sample(u, v + 1.0)).abs() < 1e-4,
                "v seam at ({u},{v})"
            );
        }
    }

    #[test]
    fn fbm_tiles_with_integer_lacunarity() {
        let noise = SeamlessNoise::new(77);
        let side = 64;
        for y in 0..side {
            for x in 0..side {
                let u = x as f32 / side as f32;
                let v = y as f32 / side as f32;
                let a = noise.fbm(u * 3.0, v * 3.0, 4, 2.0, 0.5);
                let b = noise.fbm(u * 3.0 + 1.0, v * 3.0, 4, 2.0, 0.5);
                assert!((a - b).abs() < 1e-4, "fbm seam at ({u},{v}): {a} vs {b}");
            }
        }
    }
}
