//! Visual weather v1: animated cloud cover and fog density presets.

use bevy::prelude::*;

use super::celestial::CelestialState;
use super::fog::FogStack;
use crate::data::{ConfigRegistryResource, UserSetupPrefs};
use crate::state::AppState;
use crate::world::requested_world_id;

#[derive(Resource, Clone, Debug)]
pub struct WeatherState {
    pub phase: f32,
    pub target_cloud_cover: f32,
    pub fog_density_scale: f32,
    pub base_fog_end_m: f32,
}

impl Default for WeatherState {
    fn default() -> Self {
        Self {
            phase: 0.0,
            target_cloud_cover: 0.5,
            fog_density_scale: 1.0,
            base_fog_end_m: 520.0,
        }
    }
}

pub struct WeatherPlugin;

impl Plugin for WeatherPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeatherState>()
            .add_systems(OnEnter(AppState::Running), init_weather_from_registry)
            .add_systems(
                Update,
                animate_weather.run_if(in_state(AppState::Running)),
            );
    }
}

fn init_weather_from_registry(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    mut weather: ResMut<WeatherState>,
) {
    let world_id = requested_world_id(&prefs);
    let Ok(world) = registry.0.effective_world(Some(&world_id)) else {
        return;
    };
    if let Some(profile) = registry.0.effective_weather(world) {
        weather.target_cloud_cover = profile.cloud_cover;
        weather.fog_density_scale = profile.fog_density_scale;
    }
    if let Some(fog) = registry.0.active_fog() {
        weather.base_fog_end_m = fog.distance_end_m;
    }
}

fn animate_weather(
    time: Res<Time>,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    mut weather: ResMut<WeatherState>,
    mut celestial: ResMut<CelestialState>,
    mut fog_stack: ResMut<FogStack>,
) {
    let world_id = requested_world_id(&prefs);
    let cycle_minutes = registry
        .0
        .effective_world(Some(&world_id))
        .ok()
        .and_then(|w| registry.0.effective_weather(w))
        .map(|p| p.cycle_minutes)
        .unwrap_or(20.0);
    let rate = if cycle_minutes > 0.0 {
        1.0 / (cycle_minutes * 60.0)
    } else {
        0.0
    };
    weather.phase = (weather.phase + time.delta_secs() * rate) % 1.0;
    let wave = (weather.phase * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    celestial.cloud_cover = weather.target_cloud_cover * (0.65 + wave * 0.35);

    if let Some(distance) = fog_stack.global_distance.as_mut() {
        distance.end_m = weather.base_fog_end_m / weather.fog_density_scale.max(0.25);
    }
}
