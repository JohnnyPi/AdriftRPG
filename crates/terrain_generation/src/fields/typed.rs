//! Typed field wrappers.

use serde::{Deserialize, Serialize};

use super::dense::DenseField2D;
use super::descriptor::{FieldDescriptor, FieldValueKind};

pub type ScalarField = DenseField2D<f32>;
pub type MaskField = DenseField2D<f32>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CategoricalField<T: Copy>(pub DenseField2D<T>);

impl<T: Copy> CategoricalField<T> {
    pub fn descriptor(&self) -> &FieldDescriptor {
        self.0.descriptor()
    }

    pub fn get(&self, x: u32, z: u32) -> T {
        self.0.get(x, z)
    }
}

impl<T: Copy + Default> CategoricalField<T> {
    pub fn zeros(descriptor: FieldDescriptor) -> Self {
        Self(DenseField2D::new(
            descriptor.with_kind(FieldValueKind::Categorical),
        ))
    }

    pub fn set(&mut self, x: u32, z: u32, value: T) {
        self.0.set(x, z, value);
    }
}
