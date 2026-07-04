// crates/game_bevy/src/physics/water_physics.rs
//! Water physics — buoyancy, flow, wading and submersion (VS2 Phase 11).

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::physics::props::DynamicCrate;
use crate::player::{Player, PlayerMovementState};
use crate::state::AppState;
use crate::terrain::TerrainFeatureRegistry;
use crate::ui::WaterPhysicsTweaks;
use terrain_generation::WaterQuery;

/// Depth (at the capsule center) beyond `shallow_depth_m` over which drag
/// ramps from the wading scale down to the fully submerged scale.
const SUBMERGE_RAMP_M: f32 = 1.0;

/// Per-tick horizontal velocity multiplier when fully submerged. The player
/// does not float: they sink to the underwater terrain and move along it
/// slowly, as heavy wading rather than free walking.
const SUBMERGED_SPEED_SCALE: f32 = 0.45;

/// Terminal sink speed while submerged — water resistance caps free fall so
/// stepping off the shelf reads as sinking, not plummeting.
const SUBMERGED_TERMINAL_SINK_MPS: f32 = 2.5;

/// Per-tick damping applied to upward velocity while submerged (kills full
/// strength jumps underwater without forbidding small hops off the bottom).
const SUBMERGED_JUMP_DAMP: f32 = 0.85;

const RIVER_FLOW_RADIUS_M: f32 = 6.0;
const RIVER_CACHE_RESCAN_DISTANCE_M: f32 = 12.0;

#[derive(Component, Debug, Default)]
pub struct RiverFlowCache {
    pub segment_hint: usize,
    pub last_xz: Vec2,
    pub hydrology_epoch: u32,
}

/// Nearest river segment to `(x, z)`; returns lateral distance and flow direction.
pub fn nearest_river_segment(
    river: &terrain_generation::RiverSpline,
    x: f32,
    z: f32,
    segment_hint: usize,
    segment_count: usize,
    allow_full_scan: bool,
) -> (f32, Vec2, usize) {
    if segment_count == 0 {
        return (f32::MAX, Vec2::ZERO, 0);
    }

    let mut best_dist = f32::MAX;
    let mut best_dir = Vec2::ZERO;
    let mut best_segment = segment_hint.min(segment_count - 1);

    let scan_segment =
        |i: usize, best_dist: &mut f32, best_dir: &mut Vec2, best_segment: &mut usize| {
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
                return;
            }
            let t = ((x - ax) * dx + (z - az) * dz) / len2;
            let t = t.clamp(0.0, 1.0);
            let cx = ax + dx * t;
            let cz = az + dz * t;
            let dist = Vec2::new(x - cx, z - cz).length();
            if dist < *best_dist {
                *best_dist = dist;
                *best_dir = Vec2::new(dx, dz).normalize_or_zero();
                *best_segment = i;
            }
        };

    let start = segment_hint.saturating_sub(2);
    let end = (segment_hint + 3).min(segment_count);
    for i in start..end {
        scan_segment(i, &mut best_dist, &mut best_dir, &mut best_segment);
    }

    if allow_full_scan && (best_dist > RIVER_FLOW_RADIUS_M || end - start < segment_count) {
        for i in 0..segment_count {
            if i >= start && i < end {
                continue;
            }
            scan_segment(i, &mut best_dist, &mut best_dir, &mut best_segment);
        }
    }

    (best_dist, best_dir, best_segment)
}

#[derive(Component, Default)]
pub struct WetnessState {
    pub wetness: f32,
}

#[derive(Component)]
pub struct WaterSensor;

/// Pure description of how a given immersion depth affects the player.
/// Kept free of ECS types so it is directly unit-testable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WadingProfile {
    /// Per-tick multiplier for horizontal velocity (1.0 = unimpeded).
    pub horizontal_scale: f32,
    /// Maximum downward speed, if the water is deep enough to cap it.
    pub terminal_sink_mps: Option<f32>,
    /// Wading band: deep enough to slow movement, shallow enough to walk.
    pub in_shallow_water: bool,
    /// Capsule center below the surface by more than the shallow band.
    pub submerged: bool,
}

