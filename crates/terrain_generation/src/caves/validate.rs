//! Cave system validation metrics.

use crate::contract::coordinates::WorldPosition;
use crate::fields::scalar::ScalarField;
use crate::spawn::PLAYER_SPAWN_MIN_CLEARANCE_M;

use super::graph::{CaveGraphRegistry, CaveNodeKind, CaveSystem};
use super::sdf::CaveSubtractOps;

pub struct CaveValidationReport {
    pub system_count: usize,
    pub traversable_systems: usize,
    pub mouth_breaches: u32,
    pub min_clearance_m: f32,
}

pub fn validate_cave_systems(
    registry: &CaveGraphRegistry,
    ops: &CaveSubtractOps,
    elevation: &ScalarField,
    sea_level_m: f32,
) -> CaveValidationReport {
    let mut mouth_breaches = 0u32;
    let mut min_clearance = f32::MAX;

    for system in &registry.systems {
        if system
            .nodes
            .iter()
            .any(|n| n.kind == CaveNodeKind::Entrance)
        {
            if check_mouth_breach(system, ops, elevation, sea_level_m) {
                mouth_breaches += 1;
            }
            let clearance = sample_path_clearance(system, ops);
            min_clearance = min_clearance.min(clearance);
        }
    }

    if min_clearance == f32::MAX {
        min_clearance = 0.0;
    }

    CaveValidationReport {
        system_count: registry.system_count(),
        traversable_systems: registry.traversable_system_count(),
        mouth_breaches,
        min_clearance_m: min_clearance,
    }
}

fn check_mouth_breach(
    system: &CaveSystem,
    ops: &CaveSubtractOps,
    elevation: &ScalarField,
    sea_level_m: f32,
) -> bool {
    let Some(entrance) = system
        .nodes
        .iter()
        .find(|n| n.kind == CaveNodeKind::Entrance)
    else {
        return false;
    };
    let wx = entrance.position.0.x;
    let wz = entrance.position.0.z;
    let surface = elevation.sample_at_world(crate::contract::coordinates::WorldXZ::new(wx, wz));
    let head_y = (surface + 2.0) as f64;
    let head_density =
        ops.apply_subtract(head_y as f32 - surface, WorldPosition::new(wx, head_y, wz));
    head_density > 0.5 && surface > sea_level_m + 1.0
}

fn sample_path_clearance(system: &CaveSystem, ops: &CaveSubtractOps) -> f32 {
    if system.edges.is_empty() {
        return 0.0;
    }
    let mut min_clear = f32::MAX;
    for edge in &system.edges {
        let Some(a) = system.nodes.get(edge.from) else {
            continue;
        };
        let Some(b) = system.nodes.get(edge.to) else {
            continue;
        };
        for step in 0..=8 {
            let t = step as f32 / 8.0;
            let px = a.position.0.x + (b.position.0.x - a.position.0.x) * t as f64;
            let py = a.position.0.y + (b.position.0.y - a.position.0.y) * t as f64;
            let pz = a.position.0.z + (b.position.0.z - a.position.0.z) * t as f64;
            let pos = WorldPosition::new(px, py, pz);
            let cavity = ops.sdf_at(pos);
            if cavity < 0.0 {
                let clearance = -cavity;
                min_clear = min_clear.min(clearance);
            }
        }
    }
    if min_clear == f32::MAX {
        0.0
    } else {
        min_clear
    }
}

pub fn meets_traversal_gate(report: &CaveValidationReport) -> bool {
    report.traversable_systems > 0 && report.min_clearance_m >= PLAYER_SPAWN_MIN_CLEARANCE_M * 0.5
}
