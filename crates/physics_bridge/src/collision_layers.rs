//! Project-owned collision layer definitions mapped to Avian.

use avian3d::prelude::*;

#[derive(PhysicsLayer, Clone, Copy, Debug, Default)]
pub enum CollisionLayer {
    #[default]
    Terrain,
    Player,
    Npc,
    DynamicProp,
    StaticProp,
    WaterSensor,
    InteractionSensor,
    CameraProbe,
    Trigger,
}

pub fn terrain_layers() -> CollisionLayers {
    CollisionLayers::new(
        [CollisionLayer::Terrain],
        [
            CollisionLayer::Player,
            CollisionLayer::Npc,
            CollisionLayer::DynamicProp,
            CollisionLayer::StaticProp,
            CollisionLayer::CameraProbe,
        ],
    )
}

pub fn player_layers() -> CollisionLayers {
    CollisionLayers::new(
        [CollisionLayer::Player],
        [
            CollisionLayer::Terrain,
            CollisionLayer::DynamicProp,
            CollisionLayer::StaticProp,
            CollisionLayer::Trigger,
        ],
    )
}

pub fn dynamic_prop_layers() -> CollisionLayers {
    CollisionLayers::new(
        [CollisionLayer::DynamicProp],
        [
            CollisionLayer::Terrain,
            CollisionLayer::Player,
            CollisionLayer::Npc,
            CollisionLayer::DynamicProp,
            CollisionLayer::StaticProp,
        ],
    )
}

pub fn water_sensor_layers() -> CollisionLayers {
    CollisionLayers::new(
        [CollisionLayer::WaterSensor],
        [CollisionLayer::Player, CollisionLayer::DynamicProp],
    )
}

pub fn camera_probe_layers() -> CollisionLayers {
    CollisionLayers::new([CollisionLayer::CameraProbe], [CollisionLayer::Terrain])
}

pub fn moving_platform_layers() -> CollisionLayers {
    CollisionLayers::new(
        [CollisionLayer::StaticProp],
        [
            CollisionLayer::Terrain,
            CollisionLayer::Player,
            CollisionLayer::DynamicProp,
        ],
    )
}

pub fn layers_for_profile(profile: crate::body_spec::CollisionProfileId) -> CollisionLayers {
    use crate::body_spec::CollisionProfileId;
    match profile {
        CollisionProfileId::Terrain => terrain_layers(),
        CollisionProfileId::Player => player_layers(),
        CollisionProfileId::DynamicProp => dynamic_prop_layers(),
        CollisionProfileId::StaticProp => moving_platform_layers(),
        CollisionProfileId::WaterSensor => water_sensor_layers(),
        CollisionProfileId::CameraProbe => camera_probe_layers(),
        CollisionProfileId::MovingPlatform => moving_platform_layers(),
    }
}
