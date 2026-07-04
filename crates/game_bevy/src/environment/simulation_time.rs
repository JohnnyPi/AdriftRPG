//! Authoritative simulation clock for day/night presentation.

use bevy::prelude::*;

use super::celestial::UpdateCelestialSet;
use crate::state::AppState;

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
            time_of_day_hours: 10.0,
            day_length_minutes: 24.0,
            time_scale: 1.0,
            auto_advance: true,
        }
    }
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct SimulationClockSet;

pub struct SimulationTimePlugin;

impl Plugin for SimulationTimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimulationTime>()
            .configure_sets(Update, SimulationClockSet.before(UpdateCelestialSet))
            .add_systems(
                Update,
                advance_simulation_clock
                    .in_set(SimulationClockSet)
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn advance_simulation_clock(time: Res<Time>, mut sim: ResMut<SimulationTime>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_clock_starts_at_morning() {
        assert_eq!(SimulationTime::default().time_of_day_hours, 10.0);
    }
}
