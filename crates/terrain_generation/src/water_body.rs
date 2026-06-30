//! Water body model and queries (VS2 §7).

use std::collections::BTreeMap;

use shared::StableId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WaterBodyId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaterBodyKind {
    Sea,
    Lake,
    Pond,
    River,
    Spring,
    Waterfall,
    CavePool,
}

#[derive(Clone, Debug)]
pub enum WaterSurfaceDefinition {
    Horizontal {
        elevation: f32,
    },
    SplineRibbon {
        control_points: Vec<RiverControlPoint>,
    },
}

#[derive(Clone, Debug)]
pub struct WaterBody {
    pub id: WaterBodyId,
    pub stable_id: StableId,
    pub kind: WaterBodyKind,
    pub surface: WaterSurfaceDefinition,
    pub material_id: StableId,
}

#[derive(Clone, Debug)]
pub struct RiverControlPoint {
    pub position_xz: [f32; 2],
    pub bed_elevation: f32,
    pub water_elevation: f32,
    pub width: f32,
    pub depth: f32,
    pub discharge: f32,
}

#[derive(Clone, Debug)]
pub struct RiverSpline {
    pub points: Vec<RiverControlPoint>,
}

#[derive(Clone, Debug)]
pub struct WaterSample {
    pub body: WaterBodyId,
    pub surface_height: f32,
    pub depth: f32,
    pub flow_velocity: [f32; 3],
    pub kind: WaterBodyKind,
}

pub trait WaterQuery: Send + Sync {
    fn water_at(&self, point: [f32; 3]) -> Option<WaterSample>;
    fn surface_height_at(&self, position_xz: [f32; 2]) -> Option<f32>;
}

#[derive(Clone, Debug, Default)]
pub struct WaterBodyRegistry {
    pub bodies: BTreeMap<WaterBodyId, WaterBody>,
    pub sea_level_m: f32,
}

impl WaterBodyRegistry {
    pub fn demo_registry(sea_level: f32, pool_elevation: f32) -> Self {
        let mut bodies = BTreeMap::new();
        bodies.insert(
            WaterBodyId(1),
            WaterBody {
                id: WaterBodyId(1),
                stable_id: StableId::new("water.sea"),
                kind: WaterBodyKind::Sea,
                surface: WaterSurfaceDefinition::Horizontal {
                    elevation: sea_level,
                },
                material_id: StableId::new("water.tropical_shallow"),
            },
        );
        bodies.insert(
            WaterBodyId(2),
            WaterBody {
                id: WaterBodyId(2),
                stable_id: StableId::new("water.upland_pool"),
                kind: WaterBodyKind::Lake,
                surface: WaterSurfaceDefinition::Horizontal {
                    elevation: pool_elevation,
                },
                material_id: StableId::new("water.freshwater"),
            },
        );
        Self {
            bodies,
            sea_level_m: sea_level,
        }
    }
}

impl WaterQuery for WaterBodyRegistry {
    fn water_at(&self, point: [f32; 3]) -> Option<WaterSample> {
        let mut best: Option<WaterSample> = None;
        for body in self.bodies.values() {
            if let WaterSurfaceDefinition::Horizontal { elevation } = body.surface {
                if point[1] < elevation {
                    let depth = elevation - point[1];
                    let sample = WaterSample {
                        body: body.id,
                        surface_height: elevation,
                        depth,
                        flow_velocity: [0.0, 0.0, 0.0],
                        kind: body.kind,
                    };
                    if best.as_ref().map(|b| b.depth).unwrap_or(f32::MAX) > depth {
                        best = Some(sample);
                    }
                }
            }
        }
        best
    }

    fn surface_height_at(&self, position_xz: [f32; 2]) -> Option<f32> {
        let _ = position_xz;
        Some(self.sea_level_m)
    }
}

#[cfg(test)]
mod water_tests {
    use super::*;

    #[test]
    fn sea_overlaps_lake_prefers_deeper_sample() {
        let registry = WaterBodyRegistry::demo_registry(2.0, 31.5);
        let sea_point = registry.water_at([128.0, 1.5, 128.0]).expect("sea");
        assert_eq!(sea_point.kind, WaterBodyKind::Sea);
        let lake_point = registry.water_at([82.0, 29.0, 196.0]).expect("lake");
        assert_eq!(lake_point.kind, WaterBodyKind::Lake);
        assert!(lake_point.depth > sea_point.depth);
    }

    #[test]
    fn underwater_camera_depth_from_sample() {
        let registry = WaterBodyRegistry::demo_registry(2.0, 31.5);
        let camera = [82.0, 30.5, 196.0];
        let sample = registry.water_at(camera).expect("submerged");
        assert!(sample.depth > 0.3);
        assert_eq!(sample.body, WaterBodyId(2));
    }
}
