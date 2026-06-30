//! Camera environment states (VS2 §11).

use bevy::prelude::*;

use crate::terrain::CameraWaterState;
use crate::ui::CameraTweaks;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CameraEnvironmentState {
    #[default]
    Explore,
    Interior,
    Underwater,
}

#[derive(Resource, Debug, Default)]
pub struct CameraEnvironment {
    pub state: CameraEnvironmentState,
    pub obstruction_hold_s: f32,
}

pub fn update_camera_environment(
    water: Res<CameraWaterState>,
    players: Query<&Transform, With<crate::player::Player>>,
    mut env: ResMut<CameraEnvironment>,
    mut cameras: Query<&mut super::components::MmoCamera>,
    tweaks: Res<CameraTweaks>,
    time: Res<Time>,
) {
    let in_cave = players
        .single()
        .map(|tf| {
            tf.translation.y < 6.0
                && tf.translation.distance(Vec3::new(26.0, 2.0, 12.0)) < 12.0
        })
        .unwrap_or(false);

    env.state = if water.submerged_depth > 0.3 {
        CameraEnvironmentState::Underwater
    } else if in_cave {
        CameraEnvironmentState::Interior
    } else {
        CameraEnvironmentState::Explore
    };

    if env.state == CameraEnvironmentState::Interior {
        env.obstruction_hold_s = (env.obstruction_hold_s + time.delta_secs()).min(0.35);
    } else {
        env.obstruction_hold_s = 0.0;
    }

    if !tweaks.use_overrides {
        return;
    }
    for mut camera in &mut cameras {
        match env.state {
            CameraEnvironmentState::Underwater => {
                camera.current_distance *= 0.92;
            }
            CameraEnvironmentState::Interior => {
                camera.current_distance *= tweaks.interior_distance_scale;
            }
            CameraEnvironmentState::Explore => {}
        }
    }
}
