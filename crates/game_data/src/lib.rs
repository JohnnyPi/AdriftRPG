// crates/game_data/src/lib.rs
//! Typed YAML definitions, validation, and compiled configuration registry.

mod compile;
mod definitions;
mod hash;
mod load;
mod material_catalog;
mod material_overrides;
mod registry;
mod surface_registry;
mod validate;

pub use compile::{
    CompiledApp, CompiledAtmosphere, CompiledBiomes, CompiledCamera, CompiledCave, CompiledDebug,
    CompiledFog, CompiledFogLocalVolume, CompiledIslandGeneration,
    CompiledLandmarkFact, CompiledLandmarks, CompiledLandmarkSign, CompiledLighting,
    CompiledOptions, CompiledPerformance, CompiledPhysics, CompiledPlayer,
    CompiledRoute, CompiledRoutes, CompiledSetupGroup, CompiledSetupParameter,
    CompiledSetupPreviewMode, CompiledSetupSchema, CompiledSky, CompiledStructure, CompiledStructurePart, CompiledTerrain, CompiledTerrainMaterials, CompiledSurfaceRules,
    CompiledVegetation, CompiledWater, CompiledWaterBodyMaterial, CompiledHydrologyBody, CompiledWorld,
    CompiledChunkResidency, CompiledWorldLod, CompiledTerrainLodTier, CompiledMaterialLod,
    CompiledContentLod, CompiledDistantLod, CompiledChunkStaging, CompiledRenderProfile,
    CompiledRenderDistanceLodTier, CompiledWeatherProfile,
};
pub use definitions::*;
pub use hash::registry_hash;
pub use load::{load_registry_from_directory, LoadedFile};
pub use material_catalog::*;
pub use material_overrides::{LayeredScalar, MaterialInvalidation, OverrideLayer};
pub use registry::ConfigRegistry;
pub use surface_registry::{
    build_surface_registry, deprecated_overlay_warnings, is_deprecated_overlay_material,
    resolve_entry_generator, CompiledSurfaceRegistry, CompiledTextureRecipe,
    MaterialDependencyIndex,
};
pub use validate::{validate_definitions, ValidationReport};
