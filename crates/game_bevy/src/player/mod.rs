mod movement;
mod spawn;

pub use movement::PlayerPlugin;
pub use spawn::spawn_player;

use bevy::prelude::*;

#[derive(Component, Debug)]
pub struct Player;

#[derive(Component, Debug)]
pub struct PlayerCapsuleVisual;

#[derive(Component, Debug, Default)]
pub struct PlayerMovementState {
    pub planar_velocity: Vec2,
}
