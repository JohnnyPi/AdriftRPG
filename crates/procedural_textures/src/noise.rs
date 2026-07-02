// crates/procedural_textures/src/noise.rs
/// Toroidal seamless value noise for tileable textures.
#[derive(Clone, Copy, Debug)]
pub struct SeamlessNoise {
    seed: u64,
}

impl SeamlessNoise {
    pub fn new(seed: u32) -> Self {
        Self {
            seed: seed as u64,
        }
    }

    fn hash(x: i32, y: i32, seed: u64) -> f32 {
        let mut h = seed
            .wrapping_mul(0x517c_c1b7_2722_0a95)
            .wrapping_add((x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15))
            .wrapping_add((y as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9));
        h ^= h >> 30;
        h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h ^= h >> 27;
        h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
        h ^= h >> 31;
        (h as u32 as f32) / u32::MAX as f32
    }

    fn smooth(t: f32) -> f32 {
        t * t * (3.0 - 2.0 * t)
    }

    /// Sample with toroidal wrap on both axes (period = 1.0 in uv space).
    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let u = u.fract();
        let v = v.fract();
        let x = u * 256.0;
        let y = v * 256.0;
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let x1 = x0 + 1;
        let y1 = y0 + 1;
        let tx = Self::smooth(x - x0 as f32);
        let ty = Self::smooth(y - y0 as f32);

        let v00 = Self::hash(x0 & 255, y0 & 255, self.seed);
        let v10 = Self::hash(x1 & 255, y0 & 255, self.seed);
        let v01 = Self::hash(x0 & 255, y1 & 255, self.seed);
        let v11 = Self::hash(x1 & 255, y1 & 255, self.seed);

        let a = v00 + tx * (v10 - v00);
        let b = v01 + tx * (v11 - v01);
        a + ty * (b - a)
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
        if norm > f32::EPSILON {
            sum / norm
        } else {
            0.0
        }
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
        if norm > f32::EPSILON {
            sum / norm
        } else {
            0.0
        }
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
}
