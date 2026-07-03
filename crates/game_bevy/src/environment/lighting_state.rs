// crates/game_bevy/src/environment/lighting_state.rs
//! Unified environment lighting state (VS2 §12).

use bevy::camera::Exposure;
use bevy::light::light_consts::lux;
use bevy::light::{AtmosphereEnvironmentMapLight, VolumetricLight};
use bevy::prelude::*;

use super::celestial::{CelestialState, MoonLight};
use crate::camera::MainGameCamera;
use crate::ui::{
    environment_intensity_for_elevation, moon_gameplay_illuminance, SUN_PEAK_SCALE,
};

#[derive(Resource, Clone, Debug)]
pub struct EnvironmentLightingState {
    pub sun_azimuth_deg: f32,
    pub sun_elevation_deg: f32,
    pub sun_illuminance: f32,
    pub sun_color: [f32; 3],
    pub ambient_brightness: f32,
    pub ambient_color: [f32; 3],
    pub exposure_ev_min: f32,
    pub exposure_ev_max: f32,
    pub exposure_bias: f32,
    pub exposure_adaptation_speed: f32,
    pub current_exposure: f32,
    pub environment_intensity_scale: f32,
    pub moon_enabled: bool,
    pub moon_illuminance: f32,
    pub moon_azimuth_deg: f32,
    pub moon_elevation_deg: f32,
    /// Last frame's sun-driven ambient (before cave dimming).
    pub effective_ambient_brightness: f32,
}

impl Default for EnvironmentLightingState {
    fn default() -> Self {
        Self {
            sun_azimuth_deg: 132.0,
            sun_elevation_deg: 48.0,
            sun_illuminance: lux::RAW_SUNLIGHT,
            sun_color: [1.0, 0.97, 0.92],
            ambient_brightness: 0.0,
            ambient_color: [0.55, 0.62, 0.75],
            exposure_ev_min: 9.0,
            exposure_ev_max: 15.0,
            exposure_bias: 0.0,
            exposure_adaptation_speed: 2.5,
            current_exposure: 13.0,
            environment_intensity_scale: 1.0,
            moon_enabled: false,
            moon_illuminance: 2.0,
            moon_azimuth_deg: 315.0,
            moon_elevation_deg: 35.0,
            effective_ambient_brightness: 0.0,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SkyVisibility(pub f32);

pub fn sun_cloud_transmission(cloud_cover: f32) -> f32 {
    1.0 - cloud_cover * 0.75
}

pub fn environment_intensity_with_clouds(base: f32, cloud_cover: f32) -> f32 {
    base * (1.0 - cloud_cover * 0.45)
}

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

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct SyncEnvironmentLightingSet;

impl Plugin for EnvironmentLightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnvironmentLightingState>()
            .configure_sets(Update, SyncEnvironmentLightingSet)
            .add_systems(Update, sync_environment_lighting.in_set(SyncEnvironmentLightingSet));
    }
}

fn sync_environment_lighting(
    time: Res<Time>,
    celestial: Res<CelestialState>,
    mut state: ResMut<EnvironmentLightingState>,
    mut commands: Commands,
    mut sun: Query<
        (Entity, &mut DirectionalLight, &mut Transform, Option<&VolumetricLight>),
        (With<super::SunLight>, Without<MoonLight>),
    >,
    mut moon: Query<(&mut DirectionalLight, &mut Transform), With<MoonLight>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut exposure: Query<&mut Exposure, With<MainGameCamera>>,
    mut env_map: Query<&mut AtmosphereEnvironmentMapLight, With<MainGameCamera>>,
) {
    state.sun_azimuth_deg = celestial.sun_azimuth_deg;
    state.sun_elevation_deg = celestial.sun_elevation_deg;
    state.sun_color = celestial.sun_color;

    let cloud_transmission = sun_cloud_transmission(celestial.cloud_cover);
    let daylight = crate::ui::sun_illuminance_for_elevation(celestial.sun_elevation_deg) / 100_000.0;
    let sun_illuminance =
        lux::RAW_SUNLIGHT * daylight.clamp(0.0, 1.0) * cloud_transmission * SUN_PEAK_SCALE;

    for (entity, mut light, mut transform, volumetric) in &mut sun {
        light.illuminance = sun_illuminance;
        light.color = Color::srgb(
            celestial.sun_color[0],
            celestial.sun_color[1],
            celestial.sun_color[2],
        );
        *transform = Transform::from_rotation(Quat::from_rotation_arc(
            -Vec3::Z,
            celestial.sun_direction,
        ));

        let volumetric_active = celestial.sun_elevation_deg >= 5.0;
        match (volumetric_active, volumetric.is_some()) {
            (true, false) => {
                commands.entity(entity).insert(VolumetricLight);
            }
            (false, true) => {
                commands.entity(entity).remove::<VolumetricLight>();
            }
            _ => {}
        }
    }

    if state.moon_enabled {
        let moon_lux = moon_gameplay_illuminance(
            celestial.sun_elevation_deg,
            celestial.moon_elevation_deg,
            celestial.moon_phase,
            state.moon_illuminance,
            celestial.cloud_cover,
        );
        for (mut light, mut transform) in &mut moon {
            light.illuminance = moon_lux;
            *transform = Transform::from_rotation(Quat::from_rotation_arc(
                -Vec3::Z,
                celestial.moon_direction,
            ));
        }
    } else {
        for (mut light, _) in &mut moon {
            light.illuminance = 0.0;
        }
    }

    let env_intensity = environment_intensity_with_clouds(
        environment_intensity_for_elevation(
            celestial.sun_elevation_deg,
            state.environment_intensity_scale,
        ),
        celestial.cloud_cover,
    );
    for mut env in &mut env_map {
        env.intensity = env_intensity;
    }

    state.effective_ambient_brightness = crate::ui::ambient_brightness_for_elevation(
        celestial.sun_elevation_deg,
        state.ambient_brightness,
    );
    ambient.brightness = 0.0;

    let target_ev = celestial.exposure_ev100;
    let speed = state.exposure_adaptation_speed;
    if speed > 0.0 {
        let alpha = (speed * time.delta_secs()).min(1.0);
        state.current_exposure += (target_ev - state.current_exposure) * alpha;
    } else {
        state.current_exposure = target_ev;
    }

    for mut cam_exposure in &mut exposure {
        cam_exposure.ev100 = state.current_exposure;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_cover_attenuates_sun_transmission() {
        assert!(sun_cloud_transmission(1.0) < sun_cloud_transmission(0.0));
        assert!((sun_cloud_transmission(1.0) - 0.25).abs() < 1e-5);
    }

    #[test]
    fn cloud_cover_reduces_environment_intensity() {
        let clear = environment_intensity_with_clouds(1.0, 0.0);
        let overcast = environment_intensity_with_clouds(1.0, 1.0);
        assert!(overcast < clear);
    }
}
