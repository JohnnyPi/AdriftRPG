// crates/game_bevy/src/environment/biome_context.rs
use crate::ui::WaterPhysicsTweaks;
use shared::smoothstep;
use terrain_generation::{CAVITY_EXTERIOR_MARGIN, RecipeDensitySource, ValueNoise, cavity_sdf_at};

/// Slope above which exposed rock replaces the biome default surface material.
pub const ROCK_SLOPE_DEG: f32 = 35.0;

const ELEVATION_COOLING: f32 = 0.02;
const BASE_TEMPERATURE: f32 = 0.65;
const COAST_HUMIDITY_SCALE: f32 = 60.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiomeSampleContext {
    pub world_y: f32,
    /// Surface elevation above sea level at this XZ column (not sample `world_y`).
    pub elevation: f32,
    pub slope_degrees: f32,
    pub distance_to_water: f32,
    pub distance_to_river: f32,
    pub cave_depth: f32,
    pub cave_exposure: f32,
    pub moisture: f32,
    pub effective_moisture: f32,
    pub transition_noise: f32,
    pub temperature: f32,
    pub continentalness: f32,
    pub coast_humidity: f32,
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
}

/// Per-chunk XZ cache so voxel material assignment does not call `surface_height_at` per voxel.
#[derive(Clone)]
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
                heights[lz * side + lx] = source.column_surface_height_at(wx as f32, wz as f32);
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

    pub fn context_at(
        &self,
        source: &RecipeDensitySource,
        wx: i32,
        y: f32,
        wz: i32,
    ) -> BiomeSampleContext {
        let column = self.column(wx, wz);
        context_from_column(source, &column, wx as f32, y, wz as f32)
    }
}

impl BiomeSampleContext {
    pub fn sample(source: &RecipeDensitySource, x: f32, y: f32, z: f32) -> Self {
        let surface_y = source.column_surface_height_at(x, z);
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
        context_from_column(source, &column, x, y, z)
    }

    pub fn is_underwater(&self) -> bool {
        self.world_y < self.sea_level_m + WaterPhysicsTweaks::DEFAULT_SHALLOW_DEPTH_M
    }

    pub fn is_cave(&self) -> bool {
        self.cave_exposure > 0.55
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
            cave_exposure: 0.0,
            moisture: 0.5,
            effective_moisture: 0.5,
            transition_noise: 0.5,
            temperature: 0.5,
            continentalness: 0.5,
            coast_humidity: 0.1,
            sea_level_m: 2.0,
        }
    }
}

fn climate_noise_scales(source: &RecipeDensitySource) -> (f32, f32, f32) {
    let feature_wavelength = source.climate_extent_m() / 3.0;
    let moisture_scale = 1.0 / feature_wavelength.max(32.0);
    (moisture_scale, moisture_scale * 0.4, moisture_scale * 1.5)
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
    let (moisture_scale, continental_scale, transition_scale) = climate_noise_scales(source);
    let mut moisture = noise.fbm(
        recipe_x * moisture_scale,
        0.0,
        recipe_z * moisture_scale,
        3,
        2.0,
        0.5,
    );
    let continentalness = noise.fbm(
        recipe_x * continental_scale,
        0.0,
        recipe_z * continental_scale,
        2,
        2.0,
        0.5,
    );
    let transition_noise = noise.sample(
        recipe_x * transition_scale,
        0.0,
        recipe_z * transition_scale,
    );
    let coast_humidity = (-distance_to_water / COAST_HUMIDITY_SCALE).exp() * 0.22;

    if let Some(atlas) = source.atlas() {
        let wetness =
            terrain_surface::normalize_wetness(atlas.sample_wetness(x, z), atlas.max_wetness());
        moisture = (moisture * 0.45 + wetness * 0.55).clamp(0.0, 1.0);
    }

    let effective_moisture = (moisture + coast_humidity + continentalness * 0.08).clamp(0.0, 1.0);
    let elevation = surface_y - source.recipe().sea_level;
    let temperature = (BASE_TEMPERATURE - elevation * ELEVATION_COOLING + transition_noise * 0.08)
        .clamp(0.0, 1.0);
    let _ = (slope_degrees, distance_to_river);

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
    }
}

