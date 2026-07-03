// crates/game_bevy/src/terrain/mesh_convert.rs
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use terrain_meshing::TerrainMeshData;
use voxel_core::{ChunkCoord, CHUNK_CELLS};

pub fn mesh_from_terrain_data(data: &TerrainMeshData, cell_size_m: f32) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    if data.positions.is_empty() {
        return mesh;
    }
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        data.positions
            .iter()
            .map(|p| {
                [
                    p[0] * cell_size_m,
                    p[1] * cell_size_m,
                    p[2] * cell_size_m,
                ]
            })
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals.clone());
    let vertex_count = data.positions.len();
    let material_vertices = if data.material_vertices.len() == vertex_count {
        &data.material_vertices
    } else {
        &[] as &[terrain_surface::MaterialVertex]
    };
    if !material_vertices.is_empty() {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_COLOR,
            material_vertices
                .iter()
                .map(|v| [v.weights[0], v.weights[1], v.weights[2], v.weights[3]])
                .collect::<Vec<_>>(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            material_vertices
                .iter()
                .map(|v| [v.local_indices[0] as f32, v.local_indices[1] as f32])
                .collect::<Vec<_>>(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_1,
            material_vertices
                .iter()
                .map(|v| [v.local_indices[2] as f32, v.local_indices[3] as f32])
                .collect::<Vec<_>>(),
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_TANGENT,
            material_vertices
                .iter()
                .map(|v| [v.tint[0], v.tint[1], v.tint[2], v.overlay[0]])
                .collect::<Vec<_>>(),
        );
    } else {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_COLOR,
            vec![[1.0, 0.0, 0.0, 0.0]; vertex_count],
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_0,
            vec![[0.0, 0.0]; vertex_count],
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_UV_1,
            vec![[0.0, 0.0]; vertex_count],
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_TANGENT,
            vec![[1.0, 1.0, 1.0, 1.0]; vertex_count],
        );
    }
    mesh.insert_indices(Indices::U32(data.indices.clone()));
    mesh
}

/// Attach terrain triplanar blend channels used by `terrain_material.wgsl`.
pub fn insert_terrain_material_attributes(
    mesh: &mut Mesh,
    material_vertices: &[terrain_surface::MaterialVertex],
) {
    let vertex_count = material_vertices.len();
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_COLOR,
        material_vertices
            .iter()
            .map(|v| [v.weights[0], v.weights[1], v.weights[2], v.weights[3]])
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        material_vertices
            .iter()
            .map(|v| [v.local_indices[0] as f32, v.local_indices[1] as f32])
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_1,
        material_vertices
            .iter()
            .map(|v| [v.local_indices[2] as f32, v.local_indices[3] as f32])
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_TANGENT,
        material_vertices
            .iter()
            .map(|v| [v.tint[0], v.tint[1], v.tint[2], v.overlay[0]])
            .collect::<Vec<_>>(),
    );
    let _ = vertex_count;
}

pub fn chunk_world_transform(coord: ChunkCoord, cell_size_m: f32) -> Transform {
    let extent = CHUNK_CELLS as f32 * cell_size_m;
    Transform::from_translation(Vec3::new(
        coord.x as f32 * extent,
        coord.y as f32 * extent,
        coord.z as f32 * extent,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Vec3;

    #[test]
    fn chunk_transform_places_local_vertex_in_world_meters() {
        let coord = ChunkCoord::new(2, -1, 0);
        let cell_size_m = 1.0;
        let transform = chunk_world_transform(coord, cell_size_m);
        let local = [4.0, 8.0, 12.0];
        let world = transform.translation + Vec3::new(local[0], local[1], local[2]) * cell_size_m;
        assert!((world.x - 36.0).abs() < f32::EPSILON);
        assert!((world.y - (-8.0)).abs() < f32::EPSILON);
        assert!((world.z - 12.0).abs() < f32::EPSILON);
    }
}
