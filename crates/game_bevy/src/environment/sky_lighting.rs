//! Unified sky and lighting plugin group (SkyLightingGuide §1).

use bevy::prelude::*;

use super::{
    AtmosphereScenePlugin, CelestialPlugin, CloudPlugin, EnvironmentConfigPlugin,
    EnvironmentLightingPlugin, FogPlugin, LightingPlugin, SimulationTimePlugin, StarfieldPlugin,
    WeatherPlugin,
};
use super::volumetric_scatter::VolumetricScatterPlugin;

/// Registers the full outdoor sky + lighting stack in dependency-safe order.
pub struct SkyLightingPlugin;

impl Plugin for SkyLightingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            LightingPlugin,
            SimulationTimePlugin,
            EnvironmentConfigPlugin,
            EnvironmentLightingPlugin,
            CelestialPlugin,
            AtmosphereScenePlugin,
            VolumetricScatterPlugin,
            CloudPlugin,
            FogPlugin,
            StarfieldPlugin,
            WeatherPlugin,
        ));
    }
}
