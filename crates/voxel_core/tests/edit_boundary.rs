use voxel_core::{MaterialId, TerrainEditCommand, TerrainEditStore, CHUNK_CELLS};

#[test]
fn cross_chunk_edit_produces_matching_face_density() {
    let mut store = TerrainEditStore::new();
    let proc = |_x, _y, _z| -1.0f32;
    let mat = |_x, _y, _z, _d| MaterialId(0);
    let face_x = CHUNK_CELLS as i32;
    store.apply_command(
        &TerrainEditCommand::SubtractSphere {
            center: [face_x as f32, 8.0, 8.0],
            radius_m: 4.0,
        },
        proc,
        mat,
    );
    let left = store.sample_override(face_x, 8, 8);
    let right = store.sample_override(face_x, 8, 8);
    assert_eq!(left, right, "shared world sample must be single override");
    assert!(left.unwrap().density > 0.0);
}
