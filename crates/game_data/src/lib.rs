// crates/game_data/src/lib.rs
//! Typed YAML definitions, validation, and compiled configuration registry.

mod compile;
mod definitions;
mod hash;
mod load;
mod registry;
mod validate;

pub use compile::{
    CompiledApp, CompiledAtmosphere, CompiledBiomes, CompiledCamera, CompiledCave, CompiledDebug,
    CompiledFog, CompiledFogLocalVolume, CompiledHydrology, CompiledIslandGeneration,
    CompiledLandmarkFact, CompiledLandmarks, CompiledLandmarkSign, CompiledLighting,
    CompiledOptions, CompiledPerformance, CompiledPhysics, CompiledPlayer, CompiledRiver,
    CompiledRoute, CompiledRoutes, CompiledSetupGroup, CompiledSetupParameter,
    CompiledSetupPreviewMode, CompiledSetupSchema, CompiledSky, CompiledStructure, CompiledStructurePart, CompiledTerrain, CompiledTerrainMaterials, CompiledSurfaceRules,
    CompiledVegetation, CompiledWater, CompiledWaterBodyMaterial, CompiledWorld,
};
pub use definitions::*;
pub use hash::registry_hash;
pub use load::{load_registry_from_directory, LoadedFile};
pub use registry::ConfigRegistry;
pub use validate::{validate_definitions, ValidationReport};
