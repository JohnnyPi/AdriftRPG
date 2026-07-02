// crates/terrain_generation/src/chunk_gen.rs
use voxel_core::{fill_chunk_from_density, ChunkCoord, MaterialId, TerrainChunk, TerrainSample};

use crate::DensitySource;

pub fn generate_chunk(source: &dyn DensitySource, coord: ChunkCoord, material: MaterialId) -> TerrainChunk {
    fill_chunk_from_density(coord, |x, y, z| source.sample_density(x as f32, y as f32, z as f32), material)
}

/// Build a padded `(cells + 3)³` sample halo for meshing (1-sample border each side).
pub fn generate_padded_samples(
    source: &dyn DensitySource,
    coord: ChunkCoord,
    material: MaterialId,
) -> Vec<TerrainSample> {
    fill_padded_samples(coord, |wx, wy, wz| {
        (
            source.sample_density(wx as f32, wy as f32, wz as f32),
            material,
        )
    })
}

/// Shared padded-grid loop used by runtime meshing and procedural chunk generation.
pub fn fill_padded_samples(
    coord: ChunkCoord,
    mut sample_at: impl FnMut(i32, i32, i32) -> (f32, MaterialId),
) -> Vec<TerrainSample> {
    let cells = voxel_core::CHUNK_CELLS;
    let padded = cells + 3;
    let (ox, oy, oz) = TerrainChunk::new(coord).sample_origin();
    let mut samples = Vec::with_capacity(padded * padded * padded);
    for pz in -1..=(cells as i32 + 1) {
        for py in -1..=(cells as i32 + 1) {
            for px in -1..=(cells as i32 + 1) {
                let wx = ox + px;
                let wy = oy + py;
                let wz = oz + pz;
                let (density, material) = sample_at(wx, wy, wz);
                samples.push(TerrainSample { density, material });
            }
        }
    }
    samples
}

pub fn chunk_axis_range(extent: i32) -> impl Iterator<Item = i32> {
    let start = -(extent / 2);
    (0..extent).map(move |i| start + i)
}

pub fn iter_world_chunk_coords(extent: [i32; 3]) -> impl Iterator<Item = ChunkCoord> {
    let ex = extent[0];
    let ey = extent[1];
    let ez = extent[2];
    chunk_axis_range(ex).flat_map(move |cx| {
        chunk_axis_range(ey).flat_map(move |cy| {
            chunk_axis_range(ez).map(move |cz| ChunkCoord::new(cx, cy, cz))
        })
    })
}

pub fn padded_index(x: i32, y: i32, z: i32, padded_size: usize) -> usize {
    (x + 1) as usize + (y + 1) as usize * padded_size + (z + 1) as usize * padded_size * padded_size
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{default_vertical_slice_recipe, RecipeDensitySource};

    #[test]
    fn generates_all_world_chunk_positions() {
        let source = RecipeDensitySource::new(default_vertical_slice_recipe(48129, 2.0));
        let extent = [6i32, 3, 6];
        let count = iter_world_chunk_coords(extent)
            .map(|coord| generate_chunk(&source, coord, MaterialId(0)))
            .count();
        assert_eq!(count, 108);
    }
}
