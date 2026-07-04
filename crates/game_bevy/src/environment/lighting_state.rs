// crates/game_bevy/src/environment/lighting_state.rs
//! Lighting config from YAML plus runtime exposure/ambient outputs (VS2 §12).
//! Live sun/moon presentation lives in [`super::celestial::CelestialState`].

use bevy::camera::Exposure;
use bevy::light::light_consts::lux;
use bevy::light::{AtmosphereEnvironmentMapLight, VolumetricLight};
use bevy::prelude::*;

use super::celestial::{CelestialState, MoonLight};
use crate::camera::MainGameCamera;
use crate::ui::{SUN_PEAK_SCALE, moon_gameplay_illuminance};

#[derive(Resource, Clone, Debug)]
pub struct EnvironmentLightingState {
    /// YAML sun azimuth; used when time-of-day and manual overrides are off.
    pub authored_sun_azimuth_deg: f32,
    /// YAML sun elevation; used when time-of-day and manual overrides are off.
    pub authored_sun_elevation_deg: f32,
    /// When true, sun angles follow [`SimulationTime::time_of_day_hours`].
    pub drive_sun_from_time_of_day: bool,
    /// When true, sun angles use [`Self::override_sun_azimuth_deg`] / elevation.
    pub override_sun_angles: bool,
    pub override_sun_azimuth_deg: f32,
    pub override_sun_elevation_deg: f32,
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
    /// Phase offset into the lunar cycle (0..1) from authored atmosphere YAML.
    pub moon_phase_offset: f32,
    /// Scales peak sun lux relative to the elevation curve (`illuminance_lux / 100_000`).
    pub sun_illuminance_lux_scale: f32,
    /// Last frame's sun-driven ambient (before cave dimming).
    pub effective_ambient_brightness: f32,
}

impl Default for EnvironmentLightingState {
    fn default() -> Self {
        Self {
            authored_sun_azimuth_deg: 132.0,
            authored_sun_elevation_deg: 48.0,
            drive_sun_from_time_of_day: true,
            override_sun_angles: false,
            override_sun_azimuth_deg: 145.0,
            override_sun_elevation_deg: 42.0,
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
            moon_phase_offset: 0.0,
            sun_illuminance_lux_scale: 1.0,
            effective_ambient_brightness: 0.0,
        }
    }
}

impl EnvironmentLightingState {
    /// Bootstrap from compiled `atmosphere.yaml` on world enter.
    pub fn apply_authored_atmosphere(&mut self, atmo: &game_data::CompiledAtmosphere) {
        self.authored_sun_azimuth_deg = atmo.sun_azimuth_deg;
        self.authored_sun_elevation_deg = atmo.sun_elevation_deg;
        self.override_sun_azimuth_deg = atmo.sun_azimuth_deg;
        self.override_sun_elevation_deg = atmo.sun_elevation_deg;
        self.ambient_brightness = atmo.ambient_brightness;
        self.ambient_color = atmo.ambient_color;
        self.exposure_ev_min = atmo.exposure_ev_min;
        self.exposure_ev_max = atmo.exposure_ev_max;
        self.exposure_bias = atmo.exposure_bias;
        self.exposure_adaptation_speed = atmo.exposure_adaptation_speed;
        self.environment_intensity_scale = atmo.environment_intensity_scale;
        self.moon_enabled = atmo.moon_enabled;
        self.moon_illuminance = atmo.moon_illuminance;
        self.moon_phase_offset = atmo.moon_phase.clamp(0.0, 1.0);
        self.sun_illuminance_lux_scale = (atmo.sun_illuminance_lux / 100_000.0).max(0.0);
        self.drive_sun_from_time_of_day = true;
        self.current_exposure = crate::ui::exposure_ev_for_elevation(
            atmo.sun_elevation_deg,
            atmo.exposure_ev_min,
            atmo.exposure_ev_max,
            atmo.exposure_bias,
        );
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct SkyVisibility {
    /// Outdoor skylight reaching the player (0..1).
    pub sky: f32,
    /// Cave/underwater depth factor for ambient dimming (0..1).
    pub cave_depth: f32,
}

impl Default for SkyVisibility {
    fn default() -> Self {
        Self {
            sky: 1.0,
            cave_depth: 0.0,
        }
    }
}

pub fn sun_cloud_transmission(cloud_cover: f32) -> f32 {
    1.0 - cloud_cover * 0.75
}

pub fn environment_intensity_with_clouds(base: f32, cloud_cover: f32) -> f32 {
    base * (1.0 - cloud_cover * 0.45)
}

pub fn sun_direction_from_angles(azimuth_deg: f32, elevation_deg: f32) -> Vec3 {
    let az = azimuth_deg.to_radians();
    let el = elevation_deg.to_radians();
    Vec3::new(el.cos() * az.sin(), -el.sin(), el.cos() * az.cos()).normalize_or_zero()
}

pub struct EnvironmentLightingPlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct SyncEnvironmentLightingSet;

impl Plugin for EnvironmentLightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnvironmentLightingState>()
            .configure_sets(Update, SyncEnvironmentLightingSet)
            .add_systems(
                Update,
                sync_environment_lighting.in_set(SyncEnvironmentLightingSet),
            );
    }
}

