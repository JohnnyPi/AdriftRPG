//! Runtime-facing density sampling contract.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::coordinates::{WorldPosition, WorldXZ};
use super::metadata::WorldMetadata;
use crate::biomes::id::BiomeBlendCell;
use crate::geology::material::BedrockId;

/// Density convention: negative = solid, zero = surface, positive = air/water.
pub fn surface_density(position: WorldPosition, surface_height_m: f32) -> f32 {
    position.0.y as f32 - surface_height_m
}

/// Surface properties at a horizontal location.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct SurfaceSample {
    pub elevation_m: f32,
    pub slope: f32,
    pub macro_normal: Vec3,
    pub land_mask: f32,
    pub coast_distance_m: f32,
    pub island_id: Option<u32>,
}

/// Geology properties at a world position.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct GeologySample {
    pub bedrock: BedrockId,
    pub hardness: f32,
    pub erodibility: f32,
    pub permeability: f32,
    pub volcanic_age: f32,
    pub fracture_intensity: f32,
}

/// Vertical column summary for materialization.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ColumnSample {
    pub surface: SurfaceSample,
    pub regolith_depth_m: f32,
    pub weathering_depth_m: f32,
    pub base_bedrock: BedrockId,
    pub temperature: f32,
    pub rainfall: f32,
    pub humidity: f32,
    pub wetness: f32,
    pub soil_depth_m: f32,
    pub wave_exposure: f32,
    pub primary_biome: u8,
}

/// Runtime interface — hides all compiler internals.
pub trait WorldDensityProvider: Send + Sync + 'static {
    fn world_metadata(&self) -> &WorldMetadata;

    fn sample_density(&self, position: WorldPosition) -> f32;

    fn sample_surface(&self, horizontal: WorldXZ) -> SurfaceSample;

    fn sample_geology(&self, position: WorldPosition) -> GeologySample;

    fn sample_column(&self, horizontal: WorldXZ) -> ColumnSample;

    fn primary_river(&self) -> Option<&crate::water_body::RiverSpline> {
        None
    }

    /// Baked biome blend from worldgen compilation, when available.
    fn sample_biome_blend(&self, _horizontal: WorldXZ) -> Option<BiomeBlendCell> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_sign_convention() {
        let below = surface_density(WorldPosition::new(0.0, 10.0, 0.0), 20.0);
        let at = surface_density(WorldPosition::new(0.0, 20.0, 0.0), 20.0);
        let above = surface_density(WorldPosition::new(0.0, 30.0, 0.0), 20.0);
        assert!(below < 0.0);
        assert_eq!(at, 0.0);
        assert!(above > 0.0);
    }
}
