// crates/physics_bridge/src/body_spec.rs
//! Project-owned physics body specifications.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PhysicsBodyType {
    #[default]
    Static,
    Dynamic,
    Kinematic,
    Sensor,
}

#[derive(Clone, Debug)]
pub struct PhysicsBodySpec {
    pub body_type: PhysicsBodyType,
    pub mass: f32,
    pub friction: f32,
    pub restitution: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub collision_profile: CollisionProfileId,
}

impl Default for PhysicsBodySpec {
    fn default() -> Self {
        Self {
            body_type: PhysicsBodyType::Dynamic,
            mass: 1.0,
            friction: 0.6,
            restitution: 0.1,
            linear_damping: 0.2,
            angular_damping: 0.5,
            collision_profile: CollisionProfileId::DynamicProp,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CollisionProfileId {
    #[default]
    Terrain,
    Player,
    DynamicProp,
    StaticProp,
    WaterSensor,
    CameraProbe,
    MovingPlatform,
}
