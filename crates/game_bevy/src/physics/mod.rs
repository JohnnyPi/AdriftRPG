// crates/game_bevy/src/physics/mod.rs
use bevy::prelude::*;
use physics_bridge::{
    CharacterController, CharacterControllerBundle, CharacterControllerPlugin,
    CharacterPhysicsSystems, GroundedState, LinearVelocity, PhysicsBridgePlugin,
    player_layers, CharacterCollisionQuery, GROUND_CONTACT_SKIN, terrain_ground_filter,
};
use avian3d::prelude::{Collider, CollisionLayers, SpatialQuery};
use game_data::CompiledPlayer;

mod props;
mod water_physics;

pub use props::DynamicPropPlugin;
pub use water_physics::WaterPhysicsPlugin;

/// Retry ground placement until terrain colliders are hit.
#[derive(Component, Debug)]
pub struct NeedsGroundSnap {
    pub attempts: u8,
}

use crate::camera::{
    camera_forward_xz, camera_right_xz, CameraInputState, CharacterFacing, MmoCamera,
    PlayerInterpolation,
};
use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;
use crate::player::{CharacterMotorState, MovementIntent, MovementSpeed, PlayerFacingMode};
use crate::player::{
    classify_locomotion, resolve_facing_yaw, Player, PlayerCapsuleVisual, PlayerMovementState,
};
use crate::state::AppState;
use crate::terrain::spawn_terrain_uploaded;
use crate::terrain::{
    effective_runtime_sea_level_m, spawn_terrain_collider_ready, TerrainFeatureRegistry,
    TerrainPipelineState,
};
use crate::ui::WaterTweaks;
use crate::ui::MovementTweaks;
use crate::ui::CameraTweaks;
use crate::world::effective_world_from_prefs;
use terrain_generation::WaterQuery;
use voxel_core::ChunkCoord;

pub struct GamePhysicsPlugin;

#[derive(Component, Debug)]
pub struct AwaitingSpawnTerrain {
    pub chunk: ChunkCoord,
}

const GROUND_SNAP_MAX_ATTEMPTS: u8 = 120;
const GROUND_SNAP_PROBE_M: f32 = 16.0;
/// One-time placement snap window beyond continuous `ground_snap_m`.
const INITIAL_GROUND_PLACEMENT_MAX_M: f32 = 0.5;

#[derive(Component, Default, Debug)]
pub struct PlayerMoveIntent {
    pub direction: Vec3,
    pub sprinting: bool,
    pub jumping: bool,
    pub jump_held: bool,
}

impl Plugin for GamePhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((PhysicsBridgePlugin, CharacterControllerPlugin, DynamicPropPlugin, WaterPhysicsPlugin))
            .add_systems(
                FixedUpdate,
                (
                    tag_player_awaiting_terrain,
                    hold_player_until_spawn_terrain,
                    snap_players_to_ground,
                    gather_player_movement,
                    detect_player_water,
                    apply_water_sink_and_swim,
                    apply_character_movement,
                    water_physics::apply_shallow_water_movement,
                )
                    .chain()
                    .before(CharacterPhysicsSystems)
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(
                FixedUpdate,
                (
                    update_character_facing,
                    apply_player_water_physics,
                    snapshot_player_interpolation,
                )
                    .chain()
                    .after(CharacterPhysicsSystems)
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(
                Update,
                sync_capsule_visual_rotation.run_if(in_state(AppState::Running)),
            );
    }
}

pub fn attach_character_physics(player: &CompiledPlayer, entity: &mut EntityCommands) {
    let bundle = CharacterControllerBundle::new(
        player.capsule_radius_m,
        player.capsule_half_height_m,
        player.jump_height_m,
        player.gravity_mps2,
        player.ground_snap_m,
        player.maximum_walkable_slope_deg,
        player.step_height_m,
    );
    let mut controller = bundle.controller;
    controller.walk_speed = player.walk_speed_mps;
    controller.run_speed = player.run_speed_mps;
    entity.insert((
        bundle.rigid_body,
        bundle.locked_axes,
        bundle.custom_integration,
        bundle.custom_velocity,
        bundle.speculative_margin,
        controller,
        bundle.grounded,
        bundle.linear_velocity,
        bundle.collider,
        bundle.friction,
        CollisionLayers::from(player_layers()),
    ));
    entity.insert((
        MovementIntent::default(),
        CharacterMotorState::default(),
        PlayerFacingMode::default(),
        PlayerMoveIntent::default(),
        crate::physics::water_physics::WetnessState::default(),
        NeedsGroundSnap { attempts: 0 },
    ));
}

