//! Strata and column depth compiler pass.

use game_data::CompiledStrataLayer;

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;
use crate::fields::scalar::ScalarField;

pub mod material;

pub use material::StrataMaterialId;

pub struct StrataPass;

fn layer_thickness(
    layer: &CompiledStrataLayer,
    rainfall: f32,
    slope: f32,
    age: f32,
    vegetated: bool,
) -> f32 {
    if layer.remaining {
        return 0.0;
    }
    if layer.requires_vegetated && !vegetated {
        return 0.0;
    }
    let mut t = (layer.thickness_min_m + layer.thickness_max_m) * 0.5;
    if layer.driven_by_rainfall {
        t *= 0.6 + rainfall * 0.8;
    }
    if layer.driven_by_slope {
        t *= 1.0 - (slope / 45.0).clamp(0.0, 0.85);
    }
    if layer.driven_by_age {
        t *= 0.5 + age * 0.25;
    }
    t.clamp(layer.thickness_min_m, layer.thickness_max_m)
}

impl WorldgenPass for StrataPass {
    fn key(&self) -> PassKey {
        PassKey::Strata
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::SoilDepth,
            FieldKey::Rainfall,
            FieldKey::Slope,
            FieldKey::IslandAge,
            FieldKey::PrimaryBiome,
            FieldKey::BeachSuitability,
            FieldKey::RiverMask,
            FieldKey::ReefSuitability,
            FieldKey::WetlandMask,
            FieldKey::LandMask,
        ]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[
            FieldKey::RegolithDepth,
            FieldKey::WeatheringDepth,
            FieldKey::DepositMask,
        ]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let recipe = &ctx.recipe.strata;

        let soil = ctx
            .atlas
            .fields
            .get_scalar(Fk::SoilDepth)
            .expect("soil")
            .as_ref()
            .clone();
        let rainfall = ctx
            .atlas
            .fields
            .get_scalar(Fk::Rainfall)
            .expect("rainfall")
            .as_ref()
            .clone();
        let slope = ctx
            .atlas
            .fields
            .get_scalar(Fk::Slope)
            .expect("slope")
            .as_ref()
            .clone();
        let age = ctx
            .atlas
            .fields
            .get_scalar(Fk::IslandAge)
            .expect("age")
            .as_ref()
            .clone();
        let primary_biome = ctx
            .atlas
            .fields
            .get_categorical(Fk::PrimaryBiome)
            .expect("primary biome");
        let beach = ctx
            .atlas
            .fields
            .get_scalar(Fk::BeachSuitability)
            .expect("beach")
            .as_ref()
            .clone();
        let river = ctx
            .atlas
            .fields
            .get_scalar(Fk::RiverMask)
            .expect("river")
            .as_ref()
            .clone();
        let reef = ctx
            .atlas
            .fields
            .get_scalar(Fk::ReefSuitability)
            .expect("reef")
            .as_ref()
            .clone();
        let wetland = ctx
            .atlas
            .fields
            .get_scalar(Fk::WetlandMask)
            .expect("wetland")
            .as_ref()
            .clone();
        let land_mask = ctx
            .atlas
            .fields
            .get_scalar(Fk::LandMask)
            .expect("land")
            .as_ref()
            .clone();

        let desc = soil.descriptor.clone();
        let mut regolith = ScalarField::zeros(desc.clone());
        let mut weathering = ScalarField::zeros(desc.clone());
        let mut deposit = ScalarField::zeros(desc.clone());

        for z in 0..desc.height {
            for x in 0..desc.width {
                if land_mask.get(x, z) < 0.2 {
                    continue;
                }
                let biome_id = primary_biome.get(x, z);
                let vegetated =
                    crate::biomes::id::CompilerBiomeId::from_u8(biome_id).is_vegetated();
                let r = rainfall.get(x, z);
                let sl = slope.get(x, z);
                let a = age.get(x, z);

                let mut regolith_t = 0.5f32;
                let mut weathering_t = 1.5f32;
                for layer in &recipe.layers {
                    let t = layer_thickness(layer, r, sl, a, vegetated);
                    match layer.material.as_str() {
                        "topsoil" | "organic_soil" => {}
                        "weathered_basalt" | "weathered_rock" => weathering_t = t,
                        "basalt" => {}
                        _ => regolith_t = t.max(regolith_t),
                    }
                }
                regolith.set(x, z, regolith_t.max(0.2));
                weathering.set(x, z, weathering_t.max(0.5));

                let mut deposit_id = material::StrataMaterialId::Basalt;
                let mut deposit_strength = 0.0f32;
                for dep in &recipe.deposits {
                    let mask_value = match dep.mask.as_str() {
                        "beach_suitability" => beach.get(x, z),
                        "river_corridor" => river.get(x, z),
                        "reef_suitability" => reef.get(x, z),
                        "wetland" => wetland.get(x, z),
                        _ => 0.0,
                    };
                    if mask_value > deposit_strength {
                        deposit_strength = mask_value;
                        deposit_id = material::StrataMaterialId::from_material_name(&dep.id);
                    }
                }
                if deposit_strength > 0.35 {
                    deposit.set(x, z, u8::from(deposit_id) as f32);
                }
            }
        }

        ctx.atlas.fields.insert_scalar(Fk::RegolithDepth, regolith);
        ctx.atlas
            .fields
            .insert_scalar(Fk::WeatheringDepth, weathering);
        ctx.atlas.fields.insert_scalar(Fk::DepositMask, deposit);

        Ok(PassReport {
            pass: PassKey::Strata,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics: std::collections::BTreeMap::new(),
            warnings: vec![],
        })
    }
}
