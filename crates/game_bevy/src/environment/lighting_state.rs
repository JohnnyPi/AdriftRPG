// crates/game_bevy/src/environment/lighting_state.rs
//! Unified environment lighting state (VS2 §12).

use bevy::prelude::*;

#[derive(Resource, Clone, Debug)]
pub struct EnvironmentLightingState {
    pub sun_azimuth_deg: f32,
    pub sun_elevation_deg: f32,
    pub sun_illuminance: f32,
    pub sun_color: [f32; 3],
    pub ambient_brightness: f32,
    pub ambient_color: [f32; 3],
    pub exposure_min: f32,
    pub exposure_max: f32,
    pub exposure_target: f32,
    pub exposure_adaptation_speed: f32,
    pub current_exposure: f32,
    pub moon_enabled: bool,
    pub moon_illuminance: f32,
}

impl Default for EnvironmentLightingState {
    fn default() -> Self {
        Self {
            sun_azimuth_deg: 132.0,
            sun_elevation_deg: 48.0,
            sun_illuminance: 100_000.0,
            sun_color: [1.0, 0.97, 0.92],
            ambient_brightness: 300.0,
            ambient_color: [0.55, 0.62, 0.75],
            exposure_min: 0.4,
            exposure_max: 1.6,
            exposure_target: 1.0,
            exposure_adaptation_speed: 2.5,
            current_exposure: 1.0,
            moon_enabled: false,
            moon_illuminance: 0.15,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SkyVisibility(pub f32);

pub fn sun_direction_from_angles(azimuth_deg: f32, elevation_deg: f32) -> Vec3 {
    let az = azimuth_deg.to_radians();
    let el = elevation_deg.to_radians();
    Vec3::new(
        el.cos() * az.sin(),
        -el.sin(),
        el.cos() * az.cos(),
    )
    .normalize_or_zero()
}

pub struct EnvironmentLightingPlugin;

impl Plugin for EnvironmentLightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnvironmentLightingState>()
            .add_systems(Update, sync_environment_lighting);
    }
}

fn sync_environment_lighting(
    mut state: ResMut<EnvironmentLightingState>,
    tweaks: Option<Res<crate::ui::AtmosphereTweaks>>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<super::SunLight>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    time: Res<Time>,
) {
    if let Some(tweaks) = tweaks {
        if tweaks.use_overrides {
            state.sun_azimuth_deg = tweaks.sun_azimuth_deg;
            state.sun_elevation_deg = tweaks.sun_elevation_deg;
            state.exposure_min = tweaks.exposure_min;
            state.exposure_max = tweaks.exposure_max;
        }
    }

    let target = state.exposure_target;
    let speed = state.exposure_adaptation_speed * time.delta_secs();
    state.current_exposure += (target - state.current_exposure) * speed;
    state.current_exposure = state
        .current_exposure
        .clamp(state.exposure_min, state.exposure_max);

    let dir = sun_direction_from_angles(state.sun_azimuth_deg, state.sun_elevation_deg);
    for (mut light, mut transform) in &mut sun {
        light.illuminance = state.sun_illuminance * state.current_exposure;
        light.color = Color::srgb(state.sun_color[0], state.sun_color[1], state.sun_color[2]);
        *transform = Transform::from_rotation(Quat::from_rotation_arc(-Vec3::Z, dir));
    }
    ambient.brightness = state.ambient_brightness * state.current_exposure;
    ambient.color = Color::srgb(
        state.ambient_color[0],
        state.ambient_color[1],
        state.ambient_color[2],
    );
}
