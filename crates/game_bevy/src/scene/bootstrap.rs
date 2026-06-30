use bevy::prelude::*;
use tracing::info;

use crate::data::ConfigRegistryResource;
use crate::player::spawn_player;
use crate::state::AppState;
use crate::environment::{CaveAmbientZone, SunLight};
use crate::terrain::TerrainSpawnPoint;

pub struct BootstrapScenePlugin;

impl Plugin for BootstrapScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Running), spawn_bootstrap_scene);
    }
}

fn spawn_bootstrap_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    registry: Res<ConfigRegistryResource>,
    spawn_point: Res<TerrainSpawnPoint>,
) {
    let lighting = registry.0.active_lighting().expect("lighting config");
    let performance = registry.0.active_performance().expect("performance config");
    let camera = registry.0.active_camera().expect("camera config");
    let player = registry.0.active_player().expect("player config");

    let sky_color = Color::srgb(
        lighting.fog_color[0],
        lighting.fog_color[1],
        lighting.fog_color[2],
    );
    commands.insert_resource(ClearColor(sky_color));

    ambient.color = Color::srgb(
        lighting.ambient_color[0],
        lighting.ambient_color[1],
        lighting.ambient_color[2],
    );
    ambient.brightness = lighting.ambient_brightness;

    commands.insert_resource(avian3d::prelude::Gravity(Vec3::new(
        0.0,
        -player.gravity_mps2,
        0.0,
    )));

    commands.spawn((
        SunLight,
        DirectionalLight {
            illuminance: lighting.sun_illuminance_lux,
            color: Color::srgb(
                lighting.sun_color[0],
                lighting.sun_color[1],
                lighting.sun_color[2],
            ),
            shadow_maps_enabled: lighting.sun_shadows_enabled,
            ..default()
        },
        Transform::from_rotation(Quat::from_rotation_arc(
            -Vec3::Z,
            Vec3::new(
                lighting.sun_direction[0],
                lighting.sun_direction[1],
                lighting.sun_direction[2],
            )
            .normalize_or_zero(),
        )),
    ));

    commands.spawn((
        CaveAmbientZone,
        PointLight {
            color: Color::srgb(0.35, 0.45, 0.65),
            intensity: 120000.0,
            range: 25.0,
            ..default()
        },
        Transform::from_xyz(26.0, -2.0, 12.0),
    ));

    spawn_player(
        &mut commands,
        &mut meshes,
        &mut materials,
        player,
        camera,
        lighting,
        spawn_point.0,
    );

    info!(
        target_fps = performance.target_fps,
        render_resolution = ?performance.target_resolution,
        spawn = ?spawn_point.0,
        "bootstrap scene ready"
    );
}
