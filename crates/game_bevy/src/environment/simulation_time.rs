//! Automatic day/night cycle advancing simulation clock.

use bevy::prelude::*;

use super::celestial::CelestialState;
use crate::state::AppState;
use crate::ui::AtmosphereTweaks;

#[derive(Resource, Clone, Debug)]
pub struct SimulationTime {
    pub time_of_day_hours: f32,
    pub day_length_minutes: f32,
    pub time_scale: f32,
    pub auto_advance: bool,
}

impl Default for SimulationTime {
    fn default() -> Self {
        Self {
            time_of_day_hours: 10.5,
            day_length_minutes: 24.0,
            time_scale: 1.0,
            auto_advance: true,
        }
    }
}

pub struct SimulationTimePlugin;

impl Plugin for SimulationTimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimulationTime>()
            .add_systems(
                Update,
                (advance_simulation_time, apply_simulation_time_to_lighting)
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn advance_simulation_time(time: Res<Time>, mut sim: ResMut<SimulationTime>) {
    if !sim.auto_advance {
        return;
    }
    let day_seconds = sim.day_length_minutes * 60.0;
    if day_seconds <= 0.0 {
        return;
    }
    let hours_per_second = 24.0 / day_seconds;
    sim.time_of_day_hours =
        (sim.time_of_day_hours + time.delta_secs() * hours_per_second * sim.time_scale) % 24.0;
}

fn apply_simulation_time_to_lighting(
    sim: Res<SimulationTime>,
    mut atmosphere: ResMut<AtmosphereTweaks>,
    mut celestial: ResMut<CelestialState>,
) {
    if !sim.auto_advance {
        return;
    }
    atmosphere.time_of_day_hours = sim.time_of_day_hours;
    let (azimuth, elevation) = crate::ui::sun_angles_from_time_of_day(sim.time_of_day_hours);
    atmosphere.sun_azimuth_deg = azimuth;
    atmosphere.sun_elevation_deg = elevation;
    celestial.sun_azimuth_deg = azimuth;
    celestial.sun_elevation_deg = elevation;
    celestial.sun_direction = crate::environment::lighting_state::sun_direction_from_angles(
        azimuth,
        elevation,
    );
    celestial.sun_color = crate::ui::sun_color_for_elevation(elevation);
    celestial.exposure_ev100 = crate::ui::exposure_ev_for_elevation(
        elevation,
        atmosphere.exposure_ev_min,
        atmosphere.exposure_ev_max,
        atmosphere.exposure_bias,
    );
}
