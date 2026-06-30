use bevy::prelude::*;
use physics_bridge::{
    CharacterController, CharacterControllerBundle, CharacterControllerPlugin,
    CharacterPhysicsSystems, GroundedState, LinearVelocity, PhysicsBridgePlugin,
};
use game_data::CompiledPlayer;

use crate::camera::{
    camera_forward_xz, camera_right_xz, CameraInputState, CharacterFacing, MmoCamera,
    PlayerInterpolation,
};
use crate::data::ConfigRegistryResource;
use crate::player::{Player, PlayerCapsuleVisual, PlayerMovementState};
use crate::state::AppState;
use crate::terrain::{ChunkState, TerrainPipelineState};
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
}

impl Plugin for GamePhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((PhysicsBridgePlugin, CharacterControllerPlugin))
            .add_systems(
                FixedUpdate,
                (
                    tag_player_awaiting_terrain,
                    hold_player_until_spawn_terrain,
                    gather_player_movement,
                    apply_character_movement,
                )
                    .chain()
                    .before(CharacterPhysicsSystems)
                    .run_if(in_state(AppState::Running)),
            )
            .add_systems(
                FixedUpdate,
                (
                    update_character_facing,
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
        PlayerMoveIntent::default(),
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
    mut players: Query<&mut PlayerMoveIntent, (With<Player>, Without<AwaitingSpawnTerrain>)>,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };
    let Ok(mut intent) = players.single_mut() else {
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
    intent.direction = (forward * axis.y + right * axis.x).normalize_or_zero();
    intent.sprinting =
        keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    intent.jumping = keyboard.just_pressed(KeyCode::Space);
}

fn apply_character_movement(
    time: Res<Time<Fixed>>,
    registry: Res<ConfigRegistryResource>,
    intent: Query<&PlayerMoveIntent, (With<Player>, Without<AwaitingSpawnTerrain>)>,
    mut players: Query<
        (
            &CharacterController,
            &GroundedState,
            &mut LinearVelocity,
            &mut PlayerMovementState,
        ),
        (With<Player>, Without<AwaitingSpawnTerrain>),
    >,
) {
    let Ok(move_intent) = intent.single() else {
        return;
    };
    let Ok((controller, grounded, mut velocity, mut movement)) = players.single_mut() else {
        return;
    };

    let player = registry.0.active_player().expect("player config");
    let dt = time.delta_secs();

    let target_speed = if move_intent.sprinting {
        controller.run_speed
    } else {
        controller.walk_speed
    };

    let desired_velocity = Vec3::new(
        move_intent.direction.x,
        0.0,
        move_intent.direction.z,
    ) * target_speed;
    let current_planar = Vec2::new(velocity.x, velocity.z);
    let desired_planar = Vec2::new(desired_velocity.x, desired_velocity.z);

    let accelerating = desired_planar.length_squared() > current_planar.length_squared();
    let rate = if accelerating {
        player.acceleration_mps2
    } else {
        player.deceleration_mps2
    };

    let new_planar = approach_vec2(current_planar, desired_planar, rate * dt);
    velocity.x = new_planar.x;
    velocity.z = new_planar.y;
    movement.planar_velocity = new_planar;

    if move_intent.jumping && grounded.grounded {
        velocity.y = controller.jump_speed;
    }
}

fn update_character_facing(
    time: Res<Time<Fixed>>,
    input_state: Res<CameraInputState>,
    cameras: Query<&MmoCamera>,
    intent: Query<&PlayerMoveIntent, With<Player>>,
    mut players: Query<(&mut CharacterFacing, &mut Transform), With<Player>>,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };
    let Ok(move_intent) = intent.single() else {
        return;
    };
    let Ok((mut facing, mut transform)) = players.single_mut() else {
        return;
    };

    if input_state.steering_character() {
        facing.desired_yaw = camera.intent_yaw();
    } else if move_intent.direction.length_squared() > 0.001 {
        facing.desired_yaw = move_intent.direction.x.atan2(move_intent.direction.z);
    }

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
