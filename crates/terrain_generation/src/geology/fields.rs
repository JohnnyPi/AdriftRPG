//! Geology field generation.

use game_data::CompiledGeologyRecipe;

use crate::contract::coordinates::WorldXZ;
use crate::contract::version::derive_seed;
use crate::fields::key::FieldKey as Fk;
use crate::fields::scalar::ScalarField;
use crate::fields::typed::CategoricalField;
use crate::geology::material::BedrockId;
use crate::islands::skeleton::IslandSkeleton;
use crate::world::atlas::WorldAtlas;

pub fn generate_geology_fields(
    atlas: &mut WorldAtlas,
    recipe: &CompiledGeologyRecipe,
    skeleton: &IslandSkeleton,
    world_seed: u64,
) {
    let influence = atlas
        .fields
        .get_scalar(Fk::IslandInfluence)
        .expect("island influence");
    let age_field = atlas.fields.get_scalar(Fk::IslandAge).expect("island age");
    let coast = atlas
        .fields
        .get_scalar(Fk::CoastDistance)
        .expect("coast distance");

    let desc = influence.descriptor().clone();
    let mut bedrock = CategoricalField::<BedrockId>::zeros(desc.clone());
    let mut hardness = ScalarField::zeros(desc.clone());
    let mut erodibility = ScalarField::zeros(desc.clone());
    let mut permeability = ScalarField::zeros(desc.clone());
    let mut fracture = ScalarField::zeros(desc.clone());
    let mut value_constraint = ScalarField::zeros(desc.clone());
    let mut gradient_constraint = ScalarField::zeros(desc.clone());

    let primary = skeleton.volcanic_centers.first();

    for z in 0..desc.height {
        for x in 0..desc.width {
            let wx = desc.origin_x() + x as f64 * desc.cell_size_m;
            let wz = desc.origin_z() + z as f64 * desc.cell_size_m;
            let world = WorldXZ::new(wx, wz);
            let inf = influence.sample_at_world(world);
            let age = age_field.sample_at_world(world);
            let coast_d = coast.sample_at_world(world).max(0.0);

            let rock = if inf < 0.05 {
                BedrockId::Ocean
            } else if age < recipe.weathering_age_threshold_myr {
                BedrockId::Basalt
            } else if coast_d < recipe.coastal_weathering_band_m {
                BedrockId::WeatheredBasalt
            } else if inf > 0.7 && age < recipe.tuff_age_threshold_myr {
                BedrockId::Tuff
            } else {
                BedrockId::WeatheredBasalt
            };

            let props = rock.default_properties();
            let age_factor = (age / 10.0).clamp(0.0, 1.0);
            let h = props.hardness * (1.0 - age_factor * 0.3);
            let e = props.erodibility * (1.0 + age_factor * 0.4);

            bedrock.set(x, z, rock);
            hardness.set(x, z, h);
            erodibility.set(x, z, e);
            permeability.set(x, z, props.permeability);
            fracture.set(x, z, fracture_at(world, skeleton, inf, world_seed));

            if let Some(center) = primary {
                let dx = world.x() - center.position.x();
                let dz = world.z() - center.position.z();
                let dist = (dx * dx + dz * dz).sqrt();
                if dist < center.radius_m as f64 * 0.5 {
                    value_constraint.set(x, z, 0.85);
                    gradient_constraint.set(x, z, 0.7);
                }
            }
            if inf > 0.02 && inf < 0.15 {
                gradient_constraint.set(x, z, gradient_constraint.get(x, z).max(0.6));
            }
        }
    }

    atlas.fields.insert_scalar(Fk::RockHardness, hardness);
    atlas.fields.insert_scalar(Fk::Erodibility, erodibility);
    atlas.fields.insert_scalar(Fk::Permeability, permeability);
    atlas.fields.insert_scalar(Fk::FractureIntensity, fracture);
    atlas
        .fields
        .insert_scalar(Fk::ValueConstraint, value_constraint);
    atlas
        .fields
        .insert_scalar(Fk::GradientConstraint, gradient_constraint);
    atlas.fields.insert_categorical(Fk::Bedrock, bedrock);
}

fn fracture_at(world: WorldXZ, skeleton: &IslandSkeleton, influence: f32, world_seed: u64) -> f32 {
    if influence < 0.1 {
        return 0.0;
    }
    let mut fracture = 0.1f32;
    for ridge in &skeleton.ridges {
        let dx = world.x() - ridge.origin.x();
        let dz = world.z() - ridge.origin.z();
        let along = dx * ridge.direction.x + dz * ridge.direction.y;
        let across = (-dx * ridge.direction.y + dz * ridge.direction.x).abs();
        if along > 0.0 && along < ridge.length_m as f64 && across < ridge.width_m as f64 {
            fracture = fracture.max(0.6);
        }
    }
    let _ = derive_seed(world_seed, "fracture", None, 0);
    fracture
}
