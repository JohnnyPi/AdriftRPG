// crates/terrain_surface/src/scoring.rs
//! Weighted scoring curves for surface classification.

use crate::context::{smoothstep, SurfaceContext};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScoreCurveKind {
    Linear,
    SmoothRamp,
    InverseSmoothRamp,
    SmoothBand,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScoreCurve {
    pub kind: ScoreCurveKind,
    pub start: f32,
    pub end: f32,
    pub weight: f32,
    pub band_exit: Option<f32>,
}

impl ScoreCurve {
    pub fn evaluate(&self, value: f32) -> f32 {
        let factor = match self.kind {
            ScoreCurveKind::Linear => value.clamp(0.0, 1.0),
            ScoreCurveKind::SmoothRamp => smoothstep(self.start, self.end, value),
            ScoreCurveKind::InverseSmoothRamp => {
                1.0 - smoothstep(self.start, self.end, value)
            }
            ScoreCurveKind::SmoothBand => {
                let enter = smoothstep(self.start, self.end, value);
                let exit_end = self.band_exit.unwrap_or(self.end + 10.0);
                let exit = 1.0 - smoothstep(self.end, exit_end, value);
                (enter * exit).clamp(0.0, 1.0)
            }
        };
        factor * self.weight
    }
}

#[derive(Clone, Debug, Default)]
pub struct SurfaceScoreRule {
    pub base: f32,
    pub curves: Vec<(ScoreField, ScoreCurve)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScoreField {
    SlopeDegrees,
    Moisture,
    SoilDepth,
    CliffMask,
    BeachMask,
    WetlandMask,
    CoastDistance,
    Exposure,
    Sediment,
    Elevation,
}

impl ScoreField {
    pub fn sample(&self, context: &SurfaceContext) -> f32 {
        match self {
            Self::SlopeDegrees => context.slope_degrees,
            Self::Moisture => context.moisture,
            Self::SoilDepth => context.soil_depth_m,
            Self::CliffMask => smoothstep(25.0, 45.0, context.slope_degrees),
            Self::BeachMask => context.soft.beach,
            Self::WetlandMask => context.soft.wetland,
            Self::CoastDistance => context.coast_distance_m,
            Self::Exposure => context.wave_exposure,
            Self::Sediment => context.mineral_deposition,
            Self::Elevation => context.elevation_m,
        }
    }
}

impl SurfaceScoreRule {
    pub fn score(&self, context: &SurfaceContext) -> f32 {
        let mut total = self.base.max(0.0);
        for (field, curve) in &self.curves {
            total += curve.evaluate(field.sample(context));
        }
        total.max(0.0)
    }
}

pub fn normalize_scores(scores: &mut [(crate::material_id::MaterialKey, f32)]) {
    let sum: f32 = scores.iter().map(|(_, s)| s.max(0.0)).sum();
    if sum <= f32::EPSILON {
        return;
    }
    for (_, score) in scores.iter_mut() {
        *score = score.max(0.0) / sum;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{BiomeId, GeologyId, SoftBiomeWeights, SurfaceContext};
    use crate::material_id::MaterialKey;

    fn test_context(slope: f32, moisture: f32) -> SurfaceContext {
        SurfaceContext {
            world_position: [0.0, 0.0, 0.0],
            world_normal: [0.0, 1.0, 0.0],
            elevation_m: 10.0,
            sea_level_m: 0.0,
            water_depth_m: 0.0,
            slope_degrees: slope,
            moisture,
            soil_depth_m: 0.8,
            coast_distance_m: 100.0,
            river_distance_m: 80.0,
            wave_exposure: 0.5,
            cave_exposure: 0.0,
            mineral_deposition: 0.2,
            biome: BiomeId::Grassland,
            geology: GeologyId::Basalt,
            soft: SoftBiomeWeights::default(),
        }
    }

    #[test]
    fn smooth_ramp_increases_with_slope() {
        let rule = SurfaceScoreRule {
            base: 0.1,
            curves: vec![(
                ScoreField::SlopeDegrees,
                ScoreCurve {
                    kind: ScoreCurveKind::SmoothRamp,
                    start: 20.0,
                    end: 50.0,
                    weight: 1.0,
                    band_exit: None,
                },
            )],
        };
        let low = rule.score(&test_context(10.0, 0.5));
        let high = rule.score(&test_context(40.0, 0.5));
        assert!(high > low);
    }

    #[test]
    fn normalize_scores_sums_to_one() {
        let mut scores = vec![
            (MaterialKey::new("a"), 2.0),
            (MaterialKey::new("b"), 1.0),
        ];
        normalize_scores(&mut scores);
        let sum: f32 = scores.iter().map(|(_, s)| *s).sum();
        assert!((sum - 1.0).abs() < 0.001);
    }
}
