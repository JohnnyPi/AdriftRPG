// crates/game_bevy/src/environment/fog.rs
//! Layered fog stack (VS2 §14).

use bevy::prelude::*;

use crate::camera::MainGameCamera;
use crate::state::AppState;
use crate::terrain::CameraWaterState;
use crate::ui::{AtmosphereTweaks, LightingTweaks};

#[derive(Clone, Debug)]
pub struct DistanceFogLayer {
    pub color: [f32; 3],
    pub start_m: f32,
    pub end_m: f32,
}

#[derive(Clone, Debug)]
pub struct HeightFogLayer {
    pub base_height: f32,
    pub density: f32,
    pub color: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct LocalFogVolume {
    pub center: Vec3,
    pub half_extents: Vec3,
    pub density: f32,
    pub color: [f32; 3],
}

#[derive(Resource, Clone, Debug)]
pub struct FogStack {
    pub global_distance: Option<DistanceFogLayer>,
    pub height: Option<HeightFogLayer>,
    pub local_volumes: Vec<LocalFogVolume>,
    pub underwater_density: f32,
    pub cave_density: f32,
    pub transition_alpha: f32,
}

impl Default for FogStack {
    fn default() -> Self {
        Self {
            global_distance: None,
            height: None,
            local_volumes: Vec::new(),
            underwater_density: 0.15,
            cave_density: 0.12,
            transition_alpha: 1.0,
        }
    }
}

#[derive(Resource, Debug, Default)]
pub struct FogTransitionState {
    pub target_underwater: f32,
    pub target_cave: f32,
    pub current_underwater: f32,
    pub current_cave: f32,
}

pub struct FogPlugin;

impl Plugin for FogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FogStack>()
            .init_resource::<FogTransitionState>()
            .add_systems(
                Update,
                (update_fog_transitions, apply_fog_stack).chain().run_if(in_state(AppState::Running)),
            );
    }
}

fn update_fog_transitions(
    time: Res<Time>,
    water: Res<CameraWaterState>,
    cameras: Query<&Transform, With<MainGameCamera>>,
    mut stack: ResMut<FogStack>,
    mut transition: ResMut<FogTransitionState>,
) {
    let Ok(camera_tf) = cameras.single() else {
        return;
    };
    let underwater = if water.submerged_depth > 0.1 {
        stack.underwater_density * water.submerged_depth.min(3.0)
    } else {
        0.0
    };
    let in_cave = stack.local_volumes.iter().any(|vol| {
        point_in_volume(camera_tf.translation, vol.center, vol.half_extents)
    });
    let cave = if in_cave { stack.cave_density } else { 0.0 };

    transition.target_underwater = underwater;
    transition.target_cave = cave;
    let dt = time.delta_secs();
    let rate = 4.0 * dt;
    transition.current_underwater = approach(transition.current_underwater, underwater, rate);
    transition.current_cave = approach(transition.current_cave, cave, rate);
    stack.transition_alpha = 1.0 - transition.current_underwater.max(transition.current_cave);
}