fn sync_environment_lighting(
    time: Res<Time>,
    celestial: Res<CelestialState>,
    mut state: ResMut<EnvironmentLightingState>,
    mut commands: Commands,
    mut sun: Query<
        (
            Entity,
            &mut DirectionalLight,
            &mut Transform,
            Option<&VolumetricLight>,
        ),
        (With<super::SunLight>, Without<MoonLight>),
    >,
    mut moon: Query<(&mut DirectionalLight, &mut Transform), With<MoonLight>>,
    mut exposure: Query<&mut Exposure, With<MainGameCamera>>,
    mut env_map: Query<&mut AtmosphereEnvironmentMapLight, With<MainGameCamera>>,
) {
    let cloud_transmission = sun_cloud_transmission(celestial.cloud_cover);
    let daylight =
        crate::ui::sun_illuminance_for_elevation(celestial.sun_elevation_deg) / 100_000.0;
    let sun_illuminance = lux::RAW_SUNLIGHT
        * daylight.clamp(0.0, 1.0)
        * cloud_transmission
        * SUN_PEAK_SCALE
        * state.sun_illuminance_lux_scale;

    for (entity, mut light, mut transform, volumetric) in &mut sun {
        light.illuminance = sun_illuminance;
        light.color = Color::srgb(
            celestial.sun_color[0],
            celestial.sun_color[1],
            celestial.sun_color[2],
        );
        *transform =
            Transform::from_rotation(Quat::from_rotation_arc(-Vec3::Z, celestial.sun_direction));

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

    let env_intensity =
        environment_intensity_with_clouds(celestial.environment_intensity, celestial.cloud_cover);
    for mut env in &mut env_map {
        env.intensity = env_intensity;
    }

    state.effective_ambient_brightness = crate::ui::ambient_brightness_for_elevation(
        celestial.sun_elevation_deg,
        state.ambient_brightness,
    );
    // `apply_cave_atmosphere` owns `GlobalAmbientLight.brightness` every frame.

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
    use crate::ui::sun_illuminance_for_elevation;

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

    #[test]
    fn sun_lux_scale_multiplies_peak_illuminance() {
        let scale = 0.5;
        let daylight = sun_illuminance_for_elevation(45.0) / 100_000.0;
        let scaled = 100_000.0 * daylight * scale * crate::ui::SUN_PEAK_SCALE;
        let full = 100_000.0 * daylight * crate::ui::SUN_PEAK_SCALE;
        assert!(scaled < full);
        assert!((scaled / full - scale).abs() < 1e-4);
    }
}
