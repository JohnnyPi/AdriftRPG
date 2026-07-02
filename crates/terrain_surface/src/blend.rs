// crates/terrain_surface/src/blend.rs
use crate::chunk_palette::ChunkSlotRemapper;
use crate::material_id::MaterialKey;
use crate::registry::MaterialLayerRegistry;

#[derive(Clone, Debug)]
pub struct SurfaceMaterialBlend {
    pub materials: [MaterialKey; 4],
    pub weights: [f32; 4],
}

impl SurfaceMaterialBlend {
    pub fn single(material: MaterialKey) -> Self {
        Self {
            materials: [
                material.clone(),
                material.clone(),
                material.clone(),
                material,
            ],
            weights: [1.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn normalize(mut self) -> Self {
        for weight in &mut self.weights {
            *weight = weight.max(0.0);
        }
        let sum: f32 = self.weights.iter().sum();
        if sum <= f32::EPSILON {
            self.weights = [1.0, 0.0, 0.0, 0.0];
        } else {
            for weight in &mut self.weights {
                *weight /= sum;
            }
        }
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MaterialVertex {
    pub local_indices: [u8; 4],
    pub weights: [f32; 4],
}

pub fn resolve_blend(
    blend: SurfaceMaterialBlend,
    layers: &MaterialLayerRegistry,
) -> ([u32; 4], [f32; 4]) {
    let mut indices = [0u32; 4];
    for (out, material) in indices.iter_mut().zip(blend.materials.iter()) {
        *out = layers.layer_or_fallback(material);
    }
    (indices, blend.weights)
}

pub fn remap_blend_to_local_slots(
    global_indices: [u32; 4],
    weights: [f32; 4],
    remapper: &mut ChunkSlotRemapper,
) -> MaterialVertex {
    let mut local_indices = [0u8; 4];
    for (local, &global) in local_indices.iter_mut().zip(global_indices.iter()) {
        *local = remapper.allocate_global(global);
    }
    MaterialVertex {
        local_indices,
        weights,
    }
}

pub fn validate_blend(blend: &SurfaceMaterialBlend) {
    assert!(
        blend.weights.iter().all(|value| value.is_finite() && *value >= 0.0),
        "invalid blend weights"
    );
    let sum: f32 = blend.weights.iter().sum();
    assert!((sum - 1.0).abs() < 0.01, "blend weights sum to {sum}");
}

pub trait SurfaceClassifier: Send + Sync {
    fn classify(&self, context: &crate::context::SurfaceContext) -> SurfaceMaterialBlend;
}

pub trait SurfaceMeshResolver: Send + Sync {
    fn vertex_blend(&self, position: [f32; 3], normal: [f32; 3]) -> MaterialVertex;
    fn chunk_palette(&self) -> crate::chunk_palette::ChunkSlotPalette;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::MaterialLayerRegistry;

    #[test]
    fn local_slot_remap_is_stable_within_chunk() {
        let registry = MaterialLayerRegistry::from_layer_order(&[
            MaterialKey::new("grass"),
            MaterialKey::new("sand"),
            MaterialKey::new("rock"),
        ]);
        let mut remapper = ChunkSlotRemapper::new();
        let (globals, weights) = resolve_blend(
            SurfaceMaterialBlend {
                materials: [
                    MaterialKey::new("sand"),
                    MaterialKey::new("grass"),
                    MaterialKey::new("grass"),
                    MaterialKey::new("grass"),
                ],
                weights: [0.6, 0.4, 0.0, 0.0],
            },
            &registry,
        );
        let v0 = remap_blend_to_local_slots(globals, weights, &mut remapper);
        let v1 = remap_blend_to_local_slots(globals, weights, &mut remapper);
        assert_eq!(v0.local_indices, v1.local_indices);
        let palette = remapper.finish();
        assert_eq!(palette.slot_count(), 2);
    }

    #[test]
    fn ninth_material_merges_into_existing_slot() {
        let mut remapper = ChunkSlotRemapper::new();
        for global in 0..9 {
            remapper.allocate_global(global);
        }
        let palette = remapper.finish();
        assert_eq!(palette.slot_count(), 8);
    }
}
