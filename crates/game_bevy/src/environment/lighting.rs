use bevy::prelude::*;

use crate::data::ConfigRegistryResource;
use crate::player::Player;
use crate::state::AppState;
use crate::terrain::TerrainPipelineState;
use crate::ui::LightingTweaks;
use terrain_generation::RecipeDensitySource;

#[derive(Component)]
pub struct SunLight;

#[derive(Component)]
pub struct CaveAmbientZone;

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                apply_lighting_hot_reload,
                apply_cave_atmosphere,
                update_sky_visibility,
            )
                .run_if(in_state(AppState::Running)),
        );
    }
}

fn apply_lighting_hot_reload(
    registry: Res<ConfigRegistryResource>,
    tweaks: Res<LightingTweaks>,
    mut last_hash: Local<Option<String>>,
    mut clear: ResMut<ClearColor>,
) {
    let hash = registry.0.hash.clone();
    if last_hash.as_ref() == Some(&hash) && !tweaks.override_fog {
        return;
    }
    *last_hash = Some(hash);

    let Ok(lighting) = registry.0.active_lighting() else {
        return;
    };

    let fog_color = if tweaks.override_fog {
        tweaks.fog_color
    } else {
        lighting.fog_color
    };

    clear.0 = Color::srgb(fog_color[0] * 0.85, fog_color[1] * 0.9, fog_color[2] * 1.05);
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
        sky_vis.0 = 1.0;
        return;
    };
    sky_vis.0 = sky_visibility_at(source, player_tf.translation);
}

pub fn sky_visibility_at(source: &RecipeDensitySource, position: Vec3) -> f32 {
    let sea = source.recipe().sea_level;
    if position.y < sea - 2.0 {
        let density = source.density_at(position.x, position.y + 2.0, position.z);
        if density < 0.0 {
            return 0.15;
        }
    }
    1.0 - cave_depth_factor(source, position) * 0.85
}

fn apply_cave_atmosphere(
    pipeline: Res<TerrainPipelineState>,
    player: Query<(&Transform, &super::lighting_state::SkyVisibility), With<Player>>,
    mut zones: Query<(&Transform, &mut PointLight), With<CaveAmbientZone>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    registry: Res<ConfigRegistryResource>,
) {
    let Ok(lighting) = registry.0.active_lighting() else {
        return;
    };
    let Ok((player_tf, sky_vis)) = player.single() else {
        return;
    };
    let Some(source) = pipeline.density_source.as_ref() else {
        return;
    };

    let cave_factor = cave_depth_factor(source, player_tf.translation);
    let sky_factor = sky_vis.0;
    for (tf, mut light) in &mut zones {
        let dist = player_tf.translation.distance(tf.translation);
        light.intensity = if dist < 25.0 {
            120000.0 * (1.0 - dist / 25.0) * cave_factor
        } else {
            0.0
        };
    }

    let base = lighting.ambient_brightness * sky_factor;
    ambient.brightness = base * (1.0 - cave_factor * 0.55);
    ambient.color = Color::srgb(
        lighting.ambient_color[0] * (1.0 - cave_factor * 0.3),
        lighting.ambient_color[1] * (1.0 - cave_factor * 0.2),
        lighting.ambient_color[2] * (1.0 - cave_factor * 0.1) + cave_factor * 0.15,
    );
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

/// Stub trait for future global illumination / light propagation.
#[allow(dead_code)]
pub trait LightPropagationBackend: Send + Sync {
    fn propagate(&self, _origin: Vec3) -> f32 {
        1.0
    }
}

#[allow(dead_code)]
pub struct StubLightPropagation;

impl LightPropagationBackend for StubLightPropagation {}

/// Simulation time stub for day/night cycle.
#[allow(dead_code)]
pub trait SimulationTime: Send + Sync {
    fn hours(&self) -> f32 {
        10.5
    }
}

#[allow(dead_code)]
pub struct FixedMorningTime;

impl SimulationTime for FixedMorningTime {}

/// Celestial lighting stub (sun/moon orbit, phases).
#[allow(dead_code)]
pub trait CelestialLightingBackend: Send + Sync {
    fn sun_direction(&self) -> Vec3 {
        Vec3::new(-0.4, -0.85, -0.3).normalize_or_zero()
    }

    fn moon_direction(&self) -> Vec3 {
        Vec3::new(0.5, 0.6, 0.2).normalize_or_zero()
    }

    fn moon_phase(&self) -> f32 {
        0.25
    }
}

#[allow(dead_code)]
pub struct FixedCelestialLighting;

impl CelestialLightingBackend for FixedCelestialLighting {}
