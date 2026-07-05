//! Water channel and basin carving pass.

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;
use crate::fields::scalar::ScalarField;
use crate::hydrology::realize::realize_hydrology_from_atlas;
use crate::island_gen::{ErosionParams, IslandGenParams, carve_river_channels};

pub struct WaterCarvePass;

impl WorldgenPass for WaterCarvePass {
    fn key(&self) -> PassKey {
        PassKey::WaterCarve
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::CoastalElevation,
            FieldKey::ErodedElevation,
            FieldKey::FinalElevation,
            FieldKey::RiverMask,
            FieldKey::LakeMask,
            FieldKey::LagoonSuitability,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[FieldKey::CarvedElevation]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let mut carved = ctx
            .atlas
            .fields
            .get_scalar(Fk::CoastalElevation)
            .or_else(|| ctx.atlas.fields.get_scalar(Fk::ErodedElevation))
            .or_else(|| ctx.atlas.fields.get_scalar(Fk::FinalElevation))
            .expect("elevation")
            .as_ref()
            .clone();

        if let Some(graph) = ctx.atlas.graphs.hydrology.as_ref() {
            if let Some(ref river) = graph.primary_river {
                let params = IslandGenParams {
                    erosion: ErosionParams {
                        river_bank_width_m: 6.0,
                        river_carve_strength: 1.0,
                        stream_power_iterations: 0,
                        m: 0.0,
                        n: 0.0,
                        maximum_step_m: 0.0,
                        stream_power_erodibility: 0.0,
                        thermal_iterations: 0,
                        thermal_transfer_rate: 0.0,
                        thermal_talus_deg: 0.0,
                    },
                    ..IslandGenParams::default()
                };
                let mut field = scalar_to_field2d(&carved);
                carve_river_channels(&mut field, river, &params);
                apply_field2d_to_scalar(&field, &mut carved);
            }
        }

        carve_lake_basins(&mut carved, ctx);
        carve_lagoon_basins(&mut carved, ctx);

        ctx.atlas
            .fields
            .insert_scalar(Fk::CarvedElevation, carved.clone());

        let cave_pools = ctx
            .atlas
            .graphs
            .cave_systems
            .as_ref()
            .map(collect_cave_pools)
            .unwrap_or_default();
        let products = realize_hydrology_from_atlas(&ctx.atlas, &cave_pools);
        ctx.atlas.graphs.hydrology_products = Some(products);

        let mut metrics = std::collections::BTreeMap::new();
        if let Some(ref hp) = ctx.atlas.graphs.hydrology_products {
            metrics.insert("lake_body_count".into(), hp.lakes.len() as f64);
            metrics.insert("lagoon_body_count".into(), hp.lagoons.len() as f64);
            metrics.insert("waterfall_body_count".into(), hp.waterfalls.len() as f64);
        }

        Ok(PassReport {
            pass: PassKey::WaterCarve,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics,
            warnings: vec![],
        })
    }
}

fn collect_cave_pools(
    registry: &crate::caves::graph::CaveGraphRegistry,
) -> Vec<(f64, f64, f64, f32)> {
    let mut pools = Vec::new();
    for system in &registry.systems {
        extract_pools(system, &mut pools);
    }
    pools
}

fn extract_pools(system: &crate::caves::graph::CaveSystem, out: &mut Vec<(f64, f64, f64, f32)>) {
    use crate::caves::graph::CaveNodeKind;
    for node in &system.nodes {
        if node.kind == CaveNodeKind::Pool {
            out.push((
                node.position.0.x,
                node.position.0.y,
                node.position.0.z,
                node.radius_m,
            ));
        }
    }
}

fn carve_lake_basins(carved: &mut ScalarField, ctx: &CompileContext) {
    let Some(graph) = ctx.atlas.graphs.hydrology.as_ref() else {
        return;
    };
    for basin in &graph.lakes {
        for (x, z) in &basin.cells {
            let current = carved.get(*x, *z);
            if current > basin.surface_elevation_m {
                carved.set(*x, *z, basin.surface_elevation_m);
            }
        }
    }
}

fn carve_lagoon_basins(carved: &mut ScalarField, ctx: &CompileContext) {
    let Some(lagoon) = ctx.atlas.fields.get_scalar(Fk::LagoonSuitability) else {
        return;
    };
    let lagoon = lagoon.as_ref();
    let max_depth = ctx.recipe.coast.lagoon_max_depth_m;
    let sea = ctx.recipe.extent.sea_level_m;
    let w = lagoon.descriptor.width;
    let h = lagoon.descriptor.height;
    for z in 0..h {
        for x in 0..w {
            let suit = lagoon.get(x, z);
            if suit < 0.4 {
                continue;
            }
            let target = sea - max_depth * suit;
            let current = carved.get(x, z);
            if current > target {
                carved.set(x, z, target);
            }
        }
    }
}

fn scalar_to_field2d(field: &ScalarField) -> crate::field2d::Field2D<f32> {
    let d = &field.descriptor;
    let mut out = crate::field2d::Field2D::<f32>::new(
        d.width,
        d.height,
        [d.origin_x() as f32, d.origin_z() as f32],
        d.cell_size_m as f32,
    );
    for z in 0..d.height {
        for x in 0..d.width {
            out.set(x, z, field.get(x, z));
        }
    }
    out
}

fn apply_field2d_to_scalar(src: &crate::field2d::Field2D<f32>, dst: &mut ScalarField) {
    for z in 0..src.height {
        for x in 0..src.width {
            dst.set(x, z, src.get(x, z));
        }
    }
}
