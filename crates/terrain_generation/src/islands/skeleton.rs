//! Volcanic skeleton graph — semantic island form, not terrain yet.

use glam::DVec2;
use serde::{Deserialize, Serialize};

use game_data::CompiledIslandRecipe;

use crate::contract::coordinates::WorldXZ;
use crate::contract::version::derive_seed;
use crate::islands::seed::IslandSeed;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IslandSkeleton {
    pub volcanic_centers: Vec<VolcanicCenter>,
    pub ridges: Vec<StructuralRidge>,
    pub calderas: Vec<CalderaDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VolcanicCenter {
    pub id: u32,
    pub position: WorldXZ,
    pub age_myr: f32,
    pub radius_m: f32,
    pub target_height_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StructuralRidge {
    pub origin: WorldXZ,
    pub direction: DVec2,
    pub length_m: f32,
    pub width_m: f32,
    pub height_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalderaDescriptor {
    pub center: WorldXZ,
    pub radius_m: f32,
    pub depth_m: f32,
}

pub fn build_skeleton(
    seed: &IslandSeed,
    recipe: &CompiledIslandRecipe,
    world_seed: u64,
) -> IslandSkeleton {
    let mut volcanic_centers = vec![VolcanicCenter {
        id: 0,
        position: seed.center,
        age_myr: seed.age_myr,
        radius_m: recipe.volcano.shield_radius_m,
        target_height_m: recipe.volcano.peak_height_m,
    }];

    for i in 0..recipe.volcano.secondary_vents {
        let local_seed = derive_seed(world_seed, "secondary_vent", None, i as u64 + 1);
        let angle = (local_seed as f64 / u64::MAX as f64) * std::f64::consts::TAU;
        let dist = recipe.volcano.shield_radius_m * 0.35;
        volcanic_centers.push(VolcanicCenter {
            id: i + 1,
            position: WorldXZ::new(
                seed.center.x() + angle.cos() * dist as f64,
                seed.center.z() + angle.sin() * dist as f64,
            ),
            age_myr: seed.age_myr * 0.8,
            radius_m: recipe.volcano.shield_radius_m * 0.25,
            target_height_m: recipe.volcano.peak_height_m * 0.4,
        });
    }

    let mut ridges = Vec::new();
    for i in 0..recipe.volcano.ridge_count.max(1) {
        let local_seed = derive_seed(world_seed, "ridge", None, i as u64);
        let angle = (local_seed as f64 / u64::MAX as f64) * std::f64::consts::TAU;
        ridges.push(StructuralRidge {
            origin: seed.center,
            direction: DVec2::new(angle.cos(), angle.sin()).normalize(),
            length_m: seed.major_radius_m * 0.6,
            width_m: seed.major_radius_m * 0.08,
            height_m: recipe.volcano.peak_height_m * 0.25,
        });
    }

    let calderas = if recipe.volcano.caldera_radius_m > 0.0 {
        vec![CalderaDescriptor {
            center: seed.center,
            radius_m: recipe.volcano.caldera_radius_m,
            depth_m: recipe.volcano.caldera_depth_m,
        }]
    } else {
        vec![]
    };

    IslandSkeleton {
        volcanic_centers,
        ridges,
        calderas,
    }
}
