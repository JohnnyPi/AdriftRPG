//! Typed YAML definitions, validation, and compiled configuration registry.

mod compile;
mod definitions;
mod hash;
mod load;
mod registry;
mod validate;

pub use compile::{
    CompiledApp, CompiledBiomes, CompiledCamera, CompiledCave, CompiledDebug, CompiledLighting,
    CompiledPerformance, CompiledPlayer, CompiledTerrain, CompiledTerrainMaterials,
    CompiledVegetation, CompiledWater, CompiledWorld,
};
pub use definitions::*;
pub use hash::registry_hash;
pub use load::{load_registry_from_directory, LoadedFile};
pub use registry::ConfigRegistry;
pub use validate::ValidationReport;
