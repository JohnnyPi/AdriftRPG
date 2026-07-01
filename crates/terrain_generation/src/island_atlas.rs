//! Aligned island-scale field products (VS3 §2).

use crate::field2d::Field2D;
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

#[derive(Clone, Debug)]
pub struct IslandAtlas {
    pub resolution: GenerationResolution,
    pub seed: u64,
    pub sea_level_m: f32,
    /// Analytical micro-detail amplitude applied at voxel sample time.
    pub voxel_amplitude_m: f32,
    pub origin: [f32; 2],
    /// Absolute land elevation at regional tier spacing.
    pub elevation_regional: Field2D<f32>,
    /// Residual detail at local tier spacing (local noise, river carve, beaches).
    pub elevation_local: Field2D<f32>,
    pub bathymetry: Field2D<f32>,
    pub island_mask: Field2D<f32>,
    pub slope: Field2D<f32>,
    pub coast_distance: Field2D<f32>,
    pub filled_elevation: Field2D<f32>,
    pub flow_direction: Field2D<u8>,
    pub flow_accumulation: Field2D<f32>,
    pub river_mask: Field2D<f32>,
    pub wetness: Field2D<f32>,
    pub sediment: Field2D<f32>,
    pub cliff_mask: Field2D<f32>,
    pub beach_mask: Field2D<f32>,
    pub soil_depth: Field2D<f32>,
    pub biome_weights: Field2D<BiomeWeights>,
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

    pub fn surface_height_at(&self, x: f32, z: f32) -> f32 {
        let mask = self.island_mask.sample_bilinear(x, z);
        if mask > 0.01 {
            return self.composed_land_elevation_at(x, z);
        }
        let land_elev = self.composed_land_elevation_at(x, z);
        if land_elev > self.sea_level_m + 0.25 {
            return land_elev;
        }
        self.bathymetry.sample_bilinear(x, z)
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
}
