//! Voxel terrain core types. No Bevy dependency.

pub mod chunk;
pub mod coords;
pub mod edits;
pub mod extensions;
pub mod sample;
pub mod terrain_chunk;

pub use chunk::{CHUNK_CELLS, CHUNK_SAMPLES, CELL_COUNT, SAMPLE_COUNT};
pub use coords::{ChunkCoord, LocalCell, LocalSample, WorldCell, WorldSample};
pub use edits::{ChunkDelta, DensityDelta, TerrainEditCommand, TerrainEditStore};
pub use extensions::{ChunkInterestProvider, FullExtentInterestProvider, SimulationLodProvider};
pub use sample::{MaterialId, TerrainSample};
pub use terrain_chunk::{fill_chunk_from_density, TerrainChunk};
