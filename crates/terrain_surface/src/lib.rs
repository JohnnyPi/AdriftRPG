// crates/terrain_surface/src/lib.rs
//! Surface classification, material blending, and chunk palette resolution.
//!
//! # Material palette hierarchy
//!
//! Terrain materials flow through four representations (outer → inner):
//!
//! 1. **World catalog** — [`MaterialLayerRegistry`] maps stable [`MaterialKey`] names
//!    (from YAML `terrain_materials`) to global texture-array layer indices.
//! 2. **Region palette** — [`MaterialRegionPalette`] picks up to eight dominant
//!    [`MaterialKey`] surfaces across a [`MaterialRegionCoord`] (default 4×4 chunks),
//!    cached in [`MaterialRegionPaletteCache`].
//! 3. **Chunk slot palette** — [`ChunkSlotRemapper`] / [`ChunkSlotPalette`] compress
//!    the globals used by one chunk into eight local slots (`0..7`) for the shader.
//! 4. **Vertex indices** — [`MaterialVertex::local_indices`] + weight vectors uploaded
//!    per mesh corner; [`remap_blend_to_local_slots`] performs the final mapping.
//!
//! [`RuleSurfaceClassifier`] produces a four-material [`SurfaceMaterialBlend`] in
//! material-key space; [`resolve_blend`] converts keys to global layers before remapping.
mod biome_bridge;
mod blend;
mod chunk_palette;
mod classifier;
mod context;
mod material_id;
mod overlay;
mod region_palette;
mod registry;
mod scoring;

pub use blend::{
    MaterialVertex, SurfaceClassifier, SurfaceMaterialBlend, SurfaceMeshResolver,
    remap_blend_to_local_slots, resolve_blend, validate_blend,
};
pub use chunk_palette::{CHUNK_LOCAL_SLOT_COUNT, ChunkSlotPalette, ChunkSlotRemapper, UNUSED_SLOT};
#[cfg(any(test, feature = "test-oracle"))]
pub use classifier::IslandSurfaceClassifier;
pub use classifier::{
    RuleSurfaceClassifier, SurfaceBlendEntry, SurfaceClassifierPreset, SurfaceConditions,
    SurfaceGate, SurfaceGateWeights, SurfaceRamp, SurfaceRuleSet, SurfaceWeightedMix,
};
pub use biome_bridge::{
    compiler_biome_to_presentation, merge_soft_biome_sources, soft_weights_from_blend_cell,
    soft_weights_from_primary_u8,
};
pub use context::{
    BiomeId, EnvironmentSample, GeologyId, SoftBiomeWeights, SurfaceContext,
    compute_soft_biome_weights, default_wetness_normalization, merge_soft_with_atlas,
    normalize_wetness, saturate, slope_degrees, smoothstep,
};
pub use material_id::MaterialKey;
pub use overlay::{
    OverlayResponseParams, SurfaceOverlayState, compute_overlay_state,
    overlay_response_for_material_name,
};
pub use region_palette::{
    DEFAULT_REGION_CHUNKS, MAX_REGION_SURFACES, MaterialRegionCoord, MaterialRegionPalette,
    MaterialRegionPaletteCache, SurfaceCoverage,
};
pub use registry::MaterialLayerRegistry;
#[doc(hidden)]
pub use scoring::{ScoreCurve, ScoreCurveKind, ScoreField, SurfaceScoreRule, normalize_scores};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_weights_sum_to_one() {
        for moisture in (0..20).map(|i| i as f32 / 19.0) {
            let sample = EnvironmentSample {
                elevation: 10.0,
                slope_degrees: 8.0,
                moisture,
                effective_moisture: moisture,
                transition_noise: 0.5,
                temperature: 0.5,
                distance_to_water: 50.0,
                distance_to_river: 100.0,
                cave_depth: 0.0,
                world_y: 10.0,
            };
            let w = compute_soft_biome_weights(&sample);
            let sum = w.grassland
                + w.forest
                + w.scrub
                + w.coastal_scrub
                + w.wetland
                + w.beach
                + w.alpine
                + w.rocky;
            assert!((sum - 1.0).abs() < 0.01, "moisture={moisture} sum={sum}");
        }
    }
}
