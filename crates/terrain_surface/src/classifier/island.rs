// crates/terrain_surface/src/classifier/island.rs
use std::collections::HashMap;

use crate::blend::SurfaceMaterialBlend;
use crate::blend::SurfaceClassifier;
use crate::context::{saturate, smoothstep, BiomeId, GeologyId, SurfaceContext};
use crate::material_id::MaterialKey;

fn mat(name: &str) -> MaterialKey {
    MaterialKey::new(name)
}

#[derive(Default)]
pub struct IslandSurfaceClassifier;

impl SurfaceClassifier for IslandSurfaceClassifier {
    fn classify(&self, c: &SurfaceContext) -> SurfaceMaterialBlend {
        if c.cave_exposure > 0.55 {
            return classify_cave(c);
        }
        if c.water_depth_m > 0.05 {
            return SurfaceMaterialBlend::single(mat("wet_rock"));
        }

        let coast_gate = (1.0 - smoothstep(18.0, 22.0, c.coast_distance_m))
            * smoothstep(c.sea_level_m + 5.0, c.sea_level_m + 2.0, c.elevation_m);
        let river_gate = 1.0 - smoothstep(3.0, 7.0, c.river_distance_m);
        let cliff_gate = smoothstep(46.0, 50.0, c.slope_degrees);

        weighted_blend(&[
            (coast_gate, classify_coast(c)),
            (river_gate * (1.0 - coast_gate), classify_river(c)),
            (cliff_gate, classify_cliff(c)),
            (
                (1.0 - coast_gate) * (1.0 - river_gate) * (1.0 - cliff_gate),
                classify_land(c),
            ),
        ])
    }
}

fn weighted_blend(parts: &[(f32, SurfaceMaterialBlend)]) -> SurfaceMaterialBlend {
    let mut weights: HashMap<MaterialKey, f32> = HashMap::new();
    for (gate, blend) in parts {
        if *gate <= f32::EPSILON {
            continue;
        }
        for i in 0..4 {
            let w = blend.weights[i] * gate;
            if w > 0.0 {
                *weights.entry(blend.materials[i].clone()).or_default() += w;
            }
        }
    }
    let mut ranked: Vec<(MaterialKey, f32)> = weights.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let default = mat("grass");
    let mut materials = [default.clone(), default.clone(), default.clone(), default];
    let mut w = [0.0; 4];
    for (i, (mat_key, wt)) in ranked.into_iter().take(4).enumerate() {
        materials[i] = mat_key;
        w[i] = wt;
    }
    SurfaceMaterialBlend { materials, weights: w }.normalize()
}

fn classify_cliff(c: &SurfaceContext) -> SurfaceMaterialBlend {
    let moss = smoothstep(0.45, 0.95, c.moisture)
        * (1.0 - smoothstep(70.0, 88.0, c.slope_degrees));
    let fresh_rock = smoothstep(68.0, 88.0, c.slope_degrees);

    SurfaceMaterialBlend {
        materials: [
            mat("rock"),
            mat("rock"),
            mat("forest_floor"),
            mat("grass"),
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
            mat("sand"),
            mat("wet_rock"),
            mat("rock"),
            mat("grass"),
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
            mat("sand"),
            mat("wet_rock"),
            mat("grass"),
            mat("forest_floor"),
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
            mat("cave_stone"),
            mat("flowstone"),
            mat("forest_floor"),
            mat("limestone"),
        ],
        weights: [
            (1.0 - flowstone) * (1.0 - limestone_w),
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

    let default_blend = SurfaceMaterialBlend {
        materials: [
            mat("grass"),
            mat("grass"),
            mat("forest_floor"),
            mat("rock"),
        ],
        weights: [
            forest * (1.0 - litter) + scrub * 0.3,
            grass + scrub * 0.5,
            moss + litter * forest,
            exposed_rock + alpine * 0.2,
        ],
    }
    .normalize();

    let alpine_blend = SurfaceMaterialBlend {
        materials: [
            mat("rock"),
            mat("rock"),
            mat("forest_floor"),
            mat("grass"),
        ],
        weights: [
            alpine * 0.5,
            alpine * 0.35 + exposed_rock,
            moss * 0.2,
            grass * 0.1,
        ],
    }
    .normalize();

    let wetland_blend = SurfaceMaterialBlend {
        materials: [
            mat("wet_rock"),
            mat("forest_floor"),
            mat("sand"),
            mat("grass"),
        ],
        weights: [
            wetland * 0.55,
            moss * 0.25,
            wetland * 0.2,
            forest * 0.15,
        ],
    }
    .normalize();

    let alpine_gate = smoothstep(0.45, 0.65, alpine);
    let wetland_gate = smoothstep(0.15, 0.35, wetland) * (1.0 - alpine_gate);

    weighted_blend(&[
        ((1.0 - alpine_gate) * (1.0 - wetland_gate), default_blend),
        (alpine_gate, alpine_blend),
        (wetland_gate, wetland_blend),
    ])
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
    fn cliff_includes_rock() {
        let classifier = IslandSurfaceClassifier;
        let mut ctx = base_context();
        ctx.slope_degrees = 72.0;
        ctx.moisture = 0.2;
        ctx.soil_depth_m = 0.1;
        let result = classifier.classify(&ctx);
        validate_blend(&result);
        assert!(result.materials.iter().any(|m| m.as_str() == "rock"));
    }

    #[test]
    fn coast_uses_sand() {
        let classifier = IslandSurfaceClassifier;
        let mut ctx = base_context();
        ctx.coast_distance_m = 5.0;
        ctx.elevation_m = 1.0;
        let result = classifier.classify(&ctx);
        validate_blend(&result);
        assert!(result.materials.iter().any(|m| m.as_str() == "sand"));
    }

    #[test]
    fn cave_limestone_uses_distinct_slot() {
        let classifier = IslandSurfaceClassifier;
        let mut ctx = base_context();
        ctx.cave_exposure = 0.8;
        ctx.geology = GeologyId::Limestone;
        let result = classifier.classify(&ctx);
        validate_blend(&result);
        assert!(result.materials.iter().any(|m| m.as_str() == "limestone"));
    }
}
