use std::collections::BTreeMap;

use crate::material_id::TerrainMaterialId;

#[derive(Clone, Debug, Default)]
pub struct MaterialLayerRegistry {
    by_material: BTreeMap<TerrainMaterialId, u32>,
    by_layer: Vec<TerrainMaterialId>,
}

impl MaterialLayerRegistry {
    pub fn build(ordered_materials: impl IntoIterator<Item = TerrainMaterialId>) -> Self {
        let mut registry = Self::default();
        for material in ordered_materials {
            let layer = registry.by_layer.len() as u32;
            assert!(
                registry.by_material.insert(material, layer).is_none(),
                "duplicate material {material:?}"
            );
            registry.by_layer.push(material);
        }
        registry
    }

    pub fn from_core_set() -> Self {
        Self::build(crate::material_id::INITIAL_ISLAND_LAYERS.iter().copied())
    }

    pub fn layer(&self, material: TerrainMaterialId) -> u32 {
        *self
            .by_material
            .get(&material)
            .expect("material missing from texture array")
    }

    pub fn layer_count(&self) -> u32 {
        self.by_layer.len() as u32
    }

    pub fn material_at_layer(&self, layer: u32) -> Option<TerrainMaterialId> {
        self.by_layer.get(layer as usize).copied()
    }
}
