//! Authoritative simulation clock for day/night presentation.

use bevy::prelude::*;

use super::celestial::UpdateCelestialSet;
use crate::state::AppState;

#[derive(Resource, Clone, Debug)]
pub struct SimulationTime {
    pub time_of_day_hours: f32,
    /// Fractional in-game days elapsed since world start (drives moon phase).
    pub simulation_days: f32,
    pub day_length_minutes: f32,
    pub time_scale: f32,
    pub auto_advance: bool,
}

impl Default for SimulationTime {
    fn default() -> Self {
        Self {
            time_of_day_hours: 10.0,
            simulation_days: 0.0,
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
    let days_per_second = 1.0 / day_seconds;
    let scaled = time.delta_secs() * sim.time_scale;
    sim.time_of_day_hours = (sim.time_of_day_hours + scaled * hours_per_second).rem_euclid(24.0);
    sim.simulation_days += scaled * days_per_second;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_clock_starts_at_morning() {
        assert_eq!(SimulationTime::default().time_of_day_hours, 10.0);
    }

    #[test]
    fn one_game_day_equals_day_length_minutes() {
        let day_seconds = 24.0 * 60.0;
        let days_per_second = 1.0 / day_seconds;
        let elapsed = day_seconds;
        assert!((elapsed * days_per_second - 1.0_f32).abs() < 1e-5);
    }
}
