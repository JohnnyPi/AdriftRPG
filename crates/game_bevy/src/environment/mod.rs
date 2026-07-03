// crates/game_bevy/src/environment/mod.rs
pub mod atmosphere;
pub mod audio;
pub mod biome_context;
pub mod biomes;
pub mod celestial;
pub mod clouds;
pub mod config_init;
pub mod fog;
pub mod lighting;
pub mod lighting_state;
pub mod materials;
pub mod sky_config;
pub mod surface;
pub mod volumetric_scatter;

pub use atmosphere::AtmosphereScenePlugin;
pub use audio::EnvironmentAudioStubPlugin;
pub use biomes::{BiomeCatalog, BiomeInitSet, BiomePlugin};
pub use celestial::CelestialPlugin;
pub use clouds::CloudPlugin;
pub use config_init::EnvironmentConfigPlugin;
pub use fog::FogPlugin;
pub use lighting::{LightingPlugin, SunLight};
pub use lighting_state::EnvironmentLightingPlugin;
pub use sky_config::{SkyEffectsRevision, SkyPresentationConfig};
pub use volumetric_scatter::VolumetricScatterPlugin;
