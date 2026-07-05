//! Island footprint as warped distance influence field.

use glam::DVec2;

use crate::contract::coordinates::WorldXZ;
use crate::contract::version::derive_seed;
use crate::fields::descriptor::FieldDescriptor;
use crate::fields::scalar::ScalarField;
use crate::islands::seed::IslandSeed;
use crate::noise::ValueNoise;

pub fn rotate_into_island_space(offset: DVec2, rotation_rad: f32) -> DVec2 {
    let c = rotation_rad.cos() as f64;
    let s = rotation_rad.sin() as f64;
    DVec2::new(offset.x * c - offset.y * s, offset.x * s + offset.y * c)
}

pub fn ellipse_influence(local: DVec2, major_m: f32, minor_m: f32) -> f32 {
    let normalized = DVec2::new(local.x / major_m as f64, local.y / minor_m as f64);
    let r = normalized.length();
    1.0 - r as f32
}

pub fn warp_offset(world: WorldXZ, seed: &IslandSeed, world_seed: u64) -> DVec2 {
    if seed.warp_amplitude_m <= 0.0 {
        return DVec2::ZERO;
    }
    let noise_seed = derive_seed(world_seed, "footprint_warp", None, seed.id as u64);
    let noise = ValueNoise::new(noise_seed);
    let scale = seed.warp_wavelength_m.max(1.0);
    let wx = world.x() as f32 / scale;
    let wz = world.z() as f32 / scale;
    let nx = noise.sample(wx, 0.0, wz) * 2.0 - 1.0;
    let nz = noise.sample(wx + 100.0, 0.0, wz + 100.0) * 2.0 - 1.0;
    DVec2::new(
        nx as f64 * seed.warp_amplitude_m as f64,
        nz as f64 * seed.warp_amplitude_m as f64,
    )
}

pub fn influence_at(world: WorldXZ, seed: &IslandSeed, world_seed: u64) -> f32 {
    let warp = warp_offset(world, seed, world_seed);
    let warped = WorldXZ::new(world.x() + warp.x, world.z() + warp.y);
    let offset = DVec2::new(warped.x() - seed.center.x(), warped.z() - seed.center.z());
    let local = rotate_into_island_space(offset, -seed.rotation_rad);
    ellipse_influence(local, seed.major_radius_m, seed.minor_radius_m)
}

pub fn generate_influence_field(
    descriptor: FieldDescriptor,
    seed: &IslandSeed,
    world_seed: u64,
) -> ScalarField {
    let mut field = ScalarField::zeros(descriptor);
    for z in 0..field.descriptor.height {
        for x in 0..field.descriptor.width {
            let wx = field.descriptor.origin_x() + x as f64 * field.descriptor.cell_size_m;
            let wz = field.descriptor.origin_z() + z as f64 * field.descriptor.cell_size_m;
            let v = influence_at(WorldXZ::new(wx, wz), seed, world_seed);
            field.set(x, z, v);
        }
    }
    field
}

pub fn generate_island_id_field(descriptor: FieldDescriptor, seed: &IslandSeed) -> ScalarField {
    let mut field = ScalarField::zeros(descriptor);
    for z in 0..field.descriptor.height {
        for x in 0..field.descriptor.width {
            let wx = field.descriptor.origin_x() + x as f64 * field.descriptor.cell_size_m;
            let wz = field.descriptor.origin_z() + z as f64 * field.descriptor.cell_size_m;
            let inf = influence_at(WorldXZ::new(wx, wz), seed, 0);
            field.set(x, z, if inf > 0.0 { seed.id as f32 } else { 0.0 });
        }
    }
    field
}

pub fn generate_age_field(descriptor: FieldDescriptor, seed: &IslandSeed) -> ScalarField {
    let mut field = ScalarField::zeros(descriptor);
    for z in 0..field.descriptor.height {
        for x in 0..field.descriptor.width {
            let wx = field.descriptor.origin_x() + x as f64 * field.descriptor.cell_size_m;
            let wz = field.descriptor.origin_z() + z as f64 * field.descriptor.cell_size_m;
            let inf = influence_at(WorldXZ::new(wx, wz), seed, 0);
            field.set(x, z, if inf > 0.0 { seed.age_myr } else { 0.0 });
        }
    }
    field
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_data::{CompiledFootprint, CompiledIslandRecipe, CompiledVolcano};

    fn test_seed() -> IslandSeed {
        let recipe = CompiledIslandRecipe {
            id: "test".into(),
            island_index: 0,
            center_x_m: 0.0,
            center_z_m: 0.0,
            age_myr: 2.0,
            uplift: 1.0,
            volcanic_activity: 1.0,
            footprint: CompiledFootprint {
                major_radius_m: 5000.0,
                minor_radius_m: 4000.0,
                rotation_rad: 0.0,
                warp_amplitude_m: 200.0,
                warp_wavelength_m: 3000.0,
            },
            volcano: CompiledVolcano {
                peak_height_m: 1200.0,
                shield_radius_m: 8000.0,
                caldera_radius_m: 400.0,
                caldera_depth_m: 80.0,
                secondary_vents: 0,
                ridge_count: 2,
            },
        };
        IslandSeed::from_compiled(&recipe, 42)
    }

    #[test]
    fn center_has_positive_influence() {
        let seed = test_seed();
        let inf = influence_at(WorldXZ::new(0.0, 0.0), &seed, 42);
        assert!(inf > 0.5);
    }

    #[test]
    fn far_field_is_negative() {
        let seed = test_seed();
        let inf = influence_at(WorldXZ::new(20000.0, 0.0), &seed, 42);
        assert!(inf < 0.0);
    }
}
