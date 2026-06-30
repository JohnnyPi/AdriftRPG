use bevy::prelude::*;
use terrain_generation::RecipeDensitySource;
use voxel_core::MaterialId;

use super::biome_context::{BiomeSampleContext, ChunkColumnCache, ROCK_SLOPE_DEG};
use super::biomes::{biome_color, classify_biome_with_context, BiomeCatalog, BiomeKind};

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
    let wx = x as i32;
    let wz = z as i32;

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
        let ctx = BiomeSampleContext {
            world_y: 20.0,
            elevation: 18.0,
            slope_degrees: 50.0,
            distance_to_water: 80.0,
            distance_to_river: f32::MAX,
            cave_depth: 0.0,
            moisture: 0.5,
            effective_moisture: 0.5,
            transition_noise: 0.5,
            temperature: 0.5,
            continentalness: 0.5,
            coast_humidity: 0.1,
            rain_shadow: 0.0,
        };
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
        let ctx = BiomeSampleContext {
            world_y: 12.0,
            elevation: 10.0,
            slope_degrees: 5.0,
            distance_to_water: 80.0,
            distance_to_river: f32::MAX,
            cave_depth: 0.0,
            moisture: 0.5,
            effective_moisture: 0.5,
            transition_noise: 0.5,
            temperature: 0.5,
            continentalness: 0.5,
            coast_humidity: 0.1,
            rain_shadow: 0.0,
        };
        let mat = surface_material_for(&catalog, BiomeKind::Grassland, &ctx);
        assert_eq!(mat.0, 0);
    }
}
