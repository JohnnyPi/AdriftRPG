/// Deterministic 3D value noise for terrain detail (no external deps).
pub struct ValueNoise {
    seed: u64,
}

impl ValueNoise {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    fn hash(&self, x: i32, y: i32, z: i32) -> f32 {
        let mut h = self.seed;
        h = h.wrapping_mul(374761393).wrapping_add(x as u64);
        h = h.wrapping_mul(668265263).wrapping_add(y as u64);
        h = h.wrapping_mul(2147483647).wrapping_add(z as u64);
        h ^= h >> 13;
        h = h.wrapping_mul(1274126177);
        (h & 0xFFFF) as f32 / 65535.0
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
}
