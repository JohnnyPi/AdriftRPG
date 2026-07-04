// crates/voxel_core/tests/edit_boundary.rs
use voxel_core::{CHUNK_CELLS, MaterialId, TerrainEditCommand, TerrainEditStore};

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
    let sample = store
        .sample_override(face_x, 8, 8)
        .expect("shared world sample");
    assert!(sample.density > 0.0, "subtract should push toward air");
}
