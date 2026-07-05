//! Geology-driven volumetric cave generation (Phase 14).

pub mod graph;
pub mod graph_gen;
pub mod pass;
pub mod recipe;
pub mod sdf;
pub mod suitability;
pub mod validate;

pub use graph::{CaveFamily, CaveGraphRegistry, CaveNodeKind, CaveSystem};
pub use pass::CavePass;
pub use sdf::{CaveSubtractOps, SubtractShape};
pub use validate::{CaveValidationReport, validate_cave_systems};
