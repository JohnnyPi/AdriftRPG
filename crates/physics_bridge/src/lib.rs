//! Physics integration wrappers using Avian3D.

mod body_spec;
mod character;
mod collision;
mod collision_layers;
mod plugin;

pub use body_spec::{CollisionProfileId, PhysicsBodySpec, PhysicsBodyType};
pub use character::{
    CharacterController, CharacterControllerBundle, CharacterControllerPlugin,
    CharacterPhysicsSystems, GroundedState,
};
pub use collision::CharacterCollisionQuery;
pub use collision_layers::{
    camera_probe_layers, dynamic_prop_layers, layers_for_profile, moving_platform_layers,
    player_layers, terrain_layers, water_sensor_layers, CollisionLayer,
};
pub use avian3d::prelude::LinearVelocity;
pub use plugin::PhysicsBridgePlugin;
