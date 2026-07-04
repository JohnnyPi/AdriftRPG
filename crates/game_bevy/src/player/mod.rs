// crates/game_bevy/src/player/mod.rs
mod motor;
mod movement;
mod spawn;

pub use motor::{
    CharacterMotorState, MovementIntent, MovementSpeed, PlayerFacingMode, classify_locomotion,
    resolve_facing_yaw,
};
pub use movement::{CharacterMotorPlugin, PlayerPlugin};
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
