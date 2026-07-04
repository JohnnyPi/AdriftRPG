// crates/game_bevy/src/environment/audio.rs
//! Environmental audio hook stubs (VS2 §17).

use bevy::prelude::*;

use crate::player::Player;
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
            .add_systems(
                Update,
                update_audio_state.run_if(in_state(AppState::Running)),
            );
    }
}

#[derive(Resource, Default, Debug)]
pub struct EnvironmentAudioState {
    pub zone: &'static str,
}

fn spawn_audio_stubs(mut commands: Commands) {
    commands.spawn((
        AudioEmitterStub {
            label: "coast_waves",
        },
        Transform::from_xyz(-30.0, 2.0, -25.0),
    ));
    commands.spawn((
        AudioEmitterStub {
            label: "river_flow",
        },
        Transform::from_xyz(100.0, 4.0, 150.0),
    ));
    commands.spawn((
        AudioEmitterStub { label: "cave_drip" },
        Transform::from_xyz(24.0, 6.0, 10.0),
    ));
}

fn update_audio_state(
    water: Res<CameraWaterState>,
    player: Query<&Transform, With<Player>>,
    emitters: Query<(&AudioEmitterStub, &Transform)>,
    mut audio: ResMut<EnvironmentAudioState>,
) {
    if water.submerged_depth > 0.3 {
        audio.zone = "underwater";
        return;
    }
    let Ok(player_tf) = player.single() else {
        audio.zone = "outdoor";
        return;
    };
    let listener = player_tf.translation;
    let mut nearest_label = "outdoor";
    let mut nearest_dist_sq = f32::MAX;
    for (stub, tf) in &emitters {
        let dist_sq = listener.distance_squared(tf.translation);
        if dist_sq < nearest_dist_sq {
            nearest_dist_sq = dist_sq;
            nearest_label = stub.label;
        }
    }
    audio.zone = nearest_label;
}
