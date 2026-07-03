// crates/game_bevy/src/environment/materials.rs
use bevy::prelude::*;
use terrain_generation::RecipeDensitySource;
use voxel_core::MaterialId;

use super::biome_context::{BiomeSampleContext, ChunkColumnCache, ROCK_SLOPE_DEG};
use super::biomes::{biome_color, classify_biome_with_context, BiomeKind};
use crate::environment::BiomeCatalog;

const SURFACE_VOXEL_BAND: f32 = 1.0;
const WET_ROCK_DISTANCE_M: f32 = 12.0;

pub fn assign_material_color(catalog: &BiomeCatalog, material_id: u16) -> Color {
    let kind = material_id_to_biome_kind(material_id);
    biome_color(catalog, kind)
}

pub fn surface_material_for(
    catalog: &BiomeCatalog,
    biome: BiomeKind,
    ctx: &BiomeSampleContext,
) -> MaterialId {
    if biome == BiomeKind::Cave {
        return MaterialId(catalog.material_id_for(BiomeKind::Cave));
    }
    if biome == BiomeKind::ShallowWater {
        return MaterialId(catalog.material_id_for(BiomeKind::ShallowWater));
    }
    if ctx.slope_degrees > ROCK_SLOPE_DEG {
        return MaterialId(2);
    }
    if biome == BiomeKind::Beach || ctx.distance_to_water < WET_ROCK_DISTANCE_M {
        return MaterialId(catalog.material_id_for(BiomeKind::Beach));
    }
    MaterialId(catalog.material_id_for(biome))
}

pub fn material_for_world(
    catalog: &BiomeCatalog,
    source: &RecipeDensitySource,
    x: f32,
    y: f32,
    z: f32,
    density: f32,
) -> MaterialId {
    material_for_world_with_cache(catalog, source, None, x, y, z, density)
}

pub fn material_for_world_with_cache(
    catalog: &BiomeCatalog,
    source: &RecipeDensitySource,
    cache: Option<&ChunkColumnCache>,
    x: f32,
    y: f32,
    z: f32,
    density: f32,
) -> MaterialId {
    let wx = x.floor() as i32;
    let wz = z.floor() as i32;
    debug_assert!(
        (x - wx as f32).abs() < f32::EPSILON && (z - wz as f32).abs() < f32::EPSILON,
        "material sampling expects integer world XZ"
    );

    let (surface_y, ctx) = if let Some(cache) = cache {
        let column = cache.column(wx, wz);
        (column.surface_y, cache.context_at(source, wx, y, wz))
    } else {
        let surface_y = source.surface_height_at(x, z);
        let ctx = BiomeSampleContext::sample(source, x, y, z);
        (surface_y, ctx)
    };

    let near_surface = (y - surface_y).abs() <= SURFACE_VOXEL_BAND;
    let sample_y = if near_surface { surface_y } else { y };
    let sample_density = if near_surface { -0.1 } else { density };

    let ctx = if near_surface && sample_y != y {
        if let Some(cache) = cache {
            cache.context_at(source, wx, sample_y, wz)
        } else {
            BiomeSampleContext::sample(source, x, sample_y, z)
        }
    } else {
        ctx
    };

    let biome = classify_biome_with_context(catalog, source, &ctx, sample_density);
    surface_material_for(catalog, biome, &ctx)
}

pub fn terrain_material_key_from_paint_material(material: MaterialId) -> terrain_surface::MaterialKey {
    match material_id_to_biome_kind(material.0) {
        BiomeKind::Beach => terrain_surface::MaterialKey::new("sand"),
        BiomeKind::RockyUpland | BiomeKind::Alpine => terrain_surface::MaterialKey::new("rock"),
        BiomeKind::Forest => terrain_surface::MaterialKey::new("forest_floor"),
        BiomeKind::Wetland | BiomeKind::Riverbank => terrain_surface::MaterialKey::new("wet_rock"),
        BiomeKind::Cave => terrain_surface::MaterialKey::new("cave_stone"),
        BiomeKind::Scrub | BiomeKind::CoastalScrub => terrain_surface::MaterialKey::new("scrub"),
        _ => terrain_surface::MaterialKey::new("grass"),
    }
}

fn material_id_to_biome_kind(material_id: u16) -> BiomeKind {
    match material_id {
        1 => BiomeKind::Beach,
        2 => BiomeKind::RockyUpland,
        3 => BiomeKind::Cave,
        4 => BiomeKind::Wetland,
        5 => BiomeKind::Forest,
        6 => BiomeKind::Scrub,
        _ => BiomeKind::Grassland,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steep_slope_uses_rock_without_changing_biome() {
        let catalog = BiomeCatalog {
            rules: vec![game_data::BiomeRuleDefinition::new(
                "grassland",
                0,
                [0.34, 0.52, 0.28],
            )],
        };
        let ctx = BiomeSampleContext::for_test(20.0, 18.0, 50.0, 80.0);
        let mat = surface_material_for(&catalog, BiomeKind::Grassland, &ctx);
        assert_eq!(mat.0, 2);
    }

    #[test]
    fn gentle_grassland_uses_yaml_material_id() {
        let catalog = BiomeCatalog {
            rules: vec![game_data::BiomeRuleDefinition::new(
                "grassland",
                0,
                [0.34, 0.52, 0.28],
            )],
        };
        let ctx = BiomeSampleContext::for_test(12.0, 10.0, 5.0, 80.0);
        let mat = surface_material_for(&catalog, BiomeKind::Grassland, &ctx);
        assert_eq!(mat.0, 0);
    }
}
