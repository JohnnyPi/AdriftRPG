//! Final milestone validation pass.

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;

pub struct FinalValidationPass;

impl WorldgenPass for FinalValidationPass {
    fn key(&self) -> PassKey {
        PassKey::FinalValidation
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::ErodedElevation,
            FieldKey::FinalElevation,
            FieldKey::Bedrock,
            FieldKey::LandMask,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let sea = ctx.recipe.extent.sea_level_m;

        let final_elev = ctx
            .atlas
            .fields
            .get_scalar(Fk::ErodedElevation)
            .or_else(|| ctx.atlas.fields.get_scalar(Fk::FinalElevation))
            .expect("surface elevation");
        let influence = ctx
            .atlas
            .fields
            .get_scalar(Fk::IslandInfluence)
            .expect("island influence");

        let total = (final_elev.descriptor.width * final_elev.descriptor.height) as f32;
        let mut land_cells = 0f32;
        let mut max_elev = f32::NEG_INFINITY;

        for z in 0..final_elev.descriptor.height {
            for x in 0..final_elev.descriptor.width {
                let e = final_elev.get(x, z);
                max_elev = max_elev.max(e);
                if e >= sea && influence.get(x, z) > 0.05 {
                    land_cells += 1.0;
                }
            }
        }

        let land_fraction = land_cells / total;
        if let Some(validation) = &ctx.recipe.validation {
            if land_fraction < validation.land_fraction_min
                || land_fraction > validation.land_fraction_max
            {
                return Err(WorldgenError::Validation(format!(
                    "land fraction {land_fraction} outside [{}, {}]",
                    validation.land_fraction_min, validation.land_fraction_max
                )));
            }
            if max_elev < validation.min_peak_elevation_m {
                return Err(WorldgenError::Validation(format!(
                    "max elevation {max_elev} below minimum {}",
                    validation.min_peak_elevation_m
                )));
            }
        }

        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert("land_fraction".into(), land_fraction as f64);
        metrics.insert("max_elevation".into(), max_elev as f64);

        if let Some(reef) = ctx.atlas.fields.get_scalar(Fk::ReefSuitability) {
            let cell = ctx.atlas.control_descriptor.cell_size_m;
            let area = crate::coast::reef_area_m2(reef.as_ref(), 0.35, cell);
            metrics.insert("reef_area_m2".into(), area);
            if let Some(v) = &ctx.recipe.validation {
                if v.reef_area_min_m2 > 0.0 && area < v.reef_area_min_m2 as f64 {
                    return Err(WorldgenError::Validation(format!(
                        "reef area {area} m² below minimum {}",
                        v.reef_area_min_m2
                    )));
                }
            }
        }
        if let Some(lagoon) = ctx.atlas.fields.get_scalar(Fk::LagoonSuitability) {
            let count = crate::coast::count_lagoon_components(lagoon.as_ref(), 0.4);
            metrics.insert("lagoon_component_count".into(), count as f64);
            if let Some(v) = &ctx.recipe.validation {
                if v.lagoon_count_min > 0 && count < v.lagoon_count_min {
                    return Err(WorldgenError::Validation(format!(
                        "lagoon count {count} below minimum {}",
                        v.lagoon_count_min
                    )));
                }
            }
        }
        if let Some(biome) = ctx.atlas.graphs.biome.as_ref() {
            let mut ids = std::collections::BTreeSet::new();
            for cell in &biome.cells {
                if u8::from(cell.primary) > 0 {
                    ids.insert(u8::from(cell.primary));
                }
            }
            metrics.insert("land_biome_count".into(), ids.len() as f64);
            if let Some(v) = &ctx.recipe.validation {
                if v.biome_entropy_min > 0 && ids.len() < v.biome_entropy_min as usize {
                    return Err(WorldgenError::Validation(format!(
                        "distinct biome count {} below minimum {}",
                        ids.len(),
                        v.biome_entropy_min
                    )));
                }
            }
        }

        if let Some(caves) = ctx.atlas.graphs.cave_systems.as_ref() {
            let count = caves.system_count();
            let traversable = caves.traversable_system_count();
            metrics.insert("cave_system_count".into(), count as f64);
            metrics.insert("traversable_cave_systems".into(), traversable as f64);
            if let Some(v) = &ctx.recipe.validation {
                if v.min_cave_systems > 0 && count < v.min_cave_systems as usize {
                    return Err(WorldgenError::Validation(format!(
                        "cave system count {count} below minimum {}",
                        v.min_cave_systems
                    )));
                }
                if v.min_traversable_cave_systems > 0
                    && traversable < v.min_traversable_cave_systems as usize
                {
                    return Err(WorldgenError::Validation(format!(
                        "traversable cave systems {traversable} below minimum {}",
                        v.min_traversable_cave_systems
                    )));
                }
            }
        }

        Ok(PassReport {
            pass: PassKey::FinalValidation,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: vec![],
            metrics,
            warnings: vec![],
        })
    }
}
