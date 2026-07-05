//! Field framework for world atlas storage.

pub mod dense;
pub mod descriptor;
pub mod key;
pub mod registry;
pub mod resampling;
pub mod sampling;
pub mod scalar;
pub mod typed;

pub use dense::{DenseField2D, FieldStorage};
pub use descriptor::{FieldDescriptor, FieldValueKind, SampleLayout};
pub use key::FieldKey;
pub use registry::FieldRegistry;
pub use scalar::ScalarField;
pub use typed::{CategoricalField, MaskField};
