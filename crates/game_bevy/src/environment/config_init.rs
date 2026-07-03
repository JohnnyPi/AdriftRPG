// crates/game_bevy/src/environment/config_init.rs
//! Bootstrap atmosphere, fog, and sky from compiled YAML (VS2 §18).

use bevy::prelude::*;

use crate::data::{ConfigRegistryResource, UserSetupPrefs};
use crate::state::AppState;
use crate::ui::{AtmosphereTweaks, LightingTweaks, WorldTweaks};
use crate::world::requested_world_id;

use super::fog::{DistanceFogLayer, FogStack, HeightFogLayer, LocalFogVolume};
use super::lighting_state::EnvironmentLightingState;
use super::sky::SkyState;

pub struct EnvironmentConfigPlugin;

impl Plugin for EnvironmentConfigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::Running),
            init_presentation_from_registry,
        );
    }
}

fn init_presentation_from_registry(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    _world_tweaks: Res<WorldTweaks>,
    mut lighting_state: ResMut<EnvironmentLightingState>,
    mut atmosphere: ResMut<AtmosphereTweaks>,
    mut lighting_tweaks: ResMut<LightingTweaks>,
    mut fog_stack: ResMut<FogStack>,
    mut sky_state: ResMut<SkyState>,
) {
    refresh_presentation_for_profile(
        &registry,
        &prefs,
        &mut sky_state,
        &mut atmosphere,
    );

    commands.insert_resource(ClearColor(Color::srgb(
        sky_state.horizon_color[0],
        sky_state.horizon_color[1],
        sky_state.horizon_color[2],
    )));

    if let Some(atmo) = registry.0.active_atmosphere() {
        lighting_state.sun_azimuth_deg = atmo.sun_azimuth_deg;
        lighting_state.sun_elevation_deg = atmo.sun_elevation_deg;
        lighting_state.sun_illuminance = atmo.sun_illuminance_lux;
        lighting_state.sun_color = atmo.sun_color;
        lighting_state.ambient_brightness = atmo.ambient_brightness;
        lighting_state.ambient_color = atmo.ambient_color;
        lighting_state.current_exposure = atmo.exposure_target;
        lighting_state.exposure_target = atmo.exposure_target;
        lighting_state.exposure_adaptation_speed = atmo.exposure_adaptation_speed;
        lighting_state.moon_enabled = atmo.moon_enabled;
        lighting_state.moon_illuminance = atmo.moon_illuminance;

        let moon_dir = super::lighting_state::sun_direction_from_angles(
            atmo.moon_azimuth_deg,
            atmo.moon_elevation_deg,
        );
        sky_state.moon = super::sky::CelestialBodyState {
            direction: moon_dir,
            angular_radius: atmo.moon_angular_radius,
            brightness: if atmo.moon_enabled {
                atmo.moon_illuminance
            } else {
                0.0
            },
            phase: atmo.moon_phase,
        };

        atmosphere.sun_azimuth_deg = atmo.sun_azimuth_deg;
        atmosphere.sun_elevation_deg = atmo.sun_elevation_deg;
    }

    if let Some(fog) = registry.0.active_fog() {
        let world_id = requested_world_id(&prefs);
        let world_profile = registry
            .0
            .effective_world(Some(&world_id))
            .ok()
            .or_else(|| registry.0.active_world().ok());

        fog_stack.global_distance = Some(DistanceFogLayer {
            color: fog.distance_color,
            start_m: fog.distance_start_m,
            end_m: fog.distance_end_m,
        });
        fog_stack.height = Some(HeightFogLayer {
            base_height: fog.height_base_m,
            density: fog.height_density,
            color: fog.height_color,
        });
        fog_stack.underwater_density = fog.underwater_density;
        fog_stack.cave_density = fog.cave_density;

        let mut volumes: Vec<LocalFogVolume> = fog
            .local_volumes
            .iter()
            .map(|v| LocalFogVolume {
                center: Vec3::new(v.center[0], v.center[1], v.center[2]),
                half_extents: Vec3::new(v.half_extents[0], v.half_extents[1], v.half_extents[2]),
                density: v.density,
                color: v.color,
            })
            .collect();

        if let Some(world) = world_profile.as_ref() {
            if let Some(landmarks) = registry.0.effective_landmarks(world) {
                for v in &landmarks.fog_volumes {
                    let center = world.recipe_to_world(v.center);
                    volumes.push(LocalFogVolume {
                        center: Vec3::new(center[0], center[1], center[2]),
                        half_extents: Vec3::new(
                            v.half_extents[0],
                            v.half_extents[1],
                            v.half_extents[2],
                        ),
                        density: v.density,
                        color: v.color,
                    });
                }
            }
        }

        if volumes.is_empty() {
            volumes.push(LocalFogVolume {
                center: Vec3::new(26.0, 4.0, 12.0),
                half_extents: Vec3::new(8.0, 4.0, 8.0),
                density: fog.cave_density,
                color: fog.cave_color,
            });
        }

        fog_stack.local_volumes = volumes;

        lighting_tweaks.fog_color = fog.distance_color;
        lighting_tweaks.fog_start_m = fog.distance_start_m;
        lighting_tweaks.fog_end_m = fog.distance_end_m;
        atmosphere.height_fog_density = fog.height_density;
        atmosphere.underwater_fog_density = fog.underwater_density;
    }
}

pub fn refresh_presentation_for_profile(
    registry: &ConfigRegistryResource,
    prefs: &UserSetupPrefs,
    sky_state: &mut SkyState,
    atmosphere: &mut AtmosphereTweaks,
) {
    let world_id = requested_world_id(prefs);
    let world_profile = registry
        .0
        .effective_world(Some(&world_id))
        .ok()
        .or_else(|| registry.0.active_world().ok());

    if let Some(world) = world_profile.as_ref() {
        if let Some(sky) = registry.0.effective_sky(world) {
            apply_sky(sky_state, atmosphere, sky);
            return;
        }
    }
    if let Some(sky) = registry.0.active_sky() {
        apply_sky(sky_state, atmosphere, sky);
    }
}

fn apply_sky(
    sky_state: &mut SkyState,
    atmosphere: &mut AtmosphereTweaks,
    sky: &game_data::CompiledSky,
) {
    sky_state.zenith_color = sky.zenith_color;
    sky_state.horizon_color = sky.horizon_color;
    sky_state.mie_strength = sky.mie_strength;
    sky_state.sun_disc_radius = sky.sun_disc_radius;
    sky_state.stars_enabled = sky.stars_enabled;
    sky_state.stars_density = sky.stars_density;
    sky_state.clouds_enabled = sky.clouds_enabled;
    sky_state.clouds_opacity = sky.clouds_opacity;
    sky_state.clouds_speed = sky.clouds_speed;
    sky_state.clouds_direction_deg = sky.clouds_direction_deg;
    sky_state.clouds_altitude = sky.clouds_altitude;
    sky_state.night_zenith_color = sky.night_zenith_color;
    sky_state.night_horizon_color = sky.night_horizon_color;

    atmosphere.zenith_color = sky.zenith_color;
    atmosphere.horizon_color = sky.horizon_color;
    atmosphere.mie_strength = sky.mie_strength;
}