impl WadingProfile {
    pub const DRY: Self = Self {
        horizontal_scale: 1.0,
        terminal_sink_mps: None,
        in_shallow_water: false,
        submerged: false,
    };
}

/// Map immersion depth (measured at the capsule center) to movement effects.
///
/// 0..shallow_depth: drag blends from none to the authored shallow scale.
/// shallow_depth..+ramp: drag blends on toward the submerged scale, sinking
/// becomes speed-capped, and jumps are damped. Beyond that: fully submerged.
pub fn wading_profile(depth_at_center: f32, tweaks: &WaterPhysicsTweaks) -> WadingProfile {
    if depth_at_center <= 0.0 {
        return WadingProfile::DRY;
    }
    let shallow_depth = tweaks.shallow_depth_m.max(0.05);
    if depth_at_center < shallow_depth {
        let t = depth_at_center / shallow_depth;
        return WadingProfile {
            horizontal_scale: 1.0 + (tweaks.shallow_speed_scale - 1.0) * t,
            terminal_sink_mps: None,
            in_shallow_water: true,
            submerged: false,
        };
    }
    let t = ((depth_at_center - shallow_depth) / SUBMERGE_RAMP_M).clamp(0.0, 1.0);
    WadingProfile {
        horizontal_scale: tweaks.shallow_speed_scale
            + (SUBMERGED_SPEED_SCALE - tweaks.shallow_speed_scale) * t,
        terminal_sink_mps: Some(SUBMERGED_TERMINAL_SINK_MPS),
        in_shallow_water: false,
        submerged: true,
    }
}

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
    registry: Res<crate::data::ConfigRegistryResource>,
    features: Res<TerrainFeatureRegistry>,
    tweaks: Res<WaterPhysicsTweaks>,
    mut bodies: Query<(&Transform, &mut LinearVelocity), With<DynamicCrate>>,
) {
    let Some(water) = features.water_registry() else {
        return;
    };
    let gravity = registry
        .0
        .active_physics()
        .map(|p| p.gravity_mps2)
        .unwrap_or(18.0);
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
    mut bodies: Query<(&Transform, &mut LinearVelocity, &mut RiverFlowCache), With<DynamicCrate>>,
) {
    let Some(river) = features.rivers.get(&1) else {
        return;
    };
    let Some(bounds) = features.river_flow_bounds else {
        return;
    };
    let epoch = features.hydrology_epoch;
    for (tf, mut vel, mut cache) in &mut bodies {
        let x = tf.translation.x;
        let z = tf.translation.z;
        if !bounds.contains_xz(x, z) {
            continue;
        }

        let hint = bounds.clamp_segment_hint(if cache.hydrology_epoch == epoch {
            cache.segment_hint
        } else {
            0
        });
        let last_xz = if cache.hydrology_epoch == epoch {
            cache.last_xz
        } else {
            Vec2::new(x, z)
        };
        let needs_full_scan = cache.hydrology_epoch != epoch
            || last_xz.distance_squared(Vec2::new(x, z))
                > RIVER_CACHE_RESCAN_DISTANCE_M * RIVER_CACHE_RESCAN_DISTANCE_M;

        let (best_dist, best_dir, best_segment) =
            nearest_river_segment(river, x, z, hint, bounds.segment_count, needs_full_scan);

        cache.segment_hint = best_segment;
        cache.last_xz = Vec2::new(x, z);
        cache.hydrology_epoch = epoch;

        if best_dist < RIVER_FLOW_RADIUS_M {
            vel.x += best_dir.x * 0.4 * tweaks.flow_multiplier;
            vel.z += best_dir.y * 0.4 * tweaks.flow_multiplier;
        }
    }
}

