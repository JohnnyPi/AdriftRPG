use std::ops::{Div, Rem};

/// Chunk position in chunk-space coordinates. May be negative.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChunkCoord {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

/// World-space voxel cell coordinate in meters (1 cell = 1 meter).
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
            floor_div(self.x, crate::CHUNK_CELLS as i32),
            floor_div(self.y, crate::CHUNK_CELLS as i32),
            floor_div(self.z, crate::CHUNK_CELLS as i32),
        )
    }

    pub fn local_cell(&self) -> LocalCell {
        LocalCell::new(
            positive_mod(self.x, crate::CHUNK_CELLS as i32) as u8,
            positive_mod(self.y, crate::CHUNK_CELLS as i32) as u8,
            positive_mod(self.z, crate::CHUNK_CELLS as i32) as u8,
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

    pub fn chunk_coord(&self) -> ChunkCoord {
        ChunkCoord::new(
            floor_div(self.x, crate::CHUNK_SAMPLES as i32 - 1),
            floor_div(self.y, crate::CHUNK_SAMPLES as i32 - 1),
            floor_div(self.z, crate::CHUNK_SAMPLES as i32 - 1),
        )
    }

    pub fn local_sample(&self, chunk: ChunkCoord) -> LocalSample {
        let origin_x = chunk.x * crate::CHUNK_CELLS as i32;
        let origin_y = chunk.y * crate::CHUNK_CELLS as i32;
        let origin_z = chunk.z * crate::CHUNK_CELLS as i32;
        LocalSample::new(
            (self.x - origin_x) as u8,
            (self.y - origin_y) as u8,
            (self.z - origin_z) as u8,
        )
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

fn floor_div(value: i32, divisor: i32) -> i32 {
    let quotient = value.div(divisor);
    if value.rem(divisor) < 0 {
        quotient - 1
    } else {
        quotient
    }
}

fn positive_mod(value: i32, divisor: i32) -> i32 {
    let remainder = value.rem(divisor);
    if remainder < 0 {
        remainder + divisor
    } else {
        remainder
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
        let local = sample.local_sample(chunk);
        assert_eq!(local, LocalSample::new(0, 0, 0));
    }
}
