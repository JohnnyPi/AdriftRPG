//! Shared celestial state driving sun, moon, fog, and exposure (SkyLightingGuide §1).

use bevy::prelude::*;
use shared::lerp_rgb;

use super::lighting_state::sun_direction_from_angles;
use super::sky_config::{SkyPresentationConfig, night_mix_from_elevation};
use crate::state::AppState;

/// Moon presentation tracks the sun (opposite azimuth, mirrored elevation).
pub fn moon_angles_from_sun(sun_azimuth_deg: f32, sun_elevation_deg: f32) -> (f32, f32) {
    (
        (sun_azimuth_deg + 180.0).rem_euclid(360.0),
        (-sun_elevation_deg).clamp(-45.0, 45.0),
    )
}

pub fn moon_direction_from_sun(sun_azimuth_deg: f32, sun_elevation_deg: f32) -> Vec3 {
    let (azimuth, elevation) = moon_angles_from_sun(sun_azimuth_deg, sun_elevation_deg);
    sun_direction_from_angles(azimuth, elevation)
}

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
    /// Elevation-based env-map intensity (cloud attenuation applied in sync).
    pub environment_intensity: f32,
    /// Fog inscattering tint blended at twilight.
    pub fog_inscattering: [f32; 3],
    pub fog_extinction: [f32; 3],
}

impl Default for CelestialState {
    fn default() -> Self {
        let (azimuth, elevation) = crate::ui::sun_angles_from_time_of_day(10.0);
        let (moon_azimuth, moon_elevation) = moon_angles_from_sun(azimuth, elevation);
        Self {
            time_of_day_hours: 10.0,
            sun_azimuth_deg: azimuth,
            sun_elevation_deg: elevation,
            sun_direction: sun_direction_from_angles(azimuth, elevation),
            sun_color: [1.0, 0.97, 0.92],
            moon_azimuth_deg: moon_azimuth,
            moon_elevation_deg: moon_elevation,
            moon_direction: sun_direction_from_angles(moon_azimuth, moon_elevation),
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

impl CelestialState {
    /// Bootstrap sun/moon presentation from compiled atmosphere on world enter.
    pub fn apply_authored_atmosphere(&mut self, atmo: &game_data::CompiledAtmosphere) {
        self.sun_azimuth_deg = atmo.sun_azimuth_deg;
        self.sun_elevation_deg = atmo.sun_elevation_deg;
        let (moon_azimuth, moon_elevation) =
            moon_angles_from_sun(atmo.sun_azimuth_deg, atmo.sun_elevation_deg);
        self.moon_azimuth_deg = moon_azimuth;
        self.moon_elevation_deg = moon_elevation;
        self.moon_phase = atmo.moon_phase;
        self.moon_enabled = atmo.moon_enabled;
        self.moon_illuminance = atmo.moon_illuminance;
        self.moon_direction = moon_direction_from_sun(atmo.sun_azimuth_deg, atmo.sun_elevation_deg);
        self.sun_direction =
            sun_direction_from_angles(atmo.sun_azimuth_deg, atmo.sun_elevation_deg);
        self.sun_color = crate::ui::sun_color_for_elevation(atmo.sun_elevation_deg);
        self.exposure_ev100 = crate::ui::exposure_ev_for_elevation(
            atmo.sun_elevation_deg,
            atmo.exposure_ev_min,
            atmo.exposure_ev_max,
            atmo.exposure_bias,
        );
        self.environment_intensity = crate::ui::environment_intensity_for_elevation(
            atmo.sun_elevation_deg,
            atmo.environment_intensity_scale,
        );
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
            .configure_sets(
                Update,
                UpdateCelestialSet.before(super::lighting_state::SyncEnvironmentLightingSet),
            )
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
    sim: Res<super::simulation_time::SimulationTime>,
    sky: Res<SkyPresentationConfig>,
    lighting: Res<super::lighting_state::EnvironmentLightingState>,
) {
    celestial.time_of_day_hours = sim.time_of_day_hours;

    let mut azimuth = lighting.authored_sun_azimuth_deg;
    let mut elevation = lighting.authored_sun_elevation_deg;

    if lighting.drive_sun_from_time_of_day {
        let (az, el) = crate::ui::sun_angles_from_time_of_day(sim.time_of_day_hours);
        azimuth = az;
        elevation = el;
    } else if lighting.override_sun_angles {
        azimuth = lighting.override_sun_azimuth_deg;
        elevation = lighting.override_sun_elevation_deg;
    }
    celestial.sun_azimuth_deg = azimuth;
    celestial.sun_elevation_deg = elevation;
    celestial.sun_direction = sun_direction_from_angles(azimuth, elevation);
    celestial.sun_color = crate::ui::sun_color_for_elevation(elevation);

    celestial.environment_intensity = crate::ui::environment_intensity_for_elevation(
        elevation,
        lighting.environment_intensity_scale,
    );

    celestial.cloud_cover = if sky.clouds_enabled {
        sky.clouds_opacity
    } else {
        0.0
    };

    celestial.exposure_ev100 = crate::ui::exposure_ev_for_elevation(
        elevation,
        lighting.exposure_ev_min,
        lighting.exposure_ev_max,
        lighting.exposure_bias,
    ) - 1.5 * celestial.cloud_cover;

    let (moon_azimuth, moon_elevation) = moon_angles_from_sun(azimuth, elevation);
    celestial.moon_azimuth_deg = moon_azimuth;
    celestial.moon_elevation_deg = moon_elevation;
    celestial.moon_direction = moon_direction_from_sun(azimuth, elevation);
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

fn scale_rgb(c: [f32; 3], s: f32) -> [f32; 3] {
    [c[0] * s, c[1] * s, c[2] * s]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moon_angles_track_sun_opposite() {
        let (az, el) = moon_angles_from_sun(90.0, 30.0);
        assert!((az - 270.0).abs() < 1e-3);
        assert!((el - (-30.0)).abs() < 1e-3);
    }

    #[test]
    fn moon_direction_matches_angles() {
        let dir = moon_direction_from_sun(0.0, 45.0);
        let (az, el) = moon_angles_from_sun(0.0, 45.0);
        let expected = sun_direction_from_angles(az, el);
        assert!((dir - expected).length() < 1e-5);
    }
}
