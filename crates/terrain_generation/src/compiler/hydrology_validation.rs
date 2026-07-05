//! Hydrology validation metrics (Phase 24 lite).

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;
use crate::hydrology::fill::D8_NEIGHBORS;

pub struct HydrologyValidationPass;

impl WorldgenPass for HydrologyValidationPass {
    fn key(&self) -> PassKey {
        PassKey::HydrologyValidation
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::RiverMask,
            FieldKey::FlowDirection,
            FieldKey::LandMask,
            FieldKey::FilledElevation,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let sea = ctx.recipe.extent.sea_level_m;
        let validation = ctx.recipe.validation.as_ref();

        let river_mask = ctx
            .atlas
            .fields
            .get_scalar(Fk::RiverMask)
            .expect("river mask");
        let direction = ctx
            .atlas
            .fields
            .get_categorical(Fk::FlowDirection)
            .expect("flow direction");
        let land_mask = ctx
            .atlas
            .fields
            .get_scalar(Fk::LandMask)
            .expect("land mask");
        let filled = ctx
            .atlas
            .fields
            .get_scalar(Fk::FilledElevation)
            .expect("filled elevation");

        let mut permanent_river_cells = 0u32;
        let mut ocean_connected = 0u32;
        let mut disconnected = 0u32;

        let w = river_mask.descriptor.width;
        let h = river_mask.descriptor.height;

        for z in 0..h {
            for x in 0..w {
                if river_mask.get(x, z) < 0.9 {
                    continue;
                }
                permanent_river_cells += 1;
                if traces_to_ocean(x, z, &direction, &land_mask, &filled, sea, w, h) {
                    ocean_connected += 1;
                } else {
                    disconnected += 1;
                }
            }
        }

        let connection_ratio = if permanent_river_cells > 0 {
            ocean_connected as f64 / permanent_river_cells as f64
        } else {
            0.0
        };
        let disconnected_fraction = if permanent_river_cells > 0 {
            disconnected as f64 / permanent_river_cells as f64
        } else {
            0.0
        };

        let river_length = ctx
            .atlas
            .graphs
            .hydrology
            .as_ref()
            .and_then(|g| g.primary_river.as_ref())
            .map(|r| {
                r.points
                    .windows(2)
                    .map(|w| {
                        let dx = w[1].position_xz[0] - w[0].position_xz[0];
                        let dz = w[1].position_xz[1] - w[0].position_xz[1];
                        (dx * dx + dz * dz).sqrt()
                    })
                    .sum::<f32>() as f64
            })
            .unwrap_or(0.0);

        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert("river_ocean_connection_ratio".into(), connection_ratio);
        metrics.insert("disconnected_river_fraction".into(), disconnected_fraction);
        metrics.insert("permanent_river_length_m".into(), river_length);
        metrics.insert(
            "has_primary_river".into(),
            if river_length > 0.0 { 1.0 } else { 0.0 },
        );

        let mut warnings = Vec::new();
        if let Some(v) = validation {
            if connection_ratio < v.river_ocean_connection_ratio_min as f64 {
                warnings.push(format!(
                    "river-ocean connection ratio {:.2} below minimum {:.2}",
                    connection_ratio, v.river_ocean_connection_ratio_min
                ));
            }
            if disconnected_fraction > v.max_disconnected_river_fraction as f64 {
                warnings.push(format!(
                    "disconnected river fraction {:.2} exceeds maximum {:.2}",
                    disconnected_fraction, v.max_disconnected_river_fraction
                ));
            }
            if v.min_permanent_river_length_m > 0.0
                && river_length < v.min_permanent_river_length_m as f64
            {
                warnings.push(format!(
                    "primary river length {:.0}m below minimum {:.0}m",
                    river_length, v.min_permanent_river_length_m
                ));
            }
        }

        Ok(PassReport {
            pass: PassKey::HydrologyValidation,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: vec![],
            metrics,
            warnings,
        })
    }
}

fn traces_to_ocean(
    start_x: u32,
    start_z: u32,
    direction: &std::sync::Arc<crate::fields::typed::CategoricalField<u8>>,
    land_mask: &std::sync::Arc<crate::fields::scalar::ScalarField>,
    filled: &std::sync::Arc<crate::fields::scalar::ScalarField>,
    sea: f32,
    w: u32,
    h: u32,
) -> bool {
    let mut visited = std::collections::HashSet::new();
    let mut cx = start_x;
    let mut cz = start_z;
    for _ in 0..500 {
        if !visited.insert((cx, cz)) {
            return false;
        }
        if land_mask.get(cx, cz) < 0.1 || filled.get(cx, cz) <= sea + 0.25 {
            return true;
        }
        let dir = direction.get(cx, cz);
        if dir == 255 {
            return false;
        }
        let (dx, dz) = D8_NEIGHBORS[dir as usize];
        let nx = cx as i32 + dx;
        let nz = cz as i32 + dz;
        if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
            return true;
        }
        cx = nx as u32;
        cz = nz as u32;
    }
    false
}
