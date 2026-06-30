//! Environmental audio hook stubs (VS2 §17).

use bevy::prelude::*;

use crate::state::AppState;
use crate::terrain::CameraWaterState;

#[derive(Component)]
pub struct AudioEmitterStub {
    pub label: &'static str,
}

pub struct EnvironmentAudioStubPlugin;

impl Plugin for EnvironmentAudioStubPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnvironmentAudioState>()
            .add_systems(OnEnter(AppState::Running), spawn_audio_stubs)
            .add_systems(Update, update_audio_state.run_if(in_state(AppState::Running)));
    }
}

#[derive(Resource, Default, Debug)]
pub struct EnvironmentAudioState {
    pub zone: &'static str,
}

fn spawn_audio_stubs(mut commands: Commands) {
    commands.spawn((
        AudioEmitterStub { label: "coast_waves" },
        Transform::from_xyz(-30.0, 2.0, -25.0),
    ));
    commands.spawn((
        AudioEmitterStub { label: "river_flow" },
        Transform::from_xyz(100.0, 4.0, 150.0),
    ));
    commands.spawn((
        AudioEmitterStub { label: "cave_drip" },
        Transform::from_xyz(24.0, 6.0, 10.0),
    ));
}

fn update_audio_state(
    water: Res<CameraWaterState>,
    mut audio: ResMut<EnvironmentAudioState>,
) {
    audio.zone = if water.submerged_depth > 0.3 {
        "underwater"
    } else {
        "outdoor"
    };
}
