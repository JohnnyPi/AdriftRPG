// crates/game_bevy/src/environment/config_init.rs
//! Bootstrap atmosphere, fog, and sky from compiled YAML (VS2 §18).

use bevy::prelude::*;

use crate::data::{ConfigRegistryResource, UserSetupPrefs};
use crate::state::AppState;
use crate::ui::{AtmosphereTweaks, LightingTweaks, WorldTweaks};
use crate::world::requested_world_id;

use super::atmosphere::atmosphere_clear_color;
use super::celestial::CelestialState;
use super::fog::{DistanceFogLayer, FogStack, HeightFogLayer, LocalFogVolume};
use super::lighting_state::EnvironmentLightingState;
use super::sky_config::{apply_sky_profile, bump_sky_effects_revision, SkyEffectsRevision, SkyPresentationConfig};

pub struct EnvironmentConfigPlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct EnvironmentInitSet;

impl Plugin for EnvironmentConfigPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkyPresentationConfig>()
            .init_resource::<SkyEffectsRevision>()
            .configure_sets(OnEnter(AppState::Running), EnvironmentInitSet)
            .add_systems(
                OnEnter(AppState::Running),
                init_presentation_from_registry.in_set(EnvironmentInitSet),
            );
    }
}

fn init_presentation_from_registry(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    _world_tweaks: Res<WorldTweaks>,
    mut lighting_state: ResMut<EnvironmentLightingState>,
    mut celestial: ResMut<CelestialState>,
    mut atmosphere: ResMut<AtmosphereTweaks>,
    mut lighting_tweaks: ResMut<LightingTweaks>,
    mut fog_stack: ResMut<FogStack>,
    mut sky_config: ResMut<SkyPresentationConfig>,
    mut sky_effects_revision: ResMut<SkyEffectsRevision>,
) {
    refresh_presentation_for_profile(
        &registry,
        &prefs,
        &mut sky_config,
        &mut atmosphere,
        &mut sky_effects_revision,
    );

    commands.insert_resource(atmosphere_clear_color());

    if let Some(atmo) = registry.0.active_atmosphere() {
        lighting_state.sun_azimuth_deg = atmo.sun_azimuth_deg;
        lighting_state.sun_elevation_deg = atmo.sun_elevation_deg;
        lighting_state.sun_illuminance = atmo.sun_illuminance_lux;
        lighting_state.sun_color = atmo.sun_color;
        lighting_state.ambient_brightness = atmo.ambient_brightness;
        lighting_state.ambient_color = atmo.ambient_color;
        lighting_state.exposure_ev_min = atmo.exposure_ev_min;
        lighting_state.exposure_ev_max = atmo.exposure_ev_max;
        lighting_state.exposure_bias = atmo.exposure_bias;
        lighting_state.exposure_adaptation_speed = atmo.exposure_adaptation_speed;
        lighting_state.environment_intensity_scale = atmo.environment_intensity_scale;
        lighting_state.moon_enabled = atmo.moon_enabled;
        lighting_state.moon_illuminance = atmo.moon_illuminance;
        lighting_state.moon_azimuth_deg = atmo.moon_azimuth_deg;
        lighting_state.moon_elevation_deg = atmo.moon_elevation_deg;

        atmosphere.exposure_ev_min = atmo.exposure_ev_min;
        atmosphere.exposure_ev_max = atmo.exposure_ev_max;
        atmosphere.exposure_bias = atmo.exposure_bias;
        atmosphere.environment_intensity_scale = atmo.environment_intensity_scale;

        lighting_state.current_exposure = crate::ui::exposure_ev_for_elevation(
            atmo.sun_elevation_deg,
            atmo.exposure_ev_min,
            atmo.exposure_ev_max,
            atmo.exposure_bias,
        );

        celestial.sun_azimuth_deg = atmo.sun_azimuth_deg;
        celestial.sun_elevation_deg = atmo.sun_elevation_deg;
        celestial.moon_azimuth_deg = atmo.moon_azimuth_deg;
        celestial.moon_elevation_deg = atmo.moon_elevation_deg;
        celestial.moon_phase = atmo.moon_phase;
        celestial.moon_enabled = atmo.moon_enabled;
        celestial.moon_illuminance = atmo.moon_illuminance;
        celestial.moon_direction = super::lighting_state::sun_direction_from_angles(
            atmo.moon_azimuth_deg,
            atmo.moon_elevation_deg,
        );
        celestial.sun_direction = super::lighting_state::sun_direction_from_angles(
            atmo.sun_azimuth_deg,
            atmo.sun_elevation_deg,
        );
        celestial.sun_color = crate::ui::sun_color_for_elevation(atmo.sun_elevation_deg);
        celestial.exposure_ev100 = crate::ui::exposure_ev_for_elevation(
            atmo.sun_elevation_deg,
            atmo.exposure_ev_min,
            atmo.exposure_ev_max,
            atmo.exposure_bias,
        );
        celestial.environment_intensity = crate::ui::environment_intensity_for_elevation(
            atmo.sun_elevation_deg,
            atmo.environment_intensity_scale,
        );

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
            inscattering_color: fog.distance_inscattering_color,
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
            fog_stack.ocean_extent_m = world.effective_ocean_extent_m();
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

        if volumes.is_empty() && world_profile.is_none() {
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
    sky_config: &mut SkyPresentationConfig,
    atmosphere: &mut AtmosphereTweaks,
    sky_effects_revision: &mut SkyEffectsRevision,
) {
    let world_id = requested_world_id(prefs);
    let world_profile = registry
        .0
        .effective_world(Some(&world_id))
        .ok()
        .or_else(|| registry.0.active_world().ok());

    if let Some(world) = world_profile.as_ref() {
        if let Some(sky) = registry.0.effective_sky(world) {
            apply_sky_profile(sky_config, atmosphere, sky);
            bump_sky_effects_revision(sky_effects_revision);
            return;
        }
    }
    if let Some(sky) = registry.0.active_sky() {
        apply_sky_profile(sky_config, atmosphere, sky);
        bump_sky_effects_revision(sky_effects_revision);
    }
}

/// Sea level (m) for the active world profile.
pub fn sea_level_for_prefs(registry: &ConfigRegistryResource, prefs: &UserSetupPrefs) -> f32 {
    let world_id = requested_world_id(prefs);
    registry
        .0
        .effective_world(Some(&world_id))
        .ok()
        .and_then(|world| registry.0.water.get(&world.water).map(|w| w.sea_level_m))
        .unwrap_or(0.0)
}
