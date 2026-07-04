// crates/voxel_core/src/lib.rs
//! Voxel terrain core types. No Bevy dependency.

pub mod chunk;
pub mod coords;
pub mod edits;
pub mod extensions;
pub mod sample;
pub mod stable_hash;
pub mod terrain_chunk;

pub use chunk::{CELL_COUNT, CHUNK_CELLS, CHUNK_SAMPLES, SAMPLE_COUNT};
pub use coords::{ChunkCoord, LocalCell, LocalSample, WorldCell, WorldSample};
pub use edits::{ChunkDelta, DensityDelta, TerrainEditCommand, TerrainEditStore};
pub use extensions::{ChunkInterestProvider, FullExtentInterestProvider, SimulationLodProvider};
pub use sample::{MaterialId, TerrainSample};
pub use stable_hash::{FNV_OFFSET, FNV_PRIME, fnv1a_hash, fnv1a_update, quantize_density_mm};
pub use terrain_chunk::{TerrainChunk, fill_chunk_from_density};
