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
    /// Last frame's sun-driven ambient (before cave dimming).
    pub effective_ambient_brightness: f32,
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
            effective_ambient_brightness: 300.0,
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
    mut state: ResMut<EnvironmentLightingState>,
    tweaks: Option<Res<crate::ui::AtmosphereTweaks>>,
    sky_state: Option<Res<super::sky::SkyState>>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<super::SunLight>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
    time: Res<Time>,
) {
    let yaml_ambient = state.ambient_brightness;
    let mut sun_illuminance = state.sun_illuminance;
    let mut ambient_brightness = yaml_ambient;

    if let Some(tweaks) = tweaks.as_ref() {
        if tweaks.drive_sun_from_time_of_day {
            let (azimuth, elevation) =
                crate::ui::sun_angles_from_time_of_day(tweaks.time_of_day_hours);
            state.sun_azimuth_deg = azimuth;
            state.sun_elevation_deg = elevation;
            sun_illuminance = crate::ui::sun_illuminance_for_elevation(elevation);
            ambient_brightness =
                crate::ui::ambient_brightness_for_elevation(elevation, yaml_ambient);
            if let Some(sky) = sky_state.as_ref() {
                let night_mix =
                    super::sky::night_mix_from_elevation(state.sun_elevation_deg);
                let horizon = lerp_rgb(sky.horizon_color, sky.night_horizon_color, night_mix);
                clear.0 = Color::srgb(
                    horizon[0] * 0.88,
                    horizon[1] * 0.92,
                    horizon[2] * 1.02,
                );
            }
        } else if tweaks.use_overrides {
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
        light.illuminance = sun_illuminance;
        light.color = Color::srgb(state.sun_color[0], state.sun_color[1], state.sun_color[2]);
        *transform = Transform::from_rotation(Quat::from_rotation_arc(-Vec3::Z, dir));
    }
    ambient.brightness = ambient_brightness;
    state.effective_ambient_brightness = ambient_brightness;
    ambient.color = Color::srgb(
        state.ambient_color[0],
        state.ambient_color[1],
        state.ambient_color[2],
    );
}

fn lerp_rgb(day: [f32; 3], night: [f32; 3], mix: f32) -> [f32; 3] {
    let t = mix.clamp(0.0, 1.0);
    [
        day[0] * (1.0 - t) + night[0] * t,
        day[1] * (1.0 - t) + night[1] * t,
        day[2] * (1.0 - t) + night[2] * t,
    ]
}
