//! Surface Nets and alternative terrain meshers. No Bevy dependency.

mod dual_contouring;
mod surface_nets;

use voxel_core::TerrainSample;

pub use dual_contouring::DualContouringMesher;
pub use surface_nets::SurfaceNetsMesher;

#[derive(Clone, Debug, Default)]
pub struct TerrainMeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// Dominant material per vertex (legacy / debug).
    pub materials: Vec<u16>,
    /// Up to four blended material IDs per vertex.
    pub material_ids: Vec<[u16; 4]>,
    /// Normalized blend weights matching `material_ids`.
    pub material_weights: Vec<[f32; 4]>,
}

#[derive(Clone, Debug)]
pub struct ChunkMeshingInput<'a> {
    pub samples: &'a [TerrainSample],
    pub chunk_cells: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum MeshingError {
    #[error("meshing failed: {0}")]
    Failed(String),
}

pub trait TerrainMesher: Send + Sync {
    fn build_mesh(&self, input: &ChunkMeshingInput<'_>) -> Result<TerrainMeshData, MeshingError>;
}