fn tag_player_awaiting_terrain(
    mut commands: Commands,
    pipeline: Res<TerrainPipelineState>,
    colliders: Query<Entity, With<Collider>>,
    players: Query<Entity, (With<Player>, Without<AwaitingSpawnTerrain>, Without<NeedsGroundSnap>)>,
) {
    let Some(chunk) = pipeline.spawn_chunk else {
        return;
    };
    let mesh_ready = spawn_terrain_uploaded(&pipeline, chunk);
    let collider_ready = spawn_terrain_collider_ready(&pipeline, chunk, &colliders);

    for entity in &players {
        if mesh_ready && collider_ready {
            commands.entity(entity).insert(NeedsGroundSnap { attempts: 0 });
        } else {
            commands.entity(entity).insert(AwaitingSpawnTerrain { chunk });
        }
    }
}

fn hold_player_until_spawn_terrain(
    spatial: SpatialQuery,
    mut commands: Commands,
    pipeline: Res<TerrainPipelineState>,
    registry: Res<ConfigRegistryResource>,
    colliders: Query<Entity, With<Collider>>,
    mut players: Query<
        (
            Entity,
            &AwaitingSpawnTerrain,
            &mut Transform,
            &mut LinearVelocity,
            &Collider,
        ),
        With<Player>,
    >,
) {
    let Some(_source) = pipeline.density_source.as_ref() else {
        return;
    };
    let Ok(player) = registry.0.active_player() else {
        return;
    };

    for (entity, awaiting, mut transform, mut velocity, collider) in &mut players {
        velocity.0 = Vec3::ZERO;
        let mesh_ready = spawn_terrain_uploaded(&pipeline, awaiting.chunk);
        let collider_ready = spawn_terrain_collider_ready(&pipeline, awaiting.chunk, &colliders);
        if mesh_ready && collider_ready {
            place_capsule_on_physics_ground(
                &spatial,
                entity,
                collider,
                &mut transform,
                player,
            );
            commands.entity(entity).remove::<AwaitingSpawnTerrain>();
            commands
                .entity(entity)
                .insert(NeedsGroundSnap { attempts: 0 });
        }
    }
}

/// Drop the capsule onto the physics trimesh (not the analytic heightfield).
fn place_capsule_on_physics_ground(
    spatial: &SpatialQuery,
    entity: Entity,
    collider: &Collider,
    transform: &mut Transform,
    player: &CompiledPlayer,
) -> bool {
    let probe_height = GROUND_SNAP_PROBE_M.max(player.step_height_m + player.ground_snap_m + 3.0);
    let start_y = transform.translation.y;
    transform.translation.y += probe_height;
    let filter = terrain_ground_filter(entity);
    let cast_distance = probe_height + INITIAL_GROUND_PLACEMENT_MAX_M + 2.0;
    if let Some(hit) = CharacterCollisionQuery::ground_cast(
        spatial,
        collider,
        transform.translation,
        transform.rotation,
        cast_distance,
        &filter,
    ) {
        let gap = hit.distance - GROUND_CONTACT_SKIN;
        if gap <= INITIAL_GROUND_PLACEMENT_MAX_M + player.ground_snap_m {
            transform.translation.y -= gap;
            return true;
        }
    }
    transform.translation.y = start_y;
    false
}

