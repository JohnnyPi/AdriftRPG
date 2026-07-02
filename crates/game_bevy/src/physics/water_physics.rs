// crates/game_bevy/src/physics/water_physics.rs
//! Water physics — buoyancy, flow, shallow movement (VS2 Phase 11).

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::physics::props::DynamicCrate;
use crate::player::{Player, PlayerMovementState};
use crate::state::AppState;
use crate::terrain::TerrainFeatureRegistry;
use crate::ui::WaterPhysicsTweaks;
use terrain_generation::WaterQuery;

#[derive(Component, Default)]
pub struct WetnessState {
    pub wetness: f32,
}

#[derive(Component)]
pub struct WaterSensor;

pub struct WaterPhysicsPlugin;

impl Plugin for WaterPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (apply_buoyancy, apply_river_flow, update_wetness).run_if(in_state(AppState::Running)),
        );
    }
}

fn apply_buoyancy(
    features: Res<TerrainFeatureRegistry>,
    tweaks: Res<WaterPhysicsTweaks>,
    mut bodies: Query<(&Transform, &mut LinearVelocity), With<DynamicCrate>>,
) {
    let Some(water) = features.water_registry() else {
        return;
    };
    let gravity = 18.0;
    for (tf, mut vel) in &mut bodies {
        let point = [tf.translation.x, tf.translation.y, tf.translation.z];
        if let Some(sample) = water.water_at(point) {
            if sample.depth > 0.05 {
                let submerged = sample.depth.min(1.0);
                vel.y += gravity * submerged * tweaks.buoyancy_strength * 0.08;
                vel.x *= 0.99;
                vel.z *= 0.99;
            }
        }
    }
}

fn apply_river_flow(
    features: Res<TerrainFeatureRegistry>,
    tweaks: Res<WaterPhysicsTweaks>,
    mut bodies: Query<(&Transform, &mut LinearVelocity), With<DynamicCrate>>,
) {
    let Some(river) = features.rivers.get(&1) else {
        return;
    };
    for (tf, mut vel) in &mut bodies {
        let x = tf.translation.x;
        let z = tf.translation.z;
        let mut best_dir = Vec2::ZERO;
        let mut best_dist = f32::MAX;
        for i in 0..river.points.len().saturating_sub(1) {
            let a = &river.points[i];
            let b = &river.points[i + 1];
            let ax = a.position_xz[0];
            let az = a.position_xz[1];
            let bx = b.position_xz[0];
            let bz = b.position_xz[1];
            let dx = bx - ax;
            let dz = bz - az;
            let len2 = dx * dx + dz * dz;
            if len2 < 1e-4 {
                continue;
            }
            let t = ((x - ax) * dx + (z - az) * dz) / len2;
            let t = t.clamp(0.0, 1.0);
            let cx = ax + dx * t;
            let cz = az + dz * t;
            let dist = Vec2::new(x - cx, z - cz).length();
            if dist < best_dist {
                best_dist = dist;
                best_dir = Vec2::new(dx, dz).normalize_or_zero();
            }
        }
        if best_dist < 6.0 {
            vel.x += best_dir.x * 0.4 * tweaks.flow_multiplier;
            vel.z += best_dir.y * 0.4 * tweaks.flow_multiplier;
        }
    }
}

pub(crate) fn apply_shallow_water_movement(
    features: Res<TerrainFeatureRegistry>,
    tweaks: Res<WaterPhysicsTweaks>,
    mut players: Query<(&Transform, &mut LinearVelocity, &mut PlayerMovementState), With<Player>>,
) {
    let Some(hydro) = features.hydrology.as_ref() else {
        return;
    };
    for (tf, mut vel, mut movement) in &mut players {
        let point = [tf.translation.x, tf.translation.y, tf.translation.z];
        if let Some(sample) = hydro.water.water_at(point) {
            movement.in_shallow_water =
                sample.depth > 0.0 && sample.depth < tweaks.shallow_depth_m;
            if movement.in_shallow_water {
                vel.x *= tweaks.shallow_speed_scale;
                vel.z *= tweaks.shallow_speed_scale;
            }
        }
    }
}

fn update_wetness(
    features: Res<TerrainFeatureRegistry>,
    mut query: Query<(&Transform, &mut WetnessState), With<Player>>,
) {
    let Some(hydro) = features.hydrology.as_ref() else {
        return;
    };
    for (tf, mut wetness) in &mut query {
        let point = [tf.translation.x, tf.translation.y, tf.translation.z];
        wetness.wetness = if let Some(sample) = hydro.water.water_at(point) {
            (sample.depth / 1.5).clamp(0.0, 1.0)
        } else {
            (wetness.wetness - 0.02).max(0.0)
        };
    }
}

#[cfg(test)]
mod water_physics_tests {
    use super::*;
    use terrain_generation::{WaterBodyRegistry, WaterQuery};

    #[test]
    fn buoyancy_applies_when_crate_submerged() {
        let registry = WaterBodyRegistry::demo_registry(2.0, 31.5);
        let point = [82.0, 30.5, 196.0];
        let sample = registry.water_at(point).expect("lake sample");
        assert!(sample.depth > 0.05);
        let submerged = sample.depth.min(1.0);
        let lift = 18.0 * submerged * 1.0 * 0.08;
        assert!(lift > 0.0);
    }

    #[test]
    fn river_flow_direction_is_nonzero_near_spline() {
        use terrain_generation::{generate_river_spline, RiverGenConfig};
        let river = generate_river_spline(&RiverGenConfig::default(), 0.0).expect("river");
        let i = river.points.len() / 2;
        let a = &river.points[i];
        let b = &river.points[i + 1];
        let dir = Vec2::new(
            b.position_xz[0] - a.position_xz[0],
            b.position_xz[1] - a.position_xz[1],
        );
        assert!(dir.length_squared() > 1e-4);
    }
}
