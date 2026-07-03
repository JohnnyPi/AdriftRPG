// crates/game_bevy/src/scene/bootstrap.rs
use bevy::light::{light_consts::lux, CascadeShadowConfig, CascadeShadowConfigBuilder};
use bevy::prelude::*;
use tracing::info;

use crate::data::ConfigRegistryResource;
use crate::environment::atmosphere::{atmosphere_clear_color, attach_volumetric_sun};
use crate::environment::celestial::MoonLight;
use crate::environment::lighting_state::sun_direction_from_angles;
use crate::environment::{SunLight};
use crate::player::spawn_player;
use crate::state::AppState;
use crate::terrain::{TerrainSpawnPoint, TerrainWorldInitSet};
use crate::environment::config_init::EnvironmentInitSet;

pub struct BootstrapScenePlugin;

impl Plugin for BootstrapScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::Running),
            spawn_bootstrap_scene
                .after(TerrainWorldInitSet)
                .after(EnvironmentInitSet),
        );
    }
}

fn spawn_bootstrap_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    registry: Res<ConfigRegistryResource>,
    spawn_point: Res<TerrainSpawnPoint>,
) {
    let lighting = registry.0.active_lighting().expect("lighting config");
    let performance = registry.0.active_performance().expect("performance config");
    let camera = registry.0.active_camera().expect("camera config");
    let player = registry.0.active_player().expect("player config");
    let physics_gravity = registry
        .0
        .active_physics()
        .map(|p| p.gravity_mps2)
        .unwrap_or(player.gravity_mps2);

    let atmo = registry.0.active_atmosphere();
    let sun_dir = atmo
        .map(|atmo| sun_direction_from_angles(atmo.sun_azimuth_deg, atmo.sun_elevation_deg))
        .unwrap_or_else(|| {
            Vec3::new(
                lighting.sun_direction[0],
                lighting.sun_direction[1],
                lighting.sun_direction[2],
            )
            .normalize_or_zero()
        });
    let sun_color = atmo
        .map(|atmo| atmo.sun_color)
        .unwrap_or(lighting.sun_color);

    commands.insert_resource(atmosphere_clear_color());
    commands.insert_resource(GlobalAmbientLight::NONE);

    commands.insert_resource(avian3d::prelude::Gravity(Vec3::new(
        0.0,
        -physics_gravity,
        0.0,
    )));

    let sun_entity = commands
        .spawn((
            SunLight,
            DirectionalLight {
                illuminance: lux::RAW_SUNLIGHT,
                color: Color::srgb(sun_color[0], sun_color[1], sun_color[2]),
                shadow_maps_enabled: lighting.sun_shadows_enabled && performance.shadows_enabled,
                ..default()
            },
            cascade_shadow_config(&performance.shadow_quality),
            Transform::from_rotation(Quat::from_rotation_arc(-Vec3::Z, sun_dir)),
        ))
        .id();
    attach_volumetric_sun(&mut commands, sun_entity);

    if let Some(atmo) = atmo {
        if atmo.moon_enabled {
            let moon_dir = sun_direction_from_angles(atmo.moon_azimuth_deg, atmo.moon_elevation_deg);
            commands.spawn((
                MoonLight,
                DirectionalLight {
                    illuminance: atmo.moon_illuminance,
                    color: Color::srgb(0.72, 0.78, 0.92),
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_rotation(Quat::from_rotation_arc(-Vec3::Z, moon_dir)),
            ));
        }
    }

    // Demo cave beacon light is spawned by the interaction module when present.

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

fn cascade_shadow_config(quality: &str) -> CascadeShadowConfig {
    let builder = match quality.to_ascii_lowercase().as_str() {
        "low" => CascadeShadowConfigBuilder {
            num_cascades: 2,
            minimum_distance: 0.5,
            maximum_distance: 80.0,
            ..default()
        },
        "medium" => CascadeShadowConfigBuilder {
            num_cascades: 3,
            minimum_distance: 0.5,
            maximum_distance: 120.0,
            ..default()
        },
        _ => CascadeShadowConfigBuilder {
            num_cascades: 4,
            minimum_distance: 0.5,
            maximum_distance: 180.0,
            ..default()
        },
    };
    builder.into()
}
