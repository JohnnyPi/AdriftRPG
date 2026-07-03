// crates/terrain_generation/src/spawn.rs
//! Player spawn resolution and object placement against terrain-only density.

use crate::recipe::RecipeDensitySource;

/// Default vertical clearance required above a terrain floor for player spawn.
pub const PLAYER_SPAWN_MIN_CLEARANCE_M: f32 = 2.0;

/// Foot offset above the resolved terrain floor.
pub const SPAWN_FLOOR_EPSILON_M: f32 = 0.05;

#[derive(Clone, Debug, Default)]
pub struct SpawnValidationReport {
    pub passed: bool,
    pub messages: Vec<String>,
    pub foot_x: f32,
    pub foot_y: f32,
    pub foot_z: f32,
}

impl RecipeDensitySource {
    /// Resolve player spawn on natural terrain, ignoring union recipe objects (pads, platforms).
    pub fn resolve_player_spawn(
        &self,
        min_clearance: f32,
        search_radius_m: f32,
    ) -> (f32, f32, f32, SpawnValidationReport) {
        let recipe = self.recipe();
        let world_x = recipe.spawn_x - recipe.coord_offset[0];
        let world_z = recipe.spawn_z - recipe.coord_offset[2];
        let mut report = SpawnValidationReport {
            foot_x: world_x,
            foot_y: recipe.sea_level,
            foot_z: world_z,
            ..Default::default()
        };

        if let Some((x, y, z, local)) = self.find_valid_spawn_near(world_x, world_z, min_clearance, 0.0)
        {
            report.passed = true;
            report.foot_x = x;
            report.foot_y = y;
            report.foot_z = z;
            report.messages = local.messages;
            return (x, y + SPAWN_FLOOR_EPSILON_M, z, report);
        }

        let mut best: Option<(f32, f32, f32, f32)> = None;
        let step = 4.0f32;
        let mut radius = step;
        while radius <= search_radius_m {
            let samples = ring_samples(world_x, world_z, radius, step);
            for (cx, cz) in samples {
                if let Some((x, y, z, local)) =
                    self.find_valid_spawn_near(cx, cz, min_clearance, radius)
                {
                    if local.passed {
                        let dist = ((x - world_x).powi(2) + (z - world_z).powi(2)).sqrt();
                        match best {
                            None => best = Some((dist, x, y, z)),
                            Some((best_dist, ..)) if dist < best_dist => {
                                best = Some((dist, x, y, z))
                            }
                            _ => {}
                        }
                    }
                }
            }
            radius += step;
        }

        if let Some((_dist, x, y, z)) = best {
            report.passed = true;
            report.foot_x = x;
            report.foot_y = y;
            report.foot_z = z;
            report.messages.push(format!(
                "Spawn relocated to terrain near authored point ({x:.1}, {y:.1}, {z:.1})"
            ));
            return (x, y + SPAWN_FLOOR_EPSILON_M, z, report);
        }

        report.passed = false;
        report.messages.push(
            "No valid terrain spawn within search radius (need walkable ground with clearance)"
                .into(),
        );
        let fallback_y = recipe.sea_level + min_clearance + 2.0;
        (world_x, fallback_y, world_z, report)
    }

    fn find_valid_spawn_near(
        &self,
        world_x: f32,
        world_z: f32,
        min_clearance: f32,
        search_radius_m: f32,
    ) -> Option<(f32, f32, f32, SpawnValidationReport)> {
        let recipe = self.recipe();
        let max_y = self
            .terrain_surface_height_at(world_x, world_z)
            .max(recipe.sea_level + min_clearance + 4.0);
        let floor = self.walkable_terrain_floor_at(world_x, world_z, max_y, min_clearance)?;

        if floor < recipe.sea_level + 1.5 {
            return None;
        }

        if !self.is_outdoor_terrain_spawn(world_x, floor, world_z, min_clearance) {
            return None;
        }

        if !self.has_terrain_support_below(world_x, floor, world_z, 2.0) {
            return None;
        }

        if self.column_fully_embedded_in_terrain(world_x, world_z, floor, floor + min_clearance) {
            return None;
        }

        let mut messages = vec![format!("Outdoor terrain floor at y={floor:.1}: OK")];
        if search_radius_m > 0.0 {
            messages.push(format!(
                "Resolved within {search_radius_m:.0} m of authored spawn"
            ));
        }
        Some((
            world_x,
            floor,
            world_z,
            SpawnValidationReport {
                passed: true,
                messages,
                foot_x: world_x,
                foot_y: floor,
                foot_z: world_z,
            },
        ))
    }

