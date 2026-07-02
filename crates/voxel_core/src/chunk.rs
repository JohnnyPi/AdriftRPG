// crates/voxel_core/src/chunk.rs
pub const CHUNK_CELLS: usize = 16;
pub const CHUNK_SAMPLES: usize = CHUNK_CELLS + 1;

pub const CELL_COUNT: usize = CHUNK_CELLS * CHUNK_CELLS * CHUNK_CELLS;
pub const SAMPLE_COUNT: usize = CHUNK_SAMPLES * CHUNK_SAMPLES * CHUNK_SAMPLES;
