//! Surface Nets and alternative terrain meshers. No Bevy dependency.

mod dual_contouring;
mod surface_nets;

use voxel_core::TerrainSample;
use terrain_surface::SurfaceMeshResolver;

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

#[derive(Clone)]
pub struct ChunkMeshingInput<'a> {
    pub samples: &'a [TerrainSample],
    pub chunk_cells: usize,
    /// When set, per-vertex material blends come from the surface classifier.
    pub surface_resolver: Option<&'a dyn SurfaceMeshResolver>,
}

impl std::fmt::Debug for ChunkMeshingInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkMeshingInput")
            .field("samples", &self.samples.len())
            .field("chunk_cells", &self.chunk_cells)
            .field("surface_resolver", &self.surface_resolver.is_some())
            .finish()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MeshingError {
    #[error("meshing failed: {0}")]
    Failed(String),
}

pub trait TerrainMesher: Send + Sync {
    fn build_mesh(&self, input: &ChunkMeshingInput<'_>) -> Result<TerrainMeshData, MeshingError>;
}
