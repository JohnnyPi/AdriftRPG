//! Island skeleton compiler pass.

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;
use crate::islands::footprint::{
    generate_age_field, generate_influence_field, generate_island_id_field,
};
use crate::islands::seed::{IslandBlueprint, IslandSeed};
use crate::islands::skeleton::build_skeleton;
use crate::islands::validation::validate_single_island;

pub struct IslandSkeletonPass;

impl WorldgenPass for IslandSkeletonPass {
    fn key(&self) -> PassKey {
        PassKey::IslandSkeleton
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[FieldKey::BoundaryMask]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::IslandInfluence,
            FieldKey::IslandId,
            FieldKey::IslandAge,
        ]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        if ctx.recipe.islands.len() != 1 {
            return Err(WorldgenError::Validation(
                "Milestone A requires exactly one island".into(),
            ));
        }

        let island_recipe = &ctx.recipe.islands[0];
        let seed = IslandSeed::from_compiled(island_recipe, ctx.recipe.seed);
        let skeleton = build_skeleton(&seed, island_recipe, ctx.recipe.seed);
        let desc = ctx.atlas.control_descriptor.clone();

        let influence = generate_influence_field(desc.clone(), &seed, ctx.recipe.seed);
        let validation = validate_single_island(&influence, &seed);
        if !validation.passed {
            return Err(WorldgenError::Validation(validation.messages.join("; ")));
        }

        let id_field = generate_island_id_field(desc.clone(), &seed);
        let age_field = generate_age_field(desc, &seed);

        ctx.atlas
            .fields
            .insert_scalar(Fk::IslandInfluence, influence);
        ctx.atlas.fields.insert_scalar(Fk::IslandId, id_field);
        ctx.atlas.fields.insert_scalar(Fk::IslandAge, age_field);

        let blueprint = IslandBlueprint {
            seed,
            skeleton: skeleton.clone(),
        };
        ctx.island_seed = Some(blueprint.seed.clone());
        ctx.skeleton = Some(skeleton);
        ctx.blueprints.push(blueprint.clone());
        ctx.atlas.islands.push(blueprint);

        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert("land_cell_count".into(), validation.land_cell_count as f64);
        metrics.insert(
            "connected_components".into(),
            validation.connected_components as f64,
        );

        Ok(PassReport {
            pass: PassKey::IslandSkeleton,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics,
            warnings: vec![],
        })
    }
}
