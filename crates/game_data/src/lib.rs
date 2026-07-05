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
    CompiledApp, CompiledAtmosphere, CompiledBiomes, CompiledCamera, CompiledCave,
    CompiledChunkResidency, CompiledChunkStaging, CompiledContentLod, CompiledDebug,
    CompiledDistantLod, CompiledFog, CompiledFogLocalVolume, CompiledHydrologyBody,
    CompiledIslandGeneration, CompiledLandmarkFact, CompiledLandmarkSign, CompiledLandmarks,
    CompiledLighting, CompiledMaterialLod, CompiledOptions, CompiledPerformance, CompiledPhysics,
    CompiledPlayer, CompiledRenderDistanceLodTier, CompiledRenderProfile, CompiledRoute,
    CompiledRoutes, CompiledSetupGroup, CompiledSetupParameter, CompiledSetupPreviewMode,
    CompiledSetupSchema, CompiledSky, CompiledStructure, CompiledStructurePart,
    CompiledSurfaceRules, CompiledTerrain, CompiledTerrainLodTier, CompiledTerrainMaterials,
    CompiledVegetation, CompiledWater, CompiledWaterBodyMaterial, CompiledWeatherProfile,
    CompiledWorld, CompiledWorldLod,
};
pub use definitions::*;
pub use hash::registry_hash;
pub use load::{LoadedFile, load_registry_from_directory};
pub use material_catalog::*;
pub use material_overrides::{LayeredScalar, MaterialInvalidation, OverrideLayer};
pub use registry::ConfigRegistry;
pub use surface_registry::{
    CompiledSurfaceRegistry, CompiledTextureRecipe, MaterialDependencyIndex,
    build_surface_registry, deprecated_overlay_warnings, is_deprecated_overlay_material,
    resolve_entry_generator,
};
pub use validate::{ValidationReport, validate_definitions};
pub use worldgen::{
    BiomeRecipeSource, BoundaryRecipeSource, CavesRecipeSource, ClimateRecipeSource,
    CoastRecipeSource, CompiledBiomeRecipe, CompiledBoundaryRecipe, CompiledCaveFamilyProfile,
    CompiledCavesRecipe, CompiledClimateRecipe, CompiledCoastRecipe, CompiledErosionRecipe,
    CompiledFootprint, CompiledGeologyRecipe, CompiledHydrologyRecipe, CompiledIslandRecipe,
    CompiledRefinementRecipe, CompiledStrataDeposit, CompiledStrataLayer, CompiledStrataRecipe,
    CompiledValidationRecipe, CompiledVolcano, CompiledWorldRecipe, ErosionRecipeSource,
    FootprintSource, GeologyRecipeSource, HydrologyRecipeSource, IslandPlacementSource,
    IslandRecipeSource, RefinementRecipeSource, ResolvedWorldBundle, StrataRecipeSource,
    ValidationRecipeSource, WorldRecipeSource, WorldgenLoadError, WorldgenSourceBundle,
    WorldgenValidationError, load_worldgen_bundle, recipe_content_hash, resolve_world_bundle,
    validate_resolved_bundle,
};

mod worldgen;
