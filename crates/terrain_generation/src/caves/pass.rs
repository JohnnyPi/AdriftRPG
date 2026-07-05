//! Cave compiler pass.

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::contract::coordinates::WorldXZ;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;

use super::graph_gen::generate_cave_systems;
use super::sdf::CaveSubtractOps;
use super::suitability::compute_cave_suitability;
use super::validate::validate_cave_systems;

pub struct CavePass;

impl WorldgenPass for CavePass {
    fn key(&self) -> PassKey {
        PassKey::Caves
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::FinalElevation,
            FieldKey::CoastalElevation,
            FieldKey::LandMask,
            FieldKey::Bedrock,
            FieldKey::Permeability,
            FieldKey::IslandAge,
            FieldKey::FlowAccumulation,
            FieldKey::RiverMask,
            FieldKey::SeaCaveSuitability,
            FieldKey::CliffSuitability,
            FieldKey::Slope,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::LavaTubeSuitability,
            FieldKey::LimestoneCaveSuitability,
            FieldKey::SeaCaveRegionSuitability,
        ]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let recipe = ctx.recipe.caves.clone();
        let suitability = compute_cave_suitability(&ctx.atlas, &recipe);

        ctx.atlas
            .fields
            .insert_scalar(Fk::LavaTubeSuitability, suitability.lava_tube.clone());
        ctx.atlas
            .fields
            .insert_scalar(Fk::LimestoneCaveSuitability, suitability.limestone.clone());
        ctx.atlas
            .fields
            .insert_scalar(Fk::SeaCaveRegionSuitability, suitability.sea_cave.clone());

        let elevation = ctx
            .atlas
            .fields
            .get_scalar(Fk::CoastalElevation)
            .or_else(|| ctx.atlas.fields.get_scalar(Fk::ErodedElevation))
            .or_else(|| ctx.atlas.fields.get_scalar(Fk::FinalElevation))
            .expect("elevation")
            .as_ref()
            .clone();
        let land = ctx
            .atlas
            .fields
            .get_scalar(Fk::LandMask)
            .expect("land")
            .as_ref()
            .clone();
        let river = ctx
            .atlas
            .fields
            .get_scalar(Fk::RiverMask)
            .map(|f| f.as_ref().clone())
            .unwrap_or_else(|| suitability.lava_tube.clone());

        let island = ctx.recipe.islands.first().expect("island");
        let island_center = WorldXZ::new(island.center_x_m, island.center_z_m);
        let sea_level = ctx.recipe.extent.sea_level_m;

        let registry = generate_cave_systems(
            &suitability,
            &elevation,
            &land,
            &river,
            &recipe,
            ctx.recipe.seed,
            sea_level,
            island_center,
        );
        let subtract_ops = CaveSubtractOps::from_systems(&registry.systems);
        let validation = validate_cave_systems(&registry, &subtract_ops, &elevation, sea_level);

        ctx.atlas.graphs.cave_systems = Some(registry);
        ctx.atlas.graphs.cave_subtract_ops = Some(subtract_ops);

        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert("cave_system_count".into(), validation.system_count as f64);
        metrics.insert(
            "traversable_cave_systems".into(),
            validation.traversable_systems as f64,
        );
        metrics.insert(
            "cave_mouth_breaches".into(),
            validation.mouth_breaches as f64,
        );
        metrics.insert(
            "cave_min_clearance_m".into(),
            validation.min_clearance_m as f64,
        );

        Ok(PassReport {
            pass: PassKey::Caves,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics,
            warnings: vec![],
        })
    }
}
