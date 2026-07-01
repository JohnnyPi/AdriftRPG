use crate::blend::SurfaceMaterialBlend;
use crate::blend::SurfaceClassifier;
use crate::context::{saturate, smoothstep, BiomeId, GeologyId, SurfaceContext};
use crate::material_id::TerrainMaterialId;

#[derive(Default)]
pub struct IslandSurfaceClassifier;

impl SurfaceClassifier for IslandSurfaceClassifier {
    fn classify(&self, c: &SurfaceContext) -> SurfaceMaterialBlend {
        if c.cave_exposure > 0.55 {
            return classify_cave(c);
        }
        if c.water_depth_m > 0.05 {
            return SurfaceMaterialBlend::single(TerrainMaterialId::RiverSilt);
        }
        if c.coast_distance_m < 20.0 && c.elevation_m < c.sea_level_m + 4.0 {
            return classify_coast(c);
        }
        if c.river_distance_m < 5.0 {
            return classify_river(c);
        }
        if c.slope_degrees > 48.0 {
            return classify_cliff(c);
        }
        classify_land(c)
    }
}

fn classify_cliff(c: &SurfaceContext) -> SurfaceMaterialBlend {
    let moss = smoothstep(0.45, 0.95, c.moisture)
        * (1.0 - smoothstep(70.0, 88.0, c.slope_degrees));
    let fresh_rock = smoothstep(68.0, 88.0, c.slope_degrees);

    SurfaceMaterialBlend {
        materials: [
            TerrainMaterialId::WeatheredBasalt,
            TerrainMaterialId::FreshBasalt,
            TerrainMaterialId::JungleMoss,
            TerrainMaterialId::TropicalRedSoil,
        ],
        weights: [
            0.70 * (1.0 - fresh_rock),
            0.30 + fresh_rock,
            moss * 0.25,
            saturate(c.soil_depth_m / 1.5) * 0.10,
        ],
    }
    .normalize()
}

fn classify_coast(c: &SurfaceContext) -> SurfaceMaterialBlend {
    let rock = smoothstep(25.0, 50.0, c.slope_degrees);
    let wave_rubble = saturate(c.wave_exposure) * (1.0 - rock);
    let shoreline = smoothstep(c.sea_level_m + 2.0, c.sea_level_m, c.elevation_m);

    SurfaceMaterialBlend {
        materials: [
            TerrainMaterialId::CoralSand,
            TerrainMaterialId::RiverSilt,
            TerrainMaterialId::WeatheredBasalt,
            TerrainMaterialId::TropicalRedSoil,
        ],
        weights: [
            (1.0 - rock) * (1.0 - wave_rubble) * 0.70,
            shoreline * 0.20,
            wave_rubble * 0.35 + rock * 0.5,
            rock * 0.5,
        ],
    }
    .normalize()
}

fn classify_river(c: &SurfaceContext) -> SurfaceMaterialBlend {
    let channel = 1.0 - smoothstep(0.0, 5.0, c.river_distance_m);
    let mud = channel * smoothstep(0.55, 0.95, c.moisture);

    SurfaceMaterialBlend {
        materials: [
            TerrainMaterialId::RiverGravel,
            TerrainMaterialId::RiverSilt,
            TerrainMaterialId::JungleLoam,
            TerrainMaterialId::JungleMoss,
        ],
        weights: [
            channel * (1.0 - mud),
            mud * 0.55 + channel * 0.25,
            1.0 - channel,
            mud * 0.20,
        ],
    }
    .normalize()
}

fn classify_cave(c: &SurfaceContext) -> SurfaceMaterialBlend {
    let flowstone = smoothstep(0.35, 0.85, c.mineral_deposition);
    let moss = smoothstep(0.60, 0.95, c.moisture) * smoothstep(0.45, 0.70, c.cave_exposure);
    let limestone_w = match c.geology {
        GeologyId::Limestone => 0.65,
        _ => 0.0,
    };

    SurfaceMaterialBlend {
        materials: [
            TerrainMaterialId::WeatheredBasalt,
            TerrainMaterialId::RiverSilt,
            TerrainMaterialId::JungleMoss,
            TerrainMaterialId::WeatheredBasalt,
        ],
        weights: [
            1.0 - flowstone,
            flowstone * 0.8,
            moss * 0.15,
            limestone_w,
        ],
    }
    .normalize()
}

