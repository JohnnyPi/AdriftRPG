// crates/terrain_surface/src/registry.rs
use std::collections::BTreeMap;

use crate::material_id::MaterialKey;

#[derive(Clone, Debug, Default)]
pub struct MaterialLayerRegistry {
    by_material: BTreeMap<MaterialKey, u32>,
    by_layer: Vec<MaterialKey>,
}

impl MaterialLayerRegistry {
    pub fn build(ordered_materials: impl IntoIterator<Item = MaterialKey>) -> Self {
        let mut registry = Self::default();
        for material in ordered_materials {
            let layer = registry.by_layer.len() as u32;
            assert!(
                registry.by_material.insert(material.clone(), layer).is_none(),
                "duplicate material {material}"
            );
            registry.by_layer.push(material);
        }
        registry
    }

    pub fn from_layer_order(layer_order: &[MaterialKey]) -> Self {
        Self::build(layer_order.iter().cloned())
    }

    pub fn layer(&self, material: &MaterialKey) -> Option<u32> {
        self.by_material.get(material).copied()
    }

    pub fn layer_or_fallback(&self, material: &MaterialKey) -> u32 {
        self.layer(material).unwrap_or(0)
    }

    pub fn layer_count(&self) -> u32 {
        self.by_layer.len() as u32
    }

    pub fn material_at_layer(&self, layer: u32) -> Option<&MaterialKey> {
        self.by_layer.get(layer as usize)
    }

    pub fn layer_order(&self) -> &[MaterialKey] {
        &self.by_layer
    }
}
