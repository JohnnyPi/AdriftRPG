// crates/terrain_generation/src/water_body.rs
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

#[derive(Clone, Copy, Debug)]
struct SplineRibbonSample {
    tangent: [f32; 2],
    water_elevation: f32,
    bed_elevation: f32,
    width: f32,
    discharge: f32,
    lateral_distance: f32,
}

fn spline_ribbon_sample(
    control_points: &[RiverControlPoint],
    position_xz: [f32; 2],
) -> Option<SplineRibbonSample> {
    if control_points.len() < 2 {
        return None;
    }
    let mut best: Option<SplineRibbonSample> = None;
    for segment in control_points.windows(2) {
        let start = &segment[0];
        let end = &segment[1];
        let ax = start.position_xz[0];
        let az = start.position_xz[1];
        let bx = end.position_xz[0];
        let bz = end.position_xz[1];
        let ab = [bx - ax, bz - az];
        let len_sq = ab[0] * ab[0] + ab[1] * ab[1];
        if len_sq <= f32::EPSILON {
            continue;
        }
        let ap = [position_xz[0] - ax, position_xz[1] - az];
        let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / len_sq).clamp(0.0, 1.0);
        let point = [ax + ab[0] * t, az + ab[1] * t];
        let lateral_dx = position_xz[0] - point[0];
        let lateral_dz = position_xz[1] - point[1];
        let lateral_distance = (lateral_dx * lateral_dx + lateral_dz * lateral_dz).sqrt();
        let tangent_len = len_sq.sqrt();
        let sample = SplineRibbonSample {
            tangent: [ab[0] / tangent_len, ab[1] / tangent_len],
            water_elevation: start.water_elevation
                + (end.water_elevation - start.water_elevation) * t,
            bed_elevation: start.bed_elevation + (end.bed_elevation - start.bed_elevation) * t,
            width: start.width + (end.width - start.width) * t,
            discharge: start.discharge + (end.discharge - start.discharge) * t,
            lateral_distance,
        };
        if best
            .as_ref()
            .is_none_or(|current| sample.lateral_distance < current.lateral_distance)
        {
            best = Some(sample);
        }
    }
    best
}

impl WaterQuery for WaterBodyRegistry {
    fn water_at(&self, point: [f32; 3]) -> Option<WaterSample> {
        let mut best: Option<WaterSample> = None;
        for body in self.bodies.values() {
            match &body.surface {
                WaterSurfaceDefinition::Horizontal { elevation } => {
                    if point[1] < *elevation {
                        let depth = *elevation - point[1];
                        let sample = WaterSample {
                            body: body.id,
                            surface_height: *elevation,
                            depth,
                            flow_velocity: [0.0, 0.0, 0.0],
                            kind: body.kind,
                        };
                        if best.as_ref().map(|b| b.depth).unwrap_or(f32::MAX) > depth {
                            best = Some(sample);
                        }
                    }
                }
                WaterSurfaceDefinition::SplineRibbon { control_points } => {
                    let Some(ribbon) = spline_ribbon_sample(control_points, [point[0], point[2]])
                    else {
                        continue;
                    };
                    let half_width = ribbon.width.max(0.1) * 0.5;
                    if ribbon.lateral_distance > half_width || point[1] >= ribbon.water_elevation {
                        continue;
                    }
                    let depth = ribbon.water_elevation - point[1];
                    let area = (ribbon.width
                        * (ribbon.water_elevation - ribbon.bed_elevation).max(0.1))
                    .max(0.1);
                    let speed = (ribbon.discharge / area).abs();
                    let sample = WaterSample {
                        body: body.id,
                        surface_height: ribbon.water_elevation,
                        depth,
                        flow_velocity: [ribbon.tangent[0] * speed, 0.0, ribbon.tangent[1] * speed],
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
        let mut best = Some(self.sea_level_m);
        for body in self.bodies.values() {
            if let WaterSurfaceDefinition::SplineRibbon { control_points } = &body.surface {
                let Some(ribbon) = spline_ribbon_sample(control_points, position_xz) else {
                    continue;
                };
                if ribbon.lateral_distance <= ribbon.width.max(0.1) * 0.5 {
                    best = Some(best.map_or(ribbon.water_elevation, |current| {
                        current.max(ribbon.water_elevation)
                    }));
                }
            }
        }
        best
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

    #[test]
    fn spline_ribbon_bodies_are_queryable() {
        let mut registry = WaterBodyRegistry::demo_registry(2.0, 31.5);
        registry.bodies.insert(
            WaterBodyId(3),
            WaterBody {
                id: WaterBodyId(3),
                stable_id: StableId::new("water.river.test"),
                kind: WaterBodyKind::River,
                surface: WaterSurfaceDefinition::SplineRibbon {
                    control_points: vec![
                        RiverControlPoint {
                            position_xz: [0.0, 0.0],
                            bed_elevation: 0.5,
                            water_elevation: 1.5,
                            width: 4.0,
                            depth: 1.0,
                            discharge: 2.0,
                        },
                        RiverControlPoint {
                            position_xz: [8.0, 0.0],
                            bed_elevation: 0.2,
                            water_elevation: 1.2,
                            width: 5.0,
                            depth: 1.0,
                            discharge: 2.4,
                        },
                    ],
                },
                material_id: StableId::new("water.river"),
            },
        );

        let sample = registry.water_at([4.0, 1.0, 0.5]).expect("river sample");
        assert_eq!(sample.kind, WaterBodyKind::River);
        assert_eq!(sample.body, WaterBodyId(3));
        assert!(sample.depth > 0.1);
        assert!(sample.flow_velocity[0].abs() > 0.01);
        assert!(registry.surface_height_at([4.0, 0.5]).unwrap() > 1.0);
    }
}