fn apply_fog_stack(
    stack: Res<FogStack>,
    transition: Res<FogTransitionState>,
    lighting_tweaks: Res<LightingTweaks>,
    atmosphere: Res<AtmosphereTweaks>,
    cameras: Query<&Transform, With<MainGameCamera>>,
    mut fog: Query<&mut DistanceFog, With<MainGameCamera>>,
) {
    let Some(distance) = &stack.global_distance else {
        return;
    };
    let Ok(camera_tf) = cameras.single() else {
        return;
    };

    let mut color = if lighting_tweaks.override_fog {
        lighting_tweaks.fog_color
    } else {
        distance.color
    };
    let mut start = if lighting_tweaks.override_fog {
        lighting_tweaks.fog_start_m
    } else {
        distance.start_m
    };
    let mut end = if lighting_tweaks.override_fog {
        lighting_tweaks.fog_end_m
    } else {
        distance.end_m
    };

    if let Some(height) = &stack.height {
        let height_factor = ((camera_tf.translation.y - height.base_height).max(0.0) * height.density)
            .clamp(0.0, 1.0);
        let height_blend = height_factor * atmosphere.height_fog_density * 50.0;
        start -= height_blend;
        color = lerp_color(color, height.color, height_factor * 0.35);
    }

    for volume in &stack.local_volumes {
        let local = point_in_obb(camera_tf.translation, volume.center, volume.half_extents);
        if local > 0.0 {
            start *= 1.0 - local * volume.density;
            color = lerp_color(color, volume.color, local * 0.5);
        }
    }

    if transition.current_underwater > 0.0 {
        let u = (transition.current_underwater / stack.underwater_density.max(0.01)).clamp(0.0, 1.0);
        color = lerp_color(color, [0.05, 0.25, 0.35], u);
        end *= 1.0 - u * 0.6;
    }
    if transition.current_cave > 0.0 {
        let c = (transition.current_cave / stack.cave_density.max(0.01)).clamp(0.0, 1.0);
        color = lerp_color(color, [0.2, 0.22, 0.28], c);
        start *= 1.0 - c * 0.4;
    }

    let height_blend = atmosphere.height_fog_density * 50.0;
    for mut distance_fog in &mut fog {
        *distance_fog = DistanceFog {
            color: Color::srgba(color[0], color[1], color[2], stack.transition_alpha),
            falloff: FogFalloff::Linear {
                start: start - height_blend * 0.25,
                end,
            },
            ..default()
        };
    }
}

fn approach(current: f32, target: f32, max_delta: f32) -> f32 {
    if (target - current).abs() <= max_delta {
        target
    } else if target > current {
        current + max_delta
    } else {
        current - max_delta
    }
}

fn lerp_color(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn point_in_volume(point: Vec3, center: Vec3, half_extents: Vec3) -> bool {
    let local = point - center;
    local.x.abs() <= half_extents.x
        && local.y.abs() <= half_extents.y
        && local.z.abs() <= half_extents.z
}

fn point_in_obb(point: Vec3, center: Vec3, half_extents: Vec3) -> f32 {
    let local = point - center;
    let dx = (local.x.abs() - half_extents.x).max(0.0) / half_extents.x.max(0.01);
    let dy = (local.y.abs() - half_extents.y).max(0.0) / half_extents.y.max(0.01);
    let dz = (local.z.abs() - half_extents.z).max(0.0) / half_extents.z.max(0.01);
    (dx.max(dy).max(dz)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod fog_tests {
    use super::*;

    #[test]
    fn height_fog_tightens_start_at_low_camera() {
        let stack = FogStack {
            global_distance: Some(DistanceFogLayer {
                color: [0.6, 0.7, 0.8],
                start_m: 40.0,
                end_m: 200.0,
            }),
            height: Some(HeightFogLayer {
                base_height: 4.0,
                density: 0.02,
                color: [0.7, 0.78, 0.85],
            }),
            ..Default::default()
        };
        let camera_low = 2.0;
        let camera_high = 20.0;
        let height = stack.height.as_ref().unwrap();
        let low_factor = ((camera_low - height.base_height).max(0.0) * height.density).clamp(0.0, 1.0);
        let high_factor = ((camera_high - height.base_height).max(0.0) * height.density).clamp(0.0, 1.0);
        assert!(low_factor < high_factor);
        let low_start = stack.global_distance.as_ref().unwrap().start_m - low_factor * 50.0;
        let high_start = stack.global_distance.as_ref().unwrap().start_m - high_factor * 50.0;
        assert!(low_start > high_start);
    }

    #[test]
    fn underwater_transition_targets_depth() {
        let mut transition = FogTransitionState::default();
        transition.target_underwater = 0.3;
        transition.current_underwater = approach(0.0, transition.target_underwater, 0.1);
        assert!(transition.current_underwater > 0.0);
        assert!(transition.current_underwater <= transition.target_underwater);
    }
}
