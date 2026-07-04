//! Ground-hugging volumetric fog for sun god-rays (SkyLightingGuide §8).
//!
//! Uses a thin sea-level `FogVolume` — never a sky-height box (see atmosphere.rs).

use bevy::light::{FogVolume, VolumetricFog};
use bevy::prelude::*;
use shared::smoothstep;

use super::celestial::CelestialState;
use super::config_init::{EnvironmentInitSet, sea_level_for_prefs};
use super::fog::FogStack;
use super::sky_config::{SkyEffectsRevision, SkyPresentationConfig};
use crate::camera::MainGameCamera;
use crate::data::{ConfigRegistryResource, UserSetupPrefs};
use crate::state::AppState;

/// Vertical half-extent of the god-ray mist shell (m).
pub const GOD_RAY_HALF_Y: f32 = 6.0;
/// Offset from sea level to volume center (m). Top = sea + 12 m.
pub const GOD_RAY_CENTER_OFFSET_Y: f32 = 6.0;
/// Documented ceiling: volume top must stay low to keep cameras above the shell.
pub const GOD_RAY_CAMERA_SAFETY_TOP_M: f32 = 12.0;

const BASE_DENSITY_FACTOR: f32 = 0.05;
const HEIGHT_FADE_RANGE_M: f32 = 8.0;
const MIN_SUN_ELEVATION_DEG: f32 = 5.0;

#[derive(Component)]
pub struct SunGodRayVolume {
    pub base_density_factor: f32,
    pub volume_top_y: f32,
}

#[derive(Resource, Default)]
struct GodRayVolumeSpawned(bool);

pub struct VolumetricScatterPlugin;

impl Plugin for VolumetricScatterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GodRayVolumeSpawned>()
            .add_systems(
                OnEnter(AppState::Running),
                spawn_god_ray_volume.after(EnvironmentInitSet),
            )
            .add_systems(
                Update,
                (
                    respawn_god_ray_on_sky_revision,
                    sync_god_ray_volume_strength,
                    sync_god_ray_colors,
                    sync_volumetric_fog_ambient,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

pub fn god_ray_volume_top_y(sea_level_m: f32) -> f32 {
    sea_level_m + GOD_RAY_CENTER_OFFSET_Y + GOD_RAY_HALF_Y
}

fn spawn_god_ray_volume(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    fog_stack: Res<FogStack>,
    sky: Res<SkyPresentationConfig>,
    celestial: Res<CelestialState>,
    mut spawned: ResMut<GodRayVolumeSpawned>,
    existing: Query<Entity, With<SunGodRayVolume>>,
) {
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }
    spawned.0 = false;
    spawn_god_ray_volume_entity(
        &mut commands,
        &registry,
        &prefs,
        &fog_stack,
        &sky,
        &celestial,
    );
    spawned.0 = true;
}

fn spawn_god_ray_volume_entity(
    commands: &mut Commands,
    registry: &ConfigRegistryResource,
    prefs: &UserSetupPrefs,
    fog_stack: &FogStack,
    sky: &SkyPresentationConfig,
    celestial: &CelestialState,
) {
    let sea_level = sea_level_for_prefs(registry, prefs);
    let half_xz = fog_stack.ocean_extent_m * 0.5;
    let center_y = sea_level + GOD_RAY_CENTER_OFFSET_Y;
    let volume_top = god_ray_volume_top_y(sea_level);
    debug_assert!(volume_top <= sea_level + GOD_RAY_CAMERA_SAFETY_TOP_M);

    let horizon = sky.horizon_color;
    let sun = celestial.sun_color;

    commands.spawn((
        SunGodRayVolume {
            base_density_factor: BASE_DENSITY_FACTOR,
            volume_top_y: volume_top,
        },
        FogVolume {
            fog_color: Color::srgba(horizon[0], horizon[1], horizon[2], 1.0),
            density_factor: BASE_DENSITY_FACTOR,
            absorption: 0.04,
            scattering: 0.40,
            scattering_asymmetry: 0.65,
            light_tint: Color::srgb(sun[0], sun[1], sun[2]),
            light_intensity: 1.0,
            ..default()
        },
        Transform::from_translation(Vec3::new(0.0, center_y, 0.0)).with_scale(Vec3::new(
            half_xz * 2.0,
            GOD_RAY_HALF_Y * 2.0,
            half_xz * 2.0,
        )),
        Visibility::default(),
    ));
}

fn respawn_god_ray_on_sky_revision(
    revision: Res<SkyEffectsRevision>,
    mut last: Local<Option<u32>>,
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    fog_stack: Res<FogStack>,
    sky: Res<SkyPresentationConfig>,
    celestial: Res<CelestialState>,
    mut spawned: ResMut<GodRayVolumeSpawned>,
    existing: Query<Entity, With<SunGodRayVolume>>,
) {
    if last.is_none() {
        *last = Some(revision.0);
        return;
    }
    if *last == Some(revision.0) {
        return;
    }
    *last = Some(revision.0);
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }
    spawned.0 = false;
    spawn_god_ray_volume_entity(
        &mut commands,
        &registry,
        &prefs,
        &fog_stack,
        &sky,
        &celestial,
    );
    spawned.0 = true;
}

