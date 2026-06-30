use terrain_generation::{RecipeDensitySource, ValueNoise};

/// Slope above which exposed rock replaces the biome default surface material.
pub const ROCK_SLOPE_DEG: f32 = 35.0;

const MOISTURE_SCALE: f32 = 0.001;
const TRANSITION_NOISE_SCALE: f32 = 0.0015;
const SLOPE_SAMPLE_EPS: f32 = 4.0;
const ELEVATION_COOLING: f32 = 0.02;
const BASE_TEMPERATURE: f32 = 0.65;
const CAVE_DEPTH_THRESHOLD: f32 = 2.0;
const SHALLOW_WATER_MARGIN: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiomeSampleContext {
    pub world_y: f32,
    pub elevation: f32,
    pub slope_degrees: f32,
    pub distance_to_water: f32,
    pub cave_depth: f32,
    pub moisture: f32,
    pub transition_noise: f32,
    pub temperature: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnClimate {
    pub surface_y: f32,
    pub slope_degrees: f32,
    pub distance_to_water: f32,
    pub moisture: f32,
    pub transition_noise: f32,
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
                heights[lz * side + lx] = source.surface_height_at(wx as f32, wz as f32);
            }
        }

        let noise = ValueNoise::new(source.recipe().seed);
        let mut columns = Vec::with_capacity(count);
        for lz in 0..side {
            for lx in 0..side {
                let wx = origin_x + lx as i32 - 1;
                let wz = origin_z + lz as i32 - 1;
                let surface_y = heights[lz * side + lx];
                let slope_degrees = slope_from_height_grid(&heights, side, lx, lz);
                let distance_to_water = source.distance_to_water_m(wx as f32, wz as f32);
                let moisture =
                    noise.fbm(wx as f32 * MOISTURE_SCALE, 0.0, wz as f32 * MOISTURE_SCALE, 3, 2.0, 0.5);
                let transition_noise = noise.sample(
                    wx as f32 * TRANSITION_NOISE_SCALE,
                    0.0,
                    wz as f32 * TRANSITION_NOISE_SCALE,
                );
                columns.push(ColumnClimate {
                    surface_y,
                    slope_degrees,
                    distance_to_water,
                    moisture,
                    transition_noise,
                });
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
        let column = ColumnClimate {
            surface_y: source.surface_height_at(x, z),
            slope_degrees: estimate_slope_deg(source, x, z),
            distance_to_water: source.distance_to_water_m(x, z),
            moisture: {
                let noise = ValueNoise::new(source.recipe().seed);
                noise.fbm(x * MOISTURE_SCALE, 0.0, z * MOISTURE_SCALE, 3, 2.0, 0.5)
            },
            transition_noise: {
                let noise = ValueNoise::new(source.recipe().seed);
                noise.sample(x * TRANSITION_NOISE_SCALE, 0.0, z * TRANSITION_NOISE_SCALE)
            },
        };
        context_from_column(source, &column, y)
    }

    pub fn is_underwater(&self) -> bool {
        self.elevation < SHALLOW_WATER_MARGIN
    }

    pub fn is_cave(&self) -> bool {
        self.cave_depth >= CAVE_DEPTH_THRESHOLD
    }
}

fn context_from_column(source: &RecipeDensitySource, column: &ColumnClimate, y: f32) -> BiomeSampleContext {
    let sea_level = source.recipe().sea_level;
    let elevation = y - sea_level;
    let cave_depth = (column.surface_y - y).max(0.0);
    let temperature = (BASE_TEMPERATURE - elevation * ELEVATION_COOLING + column.transition_noise * 0.08)
        .clamp(0.0, 1.0);

    BiomeSampleContext {
        world_y: y,
        elevation,
        slope_degrees: column.slope_degrees,
        distance_to_water: column.distance_to_water,
        cave_depth,
        moisture: column.moisture,
        transition_noise: column.transition_noise,
        temperature,
    }
}

fn slope_from_height_grid(heights: &[f32], side: usize, lx: usize, lz: usize) -> f32 {
    let hx = sample_height(heights, side, lx + 1, lz) - sample_height(heights, side, lx.saturating_sub(1), lz);
    let hz = sample_height(heights, side, lx, lz + 1) - sample_height(heights, side, lx, lz.saturating_sub(1));
    let gradient = (hx * hx + hz * hz).sqrt() / 2.0;
    gradient.atan().to_degrees()
}

fn sample_height(heights: &[f32], side: usize, lx: usize, lz: usize) -> f32 {
    let lx = lx.min(side - 1);
    let lz = lz.min(side - 1);
    heights[lz * side + lx]
}

fn estimate_slope_deg(source: &RecipeDensitySource, x: f32, z: f32) -> f32 {
    let hx = source.surface_height_at(x + SLOPE_SAMPLE_EPS, z)
        - source.surface_height_at(x - SLOPE_SAMPLE_EPS, z);
    let hz = source.surface_height_at(x, z + SLOPE_SAMPLE_EPS)
        - source.surface_height_at(x, z - SLOPE_SAMPLE_EPS);
    let gradient = (hx * hx + hz * hz).sqrt() / (2.0 * SLOPE_SAMPLE_EPS);
    gradient.atan().to_degrees()
}
