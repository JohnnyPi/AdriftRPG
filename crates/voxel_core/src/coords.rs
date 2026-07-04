// crates/voxel_core/src/coords.rs
use serde::{Deserialize, Serialize};

/// Chunk position in chunk-space coordinates. May be negative.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct ChunkCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// World-space origin of sample `(0, 0, 0)` for this chunk in meters.
    pub const fn sample_origin(self) -> (i32, i32, i32) {
        (
            self.x * crate::CHUNK_CELLS as i32,
            self.y * crate::CHUNK_CELLS as i32,
            self.z * crate::CHUNK_CELLS as i32,
        )
    }
}

/// World-space voxel cell index (1 cell = 1 meter; requires `cell_size_m == 1.0`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WorldCell {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl WorldCell {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn chunk_coord(&self) -> ChunkCoord {
        ChunkCoord::new(
            self.x.div_euclid(crate::CHUNK_CELLS as i32),
            self.y.div_euclid(crate::CHUNK_CELLS as i32),
            self.z.div_euclid(crate::CHUNK_CELLS as i32),
        )
    }

    pub fn local_cell(&self) -> LocalCell {
        LocalCell::new(
            self.x.rem_euclid(crate::CHUNK_CELLS as i32) as u8,
            self.y.rem_euclid(crate::CHUNK_CELLS as i32) as u8,
            self.z.rem_euclid(crate::CHUNK_CELLS as i32) as u8,
        )
    }
}

/// Local cell index inside a chunk, 0..=15 per axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LocalCell {
    pub x: u8,
    pub y: u8,
    pub z: u8,
}

impl LocalCell {
    pub const fn new(x: u8, y: u8, z: u8) -> Self {
        Self { x, y, z }
    }

    pub fn is_valid(&self) -> bool {
        self.x < crate::CHUNK_CELLS as u8
            && self.y < crate::CHUNK_CELLS as u8
            && self.z < crate::CHUNK_CELLS as u8
    }
}

/// World-space density sample corner coordinate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WorldSample {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl WorldSample {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Chunk that owns this sample for read/write routing.
    ///
    /// Boundary samples belong to the higher-indexed chunk: world-x = 16 maps to chunk `(1, 0, 0)`,
    /// even though chunk `(0, 0, 0)` also stores a duplicate at local sample 16. Any code that
    /// writes samples (edits) must fan out to every chunk sharing the corner — up to 8 at a corner.
    pub fn chunk_coord(&self) -> ChunkCoord {
        ChunkCoord::new(
            self.x.div_euclid(crate::CHUNK_CELLS as i32),
            self.y.div_euclid(crate::CHUNK_CELLS as i32),
            self.z.div_euclid(crate::CHUNK_CELLS as i32),
        )
    }

    pub fn local_sample(&self, chunk: ChunkCoord) -> Option<LocalSample> {
        let (origin_x, origin_y, origin_z) = chunk.sample_origin();
        let dx = self.x - origin_x;
        let dy = self.y - origin_y;
        let dz = self.z - origin_z;
        let range = 0..=(crate::CHUNK_SAMPLES as i32 - 1);
        (range.contains(&dx) && range.contains(&dy) && range.contains(&dz))
            .then(|| LocalSample::new(dx as u8, dy as u8, dz as u8))
    }
}

/// Local density sample index inside a chunk, 0..=16 per axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LocalSample {
    pub x: u8,
    pub y: u8,
    pub z: u8,
}

impl LocalSample {
    pub const fn new(x: u8, y: u8, z: u8) -> Self {
        Self { x, y, z }
    }

    pub fn is_valid(&self) -> bool {
        self.x < crate::CHUNK_SAMPLES as u8
            && self.y < crate::CHUNK_SAMPLES as u8
            && self.z < crate::CHUNK_SAMPLES as u8
    }

    pub fn linear_index(&self) -> usize {
        let x = self.x as usize;
        let y = self.y as usize;
        let z = self.z as usize;
        x + y * crate::CHUNK_SAMPLES + z * crate::CHUNK_SAMPLES * crate::CHUNK_SAMPLES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_chunk_coordinates() {
        let cell = WorldCell::new(-1, -1, -1);
        let chunk = cell.chunk_coord();
        assert_eq!(chunk, ChunkCoord::new(-1, -1, -1));

        let local = cell.local_cell();
        assert_eq!(local, LocalCell::new(15, 15, 15));
    }

    #[test]
    fn boundary_sample_local_index() {
        let sample = WorldSample::new(16, 0, 0);
        let chunk = sample.chunk_coord();
        assert_eq!(chunk, ChunkCoord::new(1, 0, 0));
        let local = sample.local_sample(chunk).expect("in-chunk sample");
        assert_eq!(local, LocalSample::new(0, 0, 0));
    }

    #[test]
    fn local_sample_rejects_out_of_chunk_coordinates() {
        let sample = WorldSample::new(-1, 0, 0);
        assert!(sample.local_sample(ChunkCoord::new(0, 0, 0)).is_none());

        let boundary = WorldSample::new(16, 16, 16);
        assert!(boundary.local_sample(ChunkCoord::new(0, 0, 0)).is_some());
        assert_eq!(
            boundary.local_sample(ChunkCoord::new(0, 0, 0)),
            Some(LocalSample::new(16, 16, 16))
        );
    }

    #[test]
    fn sample_linear_index_layout() {
        assert_eq!(LocalSample::new(0, 1, 0).linear_index(), 17);
        assert_eq!(LocalSample::new(0, 0, 1).linear_index(), 17 * 17);
        assert_eq!(LocalSample::new(1, 0, 0).linear_index(), 1);
    }
}
