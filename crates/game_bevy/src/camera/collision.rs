// crates/game_bevy/src/camera/collision.rs
use avian3d::prelude::*;
use bevy::prelude::*;

use crate::data::ConfigRegistryResource;
use physics_bridge::CollisionLayer;

use crate::camera::environment::{CameraEnvironment, CameraEnvironmentState};
use crate::ui::CameraTweaks;

use super::components::{CameraDebugSnapshot, CameraInputState, MmoCamera};
use super::{clamp_visual_delta, desired_camera_position, smooth_scalar};

pub fn resolve_collision_distance(
    current: f32,
    desired: f32,
    collision_limit: f32,
    inward_sharpness: f32,
    outward_sharpness: f32,
    dt: f32,
) -> f32 {
    let target = desired.min(collision_limit);
    let sharpness = if target < current {
        inward_sharpness
    } else {
        outward_sharpness
    };
    smooth_scalar(current, target, sharpness, dt)
}

pub fn resolve_camera_collision(
    time: Res<Time>,
    registry: Res<ConfigRegistryResource>,
    spatial: SpatialQuery,
    camera_env: Res<CameraEnvironment>,
    camera_tweaks: Res<CameraTweaks>,
    mut debug: ResMut<CameraDebugSnapshot>,
    mut cameras: Query<&mut MmoCamera>,
) {
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };

    let config = registry.0.active_camera().expect("camera config");
    let dt = clamp_visual_delta(time.delta_secs());

    let focus = camera.current_focus;
    let desired_position = desired_camera_position(
        focus,
        camera.current_yaw,
        camera.current_pitch,
        camera.current_distance,
        camera.shoulder_offset,
    );

    let desired_offset = desired_position - focus;
    let desired_distance = desired_offset.length();
    let direction = desired_offset.normalize_or_zero();

    let collision_limit = if desired_distance > 0.05 {
        let shape = Collider::sphere(config.collision_radius);
        let cast_config = ShapeCastConfig::from_max_distance(desired_distance);
        let filter = SpatialQueryFilter::default()
            .with_excluded_entities([camera.player])
            .with_mask(CollisionLayer::Terrain);

        if let Some(hit) = spatial.cast_shape(
            &shape,
            focus,
            Quat::IDENTITY,
            Dir3::new(direction).unwrap_or(Dir3::NEG_Z),
            &cast_config,
            &filter,
        ) {
            debug.hit_position = Some(focus + direction * hit.distance);
            (hit.distance - config.collision_margin).max(config.distance_minimum_m)
        } else {
            debug.hit_position = None;
            desired_distance
        }
    } else {
        debug.hit_position = None;
        desired_distance
    };

    let inward_sharpness = if camera_tweaks.use_overrides {
        camera_tweaks.collision_inward_sharpness
    } else {
        config.collision_inward_sharpness
    };
    let outward_sharpness = if camera_tweaks.use_overrides {
        camera_tweaks.collision_outward_sharpness
    } else {
        config.collision_outward_sharpness
    };

    let mut limited = resolve_collision_distance(
        camera.collision_limited_distance,
        camera.current_distance,
        collision_limit,
        inward_sharpness,
        outward_sharpness,
        dt,
    );

    if camera_env.state == CameraEnvironmentState::Interior && camera_env.obstruction_hold_s > 0.0 {
        limited = limited.min(camera.collision_limited_distance);
    }

    camera.collision_limited_distance = limited;

    debug.focus = focus;
    debug.desired_position = desired_position;
    debug.desired_distance = camera.current_distance;
    debug.collision_limited_distance = camera.collision_limited_distance;
    debug.intent_yaw = camera.intent_yaw();
    debug.character_yaw = camera.character_yaw;
}

pub fn update_camera_debug_input(
    input_state: Res<CameraInputState>,
    mut debug: ResMut<CameraDebugSnapshot>,
) {
    debug.left_look = input_state.left_look;
    debug.right_steer = input_state.right_steer;
}