fn classify_land(c: &SurfaceContext) -> SurfaceMaterialBlend {
    let soft = c.soft;
    let exposed_rock = smoothstep(10.0, 34.0, c.slope_degrees) * (1.0 - saturate(c.soil_depth_m / 2.0));
    let moss = smoothstep(0.55, 0.95, c.moisture) * (1.0 - exposed_rock);

    let forest = soft.forest;
    let grass = soft.grassland;
    let scrub = soft.scrub + soft.coastal_scrub * 0.5;
    let alpine = soft.alpine + soft.rocky * 0.6;
    let wetland = soft.wetland;

    let litter = match c.biome {
        BiomeId::Forest => 0.35,
        BiomeId::Wetland | BiomeId::Riverbank => 0.15,
        BiomeId::Scrub | BiomeId::CoastalScrub => 0.08,
        _ => 0.05,
    };

    if alpine > 0.55 {
        return SurfaceMaterialBlend {
            materials: [
                TerrainMaterialId::WeatheredBasalt,
                TerrainMaterialId::FreshBasalt,
                TerrainMaterialId::JungleMoss,
                TerrainMaterialId::TropicalRedSoil,
            ],
            weights: [
                alpine * 0.5,
                alpine * 0.35 + exposed_rock,
                moss * 0.2,
                grass * 0.1,
            ],
        }
        .normalize();
    }

    if wetland > 0.25 {
        return SurfaceMaterialBlend {
            materials: [
                TerrainMaterialId::RiverSilt,
                TerrainMaterialId::JungleMoss,
                TerrainMaterialId::RiverGravel,
                TerrainMaterialId::JungleLoam,
            ],
            weights: [
                wetland * 0.55,
                moss * 0.25,
                wetland * 0.2,
                forest * 0.15,
            ],
        }
        .normalize();
    }

    SurfaceMaterialBlend {
        materials: [
            TerrainMaterialId::JungleLoam,
            TerrainMaterialId::TropicalRedSoil,
            TerrainMaterialId::JungleMoss,
            TerrainMaterialId::WeatheredBasalt,
        ],
        weights: [
            forest * (1.0 - litter) + scrub * 0.3,
            grass + scrub * 0.5,
            moss + litter * forest,
            exposed_rock + alpine * 0.2,
        ],
    }
    .normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend::validate_blend;
    use crate::context::{SoftBiomeWeights, SurfaceContext};

    fn base_context() -> SurfaceContext {
        SurfaceContext {
            world_position: [0.0, 10.0, 0.0],
            world_normal: [0.0, 1.0, 0.0],
            elevation_m: 10.0,
            sea_level_m: 0.0,
            water_depth_m: 0.0,
            slope_degrees: 5.0,
            moisture: 0.5,
            soil_depth_m: 1.2,
            coast_distance_m: 100.0,
            river_distance_m: 100.0,
            wave_exposure: 0.0,
            cave_exposure: 0.0,
            mineral_deposition: 0.0,
            biome: BiomeId::Grassland,
            geology: GeologyId::Basalt,
            soft: SoftBiomeWeights {
                grassland: 1.0,
                ..Default::default()
            },
        }
    }

    #[test]
    fn cliff_includes_fresh_basalt() {
        let classifier = IslandSurfaceClassifier;
        let mut ctx = base_context();
        ctx.slope_degrees = 72.0;
        ctx.moisture = 0.2;
        ctx.soil_depth_m = 0.1;
        let result = classifier.classify(&ctx);
        validate_blend(result);
        assert!(
            result.materials.contains(&TerrainMaterialId::FreshBasalt)
                || result.materials.contains(&TerrainMaterialId::WeatheredBasalt)
        );
    }

    #[test]
    fn forest_grassland_transition_blends_both_soils() {
        let classifier = IslandSurfaceClassifier;
        let mut ctx = base_context();
        ctx.soft = SoftBiomeWeights {
            forest: 0.5,
            grassland: 0.5,
            ..Default::default()
        };
        ctx.moisture = 0.6;
        let result = classifier.classify(&ctx);
        validate_blend(result);
        assert!(result.weights[0] > 0.1 && result.weights[1] > 0.1);
    }

    #[test]
    fn coast_uses_sand() {
        let classifier = IslandSurfaceClassifier;
        let mut ctx = base_context();
        ctx.coast_distance_m = 5.0;
        ctx.elevation_m = 1.0;
        let result = classifier.classify(&ctx);
        validate_blend(result);
        assert!(result.materials.contains(&TerrainMaterialId::CoralSand));
    }
}
