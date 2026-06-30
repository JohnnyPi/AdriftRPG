use crate::density_ops::{capsule_sdf, ellipsoid_sdf, plane_density, solid_subtract, solid_union};
use crate::noise::ValueNoise;
use crate::river::{river_carve_offset, river_channel_at};
use crate::surface_height::{island_land_factor_warped, land_surface_height};
use crate::topology::apply_foundation_seal;
use crate::water_body::RiverSpline;
use crate::DensitySource;
use crate::surface_height::CoastModifierKind;

/// Portable terrain recipe evaluated at runtime (may originate from YAML).
#[derive(Clone, Debug)]
pub struct TerrainRecipe {
    pub seed: u64,
    pub sea_level: f32,
    pub spawn_x: f32,
    pub spawn_z: f32,
    /// Added to world X/Y/Z before evaluating authored recipe coordinates.
    pub coord_offset: [f32; 3],
    pub ops: Vec<RecipeOp>,
}

#[derive(Clone, Debug)]
pub enum RecipeOp {
    CoastalSurface {
        origin: [f32; 2],
        scale: [f32; 2],
        base_height: f32,
        height_range: f32,
        ridge_origin: [f32; 2],
        ridge_scale: [f32; 2],
        ridge_amplitude: f32,
        detail_frequency: f32,
        detail_amplitude: f32,
        detail_octaves: u32,
        regional_frequency: f32,
        regional_amplitude: f32,
        local_frequency: f32,
        local_amplitude: f32,
        ridged_amplitude: f32,
        domain_warp: f32,
    },
    ValleyBasin {
        origin: [f32; 2],
        scale: [f32; 2],
        depth_m: f32,
    },
    CoastModifier {
        kind: CoastModifierKind,
        center: [f32; 2],
        radius_m: f32,
        depth_m: f32,
        min_land_factor: f32,
        max_land_factor: f32,
    },
    Ellipsoid {
        center: [f32; 3],
        radii: [f32; 3],
        peak_noise: Option<(f32, f32)>,
        combine: CombineOp,
    },
    Capsule {
        start: [f32; 3],
        end: [f32; 3],
        radius: f32,
        combine: CombineOp,
    },
    NoisePerturb {
        scale: f32,
        amplitude: f32,
        density_min: f32,
        density_max: f32,
    },
    IslandMask {
        center: [f32; 2],
        radius_m: f32,
        falloff_m: f32,
        ocean_floor_y: f32,
        domain_warp: f32,
    },
    OceanFloor {
        origin: [f32; 2],
        scale: [f32; 2],
        base_depth_m: f32,
        variation_m: f32,
        detail_frequency: f32,
        detail_octaves: u32,
    },
    MountainPeak {
        center: [f32; 2],
        base_elevation_m: f32,
        base_radius_m: f32,
        peak_height_m: f32,
        steepness: f32,
        peak_noise: Option<(f32, f32)>,
    },
    UnderwaterTrench {
        points: Vec<[f32; 3]>,
        width_m: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombineOp {
    Union,
    Subtract,
}

#[derive(Clone, Debug)]
pub struct RiverCarveContext {
    pub spline: RiverSpline,
    pub bank_width_m: f32,
}

#[derive(Clone, Debug)]
pub struct RecipeDensitySource {
    recipe: TerrainRecipe,
    river_carve: Option<RiverCarveContext>,
}

/// Coastal inland factor from the first `CoastalSurface` op: 0 at the shore, 1 inland.
pub fn coastal_inland_factor(recipe: &TerrainRecipe, x: f32, z: f32) -> f32 {
    for op in &recipe.ops {
        if let RecipeOp::CoastalSurface { origin, scale, .. } = op {
            let nx = (x + origin[0]) / scale[0];
            let nz = (z + origin[1]) / scale[1];
            let coast = 1.0 - (nx * 0.6 + (1.0 - nz) * 0.4).clamp(0.0, 1.0);
            return coast;
        }
    }
    1.0
}

/// Approximate horizontal distance to the recipe coastline in meters.
pub fn distance_to_water_m(recipe: &TerrainRecipe, x: f32, z: f32) -> f32 {
    let inland = coastal_inland_factor(recipe, x, z);
    let max_distance = recipe
        .ops
        .iter()
        .find_map(|op| {
            if let RecipeOp::CoastalSurface { scale, .. } = op {
                Some(scale[0].max(scale[1]))
            } else {
                None
            }
        })
        .unwrap_or(96.0);
    (1.0 - inland) * max_distance
}

/// Horizontal distance to the nearest river channel centerline in recipe space.
pub fn distance_to_river_m(source: &RecipeDensitySource, x: f32, z: f32) -> f32 {
    if let Some(ref river) = source.river_carve {
        let (dist, _, _) = river_channel_at(&river.spline, x, z);
        dist
    } else {
        f32::MAX
    }
}

impl RecipeDensitySource {
    pub fn new(recipe: TerrainRecipe) -> Self {
        Self {
            recipe,
            river_carve: None,
        }
    }

    pub fn with_river_carve(mut self, ctx: RiverCarveContext) -> Self {
        self.river_carve = Some(ctx);
        self
    }

    pub fn river_carve(&self) -> Option<&RiverCarveContext> {
        self.river_carve.as_ref()
    }

    pub fn recipe(&self) -> &TerrainRecipe {
        &self.recipe
    }

    pub fn coastal_inland_factor(&self, world_x: f32, world_z: f32) -> f32 {
        coastal_inland_factor(
            &self.recipe,
            world_x + self.recipe.coord_offset[0],
            world_z + self.recipe.coord_offset[2],
        )
    }

    pub fn distance_to_water_m(&self, world_x: f32, world_z: f32) -> f32 {
        distance_to_water_m(
            &self.recipe,
            world_x + self.recipe.coord_offset[0],
            world_z + self.recipe.coord_offset[2],
        )
    }

    pub fn distance_to_river_m(&self, world_x: f32, world_z: f32) -> f32 {
        distance_to_river_m(
            self,
            world_x + self.recipe.coord_offset[0],
            world_z + self.recipe.coord_offset[2],
        )
    }

    pub fn density_at(&self, world_x: f32, world_y: f32, world_z: f32) -> f32 {
        self.density_at_recipe(
            world_x + self.recipe.coord_offset[0],
            world_y + self.recipe.coord_offset[1],
            world_z + self.recipe.coord_offset[2],
        )
    }

    /// Sample density using authored recipe-space coordinates (for tests and tooling).
    pub fn density_at_recipe(&self, x: f32, y: f32, z: f32) -> f32 {
        let noise = ValueNoise::new(self.recipe.seed);
        let island = self
            .recipe
            .ops
            .iter()
            .find_map(|op| {
                if let RecipeOp::IslandMask {
                    center,
                    radius_m,
                    falloff_m,
                    ocean_floor_y,
                    domain_warp,
                } = op
                {
                    Some((*center, *radius_m, *falloff_m, *domain_warp, *ocean_floor_y))
                } else {
                    None
                }
            });
        let ocean_floor = self.recipe.ops.iter().find_map(|op| {
            if let RecipeOp::OceanFloor {
                origin,
                scale,
                base_depth_m,
                variation_m,
                detail_frequency,
                detail_octaves,
            } = op
            {
                Some((
                    *origin,
                    *scale,
                    *base_depth_m,
                    *variation_m,
                    *detail_frequency,
                    *detail_octaves,
                ))
            } else {
                None
            }
        });

        let land_factor = island.as_ref().map(|(center, radius, falloff, domain_warp, _)| {
            let (wx, wz) = if *domain_warp > 0.0 {
                let ox = noise.fbm(x * domain_warp, 0.0, z * domain_warp, 2, 2.0, 0.5) - 0.5;
                let oz = noise.fbm(x * domain_warp + 100.0, 0.0, z * domain_warp, 2, 2.0, 0.5) - 0.5;
                (
                    x + ox * 30.0 * domain_warp,
                    z + oz * 30.0 * domain_warp,
                )
            } else {
                (x, z)
            };
            island_land_factor(wx, wz, *center, *radius, *falloff)
        });

        let mut density = f32::MAX;

        for op in &self.recipe.ops {
            match op {
                RecipeOp::CoastalSurface { .. } => {
                    let land_surface = land_surface_height(&self.recipe, x, z);
                    let surface_y = match land_factor {
                        Some(f) if f >= 1.0 => land_surface,
                        Some(f) if f <= 0.0 => {
                            let mut ocean_y = ocean_surface_y(
                                x,
                                z,
                                &noise,
                                self.recipe.sea_level,
                                ocean_floor,
                                island.map(|(_, _, _, _, floor_y)| floor_y),
                            );
                            if let Some((_, _, _, _, floor_y)) = island {
                                ocean_y = ocean_y.min(floor_y);
                            }
                            ocean_y
                        }
                        Some(f) => {
                            let mut ocean_y = ocean_surface_y(
                                x,
                                z,
                                &noise,
                                self.recipe.sea_level,
                                ocean_floor,
                                island.map(|(_, _, _, _, floor_y)| floor_y),
                            );
                            if let Some((_, _, _, _, floor_y)) = island {
                                ocean_y = ocean_y.min(floor_y);
                            }
                            land_surface * f + ocean_y * (1.0 - f)
                        }
                        None => land_surface,
                    };
                    density = plane_density(y, surface_y);
                }
                RecipeOp::IslandMask { .. } | RecipeOp::OceanFloor { .. } | RecipeOp::ValleyBasin { .. } | RecipeOp::CoastModifier { .. } => {}
                RecipeOp::MountainPeak {
                    center,
                    base_elevation_m,
                    base_radius_m,
                    peak_height_m,
                    steepness,
                    peak_noise,
                } => {
                    let hr =
                        ((x - center[0]).powi(2) + (z - center[1]).powi(2)).sqrt();
                    if hr < *base_radius_m {
                        let t = (1.0 - hr / base_radius_m).max(0.0).powf(*steepness);
                        let mut peak_top = base_elevation_m + peak_height_m * t;
                        if let Some((freq, amp)) = peak_noise {
                            peak_top += (noise.sample(x * freq, 0.0, z * freq) - 0.5) * amp;
                        }
                        density = solid_union(density, plane_density(y, peak_top));
                    }
                }
                RecipeOp::UnderwaterTrench { points, width_m } => {
                    if points.len() >= 2 {
                        for window in points.windows(2) {
                            let start = window[0];
                            let end = window[1];
                            let sdf = capsule_sdf(
                                x,
                                y,
                                z,
                                start[0],
                                start[1],
                                start[2],
                                end[0],
                                end[1],
                                end[2],
                                width_m * 0.5,
                            );
                            density = solid_subtract(density, sdf);
                        }
                    }
                }
                RecipeOp::Ellipsoid {
                    center,
                    radii,
                    peak_noise,
                    combine,
                } => {
                    let mut cy = center[1];
                    if let Some((freq, amp)) = peak_noise {
                        cy += (noise.sample(x * freq, 0.0, z * freq) - 0.5) * amp;
                    }
                    let sdf = ellipsoid_sdf(x, y, z, center[0], cy, center[2], radii[0], radii[1], radii[2]);
                    density = apply_combine(density, sdf, *combine);
                }
                RecipeOp::Capsule {
                    start,
                    end,
                    radius,
                    combine,
                } => {
                    let sdf = capsule_sdf(
                        x,
                        y,
                        z,
                        start[0],
                        start[1],
                        start[2],
                        end[0],
                        end[1],
                        end[2],
                        *radius,
                    );
                    density = apply_combine(density, sdf, *combine);
                }
                RecipeOp::NoisePerturb {
                    scale,
                    amplitude,
                    density_min,
                    density_max,
                } => {
                    let perturb =
                        (noise.sample(x * scale, y * scale, z * scale) - 0.5) * amplitude;
                    if density > *density_min && density < *density_max {
                        density += perturb;
                    }
                }
            }
        }

        if let Some(ref river) = self.river_carve {
            let (dist, half_width, depth) = river_channel_at(&river.spline, x, z);
            let carve = river_carve_offset(dist, half_width, river.bank_width_m, depth);
            density -= carve;
        }

        apply_foundation_seal(&self.recipe, x, y, z, density)
    }

    pub fn surface_height_at(&self, world_x: f32, world_z: f32) -> f32 {
        let mut lo = self.recipe.sea_level - 10.0;
        let mut hi = 85.0;
        for _ in 0..32 {
            let mid = (lo + hi) * 0.5;
            if self.density_at(world_x, mid, world_z) <= 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        hi
    }

    pub fn surface_height_at_recipe(&self, recipe_x: f32, recipe_z: f32) -> f32 {
        self.surface_height_at(
            recipe_x - self.recipe.coord_offset[0],
            recipe_z - self.recipe.coord_offset[2],
        )
    }

    /// Top solid surface below sea level (seabed or trench floor), searching upward from depth.
    pub fn underwater_floor_at(&self, x: f32, z: f32) -> Option<f32> {
        let mut y = self.recipe.sea_level - 40.0;
        let limit = self.recipe.sea_level + 1.0;
        let mut floor = None;
        while y <= limit {
            let here = self.density_at(x, y, z);
            let above = self.density_at(x, y + 0.5, z);
            if here <= 0.0 && above > 0.0 {
                floor = Some(y);
            }
            y += 0.5;
        }
        floor
    }

    pub fn spawn_position(&self) -> (f32, f32, f32) {
        let world_x = self.recipe.spawn_x - self.recipe.coord_offset[0];
        let world_z = self.recipe.spawn_z - self.recipe.coord_offset[2];
        let surface_y = self.surface_height_at(world_x, world_z);
        (world_x, surface_y + 0.05, world_z)
    }

    /// Lowest walkable ground at `(x, z)` with at least `min_clearance` meters of air above.
    pub fn walkable_floor_at(&self, x: f32, z: f32, max_y: f32) -> Option<f32> {
        self.walkable_floor_with_clearance(x, z, max_y, 2.0)
    }

    /// Lowest solid surface with the requested vertical clearance above it.
    pub fn walkable_floor_with_clearance(
        &self,
        x: f32,
        z: f32,
        max_y: f32,
        min_clearance: f32,
    ) -> Option<f32> {
        let mut lowest = None;
        let mut y = max_y.floor();
        while y >= self.recipe.sea_level - 32.0 {
            let here = self.density_at(x, y, z);
            let above = self.density_at(x, y + 0.5, z);
            if here <= 0.0 && above > 0.0 && self.clearance_above_floor(x, y, z) >= min_clearance {
                lowest = Some(y);
            }
            y -= 0.5;
        }
        lowest
    }

    /// Vertical clearance from `floor_y` to the first solid above (open sky => large value).
    pub fn clearance_above_floor(&self, x: f32, floor_y: f32, z: f32) -> f32 {
        let mut y = floor_y + 0.5;
        while y < floor_y + 24.0 {
            if self.density_at(x, y, z) <= 0.0 {
                return y - floor_y - 0.5;
            }
            y += 0.5;
        }
        24.0
    }

    /// Whether solid terrain exists within `max_gap` meters below `foot_y`.
    pub fn has_support_below(&self, x: f32, foot_y: f32, z: f32, max_gap: f32) -> bool {
        let mut y = foot_y - 0.5;
        let limit = foot_y - max_gap;
        while y >= limit {
            if self.density_at(x, y, z) <= 0.0 {
                return true;
            }
            y -= 0.5;
        }
        false
    }

    /// True when a vertical column has no solid from `y_min` through `y_max`.
    pub fn column_is_void(&self, x: f32, z: f32, y_min: f32, y_max: f32) -> bool {
        let mut y = y_min;
        while y <= y_max {
            if self.density_at(x, y, z) <= 0.0 {
                return false;
            }
            y += 0.5;
        }
        true
    }
}

impl DensitySource for RecipeDensitySource {
    fn sample_density(&self, world_x: f32, world_y: f32, world_z: f32) -> f32 {
        self.density_at(world_x, world_y, world_z)
    }
}

fn apply_combine(base: f32, shape: f32, combine: CombineOp) -> f32 {
    match combine {
        CombineOp::Union => solid_union(base, shape),
        CombineOp::Subtract => solid_subtract(base, shape),
    }
}

pub fn island_land_factor(x: f32, z: f32, center: [f32; 2], radius_m: f32, falloff_m: f32) -> f32 {
    let dx = x - center[0];
    let dz = z - center[1];
    let dist = (dx * dx + dz * dz).sqrt();
    if dist <= radius_m {
        1.0
    } else if dist >= radius_m + falloff_m {
        0.0
    } else {
        1.0 - (dist - radius_m) / falloff_m
    }
}

pub(crate) fn island_land_factor_from_recipe(recipe: &TerrainRecipe, x: f32, z: f32) -> Option<f32> {
    let noise = ValueNoise::new(recipe.seed);
    if recipe
        .ops
        .iter()
        .any(|op| matches!(op, RecipeOp::IslandMask { .. }))
    {
        Some(island_land_factor_warped(recipe, x, z, &noise))
    } else {
        None
    }
}

fn ocean_surface_y(
    x: f32,
    z: f32,
    noise: &ValueNoise,
    sea_level: f32,
    ocean_floor: Option<([f32; 2], [f32; 2], f32, f32, f32, u32)>,
    fallback_floor_y: Option<f32>,
) -> f32 {
    if let Some((origin, scale, base_depth_m, variation_m, detail_frequency, detail_octaves)) =
        ocean_floor
    {
        let detail = (noise.fbm(
            x * detail_frequency,
            0.0,
            z * detail_frequency,
            detail_octaves,
            2.0,
            0.5,
        ) - 0.5)
            * variation_m
            * 2.0;
        let _ = (origin, scale);
        sea_level - base_depth_m + detail
    } else {
        fallback_floor_y.unwrap_or(sea_level - 8.0)
    }
}

/// Default vertical-slice recipe matching legacy hardcoded generator.
pub fn default_vertical_slice_recipe(seed: u64, sea_level: f32) -> TerrainRecipe {
    TerrainRecipe {
        seed,
        sea_level,
        spawn_x: -30.0,
        spawn_z: -25.0,
        coord_offset: [0.0, 0.0, 0.0],
        ops: vec![
            RecipeOp::CoastalSurface {
                origin: [48.0, 48.0],
                scale: [96.0, 96.0],
                base_height: 8.0,
                height_range: 14.0,
                ridge_origin: [20.0, 10.0],
                ridge_scale: [30.0, 40.0],
                ridge_amplitude: 12.0,
                detail_frequency: 0.04,
                detail_amplitude: 4.0,
                detail_octaves: 4,
                regional_frequency: 0.0,
                regional_amplitude: 0.0,
                local_frequency: 0.0,
                local_amplitude: 0.0,
                ridged_amplitude: 0.0,
                domain_warp: 0.0,
            },
            RecipeOp::Ellipsoid {
                center: [35.0, 11.0, 15.0],
                radii: [18.0, 22.0, 12.0],
                peak_noise: Some((0.1, 3.0)),
                combine: CombineOp::Union,
            },
            RecipeOp::Ellipsoid {
                center: [26.0, 13.0, 10.0],
                radii: [14.0, 17.0, 12.0],
                peak_noise: None,
                combine: CombineOp::Union,
            },
            RecipeOp::Ellipsoid {
                center: [28.0, 11.5, 8.0],
                radii: [6.5, 4.5, 4.5],
                peak_noise: None,
                combine: CombineOp::Subtract,
            },
            RecipeOp::Ellipsoid {
                center: [27.5, 9.5, 7.5],
                radii: [7.0, 5.5, 6.0],
                peak_noise: None,
                combine: CombineOp::Subtract,
            },
            RecipeOp::Ellipsoid {
                center: [30.0, 8.0, 5.0],
                radii: [4.5, 3.5, 3.5],
                peak_noise: None,
                combine: CombineOp::Subtract,
            },
            RecipeOp::Capsule {
                start: [30.0, 8.0, 5.0],
                end: [28.0, 2.0, 8.0],
                radius: 2.2,
                combine: CombineOp::Subtract,
            },
            RecipeOp::Ellipsoid {
                center: [26.0, -2.0, 12.0],
                radii: [8.0, 6.0, 7.0],
                peak_noise: None,
                combine: CombineOp::Subtract,
            },
            RecipeOp::Capsule {
                start: [26.0, -2.0, 12.0],
                end: [22.0, -1.0, 18.0],
                radius: 1.8,
                combine: CombineOp::Subtract,
            },
            RecipeOp::NoisePerturb {
                scale: 0.3,
                amplitude: 0.4,
                density_min: 0.0,
                density_max: 2.0,
            },
            RecipeOp::Capsule {
                start: [22.0, 4.5, 6.0],
                end: [31.0, 4.5, 11.0],
                radius: 3.2,
                combine: CombineOp::Union,
            },
            RecipeOp::Ellipsoid {
                center: [29.5, 5.5, 6.5],
                radii: [6.0, 4.5, 5.5],
                peak_noise: None,
                combine: CombineOp::Union,
            },
            RecipeOp::Capsule {
                start: [19.0, 13.5, 12.0],
                end: [25.0, 5.5, 9.5],
                radius: 3.0,
                combine: CombineOp::Union,
            },
            RecipeOp::Ellipsoid {
                center: [26.5, 11.5, 8.5],
                radii: [11.0, 5.5, 8.0],
                peak_noise: None,
                combine: CombineOp::Subtract,
            },
        ],
    }
}
