// crates/game_bevy/src/terrain/mesh_convert.rs
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use terrain_meshing::TerrainMeshData;
use voxel_core::{CHUNK_CELLS, ChunkCoord};

pub fn mesh_from_terrain_data(data: TerrainMeshData, cell_size_m: f32) -> Mesh {
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
            .map(|p| [p[0] * cell_size_m, p[1] * cell_size_m, p[2] * cell_size_m])
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, data.normals);
    let vertex_count = data.positions.len();
    if data.material_vertices.len() == vertex_count {
        insert_terrain_material_attributes(&mut mesh, &data.material_vertices);
    } else {
        if !data.material_vertices.is_empty() {
            warn!(
                vertex_count,
                material_count = data.material_vertices.len(),
                "terrain mesh material vertex count mismatch; using default material attributes"
            );
        }
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_COLOR,
            vec![[1.0, 0.0, 0.0, 0.0]; vertex_count],
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; vertex_count]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, vec![[0.0, 0.0]; vertex_count]);
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_TANGENT,
            vec![[1.0, 1.0, 1.0, 1.0]; vertex_count],
        );
    }
    mesh.insert_indices(Indices::U32(data.indices));
    mesh
}

/// Attach terrain triplanar blend channels used by `terrain_material.wgsl`.
pub fn insert_terrain_material_attributes(
    mesh: &mut Mesh,
    material_vertices: &[terrain_surface::MaterialVertex],
) {
    let n = material_vertices.len();
    let mut colors = Vec::with_capacity(n);
    let mut uv0 = Vec::with_capacity(n);
    let mut uv1 = Vec::with_capacity(n);
    let mut tangents = Vec::with_capacity(n);
    for v in material_vertices {
        colors.push([v.weights[0], v.weights[1], v.weights[2], v.weights[3]]);
        uv0.push([v.local_indices[0] as f32, v.local_indices[1] as f32]);
        uv1.push([v.local_indices[2] as f32, v.local_indices[3] as f32]);
        tangents.push([v.tint[0], v.tint[1], v.tint[2], v.overlay[0]]);
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv0);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uv1);
    mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
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
    use bevy::mesh::VertexAttributeValues;
    use bevy::prelude::Vec3;
    use terrain_surface::MaterialVertex;

    fn sample_mesh_data() -> TerrainMeshData {
        TerrainMeshData {
            positions: vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]],
            normals: vec![[0.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
            indices: vec![0, 1, 2],
            materials: vec![0, 1],
            material_vertices: vec![
                MaterialVertex {
                    weights: [0.5, 0.3, 0.15, 0.05],
                    weights_1: [0.0; 4],
                    local_indices: [1, 2, 3, 0],
                    tint: [0.8, 0.7, 0.6],
                    overlay: [0.25, 0.0],
                },
                MaterialVertex {
                    weights: [0.1, 0.2, 0.3, 0.4],
                    weights_1: [0.0; 4],
                    local_indices: [0, 1, 2, 3],
                    tint: [0.1, 0.2, 0.3],
                    overlay: [0.9, 0.0],
                },
            ],
            chunk_palette: Default::default(),
        }
    }

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

    #[test]
    fn mesh_from_terrain_data_packs_material_attributes() {
        let mesh = mesh_from_terrain_data(sample_mesh_data(), 2.0);
        let colors = match mesh.attribute(Mesh::ATTRIBUTE_COLOR).expect("colors") {
            VertexAttributeValues::Float32x4(values) => values.clone(),
            other => panic!("unexpected colors format: {other:?}"),
        };
        let uv0 = match mesh.attribute(Mesh::ATTRIBUTE_UV_0).expect("uv0") {
            VertexAttributeValues::Float32x2(values) => values.clone(),
            other => panic!("unexpected uv0 format: {other:?}"),
        };
        let uv1 = match mesh.attribute(Mesh::ATTRIBUTE_UV_1).expect("uv1") {
            VertexAttributeValues::Float32x2(values) => values.clone(),
            other => panic!("unexpected uv1 format: {other:?}"),
        };
        let tangents = match mesh.attribute(Mesh::ATTRIBUTE_TANGENT).expect("tangents") {
            VertexAttributeValues::Float32x4(values) => values.clone(),
            other => panic!("unexpected tangents format: {other:?}"),
        };
        assert_eq!(
            colors,
            vec![
                [0.5, 0.3, 0.15, 0.05],
                [0.1, 0.2, 0.3, 0.4],
            ]
        );
        assert_eq!(uv0, vec![[1.0, 2.0], [0.0, 1.0]]);
        assert_eq!(uv1, vec![[3.0, 0.0], [2.0, 3.0]]);
        assert_eq!(
            tangents,
            vec![[0.8, 0.7, 0.6, 0.25], [0.1, 0.2, 0.3, 0.9]]
        );
    }
}
