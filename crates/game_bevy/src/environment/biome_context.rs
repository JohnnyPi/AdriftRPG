use terrain_generation::{RecipeDensitySource, ValueNoise};

/// Slope above which exposed rock replaces the biome default surface material.
pub const ROCK_SLOPE_DEG: f32 = 35.0;

const MOISTURE_SCALE: f32 = 0.001;
const CONTINENTAL_SCALE: f32 = 0.0004;
const TRANSITION_NOISE_SCALE: f32 = 0.0015;
const ELEVATION_COOLING: f32 = 0.02;
const BASE_TEMPERATURE: f32 = 0.65;
const CAVE_DEPTH_THRESHOLD: f32 = 2.0;
const SHALLOW_WATER_MARGIN: f32 = 1.5;
const COAST_HUMIDITY_SCALE: f32 = 60.0;
const RAIN_SHADOW_RIDGE: [f32; 2] = [180.0, 196.0];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiomeSampleContext {
    pub world_y: f32,
    /// Surface elevation above sea level at this XZ column (not sample `world_y`).
    pub elevation: f32,
    pub slope_degrees: f32,
    pub distance_to_water: f32,
    pub distance_to_river: f32,
    pub cave_depth: f32,
    pub moisture: f32,
    pub effective_moisture: f32,
    pub transition_noise: f32,
    pub temperature: f32,
    pub continentalness: f32,
    pub coast_humidity: f32,
    pub rain_shadow: f32,
    sea_level_m: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnClimate {
    pub surface_y: f32,
    pub slope_degrees: f32,
    pub distance_to_water: f32,
    pub distance_to_river: f32,
    pub moisture: f32,
    pub effective_moisture: f32,
    pub transition_noise: f32,
    pub temperature: f32,
    pub continentalness: f32,
    pub coast_humidity: f32,
    pub rain_shadow: f32,
}

/// Per-chunk XZ cache so voxel material assignment does not call `surface_height_at` per voxel.
pub struct ChunkColumnCache {
    origin_x: i32,
    origin_z: i32,
    side: usize,
    columns: Vec<ColumnClimate>,
}

impl ChunkColumnCache {
    pub fn build(source: &RecipeDensitySource, origin_x: i32, origin_z: i32, side: usize) -> Self {
        let count = side * side;
        let mut heights = vec![0.0; count];
        for lz in 0..side {
            for lx in 0..side {
                let wx = origin_x + lx as i32 - 1;
                let wz = origin_z + lz as i32 - 1;
                heights[lz * side + lx] = source.terrain_surface_height_at(wx as f32, wz as f32);
            }
        }

        let noise = ValueNoise::new(source.recipe().seed);
        let mut columns = Vec::with_capacity(count);
        for lz in 0..side {
            for lx in 0..side {
                let wx = origin_x + lx as i32 - 1;
                let wz = origin_z + lz as i32 - 1;
                let surface_y = heights[lz * side + lx];
                let slope_degrees = source.terrain_slope_at(wx as f32, wz as f32);
                let distance_to_water = source.distance_to_water_m(wx as f32, wz as f32);
                let distance_to_river = source.distance_to_river_m(wx as f32, wz as f32);
                let climate = sample_climate(
                    source,
                    &noise,
                    wx as f32,
                    wz as f32,
                    surface_y,
                    slope_degrees,
                    distance_to_water,
                    distance_to_river,
                );
                columns.push(climate);
            }
        }

        Self {
            origin_x,
            origin_z,
            side,
            columns,
        }
    }

    pub fn column(&self, wx: i32, wz: i32) -> ColumnClimate {
        let lx = (wx - self.origin_x + 1).clamp(0, self.side as i32 - 1) as usize;
        let lz = (wz - self.origin_z + 1).clamp(0, self.side as i32 - 1) as usize;
        self.columns[lz * self.side + lx]
    }

    pub fn context_at(&self, source: &RecipeDensitySource, wx: i32, y: f32, wz: i32) -> BiomeSampleContext {
        let column = self.column(wx, wz);
        context_from_column(source, &column, y)
    }
}

impl BiomeSampleContext {
    pub fn sample(source: &RecipeDensitySource, x: f32, y: f32, z: f32) -> Self {
        let surface_y = source.terrain_surface_height_at(x, z);
        let slope_degrees = source.terrain_slope_at(x, z);
        let distance_to_water = source.distance_to_water_m(x, z);
        let distance_to_river = source.distance_to_river_m(x, z);
        let noise = ValueNoise::new(source.recipe().seed);
        let column = sample_climate(
            source,
            &noise,
            x,
            z,
            surface_y,
            slope_degrees,
            distance_to_water,
            distance_to_river,
        );
        context_from_column(source, &column, y)
    }

    pub fn is_underwater(&self) -> bool {
        self.world_y < self.sea_level_m + SHALLOW_WATER_MARGIN
    }

    pub fn is_cave(&self) -> bool {
        self.cave_depth >= CAVE_DEPTH_THRESHOLD
    }

    #[cfg(test)]
    pub fn for_test(
        world_y: f32,
        elevation: f32,
        slope_degrees: f32,
        distance_to_water: f32,
    ) -> Self {
        Self {
            world_y,
            elevation,
            slope_degrees,
            distance_to_water,
            distance_to_river: f32::MAX,
            cave_depth: 0.0,
            moisture: 0.5,
            effective_moisture: 0.5,
            transition_noise: 0.5,
            temperature: 0.5,
            continentalness: 0.5,
            coast_humidity: 0.1,
            rain_shadow: 0.0,
            sea_level_m: 2.0,
        }
    }
}

fn sample_climate(
    source: &RecipeDensitySource,
    noise: &ValueNoise,
    x: f32,
    z: f32,
    surface_y: f32,
    slope_degrees: f32,
    distance_to_water: f32,
    distance_to_river: f32,
) -> ColumnClimate {
    let recipe_x = x + source.recipe().coord_offset[0];
    let recipe_z = z + source.recipe().coord_offset[2];
    let moisture = noise.fbm(
        recipe_x * MOISTURE_SCALE,
        0.0,
        recipe_z * MOISTURE_SCALE,
        3,
        2.0,
        0.5,
    );
    let continentalness = noise.fbm(
        recipe_x * CONTINENTAL_SCALE,
        0.0,
        recipe_z * CONTINENTAL_SCALE,
        2,
        2.0,
        0.5,
    );
    let transition_noise = noise.sample(
        recipe_x * TRANSITION_NOISE_SCALE,
        0.0,
        recipe_z * TRANSITION_NOISE_SCALE,
    );
    let coast_humidity = (-distance_to_water / COAST_HUMIDITY_SCALE).exp() * 0.22;
    let rain_shadow = rain_shadow_at(recipe_x, recipe_z);
    let effective_moisture =
        (moisture + coast_humidity - rain_shadow + continentalness * 0.08).clamp(0.0, 1.0);
    let elevation = surface_y - source.recipe().sea_level;
    let temperature = (BASE_TEMPERATURE - elevation * ELEVATION_COOLING + transition_noise * 0.08
        - rain_shadow * 0.15)
        .clamp(0.0, 1.0);

    ColumnClimate {
        surface_y,
        slope_degrees,
        distance_to_water,
        distance_to_river,
        moisture,
        effective_moisture,
        transition_noise,
        temperature,
        continentalness,
        coast_humidity,
        rain_shadow,
    }
}

fn rain_shadow_at(x: f32, z: f32) -> f32 {
    let dx = RAIN_SHADOW_RIDGE[0] - x;
    let dz = RAIN_SHADOW_RIDGE[1] - z;
    if dx <= 0.0 || dz <= 0.0 {
        return 0.0;
    }
    let dist = (dx * dx + dz * dz).sqrt();
    (1.0 - (dist / 80.0).min(1.0)) * 0.28
}

fn context_from_column(source: &RecipeDensitySource, column: &ColumnClimate, y: f32) -> BiomeSampleContext {
    let sea_level = source.recipe().sea_level;
    let elevation = column.surface_y - sea_level;
    let cave_depth = (column.surface_y - y).max(0.0);

    BiomeSampleContext {
        world_y: y,
        elevation,
        slope_degrees: column.slope_degrees,
        distance_to_water: column.distance_to_water,
        distance_to_river: column.distance_to_river,
        cave_depth,
        moisture: column.moisture,
        effective_moisture: column.effective_moisture,
        transition_noise: column.transition_noise,
        temperature: column.temperature,
        continentalness: column.continentalness,
        coast_humidity: column.coast_humidity,
        rain_shadow: column.rain_shadow,
        sea_level_m: sea_level,
    }
}
