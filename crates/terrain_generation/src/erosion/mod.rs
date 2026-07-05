//! Erosion and sediment compiler pass.

pub mod constraints;
pub mod fluvial;
pub mod sediment;
pub mod thermal;

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;
use crate::fields::scalar::ScalarField;
use crate::hydrology::fill::priority_flood;
use crate::hydrology::routing::compute_flow;

use constraints::reapply_constraints;
use fluvial::apply_stream_power_erosion;
use sediment::transport_and_deposit_sediment;
use thermal::apply_thermal_erosion;

pub struct ErosionPass;

impl WorldgenPass for ErosionPass {
    fn key(&self) -> PassKey {
        PassKey::Erosion
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::FinalElevation,
            FieldKey::FilledElevation,
            FieldKey::FlowDirection,
            FieldKey::FlowAccumulation,
            FieldKey::Runoff,
            FieldKey::LandMask,
            FieldKey::Erodibility,
            FieldKey::RockHardness,
            FieldKey::ValueConstraint,
            FieldKey::Slope,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[FieldKey::ErodedElevation, FieldKey::SedimentThickness]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let recipe = &ctx.recipe.erosion;

        let original = ctx
            .atlas
            .fields
            .get_scalar(Fk::FinalElevation)
            .expect("final elevation")
            .as_ref()
            .clone();
        let land_mask = ctx
            .atlas
            .fields
            .get_scalar(Fk::LandMask)
            .expect("land mask")
            .as_ref()
            .clone();
        let erodibility = ctx
            .atlas
            .fields
            .get_scalar(Fk::Erodibility)
            .expect("erodibility")
            .as_ref()
            .clone();
        let hardness = ctx
            .atlas
            .fields
            .get_scalar(Fk::RockHardness)
            .expect("hardness")
            .as_ref()
            .clone();
        let value_constraint = ctx
            .atlas
            .fields
            .get_scalar(Fk::ValueConstraint)
            .expect("value constraint")
            .as_ref()
            .clone();
        let runoff = ctx
            .atlas
            .fields
            .get_scalar(Fk::Runoff)
            .expect("runoff")
            .as_ref()
            .clone();

        let mut eroded = original.clone();
        let mut sediment = ScalarField::zeros(ctx.atlas.control_descriptor.clone());

        let mut total_eroded = 0.0f64;
        for _cycle in 0..recipe.iterations {
            let filled = priority_flood(&eroded);
            let flow = compute_flow(&filled, &land_mask, &runoff);

            let elev_before = eroded.values.clone();
            apply_stream_power_erosion(
                &mut eroded,
                &flow.accumulation,
                &erodibility,
                &value_constraint,
                &land_mask,
                recipe,
            );
            let slope = ctx
                .atlas
                .fields
                .get_scalar(Fk::Slope)
                .expect("slope")
                .as_ref()
                .clone();
            transport_and_deposit_sediment(
                &mut eroded,
                &mut sediment,
                &flow.accumulation,
                &flow.direction,
                &slope,
                &land_mask,
                recipe,
            );
            apply_thermal_erosion(&mut eroded, &hardness, &land_mask, recipe);
            reapply_constraints(&mut eroded, &original, &value_constraint, &land_mask);

            for (before, after) in elev_before.iter().zip(eroded.values.iter()) {
                total_eroded += (before - after).max(0.0) as f64;
            }
        }

        ctx.atlas.fields.insert_scalar(Fk::ErodedElevation, eroded);
        ctx.atlas
            .fields
            .insert_scalar(Fk::SedimentThickness, sediment);

        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert("total_eroded_volume".into(), total_eroded);

        Ok(PassReport {
            pass: PassKey::Erosion,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics,
            warnings: vec![],
        })
    }
}
