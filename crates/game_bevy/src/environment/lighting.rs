use bevy::prelude::*;

use crate::camera::MainGameCamera;
use crate::data::ConfigRegistryResource;
use crate::player::Player;
use crate::state::AppState;
use crate::terrain::TerrainPipelineState;
use terrain_generation::RecipeDensitySource;

#[derive(Component)]
pub struct SunLight;

#[derive(Component)]
pub struct CaveAmbientZone;

#[derive(Component)]
pub struct SkyGradient;

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Running), spawn_sky_gradient)
            .add_systems(
                Update,
                (
                    apply_lighting_hot_reload,
                    apply_cave_atmosphere,
                )
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn spawn_sky_gradient(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(lighting) = registry.0.active_lighting() else {
        return;
    };
    let horizon = Color::srgb(
        lighting.fog_color[0],
        lighting.fog_color[1],
        lighting.fog_color[2],
    );
    let zenith = Color::srgb(
        lighting.sun_color[0] * 0.35 + 0.25,
        lighting.sun_color[1] * 0.35 + 0.45,
        lighting.sun_color[2] * 0.35 + 0.75,
    );
    commands.spawn((
        SkyGradient,
        Mesh3d(meshes.add(Sphere::new(500.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: horizon,
            emissive: LinearRgba::from(zenith),
            unlit: true,
            cull_mode: None,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}

fn apply_lighting_hot_reload(
    registry: Res<ConfigRegistryResource>,
    mut last_hash: Local<Option<String>>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<SunLight>>,
    mut fog: Query<&mut DistanceFog, With<MainGameCamera>>,
) {
    let hash = registry.0.hash.clone();
    if last_hash.as_ref() == Some(&hash) {
        return;
    }
    *last_hash = Some(hash);

    let Ok(lighting) = registry.0.active_lighting() else {
        return;
    };

    clear.0 = Color::srgb(
        lighting.fog_color[0] * 0.85,
        lighting.fog_color[1] * 0.9,
        lighting.fog_color[2] * 1.05,
    );
    ambient.color = Color::srgb(
        lighting.ambient_color[0],
        lighting.ambient_color[1],
        lighting.ambient_color[2],
    );
    ambient.brightness = lighting.ambient_brightness;

    if let Ok((mut light, mut transform)) = sun.single_mut() {
        light.illuminance = lighting.sun_illuminance_lux;
        light.color = Color::srgb(
            lighting.sun_color[0],
            lighting.sun_color[1],
            lighting.sun_color[2],
        );
        light.shadow_maps_enabled = lighting.sun_shadows_enabled;
        *transform = Transform::from_rotation(Quat::from_rotation_arc(
            -Vec3::Z,
            Vec3::new(
                lighting.sun_direction[0],
                lighting.sun_direction[1],
                lighting.sun_direction[2],
            )
            .normalize_or_zero(),
        ));
    }

    for mut distance_fog in &mut fog {
        *distance_fog = DistanceFog {
            color: Color::srgba(
                lighting.fog_color[0],
                lighting.fog_color[1],
                lighting.fog_color[2],
                1.0,
            ),
            falloff: FogFalloff::Linear {
                start: lighting.fog_start_m,
                end: lighting.fog_end_m,
            },
            ..default()
        };
    }
}

fn apply_cave_atmosphere(
    pipeline: Res<TerrainPipelineState>,
    player: Query<&Transform, With<Player>>,
    mut zones: Query<(&Transform, &mut PointLight), With<CaveAmbientZone>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    registry: Res<ConfigRegistryResource>,
) {
    let Ok(lighting) = registry.0.active_lighting() else {
        return;
    };
    let Ok(player_tf) = player.single() else {
        return;
    };
    let Some(source) = pipeline.density_source.as_ref() else {
        return;
    };

    let cave_factor = cave_depth_factor(source, player_tf.translation);
    for (tf, mut light) in &mut zones {
        let dist = player_tf.translation.distance(tf.translation);
        light.intensity = if dist < 25.0 {
            120000.0 * (1.0 - dist / 25.0) * cave_factor
        } else {
            0.0
        };
    }

    let base = lighting.ambient_brightness;
    ambient.brightness = base * (1.0 - cave_factor * 0.55);
    ambient.color = Color::srgb(
        lighting.ambient_color[0] * (1.0 - cave_factor * 0.3),
        lighting.ambient_color[1] * (1.0 - cave_factor * 0.2),
        lighting.ambient_color[2] * (1.0 - cave_factor * 0.1) + cave_factor * 0.15,
    );
}

fn cave_depth_factor(source: &RecipeDensitySource, position: Vec3) -> f32 {
    let sea = source.recipe().sea_level;
    if position.y > sea + 2.0 {
        return 0.0;
    }
    let density = source.density_at(position.x, position.y, position.z);
    if density > 0.0 {
        return ((sea + 2.0 - position.y) / 8.0).clamp(0.0, 1.0);
    }
    ((sea - position.y + 4.0) / 10.0).clamp(0.0, 1.0)
}

/// Stub trait for future global illumination / light propagation.
#[allow(dead_code)]
pub trait LightPropagationBackend: Send + Sync {
    fn propagate(&self, _origin: Vec3) -> f32 {
        1.0
    }
}

#[allow(dead_code)]
pub struct StubLightPropagation;

impl LightPropagationBackend for StubLightPropagation {}
