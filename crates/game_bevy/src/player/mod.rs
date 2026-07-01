mod movement;
mod motor;
mod spawn;

pub use movement::{CharacterMotorPlugin, PlayerPlugin};
pub use motor::{
    classify_locomotion, resolve_facing_yaw, CharacterMotorState, MovementIntent, MovementSpeed,
    PlayerFacingMode,
};
pub use spawn::spawn_player;

use bevy::prelude::*;

#[derive(Component, Debug)]
pub struct Player;

#[derive(Component, Debug)]
pub struct PlayerCapsuleVisual;

#[derive(Component, Debug, Default)]
pub struct PlayerMovementState {
    pub planar_velocity: Vec2,
    pub in_shallow_water: bool,
    pub in_water: bool,
    pub submerged_depth: f32,
    pub jump_buffer_remaining_s: f32,
    pub coyote_remaining_s: f32,
    pub was_grounded: bool,
}
