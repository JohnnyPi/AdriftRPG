// crates/terrain_surface/src/lib.rs
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
    remap_blend_to_local_slots, resolve_blend, validate_blend, MaterialVertex, SurfaceClassifier,
    SurfaceMaterialBlend, SurfaceMeshResolver,
};
pub use chunk_palette::{ChunkSlotPalette, ChunkSlotRemapper, CHUNK_LOCAL_SLOT_COUNT, UNUSED_SLOT};
pub use classifier::{
    IslandSurfaceClassifier, RuleSurfaceClassifier, SurfaceBlendEntry, SurfaceClassifierPreset,
    SurfaceConditions, SurfaceGate, SurfaceGateWeights, SurfaceRamp, SurfaceRuleSet,
    SurfaceWeightedMix,
};
pub use context::{
    compute_soft_biome_weights, saturate, smoothstep, slope_degrees, BiomeId, EnvironmentSample,
    GeologyId, SoftBiomeWeights, SurfaceContext,
};
pub use material_id::MaterialKey;
pub use overlay::{
    compute_overlay_state, overlay_response_for_material_name, OverlayResponseParams,
    SurfaceOverlayState,
};
pub use region_palette::{
    MaterialRegionCoord, MaterialRegionPalette, MaterialRegionPaletteCache, SurfaceCoverage,
    DEFAULT_REGION_CHUNKS, MAX_REGION_SURFACES,
};
pub use registry::MaterialLayerRegistry;
pub use scoring::{
    normalize_scores, ScoreCurve, ScoreCurveKind, ScoreField, SurfaceScoreRule,
};

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
