//! World compiler orchestration.

pub mod context;
pub mod error;
pub mod hydrology_validation;
pub mod pass;
pub mod report;
pub mod validation;

use std::path::PathBuf;

use game_data::{CompiledWorldRecipe, ResolvedWorldBundle, validate_resolved_bundle};

use crate::biomes::BiomePass;
use crate::boundary::BoundaryPass;
use crate::caves::CavePass;
use crate::climate::ClimatePass;
use crate::coast::CoastPass;
use crate::contract::manifest::WorldManifest;
use crate::contract::metadata::{RecipeHash, WorldExtent};
use crate::erosion::ErosionPass;
use crate::geology::GeologyPass;
use crate::hydrology::WaterCarvePass;
use crate::hydrology::{HydrologyFinalizePass, HydrologyPass};
use crate::islands::pass::IslandSkeletonPass;
use crate::macro_terrain::{BathymetryPass, MacroTerrainPass};
use crate::regional::RegionalRefinementPass;
use crate::soil::SoilPass;
use crate::strata::StrataPass;
use crate::world::atlas::WorldAtlas;

use self::context::CompileContext;
pub use self::error::WorldgenError;
use self::hydrology_validation::HydrologyValidationPass;
use self::pass::WorldgenPass;
use self::validation::FinalValidationPass;

/// Tooling options — not world design parameters.
#[derive(Clone, Debug, Default)]
pub struct CompileOptions {
    pub output_directory: Option<PathBuf>,
    pub write_debug_maps: bool,
    pub retain_intermediate_fields: bool,
    pub enable_parallelism: bool,
}

#[derive(Clone, Debug)]
pub enum CompileStage {
    ResolveRecipe,
    ValidateRecipe,
    AllocateAtlas,
    Boundary,
    IslandSkeleton,
    MacroTerrain,
    Bathymetry,
    Geology,
    RegionalRefinement,
    Climate,
    Hydrology,
    Erosion,
    HydrologyFinalize,
    HydrologyValidation,
    Coast,
    Soil,
    Biome,
    Strata,
    FinalValidation,
    Caves,
    WaterCarve,
    Persist,
    Complete,
    Failed,
}

/// Product of a successful world compilation.
#[derive(Clone)]
pub struct CompiledWorld {
    pub manifest: WorldManifest,
    pub atlas: WorldAtlas,
}

pub trait WorldCompiler {
    fn validate(&self, bundle: &ResolvedWorldBundle) -> Result<(), WorldgenError>;

    fn compile(
        &self,
        recipe: &CompiledWorldRecipe,
        options: &CompileOptions,
    ) -> Result<CompiledWorld, WorldgenError>;
}

pub struct DefaultWorldCompiler;

impl Default for DefaultWorldCompiler {
    fn default() -> Self {
        Self
    }
}

impl WorldCompiler for DefaultWorldCompiler {
    fn validate(&self, bundle: &ResolvedWorldBundle) -> Result<(), WorldgenError> {
        validate_resolved_bundle(bundle)?;
        Ok(())
    }

    fn compile(
        &self,
        recipe: &CompiledWorldRecipe,
        options: &CompileOptions,
    ) -> Result<CompiledWorld, WorldgenError> {
        let _ = options;
        let mut ctx = CompileContext::new(recipe);
        let mut reports = Vec::new();

        let passes: Vec<Box<dyn WorldgenPass>> = vec![
            Box::new(BoundaryPass),
            Box::new(IslandSkeletonPass),
            Box::new(MacroTerrainPass),
            Box::new(BathymetryPass),
            Box::new(GeologyPass),
            Box::new(RegionalRefinementPass),
            Box::new(ClimatePass),
            Box::new(HydrologyPass),
            Box::new(ErosionPass),
            Box::new(HydrologyFinalizePass),
            Box::new(HydrologyValidationPass),
            Box::new(CoastPass),
            Box::new(SoilPass),
            Box::new(BiomePass),
            Box::new(StrataPass),
            Box::new(CavePass),
            Box::new(WaterCarvePass),
            Box::new(FinalValidationPass),
        ];

        for pass in passes {
            reports.push(pass.run(&mut ctx)?);
        }

        let extent = WorldExtent {
            width_m: recipe.extent.width_m,
            depth_m: recipe.extent.depth_m,
            vertical_min_m: recipe.extent.vertical_min_m,
            vertical_max_m: recipe.extent.vertical_max_m,
            sea_level_m: recipe.extent.sea_level_m,
        };

        let manifest = WorldManifest {
            world_id: recipe.id.clone(),
            recipe_id: recipe.id.clone(),
            recipe_hash: RecipeHash::from_bytes(recipe.recipe_hash.0),
            generator_version: crate::contract::version::GENERATOR_VERSION,
            seed: recipe.seed,
            extent,
            sea_level_m: recipe.extent.sea_level_m,
            field_descriptors: ctx.atlas.fields.descriptors().clone(),
            pass_reports: reports,
        };

        if options.write_debug_maps {
            if let Some(dir) = &options.output_directory {
                let _ = crate::diagnostics::export::export_all_fields(&ctx.atlas, dir);
            }
        }

        Ok(CompiledWorld {
            manifest,
            atlas: ctx.atlas,
        })
    }
}

pub fn compile_world_from_bundle(
    bundle: &ResolvedWorldBundle,
    options: &CompileOptions,
) -> Result<CompiledWorld, WorldgenError> {
    let compiler = DefaultWorldCompiler;
    compiler.validate(bundle)?;
    compiler.compile(&bundle.recipe, options)
}

pub fn compile_world_from_recipe(
    recipe: &CompiledWorldRecipe,
    options: &CompileOptions,
) -> Result<CompiledWorld, WorldgenError> {
    DefaultWorldCompiler.compile(recipe, options)
}
