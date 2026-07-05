//! Hydrology graph structures stored on the world atlas.

use serde::{Deserialize, Serialize};

use crate::water_body::RiverSpline;

#[derive(Clone, Debug, Default)]
pub struct HydrologyGraph {
    pub nodes: Vec<HydroNode>,
    pub lakes: Vec<LakeBasin>,
    pub wetlands: Vec<WetlandRegion>,
    pub waterfalls: Vec<WaterfallCandidate>,
    pub primary_river: Option<RiverSpline>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HydroNode {
    pub cell_x: u32,
    pub cell_z: u32,
    pub downstream: Option<usize>,
    pub drainage_area: f32,
    pub discharge: f32,
    pub stream_order: u8,
    pub sediment: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LakeBasin {
    pub cells: Vec<(u32, u32)>,
    pub surface_elevation_m: f32,
    pub area_cells: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WetlandRegion {
    pub cells: Vec<(u32, u32)>,
    pub moisture: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaterfallCandidate {
    pub from: (u32, u32),
    pub to: (u32, u32),
    pub drop_m: f32,
    pub discharge: f32,
}
