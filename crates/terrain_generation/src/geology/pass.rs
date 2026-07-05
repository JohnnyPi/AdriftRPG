//! Geological construction pass.

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::fields::key::FieldKey;
use crate::geology::fields::generate_geology_fields;

pub struct GeologyPass;

impl WorldgenPass for GeologyPass {
    fn key(&self) -> PassKey {
        PassKey::Geology
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::IslandInfluence,
            FieldKey::IslandAge,
            FieldKey::BaseElevation,
            FieldKey::CoastDistance,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::Bedrock,
            FieldKey::RockHardness,
            FieldKey::Erodibility,
            FieldKey::Permeability,
            FieldKey::FractureIntensity,
            FieldKey::ValueConstraint,
            FieldKey::GradientConstraint,
        ]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let skeleton = ctx
            .skeleton
            .as_ref()
            .ok_or(WorldgenError::MissingPrerequisite {
                pass: PassKey::Geology,
                missing: "island skeleton",
            })?;

        generate_geology_fields(
            &mut ctx.atlas,
            &ctx.recipe.geology,
            skeleton,
            ctx.recipe.seed,
        );

        Ok(PassReport {
            pass: PassKey::Geology,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics: Default::default(),
            warnings: vec![],
        })
    }
}
