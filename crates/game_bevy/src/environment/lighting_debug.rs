//! Debug overlays for sun/moon direction, exposure, and IBL intensity.

use bevy::prelude::*;

use crate::camera::MainGameCamera;
use crate::environment::SunLight;
use crate::environment::celestial::CelestialState;
use crate::environment::celestial::MoonLight;
use crate::environment::lighting_state::EnvironmentLightingState;
use crate::state::AppState;

#[derive(Resource, Default)]
pub struct LightingDebugState {
    pub show_light_vectors: bool,
    pub show_lighting_stats: bool,
}

pub struct LightingDebugPlugin;

impl Plugin for LightingDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LightingDebugState>().add_systems(
            Update,
            (
                toggle_lighting_debug,
                draw_lighting_debug.run_if(in_state(AppState::Running)),
            ),
        );
    }
}

fn toggle_lighting_debug(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut debug: ResMut<LightingDebugState>,
) {
    if keyboard.just_pressed(KeyCode::F8) {
        debug.show_light_vectors = !debug.show_light_vectors;
    }
    if keyboard.just_pressed(KeyCode::F9) {
        debug.show_lighting_stats = !debug.show_lighting_stats;
    }
}

fn draw_lighting_debug(
    debug: Res<LightingDebugState>,
    celestial: Res<CelestialState>,
    lighting: Res<EnvironmentLightingState>,
    cameras: Query<&Transform, With<MainGameCamera>>,
    sun: Query<&Transform, (With<SunLight>, Without<MoonLight>)>,
    mut gizmos: Gizmos,
) {
    if !debug.show_light_vectors && !debug.show_lighting_stats {
        return;
    }
    let Ok(camera_tf) = cameras.single() else {
        return;
    };
    let origin = camera_tf.translation;

    if debug.show_light_vectors {
        if let Ok(sun_tf) = sun.single() {
            let sun_dir = sun_tf.forward().as_vec3();
            gizmos.arrow(
                origin,
                origin + sun_dir * 40.0,
                Color::srgb(1.0, 0.92, 0.55),
            );
        }
        let moon_dir = -celestial.sun_direction;
        gizmos.arrow(
            origin,
            origin + moon_dir * 30.0,
            Color::srgb(0.72, 0.78, 0.92),
        );
    }

    if debug.show_lighting_stats {
        let label = format!(
            "EV100 {:.2} | cloud {:.2} | moon phase {:.2} | env {:.2}",
            lighting.current_exposure,
            celestial.cloud_cover,
            celestial.moon_phase,
            celestial.environment_intensity,
        );
        gizmos.text(
            Isometry3d::new(origin + Vec3::Y * 2.5, Quat::IDENTITY),
            &label,
            16.0,
            Vec2::ZERO,
            Color::WHITE,
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::ui::{moon_phase_from_simulation_days, sun_angles_from_time_of_day};

    #[test]
    fn golden_day_parts_have_expected_sun_elevation_bands() {
        let cases = [(6.5, 2.0), (12.0, 50.0), (18.5, 2.0), (0.0, -10.0)];
        for (hours, expected_el) in cases {
            let (_, el) = sun_angles_from_time_of_day(hours);
            assert!(
                (el - expected_el).abs() < 25.0,
                "hour {hours}: elevation {el} vs expected {expected_el}"
            );
        }
    }

    #[test]
    fn moon_phase_cycles_over_lunar_month() {
        let new_moon = moon_phase_from_simulation_days(0.0, 0.0);
        let full_moon = moon_phase_from_simulation_days(29.53 * 0.5, 0.0);
        assert!(new_moon < 0.05);
        assert!(full_moon > 0.95);
    }
}
