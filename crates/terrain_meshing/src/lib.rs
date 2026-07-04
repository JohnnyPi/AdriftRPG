// crates/terrain_meshing/src/lib.rs
//! Surface Nets and alternative terrain meshers. No Bevy dependency.

mod dual_contouring;
mod surface_nets;

use terrain_surface::{ChunkSlotPalette, MaterialVertex, SurfaceMeshResolver};
use voxel_core::TerrainSample;

pub use dual_contouring::DualContouringMesher;
pub use surface_nets::SurfaceNetsMesher;

#[derive(Clone, Debug, Default)]
pub struct TerrainMeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// Dominant local slot per vertex (legacy / debug).
    pub materials: Vec<u16>,
    /// Indexed 4-way blend per vertex (local slot indices + weights).
    pub material_vertices: Vec<MaterialVertex>,
    /// Chunk-local slot → global texture-array layer mapping.
    pub chunk_palette: ChunkSlotPalette,
}

#[derive(Clone)]
pub struct ChunkMeshingInput<'a> {
    pub samples: &'a [TerrainSample],
    pub chunk_cells: usize,
    /// When > 1, skip cells during mesh extraction for distance LOD.
    pub cell_stride: u32,
    /// When set, per-vertex material blends come from the surface classifier.
    pub surface_resolver: Option<&'a dyn SurfaceMeshResolver>,
}

impl std::fmt::Debug for ChunkMeshingInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChunkMeshingInput")
            .field("samples", &self.samples.len())
            .field("chunk_cells", &self.chunk_cells)
            .field("cell_stride", &self.cell_stride)
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
