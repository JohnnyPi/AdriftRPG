//! Runtime density providers.

pub mod atlas_provider;
pub mod volumetric_provider;

pub use atlas_provider::{AtlasWorldProvider, RecipeDensityProviderAdapter};
pub use volumetric_provider::VolumetricWorldProvider;
