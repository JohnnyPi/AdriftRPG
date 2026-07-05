//! Dense 2D field storage with descriptor metadata.

use serde::{Deserialize, Serialize};

use crate::contract::coordinates::WorldXZ;

use super::descriptor::FieldDescriptor;
use super::sampling::{ScalarSampling, sample_bilinear};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DenseField2D<T> {
    pub descriptor: FieldDescriptor,
    pub values: Vec<T>,
}

impl<T: Copy> DenseField2D<T> {
    pub fn index(&self, x: u32, z: u32) -> usize {
        debug_assert!(x < self.descriptor.width && z < self.descriptor.height);
        x as usize + self.descriptor.width as usize * z as usize
    }

    pub fn get(&self, x: u32, z: u32) -> T {
        self.values[self.index(x, z)]
    }

    pub fn set(&mut self, x: u32, z: u32, value: T) {
        let i = self.index(x, z);
        self.values[i] = value;
    }

    pub fn descriptor(&self) -> &FieldDescriptor {
        &self.descriptor
    }
}

impl DenseField2D<f32> {
    pub fn zeros(descriptor: FieldDescriptor) -> Self {
        let count = (descriptor.width * descriptor.height) as usize;
        Self {
            descriptor,
            values: vec![0.0; count],
        }
    }

    pub fn sample_at_world(&self, world: WorldXZ) -> f32 {
        sample_bilinear(self, world, ScalarSampling::Bilinear)
    }

    pub fn world_to_grid(&self, world: WorldXZ) -> (f64, f64) {
        let d = &self.descriptor;
        (
            (world.x() - d.origin_x()) / d.cell_size_m,
            (world.z() - d.origin_z()) / d.cell_size_m,
        )
    }

    pub fn min_max(&self) -> (f32, f32) {
        self.values
            .iter()
            .copied()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), v| {
                (min.min(v), max.max(v))
            })
    }

    pub fn nan_count(&self) -> usize {
        self.values.iter().filter(|v| v.is_nan()).count()
    }
}

impl<T: Copy + Default> DenseField2D<T> {
    pub fn new(descriptor: FieldDescriptor) -> Self {
        let count = (descriptor.width * descriptor.height) as usize;
        Self {
            descriptor,
            values: vec![T::default(); count],
        }
    }
}

#[derive(Clone, Debug)]
pub enum FieldStorage<T> {
    Dense(DenseField2D<T>),
}

impl<T: Copy> FieldStorage<T> {
    pub fn dense(field: DenseField2D<T>) -> Self {
        Self::Dense(field)
    }

    pub fn descriptor(&self) -> &FieldDescriptor {
        match self {
            Self::Dense(f) => &f.descriptor,
        }
    }
}
