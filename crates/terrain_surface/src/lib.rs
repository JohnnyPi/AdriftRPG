mod blend;
mod classifier;
mod context;
mod material_id;
mod registry;

pub use blend::{
    resolve_blend, validate_blend, SurfaceClassifier, SurfaceMaterialBlend, SurfaceMeshResolver,
    TerrainMaterialVertex,
};
pub use classifier::IslandSurfaceClassifier;
pub use context::{
    compute_soft_biome_weights, saturate, smoothstep, slope_degrees, BiomeId, EnvironmentSample,
    GeologyId, SoftBiomeWeights, SurfaceContext,
};
pub use material_id::{TerrainMaterialId, CORE_TERRAIN_MATERIALS, INITIAL_ISLAND_LAYERS};
pub use registry::MaterialLayerRegistry;

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
