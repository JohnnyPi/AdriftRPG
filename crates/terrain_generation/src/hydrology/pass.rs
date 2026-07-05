//! Hydrology compiler passes.

use game_data::CompiledHydrologyRecipe;

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;
use crate::fields::scalar::ScalarField;

use super::features::{
    build_hydrology_graph, compute_lake_mask, compute_wetland_mask, trace_primary_river,
};
use super::fill::priority_flood;
use super::routing::{compute_flow, compute_slope, extract_river_mask};

pub struct HydrologyPass;

impl WorldgenPass for HydrologyPass {
    fn key(&self) -> PassKey {
        PassKey::Hydrology
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::FinalElevation,
            FieldKey::LandMask,
            FieldKey::Rainfall,
            FieldKey::Humidity,
            FieldKey::Permeability,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::FilledElevation,
            FieldKey::FlowDirection,
            FieldKey::FlowAccumulation,
            FieldKey::Runoff,
            FieldKey::RiverMask,
            FieldKey::LakeMask,
            FieldKey::WetlandMask,
            FieldKey::Slope,
        ]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let recipe = &ctx.recipe.hydrology;
        let sea = ctx.recipe.extent.sea_level_m;

        let elevation = ctx
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
        let rainfall = ctx
            .atlas
            .fields
            .get_scalar(Fk::Rainfall)
            .expect("rainfall")
            .as_ref()
            .clone();
        let humidity = ctx
            .atlas
            .fields
            .get_scalar(Fk::Humidity)
            .expect("humidity")
            .as_ref()
            .clone();
        let permeability = ctx
            .atlas
            .fields
            .get_scalar(Fk::Permeability)
            .expect("permeability")
            .as_ref()
            .clone();

        let slope = compute_slope(&elevation);
        let runoff = compute_runoff(&rainfall, &permeability, &land_mask, recipe);
        let filled = priority_flood(&elevation);
        let flow = compute_flow(&filled, &land_mask, &runoff);
        let river_mask = extract_river_mask(
            &flow.accumulation,
            &land_mask,
            recipe.stream_threshold,
            recipe.permanent_river_threshold,
        );

        let graph = build_hydrology_graph(
            &filled,
            &flow.accumulation,
            &flow.direction,
            &land_mask,
            &humidity,
            &slope,
            recipe,
            sea,
        );
        let lake_mask = compute_lake_mask(&graph, ctx.atlas.control_descriptor.clone());
        let wetland_mask = compute_wetland_mask(&graph, ctx.atlas.control_descriptor.clone());

        ctx.atlas.fields.insert_scalar(Fk::FilledElevation, filled);
        ctx.atlas
            .fields
            .insert_categorical(Fk::FlowDirection, flow.direction);
        ctx.atlas
            .fields
            .insert_scalar(Fk::FlowAccumulation, flow.accumulation);
        ctx.atlas.fields.insert_scalar(Fk::Runoff, runoff);
        ctx.atlas.fields.insert_scalar(Fk::RiverMask, river_mask);
        ctx.atlas.fields.insert_scalar(Fk::LakeMask, lake_mask);
        ctx.atlas
            .fields
            .insert_scalar(Fk::WetlandMask, wetland_mask);
        ctx.atlas.fields.insert_scalar(Fk::Slope, slope);
        ctx.atlas.graphs.hydrology = Some(graph);

        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert(
            "lake_count".into(),
            ctx.atlas
                .graphs
                .hydrology
                .as_ref()
                .map(|g| g.lakes.len() as f64)
                .unwrap_or(0.0),
        );
        metrics.insert(
            "waterfall_count".into(),
            ctx.atlas
                .graphs
                .hydrology
                .as_ref()
                .map(|g| g.waterfalls.len() as f64)
                .unwrap_or(0.0),
        );

        Ok(PassReport {
            pass: PassKey::Hydrology,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics,
            warnings: vec![],
        })
    }
}

pub struct HydrologyFinalizePass;