fn context_from_column(
    source: &RecipeDensitySource,
    column: &ColumnClimate,
    wx: f32,
    y: f32,
    wz: f32,
) -> BiomeSampleContext {
    let sea_level = source.recipe().sea_level;
    let elevation = column.surface_y - sea_level;
    let cave_depth = (column.surface_y - y).max(0.0);
    let recipe = source.recipe();
    let cavity = cavity_sdf_at(
        recipe,
        wx + recipe.coord_offset[0],
        y + recipe.coord_offset[1],
        wz + recipe.coord_offset[2],
    );
    let declared_cave = smoothstep(0.0, CAVITY_EXTERIOR_MARGIN, -cavity);
    let cave_exposure = declared_cave;

    BiomeSampleContext {
        world_y: y,
        elevation,
        slope_degrees: column.slope_degrees,
        distance_to_water: column.distance_to_water,
        distance_to_river: column.distance_to_river,
        cave_depth,
        cave_exposure,
        moisture: column.moisture,
        effective_moisture: column.effective_moisture,
        transition_noise: column.transition_noise,
        temperature: column.temperature,
        continentalness: column.continentalness,
        coast_humidity: column.coast_humidity,
        sea_level_m: sea_level,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terrain_generation::{
        IslandGenParams, RecipeDensitySource, build_island_atlas, default_vertical_slice_recipe,
    };

    #[test]
    fn vs3_moisture_has_no_expanded_ridge_shadow_blob() {
        let source = RecipeDensitySource::new(default_vertical_slice_recipe(48_129, 2.0))
            .with_atlas(build_island_atlas(&IslandGenParams::default()), 3.5);
        let noise = ValueNoise::new(source.recipe().seed);
        let center = sample_climate(&source, &noise, 180.0, 196.0, 12.0, 5.0, 40.0, 80.0);
        let mut neighbors = Vec::new();
        for (dx, dz) in [(-24.0, 0.0), (24.0, 0.0), (0.0, -24.0), (0.0, 24.0)] {
            neighbors.push(sample_climate(
                &source,
                &noise,
                180.0 + dx,
                196.0 + dz,
                12.0,
                5.0,
                40.0,
                80.0,
            ));
        }
        let neighbor_mean =
            neighbors.iter().map(|c| c.effective_moisture).sum::<f32>() / neighbors.len() as f32;
        assert!(
            center.effective_moisture + 0.12 >= neighbor_mean,
            "expanded ridge coords should not be a moisture sink (center={}, mean={neighbor_mean})",
            center.effective_moisture
        );
    }

    #[test]
    fn effective_moisture_spans_island_land() {
        let source = RecipeDensitySource::new(default_vertical_slice_recipe(48_129, 2.0))
            .with_atlas(build_island_atlas(&IslandGenParams::default()), 3.5);
        let atlas = source.atlas().expect("atlas");
        let spacing = atlas.spacing_m();
        let mut min_m = f32::MAX;
        let mut max_m = f32::MIN;
        for z in (0..atlas.height()).step_by(2) {
            for x in (0..atlas.width()).step_by(2) {
                let wx = atlas.origin[0] + x as f32 * spacing;
                let wz = atlas.origin[1] + z as f32 * spacing;
                if atlas.island_mask.sample_bilinear(wx, wz) < 0.4 {
                    continue;
                }
                let ctx = BiomeSampleContext::sample(&source, wx, 10.0, wz);
                min_m = min_m.min(ctx.effective_moisture);
                max_m = max_m.max(ctx.effective_moisture);
            }
        }
        assert!(
            max_m - min_m >= 0.35,
            "land moisture range too narrow: min={min_m} max={max_m}"
        );
    }
}
