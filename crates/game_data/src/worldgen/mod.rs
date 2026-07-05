//! World-generation YAML source and compiled recipe types.

pub mod compile;
pub mod definitions;
pub mod hash;
pub mod load;
pub mod validate;

pub use compile::{
    CompiledBiomeRecipe, CompiledBoundaryRecipe, CompiledCaveFamilyProfile, CompiledCavesRecipe,
    CompiledClimateRecipe, CompiledCoastRecipe, CompiledErosionRecipe, CompiledFootprint,
    CompiledGeologyRecipe, CompiledHydrologyRecipe, CompiledIslandRecipe, CompiledRefinementRecipe,
    CompiledStrataDeposit, CompiledStrataLayer, CompiledStrataRecipe, CompiledValidationRecipe,
    CompiledVolcano, CompiledWorldRecipe, ResolvedWorldBundle, resolve_world_bundle,
};
pub use definitions::*;
pub use hash::recipe_content_hash;
pub use load::{WorldgenLoadError, load_worldgen_bundle};
pub use validate::{WorldgenValidationError, validate_resolved_bundle};
