//! Coastal and marine terrain compiler pass.

mod berm;
mod classify;
mod exposure;
mod marine;

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;
use crate::hydrology::routing::compute_slope;

pub use classify::classify_coast_masks;
pub use marine::{count_lagoon_components, reef_area_m2};

pub struct CoastPass;

impl WorldgenPass for CoastPass {
    fn key(&self) -> PassKey {
        PassKey::Coast
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::ErodedElevation,
            FieldKey::CoastDistance,
            FieldKey::LandMask,
            FieldKey::SedimentThickness,
            FieldKey::RockHardness,
            FieldKey::IslandAge,
            FieldKey::Temperature,
            FieldKey::Bathymetry,
            FieldKey::WindExposure,
            FieldKey::FlowAccumulation,
            FieldKey::RiverMask,
            FieldKey::Humidity,
            FieldKey::FractureIntensity,
            FieldKey::Slope,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::BeachSuitability,
            FieldKey::CliffSuitability,
            FieldKey::ReefSuitability,
            FieldKey::LagoonSuitability,
            FieldKey::MangroveSuitability,
            FieldKey::TidalFlatSuitability,
            FieldKey::SeaCaveSuitability,
            FieldKey::WaveExposureCoastal,
            FieldKey::ShelfMask,
            FieldKey::CoastalElevation,
        ]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let recipe = &ctx.recipe.coast;
        let sea = ctx.recipe.extent.sea_level_m;

        let mut elevation = ctx
            .atlas
            .fields
            .get_scalar(Fk::ErodedElevation)
            .expect("eroded elevation")
            .as_ref()
            .clone();

        let slope = ctx
            .atlas
            .fields
            .get_scalar(Fk::Slope)
            .map(|s| s.as_ref().clone())
            .unwrap_or_else(|| compute_slope(&elevation));

        let coast_distance = ctx
            .atlas
            .fields
            .get_scalar(Fk::CoastDistance)
            .expect("coast distance")
            .as_ref()
            .clone();
        let land_mask = ctx
            .atlas
            .fields
            .get_scalar(Fk::LandMask)
            .expect("land mask")
            .as_ref()
            .clone();
        let sediment = ctx
            .atlas
            .fields
            .get_scalar(Fk::SedimentThickness)
            .expect("sediment")
            .as_ref()
            .clone();
        let wind_exposure = ctx
            .atlas
            .fields
            .get_scalar(Fk::WindExposure)
            .expect("wind exposure")
            .as_ref()
            .clone();
        let bathymetry = ctx
            .atlas
            .fields
            .get_scalar(Fk::Bathymetry)
            .expect("bathymetry")
            .as_ref()
            .clone();
        let temperature = ctx
            .atlas
            .fields
            .get_scalar(Fk::Temperature)
            .expect("temperature")
            .as_ref()
            .clone();
        let island_age = ctx
            .atlas
            .fields
            .get_scalar(Fk::IslandAge)
            .expect("island age")
            .as_ref()
            .clone();
        let river_mask = ctx
            .atlas
            .fields
            .get_scalar(Fk::RiverMask)
            .expect("river mask")
            .as_ref()
            .clone();
        let humidity = ctx
            .atlas
            .fields
            .get_scalar(Fk::Humidity)
            .expect("humidity")
            .as_ref()
            .clone();
        let fracture = ctx
            .atlas
            .fields
            .get_scalar(Fk::FractureIntensity)
            .expect("fracture")
            .as_ref()
            .clone();

        let wave_coastal = exposure::compute_coastal_wave_exposure(
            &wind_exposure,
            &coast_distance,
            &bathymetry,
            &land_mask,
            sea,
        );

        let coast_masks = classify::classify_coast_masks(
            &elevation,
            &slope,
            &coast_distance,
            &land_mask,
            &sediment,
            &wave_coastal,
            recipe,
        );

        berm::apply_beach_berms(
            &mut elevation,
            &coast_masks.beach,
            &coast_distance,
            &land_mask,
            recipe,
            sea,
            ctx.recipe.seed,
        );

        let coastal_slope = compute_slope(&elevation);
        ctx.atlas
            .fields
            .insert_scalar(Fk::Slope, coastal_slope.clone());

        let marine = marine::compute_marine_masks(
            &bathymetry,
            &coast_distance,
            &land_mask,
            &sediment,
            &temperature,
            &island_age,
            &wave_coastal,
            &river_mask,
            &coast_masks.cliff,
            &fracture,
            &coastal_slope,
            &humidity,
            recipe,
            sea,
        );

        let cell = ctx.atlas.control_descriptor.cell_size_m;
        let reef_area = marine::reef_area_m2(&marine.reef, 0.35, cell);
        let lagoon_count = marine::count_lagoon_components(&marine.lagoon, 0.4);

