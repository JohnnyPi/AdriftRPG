// crates/game_bevy/src/camera/spawn.rs
use bevy::prelude::*;
use game_data::CompiledCamera;

use crate::environment::atmosphere::atmosphere_camera_bundle;

use super::{MainGameCamera, MmoCamera};

pub fn spawn_game_camera(
    commands: &mut Commands,
    player: Entity,
    follow_target: Entity,
    camera: &CompiledCamera,
    spawn_focus: Vec3,
) -> Entity {
    let mmo_camera = MmoCamera {
        target: follow_target,
        player,
        character_yaw: 0.0,
        orbit_yaw_offset: 0.0,
        pitch: camera.pitch_default_rad,
        distance: camera.distance_default_m,
        current_yaw: 0.0,
        current_pitch: camera.pitch_default_rad,
        current_distance: camera.distance_default_m,
        current_focus: spawn_focus,
        collision_limited_distance: camera.distance_default_m,
        recenter_active: false,
        focus_height: camera.focus_height,
        focus_offset_x: camera.focus_offset_x,
        focus_offset_z: camera.focus_offset_z,
        shoulder_offset: camera.shoulder_offset,
    };

    let entity = commands.spawn((
        MainGameCamera,
        mmo_camera,
        Camera3d::default(),
        Camera {
            order: 0,
            ..default()
        },
        Projection::Perspective(PerspectiveProjection {
            near: 0.1,
            far: 2500.0,
            ..default()
        }),
        Transform::from_translation(spawn_focus),
        atmosphere_camera_bundle(),
        DistanceFog::default(),
    ));

    entity.id()
}