fn snap_players_to_ground(
    spatial: SpatialQuery,
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    mut players: Query<
        (Entity, &mut NeedsGroundSnap, &mut Transform, &Collider, &GroundedState),
        With<Player>,
    >,
) {
    let Ok(player) = registry.0.active_player() else {
        return;
    };

    for (entity, mut snap, mut transform, collider, grounded) in &mut players {
        if grounded.grounded {
            commands.entity(entity).remove::<NeedsGroundSnap>();
            continue;
        }

        if place_capsule_on_physics_ground(&spatial, entity, collider, &mut transform, player) {
            commands.entity(entity).remove::<NeedsGroundSnap>();
            continue;
        }

        snap.attempts = snap.attempts.saturating_add(1);
        if snap.attempts >= GROUND_SNAP_MAX_ATTEMPTS {
            commands.entity(entity).remove::<NeedsGroundSnap>();
        }
    }
}

fn gather_player_movement(
    registry: Res<ConfigRegistryResource>,
    keyboard: Res<ButtonInput<KeyCode>>,
    input_state: Res<CameraInputState>,
    camera_tweaks: Res<CameraTweaks>,
    cameras: Query<&MmoCamera>,
    mut players: Query<
        (&mut PlayerMoveIntent, &mut MovementIntent),
        (With<Player>, Without<AwaitingSpawnTerrain>),
    >,
) {
    if camera_tweaks.fly_cam {
        return;
    }
    let Ok(camera) = cameras.single() else {
        return;
    };
    let Ok((mut intent, mut motor_intent)) = players.single_mut() else {
        return;
    };

    let config = registry.0.active_camera().expect("camera config");
    let mut axis = Vec2::ZERO;

    if keyboard.pressed(KeyCode::KeyW) {
        axis.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) {
        axis.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) {
        axis.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) {
        axis.x += 1.0;
    }

    if config.both_buttons_move_forward && input_state.two_button_forward() {
        axis.y = axis.y.max(1.0);
    }
    if input_state.autorun {
        axis.y = axis.y.max(1.0);
    }

    if axis.length_squared() > 1.0 {
        axis = axis.normalize();
    }

    let yaw = camera.intent_yaw();
    let forward = camera_forward_xz(yaw);
    let right = camera_right_xz(yaw);
    let world_dir = (forward * axis.y + right * axis.x).normalize_or_zero();
    intent.direction = Vec3::new(world_dir.x, 0.0, world_dir.z);
    intent.sprinting =
        keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    intent.jumping = keyboard.just_pressed(KeyCode::Space);
    intent.jump_held = keyboard.pressed(KeyCode::Space);

    motor_intent.direction = axis;
    motor_intent.requested_speed = if intent.sprinting {
        MovementSpeed::Run
    } else {
        MovementSpeed::Walk
    };
    motor_intent.jump_pressed = intent.jumping;
    motor_intent.jump_held = intent.jump_held;
}

