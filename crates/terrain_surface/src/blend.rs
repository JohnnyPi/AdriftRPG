// crates/terrain_surface/src/blend.rs
use std::collections::BTreeMap;

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

/// How unused blend slots are filled after ranking merged materials by weight.
#[derive(Clone, Debug)]
#[allow(dead_code)] // `Fixed` is used by the test-oracle island classifier only.
pub enum MergeSlotPadding {
    /// Pad empty slots with a fixed material.
    Fixed(MaterialKey),
    /// Pad empty slots with the top-ranked material (`fallback` when nothing merged).
    TopRanked { fallback: MaterialKey },
}

/// Accumulate gated four-slot blends into a single blend ranked by total weight.
///
/// Does not normalize; callers invoke [`SurfaceMaterialBlend::normalize`] when needed.
pub fn merge_weighted_blends(
    parts: &[(f32, SurfaceMaterialBlend)],
    padding: MergeSlotPadding,
) -> SurfaceMaterialBlend {
    let mut weights: BTreeMap<MaterialKey, f32> = BTreeMap::new();
    for (gate, blend) in parts {
        if *gate <= f32::EPSILON {
            continue;
        }
        for i in 0..4 {
            let w = blend.weights[i] * gate;
            if w > 0.0 {
                *weights.entry(blend.materials[i].clone()).or_default() += w;
            }
        }
    }
    let mut ranked: Vec<(MaterialKey, f32)> = weights.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let slot_default = match padding {
        MergeSlotPadding::Fixed(key) => key,
        MergeSlotPadding::TopRanked { fallback } => ranked
            .first()
            .map(|(key, _)| key.clone())
            .unwrap_or(fallback),
    };

    let mut materials = [
        slot_default.clone(),
        slot_default.clone(),
        slot_default.clone(),
        slot_default,
    ];
    let mut w = [0.0; 4];
    for (i, (mat, wt)) in ranked.into_iter().take(4).enumerate() {
        materials[i] = mat;
        w[i] = wt;
    }
    SurfaceMaterialBlend {
        materials,
        weights: w,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MaterialVertex {
    pub local_indices: [u8; 4],
    pub weights: [f32; 4],
    /// Second weight vector for eight-layer regional palettes.
    pub weights_1: [f32; 4],
    /// Biome color multiplier (defaults to white).
    pub tint: [f32; 3],
    /// Dynamic overlay channels packed for the terrain shader.
    pub overlay: [f32; 2],
}

impl Default for MaterialVertex {
    fn default() -> Self {
        Self {
            local_indices: [0; 4],
            weights: [1.0, 0.0, 0.0, 0.0],
            weights_1: [0.0; 4],
            tint: [1.0, 1.0, 1.0],
            overlay: [0.0, 0.0],
        }
    }
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
        weights_1: [0.0; 4],
        tint: [1.0, 1.0, 1.0],
        overlay: [0.0, 0.0],
    }
}

pub fn validate_blend(blend: &SurfaceMaterialBlend) {
    assert!(
        blend
            .weights
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0),
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

    #[test]
    fn merge_weighted_blends_ranks_by_accumulated_gate_weight() {
        let grass = MaterialKey::new("grass");
        let sand = MaterialKey::new("sand");
        let rock = MaterialKey::new("rock");
        let merged = merge_weighted_blends(
            &[
                (
                    0.6,
                    SurfaceMaterialBlend {
                        materials: [sand.clone(), grass.clone(), grass.clone(), grass.clone()],
                        weights: [1.0, 0.0, 0.0, 0.0],
                    },
                ),
                (
                    0.2,
                    SurfaceMaterialBlend {
                        materials: [rock.clone(), grass.clone(), grass.clone(), grass.clone()],
                        weights: [1.0, 0.0, 0.0, 0.0],
                    },
                ),
            ],
            MergeSlotPadding::Fixed(grass.clone()),
        )
        .normalize();
        assert_eq!(merged.materials[0], sand);
        assert_eq!(merged.materials[1], rock);
        assert!((merged.weights[0] - 0.75).abs() < 0.01);
        assert!((merged.weights[1] - 0.25).abs() < 0.01);
    }

    #[test]
    fn merge_top_ranked_padding_uses_highest_weight_material() {
        let grass = MaterialKey::new("grass");
        let sand = MaterialKey::new("sand");
        let merged = merge_weighted_blends(
            &[(
                1.0,
                SurfaceMaterialBlend {
                    materials: [sand.clone(), grass.clone(), grass.clone(), grass.clone()],
                    weights: [0.8, 0.2, 0.0, 0.0],
                },
            )],
            MergeSlotPadding::TopRanked {
                fallback: grass.clone(),
            },
        );
        assert_eq!(merged.materials[2], sand);
        assert_eq!(merged.weights[2], 0.0);
    }
}
