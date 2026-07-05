//! Biome classification compiler pass.

pub mod id;
mod suitability;

use crate::biomes::id::CompilerBiomeId;
use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;
use crate::fields::scalar::ScalarField;
use crate::fields::typed::CategoricalField;
use crate::world::graphs::BiomeGrid;

pub use id::{BiomeBlendCell, CompilerBiomeId as BiomeId};

pub struct BiomePass;

impl WorldgenPass for BiomePass {
    fn key(&self) -> PassKey {
        PassKey::Biome
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::CoastalElevation,
            FieldKey::Slope,
            FieldKey::Rainfall,
            FieldKey::Humidity,
            FieldKey::Temperature,
            FieldKey::WindExposure,
            FieldKey::SoilDepth,
            FieldKey::LandMask,
            FieldKey::WetlandMask,
            FieldKey::RiverMask,
            FieldKey::BeachSuitability,
            FieldKey::CliffSuitability,
            FieldKey::MangroveSuitability,
            FieldKey::ReefSuitability,
            FieldKey::LagoonSuitability,
            FieldKey::TidalFlatSuitability,
            FieldKey::ShelfMask,
            FieldKey::Bathymetry,
            FieldKey::CoastDistance,
            FieldKey::FlowAccumulation,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[FieldKey::PrimaryBiome, FieldKey::BiomeBlendWeight]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let recipe = &ctx.recipe.biomes;
        let sea = ctx.recipe.extent.sea_level_m;

        let elevation = ctx
            .atlas
            .fields
            .get_scalar(Fk::CoastalElevation)
            .or_else(|| ctx.atlas.fields.get_scalar(Fk::ErodedElevation))
            .expect("elevation")
            .as_ref()
            .clone();
        let slope = ctx
            .atlas
            .fields
            .get_scalar(Fk::Slope)
            .expect("slope")
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
        let wind = ctx
            .atlas
            .fields
            .get_scalar(Fk::WindExposure)
            .expect("wind")
            .as_ref()
            .clone();
        let soil = ctx
            .atlas
            .fields
            .get_scalar(Fk::SoilDepth)
            .expect("soil")
            .as_ref()
            .clone();
        let land_mask = ctx
            .atlas
            .fields
            .get_scalar(Fk::LandMask)
            .expect("land")
            .as_ref()
            .clone();
        let wetland = ctx
            .atlas
            .fields
            .get_scalar(Fk::WetlandMask)
            .expect("wetland")
            .as_ref()
            .clone();
        let river = ctx
            .atlas
            .fields
            .get_scalar(Fk::RiverMask)
            .expect("river")
            .as_ref()
            .clone();
        let beach = ctx
            .atlas
            .fields
            .get_scalar(Fk::BeachSuitability)
            .expect("beach")
            .as_ref()
            .clone();
        let cliff = ctx
            .atlas
            .fields
            .get_scalar(Fk::CliffSuitability)
            .expect("cliff")
            .as_ref()
            .clone();
        let mangrove = ctx
            .atlas
            .fields
            .get_scalar(Fk::MangroveSuitability)
            .expect("mangrove")
            .as_ref()
            .clone();
        let reef = ctx
            .atlas
            .fields
            .get_scalar(Fk::ReefSuitability)
            .expect("reef")
            .as_ref()
            .clone();
        let lagoon = ctx
            .atlas
            .fields
            .get_scalar(Fk::LagoonSuitability)
            .expect("lagoon")
            .as_ref()
            .clone();
        let tidal = ctx
            .atlas
            .fields
            .get_scalar(Fk::TidalFlatSuitability)
            .expect("tidal")
            .as_ref()
            .clone();
        let shelf = ctx
            .atlas
            .fields
            .get_scalar(Fk::ShelfMask)
            .expect("shelf")
            .as_ref()
            .clone();
        let bathymetry = ctx
            .atlas
            .fields
            .get_scalar(Fk::Bathymetry)
            .expect("bathymetry")
            .as_ref()
            .clone();
        let coast_distance = ctx
            .atlas
            .fields
            .get_scalar(Fk::CoastDistance)
            .expect("coast distance")
            .as_ref()
            .clone();
        let flow = ctx
            .atlas
            .fields
            .get_scalar(Fk::FlowAccumulation)
            .expect("flow")
            .as_ref()
            .clone();
        let temperature = ctx
            .atlas
            .fields
            .get_scalar(Fk::Temperature)
            .expect("temperature")
            .as_ref()
            .clone();

        let desc = elevation.descriptor.clone();
        let mut primary = CategoricalField::<u8>::zeros(desc.clone());
        let mut blend_weight = ScalarField::zeros(desc.clone());
        let mut grid = BiomeGrid {
            width: desc.width,
            height: desc.height,
            cells: vec![BiomeBlendCell::default(); (desc.width * desc.height) as usize],
        };

        let mut land_biome_counts = std::collections::BTreeMap::<u8, u32>::new();
        let mut windward_forest = 0.0f64;
        let mut leeward_forest = 0.0f64;
        let mut windward_samples = 0u64;
        let mut leeward_samples = 0u64;

        for z in 0..desc.height {
            for x in 0..desc.width {
                let idx = (z * desc.width + x) as usize;
                let wetness = (flow.get(x, z) / 100.0).clamp(0.0, 1.0);
                let micro = suitability::micro_biome_noise(ctx.recipe.seed, x, z);

                let mut scores = if land_mask.get(x, z) > 0.3 {
                    suitability::score_land_biomes(
                        elevation.get(x, z),
                        slope.get(x, z),
                        rainfall.get(x, z),
                        humidity.get(x, z),
                        wetness,
                        soil.get(x, z),
                        wind.get(x, z),
                        beach.get(x, z),
                        cliff.get(x, z),
                        wetland.get(x, z),
                        river.get(x, z),
                        mangrove.get(x, z),
                        recipe,
                        sea,
                        micro,
                    )
                } else {
                    let depth = (sea - bathymetry.get(x, z)).max(0.0);
                    suitability::score_marine_biomes(
                        depth,
                        coast_distance.get(x, z).abs(),
                        reef.get(x, z),
                        lagoon.get(x, z),
                        tidal.get(x, z),
                        shelf.get(x, z),
                        temperature.get(x, z),
                        recipe,
                    )
                };

                let blend = suitability::pick_biome_blend(&mut scores);
                primary.set(x, z, u8::from(blend.primary));
                blend_weight.set(x, z, blend.primary_weight);
                grid.cells[idx] = blend;

                if land_mask.get(x, z) > 0.3 {
                    *land_biome_counts
                        .entry(u8::from(blend.primary))
                        .or_default() += 1;
                    let forest_score = scores
                        .iter()
                        .find(|s| s.id == CompilerBiomeId::Forest)
                        .map(|s| s.score as f64)
                        .unwrap_or(0.0);
                    if wind.get(x, z) > 0.55 {
                        windward_forest += forest_score;
                        windward_samples += 1;
                    } else if wind.get(x, z) < 0.35 {
                        leeward_forest += forest_score;
                        leeward_samples += 1;
                    }
                }
            }
        }

        ctx.atlas
            .fields
            .insert_categorical(Fk::PrimaryBiome, primary);
        ctx.atlas
            .fields
            .insert_scalar(Fk::BiomeBlendWeight, blend_weight);
        ctx.atlas.graphs.biome = Some(grid);

        let land_biome_entropy = land_biome_counts.len() as f64;
        let windward_mean = if windward_samples > 0 {
            windward_forest / windward_samples as f64
        } else {
            0.0
        };
        let leeward_mean = if leeward_samples > 0 {
            leeward_forest / leeward_samples as f64
        } else {
            0.0
        };

        let mut metrics = std::collections::BTreeMap::new();
        metrics.insert("land_biome_count".into(), land_biome_entropy);
        metrics.insert("windward_forest_mean".into(), windward_mean);
        metrics.insert("leeward_forest_mean".into(), leeward_mean);

        Ok(PassReport {
            pass: PassKey::Biome,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics,
            warnings: vec![],
        })
    }
}