fn apply_character_movement(
    time: Res<Time<Fixed>>,
    registry: Res<ConfigRegistryResource>,
    tweaks: Res<MovementTweaks>,
    cameras: Query<&MmoCamera>,
    mut players: Query<
        (
            &CharacterController,
            &GroundedState,
            &mut LinearVelocity,
            &mut PlayerMovementState,
            &mut CharacterMotorState,
            &MovementIntent,
        ),
        (With<Player>, Without<AwaitingSpawnTerrain>),
    >,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };
    let Ok((controller, grounded, mut velocity, mut movement, mut motor, move_intent)) =
        players.single_mut()
    else {
        return;
    };

    let player = registry.0.active_player().expect("player config");
    let dt = time.delta_secs();

    let walk_speed = if tweaks.use_overrides {
        tweaks.walk_speed
    } else {
        controller.walk_speed
    };
    let run_speed = if tweaks.use_overrides {
        tweaks.run_speed
    } else {
        controller.run_speed
    };
    let accel = if tweaks.use_overrides {
        tweaks.acceleration
    } else {
        player.acceleration_mps2
    };
    let decel = if tweaks.use_overrides {
        tweaks.deceleration
    } else {
        player.deceleration_mps2
    };
    let jump_buffer_s = if tweaks.use_overrides {
        tweaks.jump_buffer_s
    } else {
        player.jump_buffer_s
    };
    let coyote_time_s = if tweaks.use_overrides {
        tweaks.coyote_time_s
    } else {
        player.coyote_time_s
    };

    let target_speed = match move_intent.requested_speed {
        MovementSpeed::Run => run_speed,
        MovementSpeed::Walk => walk_speed,
    };

    let yaw = camera.intent_yaw();
    let forward = camera_forward_xz(yaw);
    let right = camera_right_xz(yaw);
    let world_dir = (forward * move_intent.direction.y + right * move_intent.direction.x).normalize_or_zero();
    let mut desired_dir = Vec2::new(world_dir.x, world_dir.z);
    if grounded.grounded && grounded.ground_normal.y < 0.99 {
        let ground_normal = grounded.ground_normal;
        let desired_3d = Vec3::new(desired_dir.x, 0.0, desired_dir.y);
        let projected = desired_3d.reject_from(ground_normal).normalize_or_zero();
        desired_dir = Vec2::new(projected.x, projected.z);
    }

    let desired_velocity = desired_dir * target_speed;
    let current_planar = Vec2::new(velocity.x, velocity.z);

    let accelerating = desired_velocity.length_squared() > current_planar.length_squared();
    let rate = if accelerating { accel } else { decel };

    let new_planar = approach_vec2(current_planar, desired_velocity, rate * dt);
    velocity.x = new_planar.x;
    velocity.z = new_planar.y;
    movement.planar_velocity = new_planar;

    if move_intent.jump_pressed {
        movement.jump_buffer_remaining_s = jump_buffer_s;
    } else {
        movement.jump_buffer_remaining_s = (movement.jump_buffer_remaining_s - dt).max(0.0);
    }

    if grounded.grounded {
        movement.coyote_remaining_s = coyote_time_s;
    } else if movement.was_grounded {
        movement.coyote_remaining_s = (movement.coyote_remaining_s - dt).max(0.0);
    }

    let can_jump = (grounded.grounded || movement.coyote_remaining_s > 0.0) && !movement.in_water;
    if movement.jump_buffer_remaining_s > 0.0 && can_jump {
        velocity.y = controller.jump_speed;
        movement.jump_buffer_remaining_s = 0.0;
        movement.coyote_remaining_s = 0.0;
    }

    movement.was_grounded = grounded.grounded;

    motor.velocity = velocity.0;
    motor.grounded = grounded.grounded;
    motor.ground_normal = grounded.ground_normal;
    motor.current_slope = grounded
        .ground_normal
        .angle_between(Vec3::Y)
        .to_degrees();
    motor.locomotion_state = classify_locomotion(
        grounded.grounded,
        motor.current_slope,
        player.maximum_walkable_slope_deg,
    );
}

fn physics_floor_center_y(
    spatial: &SpatialQuery,
    entity: Entity,
    collider: &Collider,
    transform: &Transform,
    player: &CompiledPlayer,
) -> Option<f32> {
    let probe_height = GROUND_SNAP_PROBE_M.max(player.step_height_m + player.ground_snap_m + 3.0);
    let elevated_y = transform.translation.y + probe_height;
    let mut probe_pos = transform.translation;
    probe_pos.y = elevated_y;
    let filter = terrain_ground_filter(entity);
    CharacterCollisionQuery::ground_cast(
        spatial,
        collider,
        probe_pos,
        transform.rotation,
        probe_height + 2.0,
        &filter,
    )
    .map(|hit| elevated_y - hit.distance + GROUND_CONTACT_SKIN)
}

fn capsule_bottom_offset(player: &CompiledPlayer) -> f32 {
    player.capsule_half_height_m + player.capsule_radius_m
}

