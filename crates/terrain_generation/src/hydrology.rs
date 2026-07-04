// crates/terrain_generation/src/hydrology.rs
//! Hydrology backend with river spline support.

#[cfg(test)]
use crate::river::{RiverGenConfig, generate_river_spline};
use crate::water_body::{RiverSpline, WaterBodyRegistry};

pub trait HydrologyBackend: Send + Sync {
    fn rainfall_mm_per_hour(&self) -> f32 {
        0.0
    }

    fn river_spline(&self) -> Option<&RiverSpline> {
        None
    }

    fn water_registry(&self) -> Option<&WaterBodyRegistry> {
        None
    }
}

#[derive(Clone, Debug)]
pub struct RiverHydrology {
    pub river: Option<RiverSpline>,
    pub water: WaterBodyRegistry,
}

impl RiverHydrology {
    #[cfg(test)]
    pub fn generate_demo(seed: u64, sea_level: f32, pool_elevation: f32) -> Self {
        let mut config = RiverGenConfig::default();
        config.seed = seed;
        let river = generate_river_spline(&config, sea_level);
        let mut water = WaterBodyRegistry::demo_registry(sea_level, pool_elevation);
        if let Some(ref spline) = river {
            use crate::water_body::{
                WaterBody, WaterBodyId, WaterBodyKind, WaterSurfaceDefinition,
            };
            use shared::StableId;
            water.bodies.insert(
                WaterBodyId(3),
                WaterBody {
                    id: WaterBodyId(3),
                    stable_id: StableId::new("water.river.demo"),
                    kind: WaterBodyKind::River,
                    surface: WaterSurfaceDefinition::SplineRibbon {
                        control_points: spline.points.clone(),
                    },
                    material_id: StableId::new("water.river"),
                },
            );
        }
        Self { river, water }
    }
}

impl HydrologyBackend for RiverHydrology {
    fn river_spline(&self) -> Option<&RiverSpline> {
        self.river.as_ref()
    }

    fn water_registry(&self) -> Option<&WaterBodyRegistry> {
        Some(&self.water)
    }
}
