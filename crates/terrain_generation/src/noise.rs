// crates/terrain_generation/src/noise.rs
/// Deterministic 3D value noise for terrain detail (no external deps).
pub struct ValueNoise {
    seed: u64,
}

impl ValueNoise {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    fn hash(&self, x: i32, y: i32, z: i32) -> f32 {
        let mut h = self
            .seed
            ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
            ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
        h ^= h >> 30;
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 27;
        h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
        h ^= h >> 31;
        ((h >> 40) as f32) / (1u64 << 24) as f32
    }

    fn smoothstep(t: f32) -> f32 {
        t * t * (3.0 - 2.0 * t)
    }

    pub fn sample(&self, x: f32, y: f32, z: f32) -> f32 {
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let z0 = z.floor() as i32;
        let tx = Self::smoothstep(x - x0 as f32);
        let ty = Self::smoothstep(y - y0 as f32);
        let tz = Self::smoothstep(z - z0 as f32);

        let c000 = self.hash(x0, y0, z0);
        let c100 = self.hash(x0 + 1, y0, z0);
        let c010 = self.hash(x0, y0 + 1, z0);
        let c110 = self.hash(x0 + 1, y0 + 1, z0);
        let c001 = self.hash(x0, y0, z0 + 1);
        let c101 = self.hash(x0 + 1, y0, z0 + 1);
        let c011 = self.hash(x0, y0 + 1, z0 + 1);
        let c111 = self.hash(x0 + 1, y0 + 1, z0 + 1);

        let x00 = c000 + (c100 - c000) * tx;
        let x10 = c010 + (c110 - c010) * tx;
        let x01 = c001 + (c101 - c001) * tx;
        let x11 = c011 + (c111 - c011) * tx;
        let y0v = x00 + (x10 - x00) * ty;
        let y1v = x01 + (x11 - x01) * ty;
        y0v + (y1v - y0v) * tz
    }

    /// Fractal sum in **[0, 1]** (normalized per octave amplitude).
    pub fn fbm(&self, x: f32, y: f32, z: f32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
        let mut sum = 0.0;
        let mut amp = 1.0;
        let mut freq = 1.0;
        let mut norm = 0.0;
        for _ in 0..octaves {
            sum += self.sample(x * freq, y * freq, z * freq) * amp;
            norm += amp;
            amp *= gain;
            freq *= lacunarity;
        }
        if norm > 0.0 {
            sum / norm
        } else {
            0.0
        }
    }

    /// 2D fractal sum in **[-1, 1]** (fbm at y = 0, remapped).
    pub fn fbm_2d(&self, x: f32, z: f32, octaves: u32) -> f32 {
        self.fbm(x, 0.0, z, octaves, 2.0, 0.5) * 2.0 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_line(noise: &ValueNoise, axis: u8, fixed: (i32, i32), len: i32) -> Vec<f32> {
        (0..len)
            .map(|i| match axis {
                0 => noise.hash(i, fixed.0, fixed.1),
                1 => noise.hash(fixed.0, i, fixed.1),
                _ => noise.hash(fixed.0, fixed.1, i),
            })
            .collect()
    }

    fn serial_correlation(values: &[f32]) -> f32 {
        if values.len() < 2 {
            return 0.0;
        }
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        let mut num = 0.0f32;
        let mut var = 0.0f32;
        for window in values.windows(2) {
            let a = window[0] - mean;
            let b = window[1] - mean;
            num += a * b;
            var += a * a;
        }
        let denom = var + f32::EPSILON;
        (num / denom).clamp(-1.0, 1.0)
    }

    #[test]
    fn hash_along_z_is_not_constant_stride() {
        let noise = ValueNoise::new(12345);
        let line = hash_line(&noise, 2, (5, 10), 32);
        let strides: Vec<f32> = line.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        let first = strides[0];
        assert!(
            !strides.iter().all(|s| (*s - first).abs() < 1e-6),
            "consecutive z hashes must not share a constant stride"
        );
    }

    #[test]
    fn hash_serial_correlation_is_bounded_per_axis() {
        let noise = ValueNoise::new(98765);
        for axis in 0..3 {
            let line = hash_line(&noise, axis, (11, 23), 4000);
            let r = serial_correlation(&line);
            assert!(
                r.abs() < 0.1,
                "axis {axis} serial correlation {r} exceeds bound"
            );
        }
    }
}