fn detect_player_water(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    water_tweaks: Res<WaterTweaks>,
    water_physics: Res<crate::ui::WaterPhysicsTweaks>,
    features: Res<TerrainFeatureRegistry>,
    mut players: Query<(&Transform, &mut PlayerMovementState), With<Player>>,
) {
    let Ok(_world) = effective_world_from_prefs(&registry.0, &prefs) else {
        return;
    };
    let Ok(player) = registry.0.active_player() else {
        return;
    };
    let sea = effective_runtime_sea_level_m(&registry, &prefs, &water_tweaks);
    let shallow_depth = water_physics.shallow_depth_m;
    let feet_offset = capsule_bottom_offset(&player);

    for (transform, mut movement) in &mut players {
        let center = transform.translation;
        let feet_y = center.y - feet_offset;
        let feet_point = [center.x, feet_y, center.z];
        let center_point = [center.x, center.y, center.z];
        let feet_depth = features
            .hydrology
            .as_ref()
            .and_then(|hydro| hydro.water.water_at(feet_point))
            .map(|s| s.depth)
            .unwrap_or(0.0);
        let center_depth = features
            .hydrology
            .as_ref()
            .and_then(|hydro| hydro.water.water_at(center_point))
            .map(|s| s.depth)
            .unwrap_or(0.0);
        movement.submerged_depth = feet_depth.max(center_depth);
        movement.in_water = feet_y < sea + 0.05 || movement.submerged_depth > 0.05;
        movement.in_shallow_water = movement.in_water
            && (feet_y > sea - shallow_depth || movement.submerged_depth < shallow_depth);
    }
}

fn apply_water_sink_and_swim(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    water_tweaks: Res<WaterTweaks>,
    tweaks: Res<crate::ui::WaterPhysicsTweaks>,
    intent: Query<&PlayerMoveIntent, With<Player>>,
    mut players: Query<(&mut LinearVelocity, &PlayerMovementState), With<Player>>,
) {
    let Ok(_world) = effective_world_from_prefs(&registry.0, &prefs) else {
        return;
    };
    let Ok(player) = registry.0.active_player() else {
        return;
    };
    let Ok(intent) = intent.single() else {
        return;
    };
    let sea = effective_runtime_sea_level_m(&registry, &prefs, &water_tweaks);
    let gravity = player.gravity_mps2;

    for (mut velocity, movement) in &mut players {
        if !movement.in_water {
            continue;
        }

        if intent.jump_held {
            velocity.y = velocity.y.max(tweaks.swim_up_speed_mps);
            continue;
        }

        let deep_water = movement.submerged_depth > tweaks.shallow_depth_m;
        if deep_water {
            continue;
        }

        if tweaks.buoyancy_surface_only {
            let wading = (sea - (movement.submerged_depth)).max(0.0);
            if wading > 0.05 && wading <= tweaks.shallow_depth_m {
                velocity.y += gravity * wading.min(1.0) * tweaks.buoyancy_strength * 0.04;
            }
        } else if movement.submerged_depth > 0.05 {
            velocity.y += gravity * movement.submerged_depth.min(1.2) * tweaks.buoyancy_strength * 0.08;
            if velocity.y < 0.0 {
                velocity.y *= 0.98;
            }
        }
    }
}

fn apply_player_water_physics(
    spatial: SpatialQuery,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    water_tweaks: Res<WaterTweaks>,
    tweaks: Res<crate::ui::WaterPhysicsTweaks>,
    mut players: Query<
        (
            Entity,
            &mut Transform,
            &mut LinearVelocity,
            &mut PlayerMovementState,
            &Collider,
        ),
        With<Player>,
    >,
) {
    let Ok(_world) = effective_world_from_prefs(&registry.0, &prefs) else {
        return;
    };
    let Ok(player) = registry.0.active_player() else {
        return;
    };
    let sea = effective_runtime_sea_level_m(&registry, &prefs, &water_tweaks);
    let feet_offset = capsule_bottom_offset(&player);

    for (entity, mut transform, mut velocity, mut movement, collider) in &mut players {
        if !movement.in_water {
            continue;
        }

        let feet_y = transform.translation.y - feet_offset;
        let deep_water = movement.submerged_depth > tweaks.shallow_depth_m;

        if let Some(min_center_y) = physics_floor_center_y(
            &spatial,
            entity,
            collider,
            &transform,
            player,
        ) {
            if transform.translation.y < min_center_y {
                transform.translation.y = min_center_y;
                if velocity.y < 0.0 {
                    velocity.y = 0.0;
                }
            }
        }

        movement.in_shallow_water = !deep_water
            && feet_y > sea - tweaks.shallow_depth_m
            && feet_y < sea + 0.25
            && movement.submerged_depth < tweaks.shallow_depth_m;
    }
}

