//! Boundary compiler pass.

use glam::DVec2;

use crate::boundary::distance::{distance_to_rect_edge, normalized_interior_mask};
use crate::boundary::falloff::generate_ocean_basin;
use crate::boundary::validation::validate_boundary_perimeter;
use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::fields::key::FieldKey;
use crate::fields::scalar::ScalarField;

pub struct BoundaryPass;

impl WorldgenPass for BoundaryPass {
    fn key(&self) -> PassKey {
        PassKey::Boundary
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::BoundaryDistance,
            FieldKey::BoundaryMask,
            FieldKey::OceanBasin,
        ]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let recipe = &ctx.recipe.boundary;
        let extent = &ctx.recipe.extent;
        let desc = ctx.atlas.control_descriptor.clone();
        let half = DVec2::new(extent.width_m * 0.5, extent.depth_m * 0.5);

        let mut boundary_distance = ScalarField::zeros(desc.clone());
        let mut boundary_mask = ScalarField::zeros(desc.clone());

        for z in 0..desc.height {
            for x in 0..desc.width {
                let wx = desc.origin_x() + x as f64 * desc.cell_size_m;
                let wz = desc.origin_z() + z as f64 * desc.cell_size_m;
                let edge_d = distance_to_rect_edge(DVec2::new(wx, wz), half);
                let mask =
                    normalized_interior_mask(edge_d, half.x, recipe.ocean_edge_start_fraction);
                boundary_distance.set(x, z, edge_d as f32);
                boundary_mask.set(x, z, mask);
            }
        }

        let ocean_basin =
            generate_ocean_basin(desc.clone(), &boundary_distance, recipe, ctx.recipe.seed);

        let validation = validate_boundary_perimeter(&ocean_basin, -100.0);
        if !validation.passed {
            return Err(WorldgenError::Validation(validation.messages.join("; ")));
        }

        ctx.atlas
            .fields
            .insert_scalar(FieldKey::BoundaryDistance, boundary_distance);
        ctx.atlas
            .fields
            .insert_scalar(FieldKey::BoundaryMask, boundary_mask);
        ctx.atlas
            .fields
            .insert_scalar(FieldKey::OceanBasin, ocean_basin);

        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert(
            "max_perimeter_elevation".into(),
            validation.max_perimeter_elevation as f64,
        );

        Ok(PassReport {
            pass: PassKey::Boundary,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics,
            warnings: vec![],
        })
    }
}
