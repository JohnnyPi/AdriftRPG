// crates/terrain_surface/src/context.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BiomeId {
    Grassland,
    Forest,
    Scrub,
    CoastalScrub,
    Wetland,
    Beach,
    Alpine,
    RockyUpland,
    Cave,
    Riverbank,
    ShallowWater,
    DeepWater,
    OffshoreShelf,
}

impl BiomeId {
    /// YAML biome rule id (`biomes.expanded_slice` and friends).
    pub const fn as_rule_id(self) -> &'static str {
        match self {
            BiomeId::Grassland => "grassland",
            BiomeId::Forest => "forest",
            BiomeId::Scrub => "scrub",
            BiomeId::CoastalScrub => "coastal_scrub",
            BiomeId::Wetland => "wetland",
            BiomeId::Beach => "beach",
            BiomeId::Alpine => "mountain_alpine",
            BiomeId::RockyUpland => "rocky_upland",
            BiomeId::Cave => "cave",
            BiomeId::Riverbank => "riverbank",
            BiomeId::ShallowWater => "shallow_water",
            BiomeId::DeepWater => "deep_water",
            BiomeId::OffshoreShelf => "offshore_shelf",
        }
    }

    /// Parse a biome rule id (snake_case or PascalCase surface-rule names).
    pub fn from_rule_id(id: &str) -> Self {
        match id {
            "grassland" | "Grassland" => BiomeId::Grassland,
            "forest" | "Forest" => BiomeId::Forest,
            "scrub" | "Scrub" => BiomeId::Scrub,
            "coastal_scrub" | "CoastalScrub" => BiomeId::CoastalScrub,
            "wetland" | "Wetland" => BiomeId::Wetland,
            "beach" | "Beach" => BiomeId::Beach,
            "mountain_alpine" | "Alpine" => BiomeId::Alpine,
            "rocky_upland" | "RockyUpland" => BiomeId::RockyUpland,
            "cave" | "Cave" => BiomeId::Cave,
            "riverbank" | "Riverbank" => BiomeId::Riverbank,
            "shallow_water" | "ShallowWater" => BiomeId::ShallowWater,
            "deep_water" | "DeepWater" => BiomeId::DeepWater,
            "offshore_shelf" | "OffshoreShelf" => BiomeId::OffshoreShelf,
            _ => BiomeId::Grassland,
        }
    }

    /// Legacy voxel material slot used before YAML biome catalogs.
    pub fn from_material_id(material_id: u16) -> Self {
        match material_id {
            1 => BiomeId::Beach,
            2 => BiomeId::RockyUpland,
            3 => BiomeId::Cave,
            4 => BiomeId::Wetland,
            5 => BiomeId::Forest,
            6 => BiomeId::Scrub,
            _ => BiomeId::Grassland,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeologyId {
    Basalt,
    Limestone,
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceContext {
    pub world_position: [f32; 3],
    pub world_normal: [f32; 3],
    pub elevation_m: f32,
    pub sea_level_m: f32,
    pub water_depth_m: f32,
    pub slope_degrees: f32,
    pub moisture: f32,
    pub soil_depth_m: f32,
    pub coast_distance_m: f32,
    pub river_distance_m: f32,
    pub wave_exposure: f32,
    pub cave_exposure: f32,
    pub mineral_deposition: f32,
    pub biome: BiomeId,
    pub geology: GeologyId,
    pub soft: SoftBiomeWeights,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SoftBiomeWeights {
    pub grassland: f32,
    pub forest: f32,
    pub scrub: f32,
    pub coastal_scrub: f32,
    pub wetland: f32,
    pub beach: f32,
    pub alpine: f32,
    pub rocky: f32,
}

impl SoftBiomeWeights {
    pub fn normalize(mut self) -> Self {
        let sum = self.grassland
            + self.forest
            + self.scrub
            + self.coastal_scrub
            + self.wetland
            + self.beach
            + self.alpine
            + self.rocky;
        if sum <= f32::EPSILON {
            self.grassland = 1.0;
            return self;
        }
        self.grassland /= sum;
        self.forest /= sum;
        self.scrub /= sum;
        self.coastal_scrub /= sum;
        self.wetland /= sum;
        self.beach /= sum;
        self.alpine /= sum;
        self.rocky /= sum;
        self
    }

    pub fn primary_biome(&self) -> BiomeId {
        self.weighted_biomes()
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(b, _)| b)
            .unwrap_or(BiomeId::Grassland)
    }

    /// Soft-weight channels in the order used for biome tint blending.
    pub fn weighted_biomes(self) -> [(BiomeId, f32); 8] {
        [
            (BiomeId::Grassland, self.grassland),
            (BiomeId::Forest, self.forest),
            (BiomeId::Scrub, self.scrub),
            (BiomeId::CoastalScrub, self.coastal_scrub),
            (BiomeId::Wetland, self.wetland),
            (BiomeId::Beach, self.beach),
            (BiomeId::Alpine, self.alpine),
            (BiomeId::RockyUpland, self.rocky),
        ]
    }
}

pub use shared::math::{range_weight, saturate, slope_degrees, smoothstep};

const DEFAULT_WETNESS_NORMALIZATION: f32 = 600.0;

/// Normalize a raw wetness sample from an island atlas to `[0, 1]`.
pub fn normalize_wetness(raw_wetness: f32, max_wetness: f32) -> f32 {
    (raw_wetness / max_wetness.max(1.0)).clamp(0.0, 1.0)
}

/// Default wetness normalization when no atlas max is available.
pub fn default_wetness_normalization() -> f32 {
    DEFAULT_WETNESS_NORMALIZATION
}

/// Environmental sample used to compute soft biome weights (Bevy-independent).
#[derive(Clone, Copy, Debug)]
pub struct EnvironmentSample {
    pub elevation: f32,
    pub slope_degrees: f32,
    pub moisture: f32,
    pub effective_moisture: f32,
    pub transition_noise: f32,
    pub temperature: f32,
    pub distance_to_water: f32,
    pub distance_to_river: f32,
    pub cave_depth: f32,
    pub world_y: f32,
}

pub fn compute_soft_biome_weights(sample: &EnvironmentSample) -> SoftBiomeWeights {
    let noise = (sample.transition_noise - 0.5) * 0.16;
    let moisture = sample.effective_moisture + noise;

    let forest = range_weight(moisture, 0.52 + noise, 0.95, 0.14)
        * range_weight(sample.elevation, 4.0, 18.0, 4.0)
        * (1.0 - smoothstep(22.0, 32.0, sample.slope_degrees));

    let grassland = range_weight(moisture, 0.12 + noise, 0.48, 0.12)
        * range_weight(sample.elevation, 2.0, 16.0, 3.0)
        * (1.0 - smoothstep(28.0, 40.0, sample.slope_degrees));

    let scrub = range_weight(moisture, 0.32 + noise, 0.58, 0.10)
        * range_weight(sample.elevation, 3.0, 14.0, 3.0)
        * (1.0 - smoothstep(20.0, 30.0, sample.slope_degrees));

    let coastal_scrub = range_weight(sample.distance_to_water, 0.0, 80.0, 20.0)
        * range_weight(sample.elevation, 2.0, 16.0, 3.0)
        * range_weight(moisture, 0.28, 0.65, 0.12);

    let wetland = range_weight(moisture, 0.58 + noise, 1.0, 0.12)
        * range_weight(sample.elevation, -1.0, 5.0, 2.0)
        * (1.0 - smoothstep(12.0, 22.0, sample.slope_degrees))
        * smoothstep(0.0, 12.0, 8.0 - sample.distance_to_river.min(12.0));

    let beach = range_weight(sample.distance_to_water, 0.0, 35.0, 12.0)
        * range_weight(sample.elevation, -1.0, 14.0, 3.0);

    let alpine = range_weight(sample.elevation, 28.0, 60.0, 8.0)
        * smoothstep(10.0, 22.0, sample.slope_degrees);

    let rocky = smoothstep(18.0, 38.0, sample.slope_degrees)
        * range_weight(sample.elevation, 8.0, 40.0, 10.0)
        * (1.0 - saturate(sample.effective_moisture));

    SoftBiomeWeights {
        grassland,
        forest,
        scrub,
        coastal_scrub,
        wetland,
        beach,
        alpine,
        rocky,
    }
    .normalize()
}

/// Blend runtime soft biome weights with baked atlas biome field weights.
pub fn merge_soft_with_atlas(
    climate: SoftBiomeWeights,
    atlas: terrain_generation::BiomeWeights,
    mix: f32,
) -> SoftBiomeWeights {
    let t = mix.clamp(0.0, 1.0);
    SoftBiomeWeights {
        grassland: climate.grassland * (1.0 - t) + atlas.grassland * t,
        forest: climate.forest * (1.0 - t) + atlas.rainforest * t,
        scrub: climate.scrub * (1.0 - t) + (atlas.volcanic_rock * 0.4 + atlas.grassland * 0.2) * t,
        coastal_scrub: climate.coastal_scrub * (1.0 - t)
            + (atlas.beach * 0.55 + atlas.grassland * 0.15) * t,
        wetland: climate.wetland * (1.0 - t) + atlas.wetland * t,
        beach: climate.beach * (1.0 - t) + atlas.beach * t,
        alpine: climate.alpine * (1.0 - t) + atlas.volcanic_rock * 0.5 * t,
        rocky: climate.rocky * (1.0 - t) + atlas.volcanic_rock * 0.55 * t,
    }
    .normalize()
}

#[cfg(test)]
mod biome_id_tests {
    use super::BiomeId;

    #[test]
    fn rule_id_round_trip_for_all_variants() {
        let all = [
            BiomeId::Grassland,
            BiomeId::Forest,
            BiomeId::Scrub,
            BiomeId::CoastalScrub,
            BiomeId::Wetland,
            BiomeId::Beach,
            BiomeId::Alpine,
            BiomeId::RockyUpland,
            BiomeId::Cave,
            BiomeId::Riverbank,
            BiomeId::ShallowWater,
            BiomeId::DeepWater,
            BiomeId::OffshoreShelf,
        ];
        for biome in all {
            assert_eq!(
                BiomeId::from_rule_id(biome.as_rule_id()),
                biome,
                "round-trip failed for {biome:?}"
            );
        }
    }

    #[test]
    fn from_rule_id_accepts_pascal_case_surface_rules() {
        assert_eq!(BiomeId::from_rule_id("CoastalScrub"), BiomeId::CoastalScrub);
        assert_eq!(BiomeId::from_rule_id("RockyUpland"), BiomeId::RockyUpland);
    }
}