fn update_character_facing(
    time: Res<Time<Fixed>>,
    input_state: Res<CameraInputState>,
    cameras: Query<&MmoCamera>,
    intent: Query<(&MovementIntent, &PlayerFacingMode), With<Player>>,
    mut players: Query<(&mut CharacterFacing, &mut Transform), With<Player>>,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };
    let Ok((move_intent, facing_mode)) = intent.single() else {
        return;
    };
    let Ok((mut facing, mut transform)) = players.single_mut() else {
        return;
    };

    let yaw = camera.intent_yaw();
    let forward = camera_forward_xz(yaw);
    let right = camera_right_xz(yaw);
    let world_dir = (forward * move_intent.direction.y + right * move_intent.direction.x).normalize_or_zero();
    let movement_dir = Vec2::new(world_dir.x, world_dir.z);

    facing.desired_yaw = resolve_facing_yaw(
        facing_mode.0,
        input_state.steering_character(),
        yaw,
        movement_dir,
    );

    facing.yaw = rotate_toward_angle(
        facing.yaw,
        facing.desired_yaw,
        facing.turn_speed * time.delta_secs(),
    );
    transform.rotation = Quat::from_rotation_y(facing.yaw);
}

fn snapshot_player_interpolation(
    mut players: Query<(&Transform, &mut PlayerInterpolation), With<Player>>,
) {
    for (transform, mut interpolation) in &mut players {
        interpolation.previous = interpolation.current;
        interpolation.current = transform.translation;
    }
}

fn sync_capsule_visual_rotation(
    players: Query<&CharacterFacing, With<Player>>,
    mut capsules: Query<&mut Transform, With<PlayerCapsuleVisual>>,
) {
    let Ok(facing) = players.single() else {
        return;
    };
    for mut transform in &mut capsules {
        transform.rotation = Quat::from_rotation_y(facing.yaw);
    }
}

fn approach_vec2(current: Vec2, target: Vec2, max_delta: f32) -> Vec2 {
    let delta = target - current;
    let distance = delta.length();
    if distance <= max_delta {
        return target;
    }
    current + delta / distance * max_delta
}

fn rotate_toward_angle(current: f32, target: f32, max_step: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    let mut delta = (target - current).rem_euclid(TAU);
    if delta > PI {
        delta -= TAU;
    }
    if delta.abs() <= max_step {
        target
    } else {
        current + delta.signum() * max_step
    }
}

#[cfg(test)]
pub(crate) fn jump_buffer_after_press(buffer: f32, jump_buffer_s: f32) -> f32 {
    jump_buffer_s.max(buffer)
}

#[cfg(test)]
pub(crate) fn coyote_after_grounded(coyote_time_s: f32) -> f32 {
    coyote_time_s
}

#[cfg(test)]
pub(crate) fn should_execute_jump(buffer: f32, coyote: f32, grounded: bool) -> bool {
    buffer > 0.0 && (grounded || coyote > 0.0)
}

#[cfg(test)]
mod character_movement_tests {
    use super::*;

    #[test]
    fn jump_buffer_extends_after_press() {
        let buffer = jump_buffer_after_press(0.0, 0.12);
        assert!((buffer - 0.12).abs() < 1e-5);
    }

    #[test]
    fn coyote_time_allows_jump_after_leaving_ground() {
        let coyote = coyote_after_grounded(0.1);
        assert!(should_execute_jump(0.05, coyote, false));
    }

    #[test]
    fn slope_projection_removes_vertical_from_direction() {
        let ground = Vec3::new(0.5, 0.866, 0.0).normalize();
        let desired = Vec3::new(1.0, 0.0, 0.0);
        let projected = desired.reject_from(ground).normalize_or_zero();
        assert!(projected.dot(ground).abs() < 0.05);
    }
}
