// crates/game_bevy/src/environment/lighting.rs
use bevy::prelude::*;

use super::lighting_state::SyncEnvironmentLightingSet;
use crate::player::Player;
use crate::state::AppState;
use crate::terrain::TerrainPipelineState;
use terrain_generation::RecipeDensitySource;

#[derive(Component)]
pub struct SunLight;

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                update_sky_visibility,
                apply_cave_atmosphere
                    .after(update_sky_visibility)
                    .after(SyncEnvironmentLightingSet),
            )
                .run_if(in_state(AppState::Running)),
        );
    }
}

fn update_sky_visibility(
    pipeline: Res<TerrainPipelineState>,
    player: Query<&Transform, With<Player>>,
    mut visibility: Query<&mut super::lighting_state::SkyVisibility, With<Player>>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let Ok(mut sky_vis) = visibility.single_mut() else {
        return;
    };
    let Some(source) = pipeline.density_source.as_ref() else {
        *sky_vis = super::lighting_state::SkyVisibility::default();
        return;
    };
    let cave = cave_depth_factor(source, player_tf.translation);
    sky_vis.cave_depth = cave;
    sky_vis.sky = sky_visibility_from_cave(cave, source, player_tf.translation);
}

fn sky_visibility_from_cave(cave_depth: f32, source: &RecipeDensitySource, position: Vec3) -> f32 {
    let sea = source.recipe().sea_level;
    if position.y < sea - 2.0 {
        let density = source.density_at(position.x, position.y + 2.0, position.z);
        if density < 0.0 {
            return 0.15;
        }
    }
    1.0 - cave_depth * 0.85
}

fn apply_cave_atmosphere(
    pipeline: Res<TerrainPipelineState>,
    player: Query<&super::lighting_state::SkyVisibility, With<Player>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    lighting_state: Res<super::lighting_state::EnvironmentLightingState>,
) {
    let Ok(sky_vis) = player.single() else {
        ambient.brightness = 0.0;
        return;
    };
    if pipeline.density_source.is_none() {
        ambient.brightness = 0.0;
        return;
    }

    let cave_factor = sky_vis.cave_depth;
    let sky_factor = sky_vis.sky;

    let base = lighting_state.effective_ambient_brightness * sky_factor;
    if cave_factor > 0.05 {
        ambient.brightness = base * (1.0 - cave_factor * 0.55).max(0.02);
        ambient.color = Color::srgb(
            lighting_state.ambient_color[0] * (1.0 - cave_factor * 0.3),
            lighting_state.ambient_color[1] * (1.0 - cave_factor * 0.2),
            lighting_state.ambient_color[2] * (1.0 - cave_factor * 0.1) + cave_factor * 0.15,
        );
    } else {
        ambient.brightness = 0.0;
    }
}

fn cave_depth_factor(source: &RecipeDensitySource, position: Vec3) -> f32 {
    let sea = source.recipe().sea_level;
    if position.y > sea + 2.0 {
        return 0.0;
    }
    let density = source.density_at(position.x, position.y, position.z);
    if density > 0.0 {
        return ((sea + 2.0 - position.y) / 8.0).clamp(0.0, 1.0);
    }
    ((sea - position.y + 4.0) / 10.0).clamp(0.0, 1.0)
}
