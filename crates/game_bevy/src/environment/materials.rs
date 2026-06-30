use bevy::prelude::*;
use voxel_core::MaterialId;

use super::biomes::{biome_color, classify_biome, BiomeCatalog, BiomeKind};

pub fn assign_material_color(catalog: &BiomeCatalog, material_id: u16) -> Color {
    let kind = match material_id {
        0 => BiomeKind::Grassland,
        1 => BiomeKind::Beach,
        2 => BiomeKind::RockyUpland,
        3 => BiomeKind::Cave,
        4 => BiomeKind::RockyUpland,
        _ => BiomeKind::Grassland,
    };
    biome_color(catalog, kind)
}

pub fn material_for_world(
    catalog: &BiomeCatalog,
    sea_level: f32,
    x: f32,
    y: f32,
    z: f32,
    density: f32,
) -> MaterialId {
    let biome = classify_biome(catalog, sea_level, x, y, z, density);
    MaterialId(match biome {
        BiomeKind::Beach => 1,
        BiomeKind::Grassland => 0,
        BiomeKind::RockyUpland => 2,
        BiomeKind::Cave => 3,
        BiomeKind::ShallowWater => 1,
    })
}
