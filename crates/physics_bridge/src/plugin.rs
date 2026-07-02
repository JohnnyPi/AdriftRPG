// crates/physics_bridge/src/plugin.rs
use avian3d::prelude::*;
use bevy::prelude::*;

pub struct PhysicsBridgePlugin;

impl Plugin for PhysicsBridgePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PhysicsPlugins::default())
            .insert_resource(Gravity(Vec3::new(0.0, -18.0, 0.0)));
    }
}