        ctx.atlas
            .fields
            .insert_scalar(Fk::BeachSuitability, coast_masks.beach.clone());
        ctx.atlas
            .fields
            .insert_scalar(Fk::CliffSuitability, coast_masks.cliff.clone());
        ctx.atlas
            .fields
            .insert_scalar(Fk::ReefSuitability, marine.reef.clone());
        ctx.atlas
            .fields
            .insert_scalar(Fk::LagoonSuitability, marine.lagoon.clone());
        ctx.atlas
            .fields
            .insert_scalar(Fk::MangroveSuitability, marine.mangrove);
        ctx.atlas
            .fields
            .insert_scalar(Fk::TidalFlatSuitability, marine.tidal_flat);
        ctx.atlas
            .fields
            .insert_scalar(Fk::SeaCaveSuitability, marine.sea_cave);
        ctx.atlas
            .fields
            .insert_scalar(Fk::WaveExposureCoastal, wave_coastal);
        ctx.atlas.fields.insert_scalar(Fk::ShelfMask, marine.shelf);
        ctx.atlas
            .fields
            .insert_scalar(Fk::CoastalElevation, elevation);

        let mut beach_coast = 0.0f64;
        let mut cliff_coast = 0.0f64;
        for z in 0..land_mask.descriptor.height {
            for x in 0..land_mask.descriptor.width {
                if land_mask.get(x, z) < 0.5 {
                    continue;
                }
                let coast = coast_distance.get(x, z);
                if coast < 0.0 || coast > 150.0 {
                    continue;
                }
                if coast_masks.beach.get(x, z) > 0.4 {
                    beach_coast += 1.0;
                }
                if coast_masks.cliff.get(x, z) > 0.4 {
                    cliff_coast += 1.0;
                }
            }
        }
        let coast_total = (beach_coast + cliff_coast).max(1.0);

        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert("reef_area_m2".into(), reef_area);
        metrics.insert("lagoon_component_count".into(), lagoon_count as f64);
        metrics.insert("beach_coast_fraction".into(), beach_coast / coast_total);

        Ok(PassReport {
            pass: PassKey::Coast,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics,
            warnings: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::coordinates::WorldXZ;
    use crate::fields::descriptor::FieldDescriptor;
    use crate::fields::key::FieldKey;
    use crate::fields::scalar::ScalarField;
    use game_data::CompiledCoastRecipe;

    fn default_coast_recipe() -> CompiledCoastRecipe {
        CompiledCoastRecipe {
            id: "test".into(),
            beach_width_max_m: 120.0,
            beach_max_slope_deg: 12.0,
            berm_height_min_m: 0.3,
            berm_height_max_m: 1.2,
            cliff_min_slope_deg: 25.0,
            cliff_min_exposure: 0.4,
            reef_min_age_myr: 0.5,
            reef_depth_min_m: 0.5,
            reef_depth_max_m: 25.0,
            reef_max_sediment: 0.35,
            reef_min_temperature: 0.55,
            lagoon_max_depth_m: 8.0,
            lagoon_reef_enclosure_min: 0.6,
            mangrove_max_slope_deg: 5.0,
            mangrove_salinity_min_m: 0.0,
            mangrove_salinity_max_m: 200.0,
        }
    }

    fn scalar_constant(desc: FieldDescriptor, value: f32) -> ScalarField {
        let mut field = ScalarField::zeros(desc);
        for z in 0..field.descriptor.height {
            for x in 0..field.descriptor.width {
                field.set(x, z, value);
            }
        }
        field
    }

    #[test]
    fn reef_score_drops_with_river_sediment_plume() {
        let desc = FieldDescriptor::new(
            FieldKey::ReefSuitability,
            WorldXZ::new(-50.0, -50.0),
            10.0,
            11,
            11,
        );
        let bathymetry = scalar_constant(desc.clone(), -10.0);
        let mut coast_distance = scalar_constant(desc.clone(), -30.0);
        coast_distance.set(5, 5, -8.0);
        let land_mask = scalar_constant(desc.clone(), 0.0);
        let mut sediment = scalar_constant(desc.clone(), 0.05);
        sediment.set(5, 5, 0.8);
        let temperature = scalar_constant(desc.clone(), 0.8);
        let island_age = scalar_constant(desc.clone(), 2.0);
        let wave_exposure = scalar_constant(desc.clone(), 0.5);
        let mut river_mask = scalar_constant(desc.clone(), 0.0);
        river_mask.set(5, 5, 1.0);
        let cliff = ScalarField::zeros(desc.clone());
        let fracture = scalar_constant(desc.clone(), 0.3);
        let slope = scalar_constant(desc.clone(), 5.0);
        let humidity = scalar_constant(desc.clone(), 0.7);

        let clean = marine::compute_marine_masks(
            &bathymetry,
            &coast_distance,
            &land_mask,
            &scalar_constant(desc.clone(), 0.05),
            &temperature,
            &island_age,
            &wave_exposure,
            &scalar_constant(desc.clone(), 0.0),
            &cliff,
            &fracture,
            &slope,
            &humidity,
            &default_coast_recipe(),
            0.0,
        );
        let plume = marine::compute_marine_masks(
            &bathymetry,
            &coast_distance,
            &land_mask,
            &sediment,
            &temperature,
            &island_age,
            &wave_exposure,
            &river_mask,
            &cliff,
            &fracture,
            &slope,
            &humidity,
            &default_coast_recipe(),
            0.0,
        );
        assert!(
            clean.reef.get(5, 5) > plume.reef.get(5, 5),
            "river sediment should suppress reef suitability"
        );
    }
}