fn sync_god_ray_volume_strength(
    celestial: Res<CelestialState>,
    cameras: Query<&Transform, With<MainGameCamera>>,
    mut volumes: Query<(&SunGodRayVolume, &mut FogVolume, &mut Visibility)>,
) {
    let Ok(camera_tf) = cameras.single() else {
        return;
    };

    let sun_up = celestial.sun_elevation_deg >= MIN_SUN_ELEVATION_DEG;
    let camera_y = camera_tf.translation.y;

    for (meta, mut fog, mut visibility) in &mut volumes {
        let fade_start = meta.volume_top_y;
        let fade_end = meta.volume_top_y + HEIGHT_FADE_RANGE_M;
        let height_factor = smoothstep(fade_start, fade_end, camera_y);

        if !sun_up || height_factor <= 0.0 {
            *visibility = Visibility::Hidden;
            continue;
        }

        *visibility = Visibility::Inherited;
        fog.density_factor = meta.base_density_factor * height_factor;
    }
}

fn sync_volumetric_fog_ambient(
    celestial: Res<CelestialState>,
    mut cameras: Query<&mut VolumetricFog, With<MainGameCamera>>,
) {
    let ambient = celestial.fog_inscattering;
    for mut fog in &mut cameras {
        fog.ambient_color = Color::srgb(ambient[0], ambient[1], ambient[2]);
        fog.ambient_intensity = fog.ambient_intensity.max(0.08);
    }
}

fn sync_god_ray_colors(
    celestial: Res<CelestialState>,
    mut volumes: Query<&mut FogVolume, With<SunGodRayVolume>>,
) {
    let horizon = celestial.fog_inscattering;
    let sun = celestial.sun_color;
    for mut fog in &mut volumes {
        fog.fog_color = Color::srgba(horizon[0], horizon[1], horizon[2], 1.0);
        fog.light_tint = Color::srgb(sun[0], sun[1], sun[2]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn god_ray_volume_top_stays_below_safety_ceiling() {
        let sea = 2.0;
        let top = god_ray_volume_top_y(sea);
        assert_eq!(top, sea + GOD_RAY_CAMERA_SAFETY_TOP_M);
        assert!(top <= sea + GOD_RAY_CAMERA_SAFETY_TOP_M);
    }

    #[test]
    fn god_ray_hidden_when_sun_low() {
        let sun_up = 4.0_f32 >= MIN_SUN_ELEVATION_DEG;
        assert!(!sun_up);
    }

    #[test]
    fn height_factor_fades_inside_volume() {
        let top = 14.0;
        assert_eq!(smoothstep(top, top + HEIGHT_FADE_RANGE_M, top), 0.0);
        assert_eq!(
            smoothstep(top, top + HEIGHT_FADE_RANGE_M, top + HEIGHT_FADE_RANGE_M),
            1.0
        );
    }
}
