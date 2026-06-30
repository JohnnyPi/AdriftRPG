use bevy::prelude::*;
use game_data::{CompiledCamera, CompiledLighting, CompiledPlayer};

use crate::camera::{
    spawn_game_camera, CameraFollowTarget, CameraPivot, CameraRig, CharacterFacing,
    PlayerInterpolation,
};
use crate::physics::attach_character_physics;

use super::{Player, PlayerCapsuleVisual, PlayerMovementState};

pub fn spawn_player(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    player: &CompiledPlayer,
    camera: &CompiledCamera,
    lighting: &CompiledLighting,
    spawn_position: Vec3,
) {
    let capsule_center_y = player.capsule_half_height_m + player.capsule_radius_m;
    let focus_offset = Vec3::new(
        camera.focus_offset_x,
        camera.focus_height,
        camera.focus_offset_z,
    );
    let spawn_center = spawn_position + Vec3::Y * capsule_center_y;
    let focus_offset_from_center = focus_offset - Vec3::Y * capsule_center_y;
    let spawn_focus = spawn_position + focus_offset;

    let mut player_entity = commands.spawn((
        Player,
        PlayerMovementState::default(),
        PlayerInterpolation {
            previous: spawn_center,
            current: spawn_center,
        },
        CharacterFacing {
            yaw: 0.0,
            desired_yaw: 0.0,
            turn_speed: player.rotation_speed_deg_per_s.to_radians(),
        },
        crate::environment::lighting_state::SkyVisibility(1.0),
        Transform::from_translation(spawn_center),
        Visibility::default(),
    ));
    attach_character_physics(player, &mut player_entity);
    let player_id = player_entity.id();

    let mut follow_target_id = Entity::PLACEHOLDER;
    player_entity.with_children(|player_root| {
        player_root.spawn((
            PlayerCapsuleVisual,
            Mesh3d(meshes.add(Capsule3d::new(
                player.capsule_radius_m,
                player.capsule_half_height_m,
            ))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.72, 0.68, 0.62),
                ..default()
            })),
            Transform::IDENTITY,
        ));

        follow_target_id = player_root
            .spawn((
                CameraFollowTarget,
                Transform::from_translation(focus_offset_from_center),
            ))
            .with_children(|follow| {
                follow
                    .spawn((CameraRig, Transform::IDENTITY))
                    .with_children(|rig| {
                        rig.spawn((CameraPivot, Transform::IDENTITY));
                    });
            })
            .id();
    });

    spawn_game_camera(
        commands,
        player_id,
        follow_target_id,
        camera,
        lighting,
        spawn_focus,
    );
}
