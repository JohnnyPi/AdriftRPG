//! Bridge compiler worldgen biomes to presentation soft-weight channels.

use terrain_generation::biomes::id::{BiomeBlendCell, CompilerBiomeId};

use crate::context::{BiomeId, SoftBiomeWeights};

/// Map a compiler biome id to the presentation biome used by YAML catalogs and tints.
pub fn compiler_biome_to_presentation(id: CompilerBiomeId) -> BiomeId {
    match id {
        CompilerBiomeId::Grassland | CompilerBiomeId::DryForest => BiomeId::Grassland,
        CompilerBiomeId::Forest | CompilerBiomeId::CloudForest | CompilerBiomeId::Mangrove => {
            BiomeId::Forest
        }
        CompilerBiomeId::Scrub | CompilerBiomeId::MontaneShrub => BiomeId::Scrub,
        CompilerBiomeId::CoastalScrub => BiomeId::CoastalScrub,
        CompilerBiomeId::Wetland
        | CompilerBiomeId::Swamp
        | CompilerBiomeId::FreshwaterWetland => BiomeId::Wetland,
        CompilerBiomeId::Beach | CompilerBiomeId::Intertidal => BiomeId::Beach,
        CompilerBiomeId::Alpine | CompilerBiomeId::VolcanicBarren => BiomeId::Alpine,
        CompilerBiomeId::RockyUpland | CompilerBiomeId::RockyCliff => BiomeId::RockyUpland,
        CompilerBiomeId::Cave => BiomeId::Cave,
        CompilerBiomeId::Riverbank => BiomeId::Riverbank,
        CompilerBiomeId::ShallowWater
        | CompilerBiomeId::Lagoon
        | CompilerBiomeId::SeagrassBed => BiomeId::ShallowWater,
        CompilerBiomeId::DeepWater
        | CompilerBiomeId::DeepCoastalWater
        | CompilerBiomeId::AbyssalBasin
        | CompilerBiomeId::OpenOcean
        | CompilerBiomeId::HydrothermalZone => BiomeId::DeepWater,
        CompilerBiomeId::OffshoreShelf
        | CompilerBiomeId::ContinentalShelf
        | CompilerBiomeId::CoralReef
        | CompilerBiomeId::ReefSlope => BiomeId::OffshoreShelf,
    }
}

/// Convert a baked biome blend cell into presentation soft weights.
pub fn soft_weights_from_blend_cell(cell: BiomeBlendCell) -> SoftBiomeWeights {
    let mut weights = SoftBiomeWeights::default();
    add_compiler_weight(&mut weights, cell.primary, cell.primary_weight);
    add_compiler_weight(&mut weights, cell.secondary, cell.secondary_weight);
    weights.normalize()
}

/// Single-biome soft weights from a primary biome field sample.
pub fn soft_weights_from_primary_u8(primary: u8) -> SoftBiomeWeights {
    let mut weights = SoftBiomeWeights::default();
    add_compiler_weight(
        &mut weights,
        CompilerBiomeId::from_u8(primary),
        1.0,
    );
    weights.normalize()
}

/// Blend compiler-derived soft weights with a runtime climate heuristic.
pub fn merge_soft_biome_sources(
    compiler: SoftBiomeWeights,
    climate: SoftBiomeWeights,
    compiler_mix: f32,
) -> SoftBiomeWeights {
    let t = compiler_mix.clamp(0.0, 1.0);
    SoftBiomeWeights {
        grassland: compiler.grassland * t + climate.grassland * (1.0 - t),
        forest: compiler.forest * t + climate.forest * (1.0 - t),
        scrub: compiler.scrub * t + climate.scrub * (1.0 - t),
        coastal_scrub: compiler.coastal_scrub * t + climate.coastal_scrub * (1.0 - t),
        wetland: compiler.wetland * t + climate.wetland * (1.0 - t),
        beach: compiler.beach * t + climate.beach * (1.0 - t),
        alpine: compiler.alpine * t + climate.alpine * (1.0 - t),
        rocky: compiler.rocky * t + climate.rocky * (1.0 - t),
    }
    .normalize()
}

fn add_compiler_weight(weights: &mut SoftBiomeWeights, id: CompilerBiomeId, amount: f32) {
    if amount <= f32::EPSILON {
        return;
    }
    match compiler_biome_to_presentation(id) {
        BiomeId::Grassland => weights.grassland += amount,
        BiomeId::Forest => weights.forest += amount,
        BiomeId::Scrub => weights.scrub += amount,
        BiomeId::CoastalScrub => weights.coastal_scrub += amount,
        BiomeId::Wetland | BiomeId::Riverbank => weights.wetland += amount,
        BiomeId::Beach | BiomeId::ShallowWater | BiomeId::OffshoreShelf => weights.beach += amount,
        BiomeId::Alpine => weights.alpine += amount,
        BiomeId::RockyUpland | BiomeId::Cave => weights.rocky += amount,
        BiomeId::DeepWater => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terrain_generation::CompilerBiomeId;

    #[test]
    fn forest_primary_dominates_grassland() {
        let cell = BiomeBlendCell {
            primary: CompilerBiomeId::Forest,
            primary_weight: 0.85,
            secondary: CompilerBiomeId::Grassland,
            secondary_weight: 0.15,
        };
        let soft = soft_weights_from_blend_cell(cell);
        assert!(soft.forest > soft.grassland);
    }

    #[test]
    fn all_compiler_ids_map_to_known_presentation() {
        for v in 0..=30u8 {
            let id = CompilerBiomeId::from_u8(v);
            let _ = compiler_biome_to_presentation(id);
        }
    }
}
