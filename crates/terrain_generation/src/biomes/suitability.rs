//! Biome suitability scoring.

use game_data::CompiledBiomeRecipe;

use crate::biomes::id::{BiomeBlendCell, CompilerBiomeId};
use crate::contract::derive_seed;

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 >= edge1 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn range_weight(value: f32, min: f32, max: f32, fade: f32) -> f32 {
    let enter = smoothstep(min, min + fade, value);
    let exit = 1.0 - smoothstep(max - fade, max, value);
    (enter * exit).clamp(0.0, 1.0)
}

pub struct BiomeScore {
    pub id: CompilerBiomeId,
    pub score: f32,
}

impl Copy for BiomeScore {}
impl Clone for BiomeScore {
    fn clone(&self) -> Self {
        *self
    }
}

pub fn score_land_biomes(
    elevation_m: f32,
    slope_deg: f32,
    rainfall: f32,
    humidity: f32,
    wetness: f32,
    soil_depth: f32,
    wind_exposure: f32,
    beach: f32,
    cliff: f32,
    wetland_mask: f32,
    river_mask: f32,
    mangrove: f32,
    recipe: &CompiledBiomeRecipe,
    sea_level_m: f32,
    micro_noise: f32,
) -> Vec<BiomeScore> {
    let elev = elevation_m - sea_level_m;
    let mut scores = Vec::new();

    if beach > 0.35 {
        scores.push(BiomeScore {
            id: CompilerBiomeId::Beach,
            score: beach,
        });
    }
    if cliff > 0.4 {
        scores.push(BiomeScore {
            id: CompilerBiomeId::RockyCliff,
            score: cliff,
        });
    }
    if mangrove > 0.35 {
        scores.push(BiomeScore {
            id: CompilerBiomeId::Mangrove,
            score: mangrove,
        });
    }
    if river_mask > 0.4 {
        scores.push(BiomeScore {
            id: CompilerBiomeId::Riverbank,
            score: river_mask,
        });
    }
    if wetland_mask > 0.35 || wetness > recipe.wetland_moisture_min {
        let swamp = wetness * (1.0 - smoothstep(15.0, 30.0, slope_deg));
        scores.push(BiomeScore {
            id: CompilerBiomeId::Swamp,
            score: swamp * 0.9,
        });
        scores.push(BiomeScore {
            id: CompilerBiomeId::FreshwaterWetland,
            score: wetland_mask.max(wetness * 0.7),
        });
        scores.push(BiomeScore {
            id: CompilerBiomeId::Wetland,
            score: wetland_mask.max(wetness * 0.6),
        });
    }

    let windward = wind_exposure * rainfall;
    let leeward = (1.0 - wind_exposure) * (1.0 - rainfall * 0.5);

    let cloud_forest = range_weight(
        elev,
        recipe.cloud_forest_elevation_min_m,
        recipe.cloud_forest_elevation_max_m,
        80.0,
    ) * humidity
        * windward.sqrt();
    scores.push(BiomeScore {
        id: CompilerBiomeId::CloudForest,
        score: cloud_forest,
    });

    let forest = range_weight(elev, 8.0, 55.0, 12.0)
        * range_weight(slope_deg, 0.0, 38.0, 10.0)
        * (rainfall * 0.6 + humidity * 0.4)
        * (0.85 + windward * 0.15 + micro_noise * 0.05);
    scores.push(BiomeScore {
        id: CompilerBiomeId::Forest,
        score: forest,
    });

    let dry_forest = range_weight(elev, 5.0, 40.0, 10.0)
        * (1.0
            - smoothstep(
                recipe.dry_forest_rainfall_max - 0.1,
                recipe.dry_forest_rainfall_max,
                rainfall,
            ))
        * leeward;
    scores.push(BiomeScore {
        id: CompilerBiomeId::DryForest,
        score: dry_forest,
    });

    let grassland = range_weight(elev, 3.0, 45.0, 10.0)
        * range_weight(slope_deg, 0.0, 30.0, 8.0)
        * (1.0 - wetness * 0.45)
        * (0.7 + leeward * 0.3);
    scores.push(BiomeScore {
        id: CompilerBiomeId::Grassland,
        score: grassland,
    });

    let scrub = range_weight(elev, 2.0, 35.0, 8.0)
        * smoothstep(
            recipe.dry_forest_rainfall_max,
            recipe.dry_forest_rainfall_max + 0.2,
            rainfall,
        )
        * leeward;
    scores.push(BiomeScore {
        id: CompilerBiomeId::Scrub,
        score: scrub,
    });
    scores.push(BiomeScore {
        id: CompilerBiomeId::CoastalScrub,
        score: scrub * 0.6 + leeward * 0.2,
    });

    if elev > recipe.montane_shrub_elevation_m {
        scores.push(BiomeScore {
            id: CompilerBiomeId::MontaneShrub,
            score: range_weight(
                elev,
                recipe.montane_shrub_elevation_m,
                recipe.montane_shrub_elevation_m + 400.0,
                60.0,
            ),
        });
        scores.push(BiomeScore {
            id: CompilerBiomeId::Alpine,
            score: range_weight(
                elev,
                recipe.montane_shrub_elevation_m + 200.0,
                recipe.montane_shrub_elevation_m + 800.0,
                100.0,
            ),
        });
    }

    let volcanic = smoothstep(recipe.volcanic_barren_slope_deg, 55.0, slope_deg)
        .max(range_weight(elev, 25.0, 60.0, 10.0));
    scores.push(BiomeScore {
        id: CompilerBiomeId::VolcanicBarren,
        score: volcanic,
    });
    scores.push(BiomeScore {
        id: CompilerBiomeId::RockyUpland,
        score: volcanic * 0.7 + smoothstep(20.0, 40.0, slope_deg) * 0.3,
    });

    let _ = soil_depth;
    scores
}

