// crates/terrain_generation/src/island_gen/biome_field.rs
//! Biome suitability weights (VS3 §15).

use crate::field2d::{Field2D, smoothstep};
use crate::island_atlas::BiomeWeights;
use crate::island_gen::params::IslandGenParams;
use shared::range_weight;

pub fn compute_biome_weights(
    elevation: &Field2D<f32>,
    slope: &Field2D<f32>,
    wetness: &Field2D<f32>,
    beach_mask: &Field2D<f32>,
    island_mask: &Field2D<f32>,
    params: &IslandGenParams,
) -> Field2D<BiomeWeights> {
    let mut biomes = Field2D::<BiomeWeights>::new(
        elevation.width,
        elevation.height,
        elevation.origin,
        elevation.spacing,
    );
    let sea = params.island.sea_level_m;

    for z in 0..elevation.height {
        for x in 0..elevation.width {
            if island_mask.get(x, z) < 0.2 {
                continue;
            }
            let elev = elevation.get(x, z) - sea;
            let sl = slope.get(x, z);
            let wet = wetness.get(x, z);
            let beach = beach_mask.get(x, z);

            let rainforest = range_weight(wet, 0.5, 1.0, 0.15)
                * range_weight(elev, 8.0, 45.0, 8.0)
                * range_weight(sl, 0.0, 38.0, 10.0);
            let grassland = range_weight(elev, 3.0, 40.0, 10.0)
                * range_weight(sl, 0.0, 30.0, 8.0)
                * (1.0 - wet * 0.4);
            let volcanic_rock = smoothstep(30.0, 55.0, sl).max(range_weight(elev, 28.0, 55.0, 8.0));
            let wetland = range_weight(wet, 0.7, 1.0, 0.1) * range_weight(sl, 0.0, 12.0, 5.0);

            biomes.set(
                x,
                z,
                BiomeWeights {
                    rainforest,
                    grassland,
                    volcanic_rock,
                    beach: beach.max(0.0),
                    wetland,
                },
            );
        }
    }
    biomes
}
