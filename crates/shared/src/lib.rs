// crates/shared/src/lib.rs
//! Shared types used across engine crates without Bevy dependencies.

mod definition;
mod error;
mod id;
pub mod math;

pub use definition::{DefinitionHeader, SUPPORTED_SCHEMA_VERSION};
pub use error::{DataError, DataResult};
pub use id::StableId;
pub use math::{
    hash_unit, lerp, lerp_rgb, range_weight, remap, saturate, slope_degrees, smoothstep,
};