pub fn score_marine_biomes(
    depth_m: f32,
    coast_abs_m: f32,
    reef: f32,
    lagoon: f32,
    tidal_flat: f32,
    shelf: f32,
    temperature: f32,
    recipe: &CompiledBiomeRecipe,
) -> Vec<BiomeScore> {
    let mut scores = Vec::new();

    if lagoon > 0.35 {
        scores.push(BiomeScore {
            id: CompilerBiomeId::Lagoon,
            score: lagoon,
        });
    }
    if reef > 0.35 {
        scores.push(BiomeScore {
            id: CompilerBiomeId::CoralReef,
            score: reef,
        });
        scores.push(BiomeScore {
            id: CompilerBiomeId::ReefSlope,
            score: reef * smoothstep(recipe.reef_depth_min_m, recipe.reef_depth_max_m, depth_m),
        });
    }
    if tidal_flat > 0.3 {
        scores.push(BiomeScore {
            id: CompilerBiomeId::Intertidal,
            score: tidal_flat,
        });
    }
    if shelf > 0.3 {
        scores.push(BiomeScore {
            id: CompilerBiomeId::ContinentalShelf,
            score: shelf,
        });
        scores.push(BiomeScore {
            id: CompilerBiomeId::OffshoreShelf,
            score: shelf * 0.85,
        });
        scores.push(BiomeScore {
            id: CompilerBiomeId::SeagrassBed,
            score: shelf * smoothstep(2.0, 15.0, depth_m) * temperature,
        });
    }
    if depth_m < 5.0 && coast_abs_m < 80.0 {
        scores.push(BiomeScore {
            id: CompilerBiomeId::ShallowWater,
            score: 1.0 - smoothstep(0.0, 5.0, depth_m),
        });
    }
    if depth_m > recipe.deep_coastal_depth_m {
        scores.push(BiomeScore {
            id: CompilerBiomeId::AbyssalBasin,
            score: smoothstep(
                recipe.deep_coastal_depth_m,
                recipe.deep_coastal_depth_m + 500.0,
                depth_m,
            ),
        });
        scores.push(BiomeScore {
            id: CompilerBiomeId::OpenOcean,
            score: 0.8,
        });
    } else if depth_m > recipe.shelf_depth_m {
        scores.push(BiomeScore {
            id: CompilerBiomeId::DeepCoastalWater,
            score: smoothstep(recipe.shelf_depth_m, recipe.deep_coastal_depth_m, depth_m),
        });
        scores.push(BiomeScore {
            id: CompilerBiomeId::DeepWater,
            score: 0.7,
        });
    } else {
        scores.push(BiomeScore {
            id: CompilerBiomeId::DeepWater,
            score: 0.5,
        });
    }

    if temperature > 0.75 && depth_m > 100.0 {
        scores.push(BiomeScore {
            id: CompilerBiomeId::HydrothermalZone,
            score: 0.15,
        });
    }

    scores
}

pub fn pick_biome_blend(scores: &mut [BiomeScore]) -> BiomeBlendCell {
    scores.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let total: f32 = scores
        .iter()
        .map(|s| s.score.max(0.0))
        .sum::<f32>()
        .max(1e-6);
    let primary = scores.first().copied().unwrap_or(BiomeScore {
        id: CompilerBiomeId::Grassland,
        score: 0.1,
    });
    let secondary = scores.get(1).copied().unwrap_or(BiomeScore {
        id: primary.id,
        score: 0.0,
    });
    BiomeBlendCell {
        primary: primary.id,
        primary_weight: (primary.score / total).clamp(0.0, 1.0),
        secondary: secondary.id,
        secondary_weight: (secondary.score / total).clamp(0.0, 1.0),
    }
}

pub fn micro_biome_noise(world_seed: u64, x: u32, z: u32) -> f32 {
    let h = derive_seed(world_seed, "biome_micro", None, (x as u64) << 32 | z as u64);
    ((h % 1000) as f32 / 1000.0 - 0.5) * 0.1
}
