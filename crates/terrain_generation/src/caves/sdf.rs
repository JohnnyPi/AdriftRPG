//! Volumetric subtract operations derived from cave graphs.

use crate::contract::coordinates::WorldPosition;
use crate::density_ops::{capsule_sdf, ellipsoid_sdf, solid_subtract};

use super::graph::{CaveNodeKind, CaveSystem, WallNoiseParams};

#[derive(Clone, Debug)]
pub enum SubtractShape {
    Capsule {
        start: [f32; 3],
        end: [f32; 3],
        radius: f32,
        noise: WallNoiseParams,
    },
    Ellipsoid {
        center: [f32; 3],
        radii: [f32; 3],
        noise: WallNoiseParams,
    },
}

#[derive(Clone, Debug, Default)]
pub struct CaveSubtractOps {
    pub shapes: Vec<SubtractShape>,
}

impl CaveSubtractOps {
    pub fn from_systems(systems: &[CaveSystem]) -> Self {
        let mut shapes = Vec::new();
        for system in systems {
            shapes.extend(system_to_shapes(system));
        }
        Self { shapes }
    }

    pub fn sdf_at(&self, position: WorldPosition) -> f32 {
        let px = position.0.x as f32;
        let py = position.0.y as f32;
        let pz = position.0.z as f32;
        let mut min_sdf = f32::MAX;
        for shape in &self.shapes {
            let sdf = match shape {
                SubtractShape::Capsule {
                    start,
                    end,
                    radius,
                    noise,
                } => {
                    let base = capsule_sdf(
                        px, py, pz, start[0], start[1], start[2], end[0], end[1], end[2], *radius,
                    );
                    base + wall_perturb(px, py, pz, noise)
                }
                SubtractShape::Ellipsoid {
                    center,
                    radii,
                    noise,
                } => {
                    let base = ellipsoid_sdf(
                        px, py, pz, center[0], center[1], center[2], radii[0], radii[1], radii[2],
                    );
                    base + wall_perturb(px, py, pz, noise)
                }
            };
            min_sdf = min_sdf.min(sdf);
        }
        min_sdf
    }

    pub fn apply_subtract(&self, solid_density: f32, position: WorldPosition) -> f32 {
        let cavity = self.sdf_at(position);
        if cavity >= f32::MAX * 0.5 {
            return solid_density;
        }
        solid_subtract(solid_density, cavity)
    }
}

fn wall_perturb(px: f32, py: f32, pz: f32, noise: &WallNoiseParams) -> f32 {
    if noise.amplitude_m <= 0.0 {
        return 0.0;
    }
    let f = noise.frequency.max(0.01);
    let n = (px * f).sin() * (py * f * 1.3).cos() * (pz * f * 0.9).sin();
    n * noise.amplitude_m
}

fn system_to_shapes(system: &CaveSystem) -> Vec<SubtractShape> {
    let mut shapes = Vec::new();
    for node in &system.nodes {
        if matches!(
            node.kind,
            CaveNodeKind::Chamber
                | CaveNodeKind::Entrance
                | CaveNodeKind::Pool
                | CaveNodeKind::Junction
        ) {
            let r = node.radius_m.max(0.8);
            shapes.push(SubtractShape::Ellipsoid {
                center: [
                    node.position.0.x as f32,
                    node.position.0.y as f32,
                    node.position.0.z as f32,
                ],
                radii: [r * 1.6, r, r * 1.4],
                noise: WallNoiseParams::default(),
            });
        }
    }
    for edge in &system.edges {
        let Some(a) = system.nodes.get(edge.from) else {
            continue;
        };
        let Some(b) = system.nodes.get(edge.to) else {
            continue;
        };
        shapes.push(SubtractShape::Capsule {
            start: [
                a.position.0.x as f32,
                a.position.0.y as f32,
                a.position.0.z as f32,
            ],
            end: [
                b.position.0.x as f32,
                b.position.0.y as f32,
                b.position.0.z as f32,
            ],
            radius: edge.radius_m.max(0.6),
            noise: edge.noise,
        });
    }
    if system.overhang_enabled {
        if let (Some(entrance), Some(last)) = (system.nodes.first(), system.nodes.last()) {
            if entrance.kind == CaveNodeKind::Entrance {
                shapes.push(SubtractShape::Capsule {
                    start: [
                        last.position.0.x as f32,
                        last.position.0.y as f32,
                        last.position.0.z as f32,
                    ],
                    end: [
                        entrance.position.0.x as f32,
                        entrance.position.0.y as f32,
                        entrance.position.0.z as f32,
                    ],
                    radius: entrance.radius_m.max(0.8),
                    noise: WallNoiseParams::default(),
                });
            }
        }
    }
    shapes
}
