//! Soil depth field generation.

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;
use crate::fields::scalar::ScalarField;

pub struct SoilPass;

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 >= edge1 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn compute_soil_depth_field(
    elevation: &ScalarField,
    slope: &ScalarField,
    sediment: &ScalarField,
    land_mask: &ScalarField,
    sea_level_m: f32,
    max_island_height_m: f32,
) -> ScalarField {
    let mut soil = ScalarField::zeros(elevation.descriptor.clone());
    let base = 1.8f32;
    let slope_penalty = 1.6f32;
    let sediment_gain = 0.35f32;
    let max_soil = 2.5f32;
    let summit_start = max_island_height_m * 0.65;

    for z in 0..soil.descriptor.height {
        for x in 0..soil.descriptor.width {
            if land_mask.get(x, z) < 0.3 {
                continue;
            }
            let relief = (elevation.get(x, z) - sea_level_m).max(0.0);
            let slope_norm = (slope.get(x, z) / 45.0).clamp(0.0, 1.0);
            let sediment_term = sediment.get(x, z) * sediment_gain;
            let summit_thin = smoothstep(summit_start, summit_start + 50.0, relief);
            let depth = (base - slope_penalty * slope_norm + sediment_term) * (1.0 - summit_thin);
            soil.set(x, z, depth.clamp(0.0, max_soil));
        }
    }
    soil
}

impl WorldgenPass for SoilPass {
    fn key(&self) -> PassKey {
        PassKey::Soil
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::CoastalElevation,
            FieldKey::ErodedElevation,
            FieldKey::Slope,
            FieldKey::SedimentThickness,
            FieldKey::LandMask,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[FieldKey::SoilDepth]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let sea = ctx.recipe.extent.sea_level_m;

        let elevation = ctx
            .atlas
            .fields
            .get_scalar(Fk::CoastalElevation)
            .or_else(|| ctx.atlas.fields.get_scalar(Fk::ErodedElevation))
            .expect("surface elevation")
            .as_ref()
            .clone();
        let slope = ctx
            .atlas
            .fields
            .get_scalar(Fk::Slope)
            .expect("slope")
            .as_ref()
            .clone();
        let sediment = ctx
            .atlas
            .fields
            .get_scalar(Fk::SedimentThickness)
            .expect("sediment")
            .as_ref()
            .clone();
        let land_mask = ctx
            .atlas
            .fields
            .get_scalar(Fk::LandMask)
            .expect("land mask")
            .as_ref()
            .clone();

        let mut max_height = sea;
        for z in 0..elevation.descriptor.height {
            for x in 0..elevation.descriptor.width {
                if land_mask.get(x, z) > 0.3 {
                    max_height = max_height.max(elevation.get(x, z));
                }
            }
        }

        let soil = compute_soil_depth_field(
            &elevation,
            &slope,
            &sediment,
            &land_mask,
            sea,
            max_height - sea,
        );

        let (min_depth, max_depth) = soil.min_max();
        ctx.atlas.fields.insert_scalar(Fk::SoilDepth, soil);

        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert("soil_depth_min".into(), min_depth as f64);
        metrics.insert("soil_depth_max".into(), max_depth as f64);

        Ok(PassReport {
            pass: PassKey::Soil,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics,
            warnings: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::coordinates::WorldXZ;
    use crate::fields::descriptor::FieldDescriptor;
    use crate::fields::key::FieldKey;

    #[test]
    fn soil_is_thicker_on_flats_than_steep_slopes() {
        let desc = FieldDescriptor::new(FieldKey::SoilDepth, WorldXZ::new(-4.0, -4.0), 4.0, 3, 3);
        let mut elevation = ScalarField::zeros(desc.clone());
        let mut slope = ScalarField::zeros(desc.clone());
        let mut sediment = ScalarField::zeros(desc.clone());
        let mut land = ScalarField::zeros(desc.clone());
        for z in 0..3 {
            for x in 0..3 {
                elevation.set(x, z, 20.0);
                sediment.set(x, z, 0.1);
                land.set(x, z, 1.0);
                slope.set(x, z, 5.0);
            }
        }
        slope.set(0, 1, 40.0);
        let soil = compute_soil_depth_field(&elevation, &slope, &sediment, &land, 0.0, 100.0);
        assert!(soil.get(1, 1) > soil.get(0, 1));
    }
}
