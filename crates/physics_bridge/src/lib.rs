//! Physics integration wrappers using Avian3D.

mod character;
mod collision;
mod plugin;

pub use character::{
    CharacterController, CharacterControllerBundle, CharacterControllerPlugin,
    CharacterPhysicsSystems, GroundedState,
};
pub use collision::CharacterCollisionQuery;
pub use avian3d::prelude::LinearVelocity;
pub use plugin::PhysicsBridgePlugin;