    /// True when the foot position is on open natural terrain (not inside authored caves).
    pub fn is_outdoor_terrain_spawn(
        &self,
        world_x: f32,
        foot_y: f32,
        world_z: f32,
        min_clearance: f32,
    ) -> bool {
        use crate::topology::outside_declared_cavities;
        let recipe = self.recipe();
        let rx = world_x + recipe.coord_offset[0];
        let rz = world_z + recipe.coord_offset[2];
        if !outside_declared_cavities(recipe, rx, foot_y + 1.0, rz) {
            return false;
        }
        self.terrain_clearance_above_floor(world_x, foot_y, world_z) >= min_clearance
    }

    /// True when the composite surface is substantially above terrain due to a union recipe object.
    pub fn is_foot_on_recipe_object(&self, world_x: f32, foot_y: f32, world_z: f32) -> bool {
        let terrain = self.terrain_surface_height_at(world_x, world_z);
        let composite = self.surface_height_at(world_x, world_z);
        composite > terrain + 0.35 && (foot_y - terrain).abs() < 0.5
    }

    /// Whether an axis-aligned box is entirely inside solid terrain (intersection is allowed).
    pub fn is_aabb_fully_embedded_in_terrain(
        &self,
        center_x: f32,
        center_y: f32,
        center_z: f32,
        half_extents: [f32; 3],
    ) -> bool {
        let hx = half_extents[0];
        let hy = half_extents[1];
        let hz = half_extents[2];
        for &dx in &[-1.0, 1.0] {
            for &dy in &[-1.0, 1.0] {
                for &dz in &[-1.0, 1.0] {
                    let x = center_x + dx * hx;
                    let y = center_y + dy * hy;
                    let z = center_z + dz * hz;
                    if self.terrain_density_at(x, y, z) > 0.0 {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn column_fully_embedded_in_terrain(
        &self,
        world_x: f32,
        world_z: f32,
        y_min: f32,
        y_max: f32,
    ) -> bool {
        let mut y = y_min;
        while y <= y_max {
            if self.terrain_density_at(world_x, y, world_z) > 0.0 {
                return false;
            }
            y += 0.5;
        }
        true
    }

    /// Snap an object's center onto terrain; returns `None` if fully embedded or no floor.
    pub fn snap_object_center_to_terrain(
        &self,
        world_x: f32,
        world_z: f32,
        half_height: f32,
        search_y: f32,
    ) -> Option<f32> {
        let floor = self.walkable_terrain_floor_at(
            world_x,
            world_z,
            search_y.max(self.terrain_surface_height_at(world_x, world_z) + 4.0),
            0.25,
        )?;
        let center_y = floor + half_height;
        if self.is_aabb_fully_embedded_in_terrain(world_x, center_y, world_z, [0.5, half_height, 0.5])
        {
            return None;
        }
        Some(center_y)
    }
}

fn ring_samples(cx: f32, cz: f32, radius: f32, step: f32) -> Vec<(f32, f32)> {
    let circumference = std::f32::consts::TAU * radius;
    let count = ((circumference / step).ceil() as u32).max(8);
    (0..count)
        .map(|i| {
            let t = i as f32 / count as f32 * std::f32::consts::TAU;
            (cx + t.cos() * radius, cz + t.sin() * radius)
        })
        .collect()
}

#[cfg(test)]
mod spawn_tests {
    use crate::{default_vertical_slice_recipe, RecipeDensitySource};

    #[test]
    fn embedded_probe_is_detected() {
        let source = RecipeDensitySource::new(default_vertical_slice_recipe(1, 0.0));
        let wx = source.recipe().spawn_x - source.recipe().coord_offset[0];
        let wz = source.recipe().spawn_z - source.recipe().coord_offset[2];
        let floor = source.terrain_surface_height_at(wx, wz);
        assert!(!source.is_aabb_fully_embedded_in_terrain(
            wx,
            floor + 1.0,
            wz,
            [0.45, 0.45, 0.45]
        ));
        assert!(source.is_aabb_fully_embedded_in_terrain(
            wx,
            floor - 3.0,
            wz,
            [0.45, 0.45, 0.45]
        ));
    }
}