impl WorldgenPass for HydrologyFinalizePass {
    fn key(&self) -> PassKey {
        PassKey::HydrologyFinalize
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::ErodedElevation,
            FieldKey::LandMask,
            FieldKey::Rainfall,
            FieldKey::Humidity,
            FieldKey::Permeability,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::FilledElevation,
            FieldKey::FlowDirection,
            FieldKey::FlowAccumulation,
            FieldKey::RiverMask,
            FieldKey::Slope,
        ]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let recipe = &ctx.recipe.hydrology;
        let sea = ctx.recipe.extent.sea_level_m;
        let cell = ctx.atlas.control_descriptor.cell_size_m as f32;

        let elevation = ctx
            .atlas
            .fields
            .get_scalar(Fk::ErodedElevation)
            .expect("eroded elevation")
            .clone();
        let land_mask = ctx
            .atlas
            .fields
            .get_scalar(Fk::LandMask)
            .expect("land mask")
            .clone();
        let rainfall = ctx
            .atlas
            .fields
            .get_scalar(Fk::Rainfall)
            .expect("rainfall")
            .clone();
        let humidity = ctx
            .atlas
            .fields
            .get_scalar(Fk::Humidity)
            .expect("humidity")
            .clone();
        let permeability = ctx
            .atlas
            .fields
            .get_scalar(Fk::Permeability)
            .expect("permeability")
            .clone();

        let slope = compute_slope(&elevation);
        let runoff = compute_runoff(&rainfall, &permeability, &land_mask, recipe);
        let filled = priority_flood(&elevation);
        let flow = compute_flow(&filled, &land_mask, &runoff);
        let river_mask = extract_river_mask(
            &flow.accumulation,
            &land_mask,
            recipe.stream_threshold,
            recipe.permanent_river_threshold,
        );

        let mut graph = build_hydrology_graph(
            &filled,
            &flow.accumulation,
            &flow.direction,
            &land_mask,
            &humidity,
            &slope,
            recipe,
            sea,
        );
        graph.primary_river = trace_primary_river(
            &filled,
            &flow.accumulation,
            &flow.direction,
            &land_mask,
            recipe,
            sea,
            cell,
        );

        ctx.atlas.fields.insert_scalar(Fk::FilledElevation, filled);
        ctx.atlas
            .fields
            .insert_categorical(Fk::FlowDirection, flow.direction);
        ctx.atlas
            .fields
            .insert_scalar(Fk::FlowAccumulation, flow.accumulation);
        ctx.atlas.fields.insert_scalar(Fk::RiverMask, river_mask);
        ctx.atlas.fields.insert_scalar(Fk::Slope, slope);
        ctx.atlas.graphs.hydrology = Some(graph);

        let has_river = ctx
            .atlas
            .graphs
            .hydrology
            .as_ref()
            .and_then(|g| g.primary_river.as_ref())
            .is_some();

        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert(
            "has_primary_river".into(),
            if has_river { 1.0 } else { 0.0 },
        );

        Ok(PassReport {
            pass: PassKey::HydrologyFinalize,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics,
            warnings: if has_river {
                vec![]
            } else {
                vec!["no primary river traced after erosion".into()]
            },
        })
    }
}

fn compute_runoff(
    rainfall: &ScalarField,
    permeability: &ScalarField,
    land_mask: &ScalarField,
    recipe: &CompiledHydrologyRecipe,
) -> ScalarField {
    let mut runoff = ScalarField::zeros(rainfall.descriptor.clone());
    for z in 0..runoff.descriptor.height {
        for x in 0..runoff.descriptor.width {
            if land_mask.get(x, z) < 0.2 {
                runoff.set(x, z, 0.0);
                continue;
            }
            let r = rainfall.get(x, z) * recipe.rainfall_weight;
            let infiltration = permeability.get(x, z).clamp(0.0, 1.0);
            runoff.set(x, z, r * (1.0 - infiltration * 0.5).max(0.1));
        }
    }
    runoff
}
