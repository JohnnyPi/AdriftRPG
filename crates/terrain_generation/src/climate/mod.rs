//! Climate simulation compiler pass.

pub mod trade_winds;

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;
use crate::hydrology::routing::compute_slope;

use trade_winds::compute_climate_fields;

pub struct ClimatePass;

impl WorldgenPass for ClimatePass {
    fn key(&self) -> PassKey {
        PassKey::Climate
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::FinalElevation,
            FieldKey::CoastDistance,
            FieldKey::LandMask,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::Temperature,
            FieldKey::Rainfall,
            FieldKey::Humidity,
            FieldKey::Evaporation,
            FieldKey::WindExposure,
            FieldKey::Slope,
        ]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let recipe = &ctx.recipe.climate;

        let elevation = ctx
            .atlas
            .fields
            .get_scalar(Fk::FinalElevation)
            .expect("final elevation")
            .as_ref()
            .clone();
        let coast = ctx
            .atlas
            .fields
            .get_scalar(Fk::CoastDistance)
            .expect("coast distance")
            .as_ref()
            .clone();
        let land_mask = ctx
            .atlas
            .fields
            .get_scalar(Fk::LandMask)
            .expect("land mask")
            .as_ref()
            .clone();

        let slope = compute_slope(&elevation);
        let fields = compute_climate_fields(&elevation, &coast, &land_mask, &slope, recipe);

        ctx.atlas
            .fields
            .insert_scalar(Fk::Temperature, fields.temperature);
        ctx.atlas
            .fields
            .insert_scalar(Fk::Rainfall, fields.rainfall);
        ctx.atlas
            .fields
            .insert_scalar(Fk::Humidity, fields.humidity);
        ctx.atlas
            .fields
            .insert_scalar(Fk::Evaporation, fields.evaporation);
        ctx.atlas
            .fields
            .insert_scalar(Fk::WindExposure, fields.wind_exposure);
        ctx.atlas.fields.insert_scalar(Fk::Slope, slope);

        Ok(PassReport {
            pass: PassKey::Climate,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics: Default::default(),
            warnings: vec![],
        })
    }
}
