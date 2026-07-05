//! Field registry for WorldAtlas.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::dense::DenseField2D;
use super::descriptor::FieldDescriptor;
use super::key::FieldKey;
use super::typed::CategoricalField;

#[derive(Clone)]
pub struct FieldRegistry {
    scalar: BTreeMap<FieldKey, Arc<DenseField2D<f32>>>,
    categorical: BTreeMap<FieldKey, Arc<CategoricalField<u8>>>,
    descriptors: BTreeMap<FieldKey, FieldDescriptor>,
}

impl Default for FieldRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FieldRegistry {
    pub fn new() -> Self {
        Self {
            scalar: BTreeMap::new(),
            categorical: BTreeMap::new(),
            descriptors: BTreeMap::new(),
        }
    }

    pub fn insert_scalar(&mut self, key: FieldKey, field: DenseField2D<f32>) {
        self.descriptors.insert(key, field.descriptor.clone());
        self.scalar.insert(key, Arc::new(field));
    }

    pub fn insert_categorical<T>(&mut self, key: FieldKey, field: CategoricalField<T>)
    where
        T: Copy + Into<u8>,
    {
        self.descriptors.insert(key, field.descriptor().clone());
        let desc = field.descriptor().clone();
        let mut storage = DenseField2D::<u8>::new(desc);
        for z in 0..field.0.descriptor.height {
            for x in 0..field.0.descriptor.width {
                storage.set(x, z, field.get(x, z).into());
            }
        }
        self.categorical
            .insert(key, Arc::new(CategoricalField(storage)));
    }

    pub fn get_scalar(&self, key: FieldKey) -> Option<Arc<DenseField2D<f32>>> {
        self.scalar.get(&key).cloned()
    }

    pub fn get_categorical(&self, key: FieldKey) -> Option<Arc<CategoricalField<u8>>> {
        self.categorical.get(&key).cloned()
    }

    pub fn descriptors(&self) -> &BTreeMap<FieldKey, FieldDescriptor> {
        &self.descriptors
    }
}
