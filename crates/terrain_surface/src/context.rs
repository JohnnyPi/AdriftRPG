#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        let channels = [
            (BiomeId::Grassland, self.grassland),
            (BiomeId::Forest, self.forest),
            (BiomeId::Scrub, self.scrub),
            (BiomeId::CoastalScrub, self.coastal_scrub),
            (BiomeId::Wetland, self.wetland),
            (BiomeId::Beach, self.beach),
            (BiomeId::Alpine, self.alpine),
            (BiomeId::RockyUpland, self.rocky),
        ];
        channels
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(b, _)| b)
            .unwrap_or(BiomeId::Grassland)
    }
}

pub fn slope_degrees(normal: [f32; 3]) -> f32 {
    let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if len <= f32::EPSILON {
        return 0.0;
    }
    let ny = (normal[1] / len).clamp(-1.0, 1.0);
    ny.acos().to_degrees()
}

#[inline]
pub fn saturate(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

#[inline]
pub fn smoothstep(start: f32, end: f32, value: f32) -> f32 {
    if (end - start).abs() < f32::EPSILON {
        return if value >= end { 1.0 } else { 0.0 };
    }
    let t = saturate((value - start) / (end - start));
    t * t * (3.0 - 2.0 * t)
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

    let alpine = range_weight(sample.elevation, 24.0, 60.0, 8.0)
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

fn range_weight(value: f32, min: f32, max: f32, fade: f32) -> f32 {
    smoothstep(min - fade, min, value) * (1.0 - smoothstep(max, max + fade, value))
}
