//! Hydrology graph pipeline for world compiler.

pub mod backend;
pub mod features;
pub mod fill;
pub mod graph;
pub mod pass;
pub mod realize;
pub mod routing;
pub mod water_carve;

pub use backend::{HydrologyBackend, RiverHydrology};
pub use graph::{HydroNode, HydrologyGraph, LakeBasin, WaterfallCandidate, WetlandRegion};
pub use pass::{HydrologyFinalizePass, HydrologyPass};
pub use realize::{
    CavePoolBody, CompiledHydrologyProducts, LagoonBody, LakeBody, WaterfallBody, WetlandBody,
    realize_hydrology_from_atlas,
};
pub use water_carve::WaterCarvePass;
