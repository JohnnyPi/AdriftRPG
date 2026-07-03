// crates/game_data/src/material_overrides.rs
//! Override precedence for the procedural texture pipeline.

use serde::{Deserialize, Serialize};

/// Layered override sources; higher numeric value wins for scalar conflicts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum OverrideLayer {
    BaseCatalog = 0,
    WorldBinding = 1,
    UserPrefs = 2,
    RuntimeTweaks = 3,
    RecipeOverride = 4,
    PaintEdit = 5,
    PersistenceDelta = 6,
}

impl OverrideLayer {
    pub const COUNT: usize = 7;

    pub fn all() -> [Self; Self::COUNT] {
        [
            Self::BaseCatalog,
            Self::WorldBinding,
            Self::UserPrefs,
            Self::RuntimeTweaks,
            Self::RecipeOverride,
            Self::PaintEdit,
            Self::PersistenceDelta,
        ]
    }
}

/// Merge a scalar parameter: highest layer with `Some` value wins.
#[derive(Clone, Debug, Default)]
pub struct LayeredScalar {
    values: [Option<f32>; OverrideLayer::COUNT],
}

impl LayeredScalar {
    pub fn set(&mut self, layer: OverrideLayer, value: f32) {
        self.values[layer as usize] = Some(value);
    }

    pub fn set_opt(&mut self, layer: OverrideLayer, value: Option<f32>) {
        if let Some(v) = value {
            self.set(layer, v);
        }
    }

    pub fn resolve(&self, fallback: f32) -> f32 {
        for layer in OverrideLayer::all().into_iter().rev() {
            if let Some(v) = self.values[layer as usize] {
                return v;
            }
        }
        fallback
    }

    pub fn winning_layer(&self) -> Option<OverrideLayer> {
        for layer in OverrideLayer::all().into_iter().rev() {
            if self.values[layer as usize].is_some() {
                return Some(layer);
            }
        }
        None
    }
}

/// Tracks which invalidation path a config change should trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialInvalidation {
    RebakeTextures,
    UpdateUniforms,
    ReclassifySurfaces,
    RemeshChunks,
    RecomputeFields,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn higher_layer_wins_scalar_merge() {
        let mut scalar = LayeredScalar::default();
        scalar.set(OverrideLayer::BaseCatalog, 1.0);
        scalar.set(OverrideLayer::RuntimeTweaks, 2.5);
        assert!((scalar.resolve(0.0) - 2.5).abs() < f32::EPSILON);
    }

    #[test]
    fn paint_beats_recipe_override() {
        let mut scalar = LayeredScalar::default();
        scalar.set(OverrideLayer::RecipeOverride, 3.0);
        scalar.set(OverrideLayer::PaintEdit, 4.0);
        assert!((scalar.resolve(0.0) - 4.0).abs() < f32::EPSILON);
    }
}
