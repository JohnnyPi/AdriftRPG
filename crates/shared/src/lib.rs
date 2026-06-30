//! Shared types used across engine crates without Bevy dependencies.

mod definition;
mod error;
mod id;

pub use definition::{DefinitionHeader, SUPPORTED_SCHEMA_VERSION};
pub use error::{DataError, DataResult};
pub use id::StableId;
