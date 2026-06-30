use crate::{ChunkMeshingInput, MeshingError, TerrainMeshData, TerrainMesher};

/// Alternative mesher stub (Dual Contouring deferred).
pub struct DualContouringMesher;

impl TerrainMesher for DualContouringMesher {
    fn build_mesh(&self, _input: &ChunkMeshingInput<'_>) -> Result<TerrainMeshData, MeshingError> {
        Err(MeshingError::Failed(
            "DualContouringMesher is not implemented".into(),
        ))
    }
}
