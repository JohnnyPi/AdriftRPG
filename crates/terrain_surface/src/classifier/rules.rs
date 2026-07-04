// crates/terrain_surface/src/classifier/rules.rs
use std::collections::BTreeMap;

use crate::SurfaceClassifier;
use crate::blend::{MergeSlotPadding, SurfaceMaterialBlend, merge_weighted_blends};
use crate::context::{BiomeId, GeologyId, SurfaceContext, smoothstep};
use crate::material_id::MaterialKey;

#[derive(Clone, Debug)]
pub struct SurfaceBlendEntry {
    pub material: MaterialKey,
    pub weight: f32,
}

#[derive(Clone, Debug, Default)]
pub struct SurfaceRamp {
    pub from: f32,
    pub to: f32,
    pub invert: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SurfaceConditions {
    pub cave_exposure_min: Option<f32>,
    pub water_depth_min: Option<f32>,
    pub coast_distance_max: Option<f32>,
    pub river_distance_max: Option<f32>,
    pub slope_min: Option<f32>,
    pub slope_max: Option<f32>,
    pub elevation_min: Option<f32>,
    pub elevation_max: Option<f32>,
    pub elevation_above_sea_min: Option<f32>,
    pub elevation_above_sea_max: Option<f32>,
    pub moisture_min: Option<f32>,
    pub moisture_max: Option<f32>,
    pub geology: Option<String>,
    pub biome: Option<String>,
    pub soft_grassland_min: Option<f32>,
    pub soft_forest_min: Option<f32>,
    pub soft_wetland_min: Option<f32>,
    pub soft_alpine_min: Option<f32>,
    pub fallback: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SurfaceGateWeights {
    pub coast_distance: Option<SurfaceRamp>,
    pub river_distance: Option<SurfaceRamp>,
    pub slope: Option<SurfaceRamp>,
    pub elevation_above_sea: Option<SurfaceRamp>,
    pub moisture: Option<SurfaceRamp>,
    pub cave_exposure: Option<SurfaceRamp>,
    pub wave_exposure: Option<SurfaceRamp>,
    pub soft_alpine: Option<SurfaceRamp>,
    pub soft_wetland: Option<SurfaceRamp>,
    pub constant: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct SurfaceGate {
    pub id: String,
    pub when: SurfaceConditions,
    pub gate_weight: SurfaceGateWeights,
    pub exclusive: bool,
    pub blend: Vec<SurfaceBlendEntry>,
    pub classifier: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SurfaceClassifierPreset {
    pub id: String,
    pub blend: Vec<SurfaceBlendEntry>,
    pub weighted_mix: Vec<SurfaceWeightedMix>,
}

#[derive(Clone, Debug)]
pub struct SurfaceWeightedMix {
    pub classifier: String,
    pub weight: f32,
}

#[derive(Clone, Debug, Default)]
pub struct SurfaceRuleSet {
    pub gates: Vec<SurfaceGate>,
    pub classifiers: BTreeMap<String, SurfaceClassifierPreset>,
}

pub struct RuleSurfaceClassifier {
    rules: SurfaceRuleSet,
    default_material: MaterialKey,
}

impl RuleSurfaceClassifier {
    pub fn new(rules: SurfaceRuleSet, default_material: MaterialKey) -> Self {
        Self {
            rules,
            default_material,
        }
    }
}

impl SurfaceClassifier for RuleSurfaceClassifier {
    fn classify(&self, context: &SurfaceContext) -> SurfaceMaterialBlend {
        let mut contributions: Vec<(f32, SurfaceMaterialBlend)> = Vec::new();
        let mut fallback_gates: Vec<&SurfaceGate> = Vec::new();

        for gate in &self.rules.gates {
            if !conditions_match(&gate.when, context) {
                continue;
            }
            if gate.when.fallback {
                fallback_gates.push(gate);
                continue;
            }
            if let Some(blend) = self.evaluate_gate(gate, context) {
                if gate.exclusive {
                    return blend.normalize();
                }
                let gate_w = evaluate_gate_weight(&gate.gate_weight, context);
                if gate_w > f32::EPSILON {
                    contributions.push((gate_w, blend));
                }
            }
        }

        let applied_weight: f32 = contributions.iter().map(|(w, _)| *w).sum();
        let fallback_weight = (1.0 - applied_weight.min(1.0)).max(0.0);
        if fallback_weight > f32::EPSILON {
            for gate in fallback_gates {
                if let Some(blend) = self.evaluate_gate(gate, context) {
                    contributions.push((fallback_weight, blend));
                    break;
                }
            }
        }

        if contributions.is_empty() {
            return SurfaceMaterialBlend::single(self.default_material.clone());
        }

        merge_weighted_blends(
            &contributions,
            MergeSlotPadding::TopRanked {
                fallback: self.default_material.clone(),
            },
        )
        .normalize()
    }
}

impl RuleSurfaceClassifier {
    fn evaluate_gate(
        &self,
        gate: &SurfaceGate,
        context: &SurfaceContext,
    ) -> Option<SurfaceMaterialBlend> {
        if let Some(ref classifier_id) = gate.classifier {
            Some(self.resolve_classifier(classifier_id, context))
        } else if !gate.blend.is_empty() {
            Some(blend_from_entries(&gate.blend, &self.default_material))
        } else {
            None
        }
    }

    fn resolve_classifier(&self, id: &str, context: &SurfaceContext) -> SurfaceMaterialBlend {
        let Some(preset) = self.rules.classifiers.get(id) else {
            return SurfaceMaterialBlend::single(self.default_material.clone());
        };
        if !preset.weighted_mix.is_empty() {
            let parts: Vec<(f32, SurfaceMaterialBlend)> = preset
                .weighted_mix
                .iter()
                .map(|mix| {
                    let blend = self.resolve_classifier(&mix.classifier, context);
                    (mix.weight, blend)
                })
                .collect();
            return merge_weighted_blends(
                &parts,
                MergeSlotPadding::TopRanked {
                    fallback: self.default_material.clone(),
                },
            );
        }
        blend_from_entries(&preset.blend, &self.default_material)
    }
}

fn blend_from_entries(
    entries: &[SurfaceBlendEntry],
    default: &MaterialKey,
) -> SurfaceMaterialBlend {
    if entries.is_empty() {
        return SurfaceMaterialBlend::single(default.clone());
    }
    let mut materials = [
        default.clone(),
        default.clone(),
        default.clone(),
        default.clone(),
    ];
    let mut weights = [0.0f32; 4];
    for (i, entry) in entries.iter().take(4).enumerate() {
        materials[i] = entry.material.clone();
        weights[i] = entry.weight.max(0.0);
    }
    SurfaceMaterialBlend { materials, weights }.normalize()
}

fn conditions_match(when: &SurfaceConditions, c: &SurfaceContext) -> bool {
    if when.fallback {
        return true;
    }
    if let Some(min) = when.cave_exposure_min {
        if c.cave_exposure < min {
            return false;
        }
    }
    if let Some(min) = when.water_depth_min {
        if c.water_depth_m < min {
            return false;
        }
    }
    if let Some(max) = when.coast_distance_max {
        if c.coast_distance_m > max {
            return false;
        }
    }
    if let Some(max) = when.river_distance_max {
        if c.river_distance_m > max {
            return false;
        }
    }
    if let Some(min) = when.slope_min {
        if c.slope_degrees < min {
            return false;
        }
    }
    if let Some(max) = when.slope_max {
        if c.slope_degrees > max {
            return false;
        }
    }
    if let Some(min) = when.elevation_min {
        if c.elevation_m < min {
            return false;
        }
    }
    if let Some(max) = when.elevation_max {
        if c.elevation_m > max {
            return false;
        }
    }
    if let Some(min) = when.elevation_above_sea_min {
        if c.elevation_m - c.sea_level_m < min {
            return false;
        }
    }
    if let Some(max) = when.elevation_above_sea_max {
        if c.elevation_m - c.sea_level_m > max {
            return false;
        }
    }
    if let Some(min) = when.moisture_min {
        if c.moisture < min {
            return false;
        }
    }
    if let Some(max) = when.moisture_max {
        if c.moisture > max {
            return false;
        }
    }
    if let Some(ref geology) = when.geology {
        let matches = match geology.as_str() {
            "Limestone" | "limestone" => c.geology == GeologyId::Limestone,
            "Basalt" | "basalt" => c.geology == GeologyId::Basalt,
            _ => false,
        };
        if !matches {
            return false;
        }
    }
    if let Some(ref biome) = when.biome {
        if !biome_matches(biome, c.biome) {
            return false;
        }
    }
    if let Some(min) = when.soft_grassland_min {
        if c.soft.grassland < min {
            return false;
        }
    }
    if let Some(min) = when.soft_forest_min {
        if c.soft.forest < min {
            return false;
        }
    }
    if let Some(min) = when.soft_wetland_min {
        if c.soft.wetland < min {
            return false;
        }
    }
    if let Some(min) = when.soft_alpine_min {
        if c.soft.alpine < min {
            return false;
        }
    }
    true
}

fn biome_matches(name: &str, biome: BiomeId) -> bool {
    BiomeId::from_rule_id(name) == biome
}

fn evaluate_gate_weight(weights: &SurfaceGateWeights, c: &SurfaceContext) -> f32 {
    let mut product = weights.constant.unwrap_or(1.0);
    if let Some(ref ramp) = weights.coast_distance {
        product *= eval_ramp(c.coast_distance_m, ramp);
    }
    if let Some(ref ramp) = weights.river_distance {
        product *= eval_ramp(c.river_distance_m, ramp);
    }
    if let Some(ref ramp) = weights.slope {
        product *= eval_ramp(c.slope_degrees, ramp);
    }
    if let Some(ref ramp) = weights.elevation_above_sea {
        product *= eval_ramp(c.elevation_m - c.sea_level_m, ramp);
    }
    if let Some(ref ramp) = weights.moisture {
        product *= eval_ramp(c.moisture, ramp);
    }
    if let Some(ref ramp) = weights.cave_exposure {
        product *= eval_ramp(c.cave_exposure, ramp);
    }
    if let Some(ref ramp) = weights.wave_exposure {
        product *= eval_ramp(c.wave_exposure, ramp);
    }
    if let Some(ref ramp) = weights.soft_alpine {
        product *= eval_ramp(c.soft.alpine, ramp);
    }
    if let Some(ref ramp) = weights.soft_wetland {
        product *= eval_ramp(c.soft.wetland, ramp);
    }
    product.max(0.0)
}

fn eval_ramp(value: f32, ramp: &SurfaceRamp) -> f32 {
    let t = smoothstep(ramp.from, ramp.to, value);
    if ramp.invert { 1.0 - t } else { t }
}

impl SurfaceRuleSet {
    /// Compile YAML surface rules into the runtime classifier model.
    pub fn from_compiled(rules: &game_data::CompiledSurfaceRules) -> Self {
        use game_data::{
            SurfaceBlendEntryDefinition, SurfaceClassifierDefinition, SurfaceConditionsDefinition,
            SurfaceGateDefinition, SurfaceGateWeightDefinition, SurfaceRampDefinition,
        };

        fn map_ramp(ramp: &SurfaceRampDefinition) -> SurfaceRamp {
            SurfaceRamp {
                from: ramp.from,
                to: ramp.to,
                invert: ramp.invert,
            }
        }

        fn map_conditions(when: &SurfaceConditionsDefinition) -> SurfaceConditions {
            SurfaceConditions {
                cave_exposure_min: when.cave_exposure_min,
                water_depth_min: when.water_depth_min,
                coast_distance_max: when.coast_distance_max,
                river_distance_max: when.river_distance_max,
                slope_min: when.slope_min,
                slope_max: when.slope_max,
                elevation_min: when.elevation_min,
                elevation_max: when.elevation_max,
                elevation_above_sea_min: when.elevation_above_sea_min,
                elevation_above_sea_max: when.elevation_above_sea_max,
                moisture_min: when.moisture_min,
                moisture_max: when.moisture_max,
                geology: when.geology.clone(),
                biome: when.biome.clone(),
                soft_grassland_min: when.soft_grassland_min,
                soft_forest_min: when.soft_forest_min,
                soft_wetland_min: when.soft_wetland_min,
                soft_alpine_min: when.soft_alpine_min,
                fallback: when.fallback,
            }
        }

        fn map_gate_weights(weights: &SurfaceGateWeightDefinition) -> SurfaceGateWeights {
            SurfaceGateWeights {
                coast_distance: weights.coast_distance.as_ref().map(map_ramp),
                river_distance: weights.river_distance.as_ref().map(map_ramp),
                slope: weights.slope.as_ref().map(map_ramp),
                elevation_above_sea: weights.elevation_above_sea.as_ref().map(map_ramp),
                moisture: weights.moisture.as_ref().map(map_ramp),
                cave_exposure: weights.cave_exposure.as_ref().map(map_ramp),
                wave_exposure: weights.wave_exposure.as_ref().map(map_ramp),
                soft_alpine: weights.soft_alpine.as_ref().map(map_ramp),
                soft_wetland: weights.soft_wetland.as_ref().map(map_ramp),
                constant: weights.constant,
            }
        }

        fn map_blend(entries: &[SurfaceBlendEntryDefinition]) -> Vec<SurfaceBlendEntry> {
            entries
                .iter()
                .map(|entry| SurfaceBlendEntry {
                    material: MaterialKey::new(entry.material.as_str()),
                    weight: entry.weight,
                })
                .collect()
        }

        fn map_classifier(def: &SurfaceClassifierDefinition) -> SurfaceClassifierPreset {
            SurfaceClassifierPreset {
                id: def.id.clone(),
                blend: map_blend(&def.blend),
                weighted_mix: def
                    .weighted_mix
                    .iter()
                    .map(|mix| SurfaceWeightedMix {
                        classifier: mix.classifier.clone(),
                        weight: mix.weight,
                    })
                    .collect(),
            }
        }

        let gates = rules
            .gates
            .iter()
            .map(|gate: &SurfaceGateDefinition| SurfaceGate {
                id: gate.id.clone(),
                when: map_conditions(&gate.when),
                gate_weight: map_gate_weights(&gate.gate_weight),
                exclusive: gate.exclusive,
                blend: map_blend(&gate.blend),
                classifier: gate.classifier.clone(),
            })
            .collect();

        let classifiers = rules
            .classifiers
            .iter()
            .map(map_classifier)
            .map(|preset| (preset.id.clone(), preset))
            .collect();

        Self { gates, classifiers }
    }
}
