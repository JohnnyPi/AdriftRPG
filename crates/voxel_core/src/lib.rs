//! Voxel terrain core types. No Bevy dependency.

pub mod chunk;
pub mod coords;
pub mod sample;
pub mod terrain_chunk;

pub use chunk::{CHUNK_CELLS, CHUNK_SAMPLES, CELL_COUNT, SAMPLE_COUNT};
pub use coords::{ChunkCoord, LocalCell, LocalSample, WorldCell, WorldSample};
pub use sample::{MaterialId, TerrainSample};
pub use terrain_chunk::{fill_chunk_from_density, TerrainChunk};
