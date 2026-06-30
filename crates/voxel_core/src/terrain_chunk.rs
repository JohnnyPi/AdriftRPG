use crate::{
    ChunkCoord, LocalSample, MaterialId, TerrainSample, CHUNK_CELLS, CHUNK_SAMPLES, SAMPLE_COUNT,
};

/// Density samples for one chunk: `17 × 17 × 17` corner values covering `16³` cells.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainChunk {
    pub coord: ChunkCoord,
    pub samples: Box<[TerrainSample; SAMPLE_COUNT]>,
}

impl TerrainChunk {
    pub fn new(coord: ChunkCoord) -> Self {
        Self {
            coord,
            samples: Box::new([TerrainSample::default(); SAMPLE_COUNT]),
        }
    }

    pub fn with_samples(coord: ChunkCoord, samples: [TerrainSample; SAMPLE_COUNT]) -> Self {
        Self {
            coord,
            samples: Box::new(samples),
        }
    }

    pub fn get(&self, local: LocalSample) -> TerrainSample {
        debug_assert!(local.is_valid());
        self.samples[local.linear_index()]
    }

    pub fn set(&mut self, local: LocalSample, sample: TerrainSample) {
        debug_assert!(local.is_valid());
        self.samples[local.linear_index()] = sample;
    }

    /// World-space origin of this chunk's sample (0,0,0) corner in meters.
    pub fn sample_origin(&self) -> (i32, i32, i32) {
        (
            self.coord.x * CHUNK_CELLS as i32,
            self.coord.y * CHUNK_CELLS as i32,
            self.coord.z * CHUNK_CELLS as i32,
        )
    }

    /// Sample on a chunk face shared with a neighbor (`axis`: 0=x, 1=y, 2=z; `high`: max face).
    pub fn border_sample(&self, axis: u8, high: bool, u: u8, v: u8) -> TerrainSample {
        let edge = if high {
            (CHUNK_SAMPLES - 1) as u8
        } else {
            0
        };
        match axis {
            0 => self.get(LocalSample::new(edge, u, v)),
            1 => self.get(LocalSample::new(u, edge, v)),
            2 => self.get(LocalSample::new(u, v, edge)),
            _ => TerrainSample::default(),
        }
    }

    /// Deterministic hash of all sample densities (quantized) for regression tests.
    pub fn density_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for sample in self.samples.iter() {
            (sample.density.to_bits()).hash(&mut hasher);
            sample.material.0.hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// Fill chunk samples from a density evaluator at world sample corners.
pub fn fill_chunk_from_density<F>(
    coord: ChunkCoord,
    mut density_at: F,
    default_material: MaterialId,
) -> TerrainChunk
where
    F: FnMut(i32, i32, i32) -> f32,
{
    let (ox, oy, oz) = TerrainChunk::new(coord).sample_origin();
    let mut chunk = TerrainChunk::new(coord);
    for z in 0..CHUNK_SAMPLES {
        for y in 0..CHUNK_SAMPLES {
            for x in 0..CHUNK_SAMPLES {
                let wx = ox + x as i32;
                let wy = oy + y as i32;
                let wz = oz + z as i32;
                chunk.set(
                    LocalSample::new(x as u8, y as u8, z as u8),
                    TerrainSample {
                        density: density_at(wx, wy, wz),
                        material: default_material,
                    },
                );
            }
        }
    }
    chunk
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_indexing_is_unique() {
        let mut seen = std::collections::HashSet::new();
        for z in 0..CHUNK_SAMPLES {
            for y in 0..CHUNK_SAMPLES {
                for x in 0..CHUNK_SAMPLES {
                    let local = LocalSample::new(x as u8, y as u8, z as u8);
                    assert!(seen.insert(local.linear_index()));
                }
            }
        }
        assert_eq!(seen.len(), SAMPLE_COUNT);
    }

    #[test]
    fn sample_linear_indices_are_unique() {
        use std::collections::HashSet;
        let mut indices = HashSet::new();
        for z in 0..=CHUNK_CELLS as u8 {
            for y in 0..=CHUNK_CELLS as u8 {
                for x in 0..=CHUNK_CELLS as u8 {
                    let local = LocalSample::new(x, y, z);
                    assert!(local.is_valid());
                    assert!(
                        indices.insert(local.linear_index()),
                        "duplicate linear index for sample ({x},{y},{z})"
                    );
                }
            }
        }
        assert_eq!(indices.len(), SAMPLE_COUNT);
    }

    #[test]
    fn border_samples_on_high_x_face() {
        let mut chunk = TerrainChunk::new(ChunkCoord::new(0, 0, 0));
        chunk.set(
            LocalSample::new(16, 3, 5),
            TerrainSample {
                density: -1.0,
                material: MaterialId(2),
            },
        );
        let border = chunk.border_sample(0, true, 3, 5);
        assert_eq!(border.density, -1.0);
    }
}
