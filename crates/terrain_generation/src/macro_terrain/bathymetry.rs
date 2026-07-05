//! Bathymetry and shelf from provisional coastline.

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::contract::coordinates::WorldXZ;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;
use crate::fields::scalar::ScalarField;
use crate::macro_terrain::coast_distance::{compute_land_mask, compute_signed_coast_distance};

pub struct BathymetryPass;

impl WorldgenPass for BathymetryPass {
    fn key(&self) -> PassKey {
        PassKey::Bathymetry
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[FieldKey::BaseElevation, FieldKey::OceanBasin]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::Bathymetry,
            FieldKey::CoastDistance,
            FieldKey::LandMask,
        ]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let sea = ctx.recipe.extent.sea_level_m;
        let base = ctx
            .atlas
            .fields
            .get_scalar(Fk::BaseElevation)
            .expect("base elevation");
        let ocean = ctx
            .atlas
            .fields
            .get_scalar(Fk::OceanBasin)
            .expect("ocean basin");
        let desc = ctx.atlas.control_descriptor.clone();

        let coast_distance = compute_signed_coast_distance(&base, sea);
        let land_mask = compute_land_mask(&base, sea);

        let mut bathymetry = ScalarField::zeros(desc.clone());

        for z in 0..desc.height {
            for x in 0..desc.width {
                let wx = desc.origin_x() + x as f64 * desc.cell_size_m;
                let wz = desc.origin_z() + z as f64 * desc.cell_size_m;
                let world = WorldXZ::new(wx, wz);
                let elev = base.get(x, z);
                let cd = coast_distance.get(x, z).max(0.0);
                let bathy = if elev >= sea {
                    elev
                } else {
                    let shelf_t = (cd / 2000.0).clamp(0.0, 1.0);
                    let shelf = sea - 40.0 * (1.0 - shelf_t);
                    elev.max(ocean.sample_at_world(world)).min(shelf)
                };
                bathymetry.set(x, z, bathy);
            }
        }

        ctx.atlas
            .fields
            .insert_scalar(Fk::CoastDistance, coast_distance);
        ctx.atlas.fields.insert_scalar(Fk::LandMask, land_mask);
        ctx.atlas.fields.insert_scalar(Fk::Bathymetry, bathymetry);

        Ok(PassReport {
            pass: PassKey::Bathymetry,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics: Default::default(),
            warnings: vec![],
        })
    }
}
