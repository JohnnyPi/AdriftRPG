use bevy::prelude::*;
use physics_bridge::{
    CharacterController, CharacterControllerBundle, CharacterControllerPlugin,
    CharacterPhysicsSystems, GroundedState, LinearVelocity, PhysicsBridgePlugin, player_layers,
};
use avian3d::prelude::CollisionLayers;
use game_data::CompiledPlayer;

mod props;
mod water_physics;

pub use props::DynamicPropPlugin;
pub use water_physics::WaterPhysicsPlugin;

use crate::camera::{
    camera_forward_xz, camera_right_xz, CameraInputState, CharacterFacing, MmoCamera,
    PlayerInterpolation,
};
use crate::data::ConfigRegistryResource;
use crate::player::{CharacterMotorState, MovementIntent, MovementSpeed, PlayerFacingMode};
use crate::player::{
    classify_locomotion, resolve_facing_yaw, Player, PlayerCapsuleVisual, PlayerMovementState,
};
use crate::state::AppState;
use crate::terrain::{ChunkState, TerrainPipelineState};
use crate::ui::MovementTweaks;
use voxel_core::ChunkCoord;

pub struct GamePhysicsPlugin;

#[derive(Component, Debug)]
pub struct AwaitingSpawnTerrain {
    pub chunk: ChunkCoord,
}

#[derive(Component, Debug)]
pub struct SpawnTerrainReleased;

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
                    gather_player_movement,
                    apply_character_movement,
                    props::inherit_platform_velocity,
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
                    enforce_water_depth,
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
    ));
}

fn tag_player_awaiting_terrain(
    mut commands: Commands,
    pipeline: Res<TerrainPipelineState>,
    players: Query<Entity, (With<Player>, Without<AwaitingSpawnTerrain>, Without<SpawnTerrainReleased>)>,
) {
    let Some(chunk) = pipeline.spawn_chunk else {
        return;
    };
    let spawn_ready = pipeline.chunks.iter().any(|c| {
        c.coord == chunk && c.state == ChunkState::Ready && c.entity.is_some()
    });
    if spawn_ready {
        return;
    }
    for entity in &players {
        commands.entity(entity).insert(AwaitingSpawnTerrain { chunk });
    }
}

fn hold_player_until_spawn_terrain(
    mut commands: Commands,
    pipeline: Res<TerrainPipelineState>,
    mut players: Query<(Entity, &AwaitingSpawnTerrain, &mut LinearVelocity), With<Player>>,
) {
    for (entity, awaiting, mut velocity) in &mut players {
        velocity.0 = Vec3::ZERO;
        let ready = pipeline.chunks.iter().any(|chunk| {
            chunk.coord == awaiting.chunk
                && chunk.state == ChunkState::Ready
                && chunk.entity.is_some()
        });
        if ready {
            commands.entity(entity).remove::<AwaitingSpawnTerrain>();
            commands.entity(entity).insert(SpawnTerrainReleased);
        }
    }
}

fn gather_player_movement(
    registry: Res<ConfigRegistryResource>,
    keyboard: Res<ButtonInput<KeyCode>>,
    input_state: Res<CameraInputState>,
    cameras: Query<&MmoCamera>,
    mut players: Query<
        (&mut PlayerMoveIntent, &mut MovementIntent),
        (With<Player>, Without<AwaitingSpawnTerrain>),
    >,
) {
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

    let can_jump = grounded.grounded || movement.coyote_remaining_s > 0.0;
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

const SHALLOW_WATER_DEPTH_M: f32 = 1.5;

fn enforce_water_depth(
    registry: Res<ConfigRegistryResource>,
    mut players: Query<
        (&mut Transform, &mut LinearVelocity, &mut PlayerMovementState),
        With<Player>,
    >,
) {
    let Ok(world) = registry.0.active_world() else {
        return;
    };
    let Some(water) = registry.0.water.get(&world.water) else {
        return;
    };
    let sea = water.sea_level_m;
    let deep_floor = sea - SHALLOW_WATER_DEPTH_M;

    for (mut transform, mut velocity, mut movement) in &mut players {
        let y = transform.translation.y;
        movement.in_shallow_water = y < sea && y > deep_floor;
        if y < deep_floor {
            transform.translation.y = deep_floor;
            if velocity.y < 0.0 {
                velocity.y = 0.0;
            }
        }
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
