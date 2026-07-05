//! Macro elevation generation without local noise.

use glam::DVec2;

use crate::compiler::context::CompileContext;
use crate::compiler::error::WorldgenError;
use crate::compiler::pass::{PassKey, WorldgenPass};
use crate::compiler::report::PassReport;
use crate::contract::coordinates::WorldXZ;
use crate::fields::key::FieldKey;
use crate::fields::key::FieldKey as Fk;
use crate::fields::scalar::ScalarField;
use crate::islands::skeleton::IslandSkeleton;

pub struct MacroTerrainPass;

impl WorldgenPass for MacroTerrainPass {
    fn key(&self) -> PassKey {
        PassKey::MacroTerrain
    }

    fn inputs(&self) -> &'static [FieldKey] {
        &[FieldKey::IslandInfluence, FieldKey::OceanBasin]
    }

    fn outputs(&self) -> &'static [FieldKey] {
        &[FieldKey::BaseElevation]
    }

    fn run(&self, ctx: &mut CompileContext) -> Result<PassReport, WorldgenError> {
        let start = std::time::Instant::now();
        let skeleton = ctx
            .skeleton
            .as_ref()
            .ok_or(WorldgenError::MissingPrerequisite {
                pass: PassKey::MacroTerrain,
                missing: "island skeleton",
            })?;
        let island_recipe = &ctx.recipe.islands[0];

        let influence = ctx
            .atlas
            .fields
            .get_scalar(Fk::IslandInfluence)
            .expect("island influence");
        let ocean = ctx
            .atlas
            .fields
            .get_scalar(Fk::OceanBasin)
            .expect("ocean basin");

        let desc = ctx.atlas.control_descriptor.clone();
        let mut base = ScalarField::zeros(desc.clone());

        for z in 0..desc.height {
            for x in 0..desc.width {
                let wx = desc.origin_x() + x as f64 * desc.cell_size_m;
                let wz = desc.origin_z() + z as f64 * desc.cell_size_m;
                let world = WorldXZ::new(wx, wz);
                let inf = influence.sample_at_world(world);
                let ocean_h = ocean.sample_at_world(world);

                let land_h = if inf > 0.02 {
                    macro_land_elevation(world, inf, skeleton, island_recipe)
                } else {
                    ocean_h
                };

                let blend = smoothstep(0.0, 0.15, inf);
                let h = ocean_h * (1.0 - blend) + land_h * blend;
                base.set(x, z, h);
            }
        }

        ctx.atlas.fields.insert_scalar(Fk::BaseElevation, base);

        Ok(PassReport {
            pass: PassKey::MacroTerrain,
            elapsed: start.elapsed(),
            seed: ctx.recipe.seed,
            outputs: self.outputs().to_vec(),
            metrics: Default::default(),
            warnings: vec![],
        })
    }
}

fn macro_land_elevation(
    world: WorldXZ,
    influence: f32,
    skeleton: &IslandSkeleton,
    island_recipe: &game_data::CompiledIslandRecipe,
) -> f32 {
    let mut h = 0.0f32;
    let footprint_base = influence.max(0.0).powf(0.7)
        * island_recipe.volcano.peak_height_m
        * 0.15
        * island_recipe.uplift;

    for center in &skeleton.volcanic_centers {
        let dx = world.x() - center.position.x();
        let dz = world.z() - center.position.z();
        let dist = (dx * dx + dz * dz).sqrt() as f32;
        let t = (1.0 - dist / center.radius_m).clamp(0.0, 1.0);
        h = h.max(center.target_height_m * t.powf(1.5));
    }

    for ridge in &skeleton.ridges {
        let offset = DVec2::new(world.x() - ridge.origin.x(), world.z() - ridge.origin.z());
        let along = (offset.x * ridge.direction.x + offset.y * ridge.direction.y) as f32;
        let across = (-offset.x * ridge.direction.y + offset.y * ridge.direction.x).abs() as f32;
        if along > 0.0 && along < ridge.length_m && across < ridge.width_m {
            let t = (1.0 - across / ridge.width_m).clamp(0.0, 1.0);
            h = h.max(ridge.height_m * t);
        }
    }

    for caldera in &skeleton.calderas {
        let dx = world.x() - caldera.center.x();
        let dz = world.z() - caldera.center.z();
        let dist = (dx * dx + dz * dz).sqrt() as f32;
        if dist < caldera.radius_m {
            let t = (1.0 - dist / caldera.radius_m).clamp(0.0, 1.0);
            h -= caldera.depth_m * t;
        }
    }

    h.max(footprint_base)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge1 <= edge0 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
