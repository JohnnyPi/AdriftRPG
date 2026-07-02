// crates/terrain_generation/src/topology.rs
//! Topological validation helpers for signed-density terrain.

use crate::recipe::{CombineOp, RecipeDensitySource, RecipeOp, TerrainRecipe};
use crate::{capsule_sdf, ellipsoid_sdf, plane_density, solid_union, ValueNoise};

/// Meters of solid guaranteed below the coastal surface outside intentional cavities.
pub const FOUNDATION_DEPTH_M: f32 = 14.0;

/// SDF margin: treat points with cavity SDF above this as outside caves (eligible for sealing).
pub const CAVITY_EXTERIOR_MARGIN: f32 = 0.35;

/// Coastal surface height including valley and coast modifiers.
pub fn coastal_surface_height(recipe: &TerrainRecipe, x: f32, z: f32) -> f32 {
    crate::surface_height::land_surface_height(recipe, x, z)
}

/// Minimum SDF across all subtract primitives (negative = inside a declared cavity).
pub fn cavity_sdf_at(recipe: &TerrainRecipe, x: f32, y: f32, z: f32) -> f32 {
    let noise = ValueNoise::new(recipe.seed);
    let mut min_sdf = f32::MAX;
    for op in &recipe.ops {
        let sdf = match op {
            RecipeOp::Ellipsoid {
                center,
                radii,
                peak_noise,
                combine: CombineOp::Subtract,
            } => {
                let mut cy = center[1];
                if let Some((freq, amp)) = peak_noise {
                    cy += (noise.sample(x * freq, 0.0, z * freq) - 0.5) * amp;
                }
                ellipsoid_sdf(x, y, z, center[0], cy, center[2], radii[0], radii[1], radii[2])
            }
            RecipeOp::Capsule {
                start,
                end,
                radius,
                combine: CombineOp::Subtract,
            } => capsule_sdf(
                x, y, z, start[0], start[1], start[2], end[0], end[1], end[2], *radius,
            ),
            _ => continue,
        };
        min_sdf = min_sdf.min(sdf);
    }
    min_sdf
}

/// True when `(x, y, z)` lies outside all declared subtract cavities.
pub fn outside_declared_cavities(recipe: &TerrainRecipe, x: f32, y: f32, z: f32) -> bool {
    cavity_sdf_at(recipe, x, y, z) > -CAVITY_EXTERIOR_MARGIN
}

/// Apply bedrock seal so outdoor columns cannot breach below the surface foundation.
pub fn apply_foundation_seal(recipe: &TerrainRecipe, x: f32, y: f32, z: f32, density: f32) -> f32 {
    apply_foundation_seal_at(
        recipe,
        x,
        y,
        z,
        density,
        coastal_surface_height(recipe, x, z),
    )
}

/// Seal against an explicit surface height (atlas or recipe coastal surface).
pub fn apply_foundation_seal_at(
    recipe: &TerrainRecipe,
    x: f32,
    y: f32,
    z: f32,
    density: f32,
    surface_y: f32,
) -> f32 {
    if !outside_declared_cavities(recipe, x, y, z) {
        return density;
    }
    if let Some(factor) = crate::recipe::island_land_factor_from_recipe(recipe, x, z) {
        if factor <= 0.0 {
            return density;
        }
    }
    let foundation = plane_density(y, surface_y - FOUNDATION_DEPTH_M);
    solid_union(density, foundation)
}

/// Count columns in a region where outdoor void exists between bedrock and the coastal surface.
pub fn count_outdoor_void_columns(
    source: &RecipeDensitySource,
    x_min: f32,
    x_max: f32,
    z_min: f32,
    z_max: f32,
    step: f32,
) -> usize {
    let recipe = source.recipe();
    let mut violations = 0usize;
    let mut x = x_min;
    while x <= x_max {
        let mut z = z_min;
        while z <= z_max {
            let rx = x + recipe.coord_offset[0];
            let rz = z + recipe.coord_offset[2];
            let surface = coastal_surface_height(recipe, rx, rz);
            if surface < recipe.sea_level {
                z += step;
                continue;
            }
            let bedrock_top = surface - FOUNDATION_DEPTH_M;
            let mut y = bedrock_top.max(recipe.sea_level - 4.0);
            let mut had_solid = false;
            let mut void_to_surface = false;
            while y <= surface + 1.0 {
                if outside_declared_cavities(recipe, rx, y, rz) {
                    let d = source.density_at(x, y, z);
                    if d <= 0.0 {
                        had_solid = true;
                        void_to_surface = false;
                    } else if had_solid {
                        void_to_surface = true;
                    }
                } else {
                    had_solid = false;
                }
                y += 0.5;
            }
            if void_to_surface {
                violations += 1;
            }
            z += step;
        }
        x += step;
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipsoid_cavity_margin_is_metric() {
        let recipe = TerrainRecipe {
            seed: 1,
            sea_level: 2.0,
            spawn_x: 0.0,
            spawn_z: 0.0,
            coord_offset: [0.0; 3],
            ops: vec![RecipeOp::Ellipsoid {
                center: [64.0, 10.0, 64.0],
                radii: [20.0, 20.0, 20.0],
                peak_noise: None,
                combine: CombineOp::Subtract,
            }],
        };
        let cy = 10.0;
        let ry = 20.0;
        let half_meter_inside = cavity_sdf_at(&recipe, 64.0, cy + ry - 0.5, 64.0);
        let just_outside_wall = cavity_sdf_at(&recipe, 64.0, cy + ry + 0.2, 64.0);
        assert!(
            half_meter_inside < -CAVITY_EXTERIOR_MARGIN,
            "0.5 m inside wall should be exempt from sealing (sdf={half_meter_inside})"
        );
        assert!(
            just_outside_wall > -CAVITY_EXTERIOR_MARGIN,
            "just outside wall should be eligible for sealing (sdf={just_outside_wall})"
        );
        assert!(
            half_meter_inside > -0.6 && half_meter_inside < -0.4,
            "metric SDF should read ~-0.5 m inside the wall (sdf={half_meter_inside})"
        );
    }
}
