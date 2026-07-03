// crates/terrain_generation/src/island_atlas.rs
//! Aligned island-scale field products (VS3 §2).

use crate::field2d::{smoothstep, Field2D};
use crate::resolution::GenerationResolution;
use crate::water_body::RiverSpline;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BiomeWeights {
    pub rainforest: f32,
    pub grassland: f32,
    pub volcanic_rock: f32,
    pub beach: f32,
    pub wetland: f32,
}

/// Island mask value below which a column is treated as pure ocean and takes
/// its height from bathymetry alone.
const MASK_OCEAN_MAX: f32 = 0.02;

/// Island mask value above which a column is treated as pure land and takes
/// its height from the composed land elevation alone. Between the two bounds
/// land and bathymetry are blended so the coastline transitions continuously.
///
/// This replaces the old hard branch (`mask > 0.01 -> land`) plus the
/// `land_elev > sea + 0.25` escape hatch: with ±regional-noise on the ocean
/// side, that escape hatch left patches of solid terrain floating exactly at
/// sea level offshore — the teal "crust" players could stand on.
const MASK_LAND_MIN: f32 = 0.60;

/// Island-scale field atlas. Each field records its tier (regional vs local spacing)
/// and epoch (post-erosion hydrology, post-beach slope, etc.) in doc comments below.
#[derive(Clone, Debug)]
pub struct IslandAtlas {
    pub resolution: GenerationResolution,
    pub seed: u64,
    pub sea_level_m: f32,
    /// Analytical micro-detail amplitude applied at voxel sample time.
    pub voxel_amplitude_m: f32,
    pub origin: [f32; 2],
    /// Regional tier, post-erosion absolute land elevation.
    pub elevation_regional: Field2D<f32>,
    /// Local tier residual (local noise, river carve, beaches) over `elevation_regional`.
    pub elevation_local: Field2D<f32>,
    /// Regional tier, from post-erosion coast distance.
    pub bathymetry: Field2D<f32>,
    /// Local tier, resampled island footprint mask.
    pub island_mask: Field2D<f32>,
    /// Local tier, post-beach slope degrees.
    pub slope: Field2D<f32>,
    /// Regional tier, two-sided distance to coastline.
    pub coast_distance: Field2D<f32>,
    /// Regional tier, priority-filled post-erosion elevation (hydrology epoch).
    pub filled_elevation: Field2D<f32>,
    /// Regional tier, D8 flow direction on `filled_elevation`.
    pub flow_direction: Field2D<u8>,
    /// Regional tier, flow accumulation on `filled_elevation`.
    pub flow_accumulation: Field2D<f32>,
    /// Regional tier, extracted from post-erosion accumulation.
    pub river_mask: Field2D<f32>,
    /// Local tier, resampled from post-erosion accumulation.
    pub wetness: Field2D<f32>,
    /// Regional tier, derived from post-erosion accumulation.
    pub sediment: Field2D<f32>,
    /// Local tier, post-beach cliff suitability.
    pub cliff_mask: Field2D<f32>,
    /// Local tier, post-beach beach suitability.
    pub beach_mask: Field2D<f32>,
    /// Local tier, post-beach soil depth from slope and sediment.
    pub soil_depth: Field2D<f32>,
    /// Local tier, post-beach biome weights.
    pub biome_weights: Field2D<BiomeWeights>,
    /// Primary river traced on post-erosion `filled_elevation`.
    pub river_graph: Option<RiverSpline>,
    pub validation_passed: bool,
    pub validation_messages: Vec<String>,
}

impl IslandAtlas {
    /// Finest rasterized spacing (local tier).
    pub fn spacing_m(&self) -> f32 {
        self.resolution.local_m
    }

    pub fn composed_land_elevation_at(&self, x: f32, z: f32) -> f32 {
        self.elevation_regional.sample_bilinear(x, z) + self.elevation_local.sample_bilinear(x, z)
    }

    /// Terrain height for this column: land elevation on the island interior,
    /// bathymetry in open ocean, and a mask-weighted blend of the two across
    /// the coastal fringe so the surface is continuous through the shoreline.
    pub fn surface_height_at(&self, x: f32, z: f32) -> f32 {
        let mask = self.island_mask.sample_bilinear(x, z).clamp(0.0, 1.0);
        if mask >= MASK_LAND_MIN {
            return self.composed_land_elevation_at(x, z);
        }
        let sea_floor = self.bathymetry.sample_bilinear(x, z);
        if mask <= MASK_OCEAN_MAX {
            return sea_floor;
        }
        let land = self.composed_land_elevation_at(x, z);
        let t = smoothstep(MASK_OCEAN_MAX, MASK_LAND_MIN, mask);
        sea_floor + (land - sea_floor) * t
    }

    pub fn slope_at(&self, x: f32, z: f32) -> f32 {
        self.slope.sample_bilinear(x, z)
    }

    pub fn width(&self) -> u32 {
        self.elevation_local.width
    }

    pub fn height(&self) -> u32 {
        self.elevation_local.height
    }

    pub fn sample_biome_weights(&self, wx: f32, wz: f32) -> BiomeWeights {
        sample_biome_weights_bilinear(&self.biome_weights, wx, wz)
    }

    pub fn sample_wetness(&self, wx: f32, wz: f32) -> f32 {
        self.wetness.sample_bilinear(wx, wz)
    }

    pub fn sample_soil_depth(&self, wx: f32, wz: f32) -> f32 {
        self.soil_depth.sample_bilinear(wx, wz)
    }

    pub fn sample_coast_distance(&self, wx: f32, wz: f32) -> f32 {
        self.coast_distance.sample_bilinear(wx, wz)
    }
}

fn sample_biome_weights_bilinear(field: &Field2D<BiomeWeights>, wx: f32, wz: f32) -> BiomeWeights {
    if field.width == 0 || field.height == 0 {
        return BiomeWeights::default();
    }
    let (lx, lz) = field.world_to_grid(wx, wz);
    let max_x = (field.width - 1) as f32;
    let max_z = (field.height - 1) as f32;
    let lx = lx.clamp(0.0, max_x);
    let lz = lz.clamp(0.0, max_z);
    let x0 = (lx.floor() as u32).min(field.width.saturating_sub(2));
    let z0 = (lz.floor() as u32).min(field.height.saturating_sub(2));
    let fx = lx - x0 as f32;
    let fz = lz - z0 as f32;
    let c00 = field.get(x0, z0);
    let c10 = field.get(x0 + 1, z0);
    let c01 = field.get(x0, z0 + 1);
    let c11 = field.get(x0 + 1, z0 + 1);
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let lerp_channel = |f: fn(BiomeWeights) -> f32| {
        let ab = lerp(f(c00), f(c10), fx);
        let cd = lerp(f(c01), f(c11), fx);
        lerp(ab, cd, fz)
    };
    BiomeWeights {
        rainforest: lerp_channel(|b| b.rainforest),
        grassland: lerp_channel(|b| b.grassland),
        volcanic_rock: lerp_channel(|b| b.volcanic_rock),
        beach: lerp_channel(|b| b.beach),
        wetland: lerp_channel(|b| b.wetland),
    }
}