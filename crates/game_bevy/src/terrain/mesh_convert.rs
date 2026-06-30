use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use terrain_meshing::TerrainMeshData;
use voxel_core::{ChunkCoord, CHUNK_CELLS};

pub fn mesh_from_terrain_data(data: &TerrainMeshData) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    if data.positions.is_empty() {
        return mesh;
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, data.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals.clone());
    if !data.material_ids.is_empty() {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            data.material_ids
                .iter()
                .map(|ids| [ids[0] as f32, ids[1] as f32])
                .collect::<Vec<_>>(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_1,
            data.material_ids
                .iter()
                .map(|ids| [ids[2] as f32, ids[3] as f32])
                .collect::<Vec<_>>(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_COLOR,
            data.material_weights
                .iter()
                .map(|w| [w[0], w[1], w[2], w[3]])
                .collect::<Vec<_>>(),
        );
    }
    mesh.insert_indices(Indices::U32(data.indices.clone()));
    mesh
}

pub fn chunk_world_transform(coord: ChunkCoord) -> Transform {
    Transform::from_translation(Vec3::new(
        coord.x as f32 * CHUNK_CELLS as f32,
        coord.y as f32 * CHUNK_CELLS as f32,
        coord.z as f32 * CHUNK_CELLS as f32,
    ))
}
