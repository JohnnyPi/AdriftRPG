//! Bevy procedural atmosphere (SkyLightingGuide §2).

use bevy::camera::{Exposure, Hdr};
use bevy::color::palettes::css::BLACK;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::{
    Atmosphere, AtmosphereEnvironmentMapLight, VolumetricFog, VolumetricLight,
    atmosphere::ScatteringMedium,
};
use bevy::pbr::{AtmosphereMode, AtmosphereSettings};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

use super::config_init::EnvironmentInitSet;
use crate::state::AppState;

#[derive(Resource, Clone)]
pub struct PlanetAtmosphereMedium(pub Handle<ScatteringMedium>);

#[derive(Component)]
pub struct PlanetAtmosphere;

pub struct AtmosphereScenePlugin;

impl Plugin for AtmosphereScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::Running),
            spawn_planet_atmosphere.after(EnvironmentInitSet),
        );
    }
}

fn spawn_planet_atmosphere(
    mut commands: Commands,
    mut scattering_mediums: ResMut<Assets<ScatteringMedium>>,
) {
    let earth_medium = scattering_mediums.add(ScatteringMedium::earth(256, 256));
    let medium = PlanetAtmosphereMedium(earth_medium);
    let atmosphere = Atmosphere::earth(medium.0.clone());
    commands.insert_resource(medium);

    // Planet center offset is applied by `Atmosphere`'s on_add hook (see bevy_light).
    commands.spawn((PlanetAtmosphere, atmosphere));

    // `clouds_enabled` is reserved for a future cloud layer. Do not spawn a
    // large `FogVolume` here: Bevy volumetric fog runs after the atmosphere sky
    // pass and, when the camera is inside the volume (y≈30–210 m for the old
    // 180 m-tall box), switches to a full-screen raymarch that composites
    // nearly black over sky pixels when `ambient_intensity` is low.
}

/// Camera components required for procedural atmosphere rendering.
pub fn atmosphere_camera_bundle() -> impl Bundle {
    (
        AtmosphereSettings {
            rendering_method: AtmosphereMode::LookupTexture,
            ..default()
        },
        AtmosphereEnvironmentMapLight {
            intensity: 0.9,
            size: UVec2::splat(256),
            ..default()
        },
        Hdr,
        Exposure::default(),
        Tonemapping::AcesFitted,
        Bloom {
            intensity: 0.08,
            low_frequency_boost: 0.4,
            low_frequency_boost_curvature: 0.5,
            ..default()
        },
        // Non-zero ambient is required when `AtmosphereEnvironmentMapLight` is
        // present; otherwise volumetric fog darkens sky pixels that raymarch
        // through any future `FogVolume` (see bevy_light::VolumetricFog docs).
        VolumetricFog::default(),
        // MSAA depth resolves can disagree with the atmosphere sky pass depth==0.0 test.
        Msaa::Off,
    )
}

/// Attach volumetric sun shafts to the directional sun (Phase 5).
pub fn attach_volumetric_sun(commands: &mut Commands, sun_entity: Entity) {
    commands.entity(sun_entity).insert(VolumetricLight);
}

/// Black clear color — atmosphere fills the background.
pub fn atmosphere_clear_color() -> ClearColor {
    ClearColor(BLACK.into())
}
