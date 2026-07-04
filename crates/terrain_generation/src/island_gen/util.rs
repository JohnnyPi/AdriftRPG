//! Shared island-gen helpers.

use crate::noise::ValueNoise;

/// Deterministic unit float in `[0, 1)` from world XZ for coast/bathymetry variation.
pub fn seeded_unit(noise: &ValueNoise, wx: f32, wz: f32) -> f32 {
    noise.sample(wx * 0.0025, 0.0, wz * 0.0025)
}
