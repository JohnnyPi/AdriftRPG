//! Shared celestial state driving sun, moon, fog, and exposure (SkyLightingGuide §1).

use bevy::prelude::*;

use super::lighting_state::sun_direction_from_angles;
use super::sky_config::{night_mix_from_elevation, SkyPresentationConfig};
use crate::state::AppState;
use crate::ui::AtmosphereTweaks;

#[derive(Resource, Clone, Debug)]
pub struct CelestialState {
    pub time_of_day_hours: f32,
    pub sun_azimuth_deg: f32,
    pub sun_elevation_deg: f32,
    /// Direction sunlight travels (matches directional light forward convention).
    pub sun_direction: Vec3,
    pub sun_color: [f32; 3],
    pub moon_azimuth_deg: f32,
    pub moon_elevation_deg: f32,
    pub moon_direction: Vec3,
    pub moon_phase: f32,
    pub moon_enabled: bool,
    pub moon_illuminance: f32,
    pub cloud_cover: f32,
    /// Target exposure in EV100 for the main camera.
    pub exposure_ev100: f32,
    /// Atmosphere environment-map intensity for the current frame.
    pub environment_intensity: f32,
    /// Fog inscattering tint blended at twilight.
    pub fog_inscattering: [f32; 3],
    pub fog_extinction: [f32; 3],
}

impl Default for CelestialState {
    fn default() -> Self {
        let (azimuth, elevation) = crate::ui::sun_angles_from_time_of_day(10.0);
        Self {
            time_of_day_hours: 10.0,
            sun_azimuth_deg: azimuth,
            sun_elevation_deg: elevation,
            sun_direction: sun_direction_from_angles(azimuth, elevation),
            sun_color: [1.0, 0.97, 0.92],
            moon_azimuth_deg: 315.0,
            moon_elevation_deg: 35.0,
            moon_direction: sun_direction_from_angles(315.0, 35.0),
            moon_phase: 1.0,
            moon_enabled: false,
            moon_illuminance: 2.0,
            cloud_cover: 0.0,
            exposure_ev100: 13.0,
            environment_intensity: 0.9,
            fog_inscattering: [0.72, 0.78, 0.88],
            fog_extinction: [0.58, 0.68, 0.76],
        }
    }
}

#[derive(Component)]
pub struct MoonLight;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct UpdateCelestialSet;

pub struct CelestialPlugin;

impl Plugin for CelestialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CelestialState>()
            .configure_sets(Update, UpdateCelestialSet.before(super::lighting_state::SyncEnvironmentLightingSet))
            .add_systems(
                Update,
                update_celestial_state
                    .in_set(UpdateCelestialSet)
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn update_celestial_state(
    mut celestial: ResMut<CelestialState>,
    tweaks: Res<AtmosphereTweaks>,
    sky: Res<SkyPresentationConfig>,
    lighting: Res<super::lighting_state::EnvironmentLightingState>,
) {
    let mut azimuth = lighting.sun_azimuth_deg;
    let mut elevation = lighting.sun_elevation_deg;
    let mut time_hours = tweaks.time_of_day_hours;

    if tweaks.drive_sun_from_time_of_day {
        let (az, el) = crate::ui::sun_angles_from_time_of_day(tweaks.time_of_day_hours);
        azimuth = az;
        elevation = el;
        time_hours = tweaks.time_of_day_hours;
    } else if tweaks.use_overrides {
        azimuth = tweaks.sun_azimuth_deg;
        elevation = tweaks.sun_elevation_deg;
    }

    celestial.time_of_day_hours = time_hours;
    celestial.sun_azimuth_deg = azimuth;
    celestial.sun_elevation_deg = elevation;
    celestial.sun_direction = sun_direction_from_angles(azimuth, elevation);
    celestial.sun_color = crate::ui::sun_color_for_elevation(elevation);

    let exposure_ev_min = if tweaks.use_overrides || tweaks.drive_sun_from_time_of_day {
        tweaks.exposure_ev_min
    } else {
        lighting.exposure_ev_min
    };
    let exposure_ev_max = if tweaks.use_overrides || tweaks.drive_sun_from_time_of_day {
        tweaks.exposure_ev_max
    } else {
        lighting.exposure_ev_max
    };
    let exposure_bias = if tweaks.use_overrides || tweaks.drive_sun_from_time_of_day {
        tweaks.exposure_bias
    } else {
        lighting.exposure_bias
    };

    let env_scale = if tweaks.use_overrides || tweaks.drive_sun_from_time_of_day {
        tweaks.environment_intensity_scale
    } else {
        lighting.environment_intensity_scale
    };
    celestial.environment_intensity =
        crate::ui::environment_intensity_for_elevation(elevation, env_scale);

    celestial.cloud_cover = if sky.clouds_enabled {
        sky.clouds_opacity
    } else {
        0.0
    };

    celestial.exposure_ev100 = crate::ui::exposure_ev_for_elevation(
        elevation,
        exposure_ev_min,
        exposure_ev_max,
        exposure_bias,
    ) - 1.5 * celestial.cloud_cover;

    celestial.moon_azimuth_deg = (azimuth + 180.0).rem_euclid(360.0);
    celestial.moon_elevation_deg = (-elevation).clamp(-45.0, 45.0);
    celestial.moon_direction =
        sun_direction_from_angles(celestial.moon_azimuth_deg, celestial.moon_elevation_deg);
    celestial.moon_enabled = lighting.moon_enabled && elevation < -2.0;
    celestial.moon_illuminance = if celestial.moon_enabled {
        lighting.moon_illuminance
    } else {
        0.0
    };

    let night_mix = night_mix_from_elevation(elevation);
    celestial.fog_inscattering = lerp_rgb(sky.horizon_color, sky.night_horizon_color, night_mix);
    celestial.fog_extinction = lerp_rgb(
        scale_rgb(sky.horizon_color, 0.92),
        scale_rgb(sky.night_horizon_color, 1.05),
        night_mix,
    );
}

fn lerp_rgb(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] * (1.0 - t) + b[0] * t,
        a[1] * (1.0 - t) + b[1] * t,
        a[2] * (1.0 - t) + b[2] * t,
    ]
}

fn scale_rgb(c: [f32; 3], s: f32) -> [f32; 3] {
    [c[0] * s, c[1] * s, c[2] * s]
}
