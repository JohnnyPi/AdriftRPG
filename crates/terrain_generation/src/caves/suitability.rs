//! 2D cave-region suitability fields.

use game_data::CompiledCavesRecipe;

use crate::contract::coordinates::WorldXZ;
use crate::fields::key::FieldKey as Fk;
use crate::fields::scalar::ScalarField;
use crate::geology::material::BedrockId;
use crate::world::atlas::WorldAtlas;

use super::graph::CaveFamily;

pub struct CaveSuitabilityFields {
    pub lava_tube: ScalarField,
    pub limestone: ScalarField,
    pub sea_cave: ScalarField,
}

pub fn compute_cave_suitability(
    atlas: &WorldAtlas,
    recipe: &CompiledCavesRecipe,
) -> CaveSuitabilityFields {
    let desc = atlas.control_descriptor.clone();
    let elevation = atlas
        .fields
        .get_scalar(Fk::CoastalElevation)
        .or_else(|| atlas.fields.get_scalar(Fk::ErodedElevation))
        .or_else(|| atlas.fields.get_scalar(Fk::FinalElevation))
        .expect("elevation");
    let land = atlas.fields.get_scalar(Fk::LandMask).expect("land");
    let bedrock = atlas.fields.get_categorical(Fk::Bedrock).expect("bedrock");
    let permeability = atlas
        .fields
        .get_scalar(Fk::Permeability)
        .expect("permeability");
    let age = atlas.fields.get_scalar(Fk::IslandAge).expect("age");
    let slope = atlas
        .fields
        .get_scalar(Fk::Slope)
        .map(|f| f.as_ref().clone())
        .unwrap_or_else(|| ScalarField::zeros(desc.clone()));
    let flow = atlas
        .fields
        .get_scalar(Fk::FlowAccumulation)
        .map(|f| f.as_ref().clone())
        .unwrap_or_else(|| ScalarField::zeros(desc.clone()));
    let river = atlas
        .fields
        .get_scalar(Fk::RiverMask)
        .map(|f| f.as_ref().clone())
        .unwrap_or_else(|| ScalarField::zeros(desc.clone()));
    let sea_cave = atlas
        .fields
        .get_scalar(Fk::SeaCaveSuitability)
        .map(|f| f.as_ref().clone())
        .unwrap_or_else(|| ScalarField::zeros(desc.clone()));
    let cliff = atlas
        .fields
        .get_scalar(Fk::CliffSuitability)
        .map(|f| f.as_ref().clone())
        .unwrap_or_else(|| ScalarField::zeros(desc.clone()));

    let mut lava = ScalarField::zeros(desc.clone());
    let mut limestone = ScalarField::zeros(desc.clone());
    let mut sea = ScalarField::zeros(desc.clone());

    let w = desc.width;
    let h = desc.height;
    for z in 0..h {
        for x in 0..w {
            let wx = desc.origin_x() + x as f64 * desc.cell_size_m + desc.cell_size_m * 0.5;
            let wz = desc.origin_z() + z as f64 * desc.cell_size_m + desc.cell_size_m * 0.5;
            let world = WorldXZ::new(wx, wz);
            if land.sample_at_world(world) < 0.2 {
                continue;
            }
            let bed = BedrockId::from_u8(bedrock.get(x, z));
            let perm = permeability.sample_at_world(world);
            let island_age = age.sample_at_world(world);
            let sl = slope.sample_at_world(world);
            let acc = flow.sample_at_world(world);
            let rv = river.sample_at_world(world);
            let elev = elevation.sample_at_world(world);

            let young_volcanic = smoothstep(recipe.lava_max_age_myr, 0.0, island_age);
            let basalt_bias = match bed {
                BedrockId::Basalt | BedrockId::Tuff | BedrockId::Ash => 1.0,
                BedrockId::WeatheredBasalt => 0.6,
                _ => 0.2,
            };
            let downhill = (acc / 200.0).clamp(0.0, 1.0);
            let not_river = (1.0_f32 - rv * 3.0).clamp(0.0, 1.0);
            lava.set(
                x,
                z,
                (young_volcanic
                    * basalt_bias
                    * downhill
                    * not_river
                    * (1.0_f32 - sl / 60.0).clamp(0.0, 1.0))
                .clamp(0.0, 1.0),
            );

            let lime_bed = match bed {
                BedrockId::Limestone => 1.0,
                BedrockId::WeatheredBasalt => 0.3,
                _ => 0.0,
            };
            let perm_bias = smoothstep(recipe.limestone_min_permeability, 0.8, perm);
            limestone.set(
                x,
                z,
                (lime_bed * 0.6 + perm_bias * 0.5)
                    * (1.0_f32 - rv * 2.0).clamp(0.0, 1.0)
                    * (1.0_f32 - sl / 50.0).clamp(0.2, 1.0),
            );

            let sc = sea_cave.sample_at_world(world);
            let cl = cliff.sample_at_world(world);
            let tidal = if elev > atlas.metadata.extent.sea_level_m + recipe.sea_tidal_band_m[0]
                && elev < atlas.metadata.extent.sea_level_m + recipe.sea_tidal_band_m[1]
            {
                1.0
            } else {
                0.3
            };
            sea.set(x, z, (sc * 0.7 + cl * 0.3) * tidal);
        }
    }

    CaveSuitabilityFields {
        lava_tube: lava,
        limestone,
        sea_cave: sea,
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < f32::EPSILON {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn family_field<'a>(fields: &'a CaveSuitabilityFields, family: CaveFamily) -> &'a ScalarField {
    match family {
        CaveFamily::LavaTube => &fields.lava_tube,
        CaveFamily::Limestone => &fields.limestone,
        CaveFamily::SeaCave => &fields.sea_cave,
        CaveFamily::Fracture | CaveFamily::Talus => &fields.lava_tube,
    }
}
