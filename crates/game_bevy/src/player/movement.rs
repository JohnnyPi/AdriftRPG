use bevy::prelude::*;

/// VS2 §9 character motor — fixed-step systems are registered by [`crate::physics::GamePhysicsPlugin`].
pub struct CharacterMotorPlugin;

impl Plugin for CharacterMotorPlugin {
    fn build(&self, _app: &mut App) {}
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, _app: &mut App) {}
}
