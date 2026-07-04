// crates/terrain_surface/src/classifier/mod.rs
#[cfg(any(test, feature = "test-oracle"))]
mod island;
mod rules;

#[cfg(any(test, feature = "test-oracle"))]
pub use island::IslandSurfaceClassifier;
pub use rules::{
    RuleSurfaceClassifier, SurfaceBlendEntry, SurfaceClassifierPreset, SurfaceConditions,
    SurfaceGate, SurfaceGateWeights, SurfaceRamp, SurfaceRuleSet, SurfaceWeightedMix,
};
