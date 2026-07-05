//! Island seed descriptors and placement.

use game_data::CompiledIslandRecipe;
use serde::{Deserialize, Serialize};

use crate::contract::coordinates::WorldXZ;
use crate::contract::version::derive_seed;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IslandSeed {
    pub id: u32,
    pub center: WorldXZ,
    pub rotation_rad: f32,
    pub major_radius_m: f32,
    pub minor_radius_m: f32,
    pub age_myr: f32,
    pub uplift: f32,
    pub volcanic_activity: f32,
    pub root_seed: u64,
    pub warp_amplitude_m: f32,
    pub warp_wavelength_m: f32,
}

impl IslandSeed {
    pub fn from_compiled(recipe: &CompiledIslandRecipe, world_seed: u64) -> Self {
        Self {
            id: recipe.island_index,
            center: WorldXZ::new(recipe.center_x_m, recipe.center_z_m),
            rotation_rad: recipe.footprint.rotation_rad,
            major_radius_m: recipe.footprint.major_radius_m,
            minor_radius_m: recipe.footprint.minor_radius_m,
            age_myr: recipe.age_myr,
            uplift: recipe.uplift,
            volcanic_activity: recipe.volcanic_activity,
            root_seed: derive_seed(world_seed, "island_seed", None, recipe.island_index as u64),
            warp_amplitude_m: recipe.footprint.warp_amplitude_m,
            warp_wavelength_m: recipe.footprint.warp_wavelength_m,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IslandBlueprint {
    pub seed: IslandSeed,
    pub skeleton: super::skeleton::IslandSkeleton,
}
