//! Compiler pass identifiers and trait.

pub use crate::fields::key::FieldKey;

use super::context::CompileContext;
use super::error::WorldgenError;
use super::report::PassReport;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PassKey {
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
    Caves,
    WaterCarve,
    FinalValidation,
}

pub trait WorldgenPass: Send + Sync {
    fn key(&self) -> PassKey;

    fn inputs(&self) -> &'static [FieldKey];

    fn outputs(&self) -> &'static [FieldKey];

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError>;
}
