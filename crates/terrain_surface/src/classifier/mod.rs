// crates/terrain_surface/src/classifier/mod.rs
mod island;
mod rules;

pub use island::IslandSurfaceClassifier;
pub use rules::{
    RuleSurfaceClassifier, SurfaceBlendEntry, SurfaceClassifierPreset, SurfaceConditions,
    SurfaceGate, SurfaceGateWeights, SurfaceRamp, SurfaceRuleSet, SurfaceWeightedMix,
};