pub(crate) fn apply_shallow_water_movement(
    features: Res<TerrainFeatureRegistry>,
    tweaks: Res<WaterPhysicsTweaks>,
    intent: Query<&crate::physics::PlayerMoveIntent, With<Player>>,
    mut players: Query<(&Transform, &mut LinearVelocity, &mut PlayerMovementState), With<Player>>,
) {
    let Some(hydro) = features.hydrology.as_ref() else {
        return;
    };
    let swim_up = intent.single().map(|i| i.jump_held).unwrap_or(false);
    for (tf, mut vel, mut movement) in &mut players {
        let point = [tf.translation.x, tf.translation.y, tf.translation.z];
        let depth = hydro
            .water
            .water_at(point)
            .map(|sample| sample.depth)
            .unwrap_or(0.0);
        let profile = wading_profile(depth, &tweaks);
        movement.in_shallow_water = profile.in_shallow_water;

        if profile.horizontal_scale < 1.0 {
            vel.x *= profile.horizontal_scale;
            vel.z *= profile.horizontal_scale;
        }
        if profile.submerged && vel.y > 0.0 && !swim_up {
            vel.y *= SUBMERGED_JUMP_DAMP;
        }
        if let Some(terminal) = profile.terminal_sink_mps {
            let cap = tweaks.submerged_sink_cap_mps.min(terminal);
            if vel.y < -cap {
                vel.y = -cap;
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
    fn river_segment_cache_matches_brute_force() {
        use terrain_generation::{RiverGenConfig, generate_river_spline};
        let river = generate_river_spline(&RiverGenConfig::default(), 0.0).expect("river");
        let mid = river.points.len() / 2;
        let x = river.points[mid].position_xz[0];
        let z = river.points[mid].position_xz[1];
        let segment_count = river.points.len().saturating_sub(1);
        let (cached_dist, _, cached_seg) =
            nearest_river_segment(&river, x, z, mid, segment_count, false);
        let (full_dist, _, full_seg) =
            nearest_river_segment(&river, x, z, mid, segment_count, true);
        assert!((cached_dist - full_dist).abs() < 1e-3);
        assert_eq!(cached_seg, full_seg);
    }

    #[test]
    fn river_flow_direction_is_nonzero_near_spline() {
        use terrain_generation::{RiverGenConfig, generate_river_spline};
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

    #[test]
    fn dry_profile_is_identity() {
        let tweaks = WaterPhysicsTweaks::default();
        assert_eq!(wading_profile(0.0, &tweaks), WadingProfile::DRY);
        assert_eq!(wading_profile(-1.0, &tweaks), WadingProfile::DRY);
    }

    #[test]
    fn wading_drag_scales_with_depth() {
        let tweaks = WaterPhysicsTweaks::default(); // shallow 1.5 m, scale 0.7
        let ankle = wading_profile(0.15, &tweaks);
        let waist = wading_profile(1.2, &tweaks);
        assert!(ankle.in_shallow_water && waist.in_shallow_water);
        assert!(!ankle.submerged && !waist.submerged);
        assert!(
            ankle.horizontal_scale > waist.horizontal_scale,
            "deeper wading must drag more: ankle {} vs waist {}",
            ankle.horizontal_scale,
            waist.horizontal_scale
        );
        assert!(waist.horizontal_scale >= tweaks.shallow_speed_scale - 1e-4);
        assert!(ankle.terminal_sink_mps.is_none());
    }

    #[test]
    fn submerged_profile_caps_sinking_and_slows_further() {
        let tweaks = WaterPhysicsTweaks::default();
        let deep = wading_profile(tweaks.shallow_depth_m + SUBMERGE_RAMP_M + 1.0, &tweaks);
        assert!(deep.submerged);
        assert!(!deep.in_shallow_water);
        assert_eq!(deep.terminal_sink_mps, Some(SUBMERGED_TERMINAL_SINK_MPS));
        assert!((deep.horizontal_scale - SUBMERGED_SPEED_SCALE).abs() < 1e-4);
    }

    #[test]
    fn profile_is_continuous_across_shallow_boundary() {
        let tweaks = WaterPhysicsTweaks::default();
        let just_below = wading_profile(tweaks.shallow_depth_m - 1e-3, &tweaks);
        let just_above = wading_profile(tweaks.shallow_depth_m + 1e-3, &tweaks);
        assert!(
            (just_below.horizontal_scale - just_above.horizontal_scale).abs() < 0.01,
            "drag must not step discontinuously at the shallow boundary"
        );
    }
}
