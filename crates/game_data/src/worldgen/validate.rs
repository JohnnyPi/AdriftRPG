//! Three-layer validation for worldgen source assets.

use thiserror::Error;

use super::compile::ResolvedWorldBundle;
use super::definitions::*;

#[derive(Debug, Error)]
pub enum WorldgenValidationError {
    #[error("missing {kind} reference: {id}")]
    MissingReference { id: String, kind: &'static str },
    #[error("semantic validation failed: {message}")]
    Semantic { message: String },
    #[error("parse error: {message}")]
    Parse { message: String },
}

pub fn validate_world_source(
    world: &WorldRecipeSource,
    bundle: &WorldgenSourceBundle,
) -> Result<(), WorldgenValidationError> {
    if world.schema_version != 1 {
        return Err(WorldgenValidationError::Semantic {
            message: format!("unsupported world schema_version {}", world.schema_version),
        });
    }
    if world.extent.width_m <= 0.0 || world.extent.depth_m <= 0.0 {
        return Err(WorldgenValidationError::Semantic {
            message: "world extent must be positive".into(),
        });
    }
    if world.resolutions.regional_cell_m < world.resolutions.local_cell_m {
        return Err(WorldgenValidationError::Semantic {
            message: "regional_cell_m must be >= local_cell_m (coarser regional tier)".into(),
        });
    }
    if world.resolutions.control_cell_m < world.resolutions.regional_cell_m {
        return Err(WorldgenValidationError::Semantic {
            message: "control_cell_m must be >= regional_cell_m (coarser control tier)".into(),
        });
    }
    if !bundle.boundaries.contains_key(&world.boundary) {
        return Err(WorldgenValidationError::MissingReference {
            id: world.boundary.clone(),
            kind: "boundary",
        });
    }
    if !bundle.geology.contains_key(&world.geology) {
        return Err(WorldgenValidationError::MissingReference {
            id: world.geology.clone(),
            kind: "geology",
        });
    }
    if !bundle.refinement.contains_key(&world.refinement) {
        return Err(WorldgenValidationError::MissingReference {
            id: world.refinement.clone(),
            kind: "refinement",
        });
    }
    if !bundle.climate.contains_key(&world.climate) {
        return Err(WorldgenValidationError::MissingReference {
            id: world.climate.clone(),
            kind: "climate",
        });
    }
    if !bundle.hydrology.contains_key(&world.hydrology) {
        return Err(WorldgenValidationError::MissingReference {
            id: world.hydrology.clone(),
            kind: "hydrology",
        });
    }
    if !bundle.erosion.contains_key(&world.erosion) {
        return Err(WorldgenValidationError::MissingReference {
            id: world.erosion.clone(),
            kind: "erosion",
        });
    }
    if !bundle.coasts.contains_key(&world.coast) {
        return Err(WorldgenValidationError::MissingReference {
            id: world.coast.clone(),
            kind: "coast",
        });
    }
    if !bundle.biomes.contains_key(&world.biomes) {
        return Err(WorldgenValidationError::MissingReference {
            id: world.biomes.clone(),
            kind: "biomes",
        });
    }
    if !bundle.strata.contains_key(&world.strata) {
        return Err(WorldgenValidationError::MissingReference {
            id: world.strata.clone(),
            kind: "strata",
        });
    }
    for island_id in &world.islands {
        if !bundle.islands.contains_key(island_id) {
            return Err(WorldgenValidationError::MissingReference {
                id: island_id.clone(),
                kind: "island",
            });
        }
    }
    Ok(())
}

pub fn validate_resolved_bundle(
    bundle: &ResolvedWorldBundle,
) -> Result<(), WorldgenValidationError> {
    let recipe = &bundle.recipe;
    let half_w = recipe.extent.width_m * 0.5;
    let half_d = recipe.extent.depth_m * 0.5;
    let margin = recipe.boundary.safety_margin_fraction as f64;

    for island in &recipe.islands {
        let max_r = island
            .footprint
            .major_radius_m
            .max(island.footprint.minor_radius_m) as f64;
        if island.center_x_m.abs() + max_r > half_w * (1.0 - margin) {
            return Err(WorldgenValidationError::Semantic {
                message: format!(
                    "island {} major axis exceeds safe interior margin",
                    island.id
                ),
            });
        }
        if island.center_z_m.abs() + max_r > half_d * (1.0 - margin) {
            return Err(WorldgenValidationError::Semantic {
                message: format!(
                    "island {} minor axis exceeds safe interior margin",
                    island.id
                ),
            });
        }
    }

    let refinement = &recipe.refinement;
    if refinement.window_stride_samples[0] > refinement.window_interior_samples[0]
        || refinement.window_stride_samples[1] > refinement.window_interior_samples[1]
    {
        return Err(WorldgenValidationError::Semantic {
            message: "window stride must not exceed interior size".into(),
        });
    }

    Ok(())
}
