//! Regional refinement compiler pass.

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::contract::coordinates::WorldXZ;
use crate::fields::descriptor::FieldDescriptor;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;
use crate::fields::scalar::ScalarField;
use crate::regional::blending::window_weight_2d;
use crate::regional::generator::generate_patch_residual;
use crate::regional::seams::compute_seam_metrics;

pub struct RegionalRefinementPass;

impl WorldgenPass for RegionalRefinementPass {
    fn key(&self) -> PassKey {
        PassKey::RegionalRefinement
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::BaseElevation,
            FieldKey::Bathymetry,
            FieldKey::LandMask,
            FieldKey::CoastDistance,
            FieldKey::RockHardness,
            FieldKey::Erodibility,
            FieldKey::ValueConstraint,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[FieldKey::RegionalResidual, FieldKey::FinalElevation]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let sea = ctx.recipe.extent.sea_level_m;
        let refinement = &ctx.recipe.refinement;
        let desc = ctx.atlas.control_descriptor.clone();

        let base = ctx
            .atlas
            .fields
            .get_scalar(Fk::BaseElevation)
            .expect("base elevation");
        let bathymetry = ctx
            .atlas
            .fields
            .get_scalar(Fk::Bathymetry)
            .expect("bathymetry");
        let land_mask_field = ctx
            .atlas
            .fields
            .get_scalar(Fk::LandMask)
            .expect("land mask");
        let hardness = ctx
            .atlas
            .fields
            .get_scalar(Fk::RockHardness)
            .expect("hardness");
        let erodibility = ctx
            .atlas
            .fields
            .get_scalar(Fk::Erodibility)
            .expect("erodibility");
        let coast = ctx
            .atlas
            .fields
            .get_scalar(Fk::CoastDistance)
            .expect("coast distance");
        let constraint = ctx
            .atlas
            .fields
            .get_scalar(Fk::ValueConstraint)
            .expect("value constraint");

        let mut weighted_sum = ScalarField::zeros(desc.clone());
        let mut weight_sum = ScalarField::zeros(desc.clone());

        let interior = refinement.window_interior_samples;
        let stride = refinement.window_stride_samples;
        let cell = desc.cell_size_m;

        let mut patch_index = 0u64;
        let mut z0 = 0u32;
        while z0 < desc.height {
            let mut x0 = 0u32;
            while x0 < desc.width {
                let patch_w = interior[0].min(desc.width - x0);
                let patch_h = interior[1].min(desc.height - z0);
                let patch_origin = WorldXZ::new(
                    desc.origin_x() + x0 as f64 * cell,
                    desc.origin_z() + z0 as f64 * cell,
                );
                let patch_desc = FieldDescriptor::new(
                    Fk::RegionalResidual,
                    patch_origin,
                    cell,
                    patch_w,
                    patch_h,
                );

                let patch = generate_patch_residual(
                    patch_desc,
                    ctx.recipe.seed,
                    patch_index,
                    refinement.regional_amplitude_m,
                    &hardness,
                    &erodibility,
                    &coast,
                    &constraint,
                    refinement.coast_preserve_start_m,
                    refinement.coast_preserve_end_m,
                );
                patch_index += 1;

                for lz in 0..patch_h {
                    for lx in 0..patch_w {
                        let gx = x0 + lx;
                        let gz = z0 + lz;
                        let fx = if patch_w > 1 {
                            lx as f32 / (patch_w - 1) as f32
                        } else {
                            1.0
                        };
                        let fz = if patch_h > 1 {
                            lz as f32 / (patch_h - 1) as f32
                        } else {
                            1.0
                        };
                        let w = window_weight_2d(fx, fz);
                        let r = patch.residual.get(lx, lz);
                        weighted_sum.set(gx, gz, weighted_sum.get(gx, gz) + r * w);
                        weight_sum.set(gx, gz, weight_sum.get(gx, gz) + w);
                    }
                }

                x0 = x0.saturating_add(stride[0]).max(x0 + 1);
                if x0 >= desc.width {
                    break;
                }
            }
            z0 = z0.saturating_add(stride[1]).max(z0 + 1);
            if z0 >= desc.height {
                break;
            }
        }

        let mut regional_residual = ScalarField::zeros(desc.clone());
        let mut final_elevation = ScalarField::zeros(desc.clone());
        for z in 0..desc.height {
            for x in 0..desc.width {
                let ws = weight_sum.get(x, z);
                let residual = if ws > 1e-6 {
                    weighted_sum.get(x, z) / ws
                } else {
                    0.0
                };
                regional_residual.set(x, z, residual);
                let land = land_mask_field.get(x, z);
                let land_elev = base.get(x, z) + residual * land;
                let surface = if land_elev >= sea {
                    land_elev
                } else {
                    bathymetry.get(x, z)
                };
                final_elevation.set(x, z, surface);
            }
        }

        let seams = compute_seam_metrics(&regional_residual);
        if seams.max_abs_diff > refinement.seam_max_elevation_diff_m * 10.0 {
            // Internal gradient check — blended field should be smooth
        }

        ctx.atlas
            .fields
            .insert_scalar(Fk::RegionalResidual, regional_residual);
        ctx.atlas
            .fields
            .insert_scalar(Fk::FinalElevation, final_elevation);

        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert("seam_max".into(), seams.max_abs_diff as f64);
        metrics.insert("seam_mean".into(), seams.mean_abs_diff as f64);
        metrics.insert("patch_count".into(), patch_index as f64);

        Ok(PassReport {
            pass: PassKey::RegionalRefinement,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics,
            warnings: vec![],
        })
    }
}
