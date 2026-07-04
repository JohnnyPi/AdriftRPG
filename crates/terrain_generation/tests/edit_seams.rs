// crates/terrain_generation/tests/edit_seams.rs
use terrain_generation::{
    DensitySource, RecipeDensitySource, default_vertical_slice_recipe, fill_padded_samples,
    padded_index,
};
use voxel_core::{CHUNK_CELLS, MaterialId, TerrainEditCommand, TerrainEditStore};

fn samples_with_edits(
    source: &RecipeDensitySource,
    store: &TerrainEditStore,
    coord: voxel_core::ChunkCoord,
) -> Vec<voxel_core::TerrainSample> {
    fill_padded_samples(coord, |wx, wy, wz| {
        if let Some(sample) = store.sample_override(wx, wy, wz) {
            (sample.density, sample.material)
        } else {
            (
                source.sample_density(wx as f32, wy as f32, wz as f32),
                MaterialId(0),
            )
        }
    })
}

#[test]
fn face_straddling_edit_matches_in_both_neighbor_chunk_halos() {
    let source = RecipeDensitySource::new(default_vertical_slice_recipe(42, 2.0));
    let mut store = TerrainEditStore::new();
    let face_x = CHUNK_CELLS as i32;
    store.apply_command(
        &TerrainEditCommand::SubtractSphere {
            center: [face_x as f32, 8.0, 8.0],
            radius_m: 4.0,
        },
        |wx, wy, wz| source.sample_density(wx as f32, wy as f32, wz as f32),
        |_wx, _wy, _wz, _d| MaterialId(0),
    );

    let left = samples_with_edits(&source, &store, voxel_core::ChunkCoord::new(0, 0, 0));
    let right = samples_with_edits(&source, &store, voxel_core::ChunkCoord::new(1, 0, 0));
    let padded = CHUNK_CELLS + 3;

    let left_idx = padded_index(face_x, 8, 8, padded);
    let right_idx = padded_index(face_x - CHUNK_CELLS as i32, 8, 8, padded);
    assert_eq!(
        left[left_idx].density, right[right_idx].density,
        "shared world sample must agree across chunk halos"
    );
    assert!(
        left[left_idx].density > 0.0,
        "subtract edit should push shared face toward air"
    );
}

#[test]
fn cross_chunk_store_is_single_override_not_duplicated() {
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
    assert!(sample.density > 0.0);
}
