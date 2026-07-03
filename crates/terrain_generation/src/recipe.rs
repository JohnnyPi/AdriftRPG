// crates/terrain_generation/src/recipe.rs
use crate::field_stack::FieldStackParams;
use crate::density_ops::{capsule_sdf, ellipsoid_sdf, plane_density, solid_subtract, solid_union};
use crate::island_atlas::IslandAtlas;
use crate::island_gen::sample_atlas_surface;
use crate::noise::ValueNoise;
use crate::river::{river_carve_offset, river_channel_at};
use crate::surface_height::{island_land_factor_warped, land_surface_height};
use crate::topology::apply_foundation_seal_at;
use crate::water_body::{RiverControlPoint, RiverSpline};
use crate::DensitySource;
use crate::surface_height::CoastModifierKind;
use std::sync::Arc;

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

#[derive(Clone, Copy, Debug)]
pub struct WorldVolumeBounds {
    pub x_min: f32,
    pub x_max: f32,
    pub y_min: f32,
    pub y_max: f32,
    pub z_min: f32,
    pub z_max: f32,
}

impl WorldVolumeBounds {
    pub fn from_compiled_world(world: &game_data::CompiledWorld) -> Self {
        let (mins, maxs) = world.axis_bounds_m();
        Self {
            x_min: mins[0],
            y_min: mins[1],
            z_min: mins[2],
            x_max: maxs[0],
            y_max: maxs[1],
            z_max: maxs[2],
        }
    }

    /// Keep terrain iso-surfaces inside the chunk volume so every column can mesh.
    pub fn clamp_surface_y(&self, surface_y: f32) -> f32 {
        const CEILING_MARGIN_M: f32 = 2.0;
        surface_y.clamp(
            self.y_min + crate::topology::FOUNDATION_DEPTH_M,
            self.y_max - CEILING_MARGIN_M,
        )
    }
}

#[derive(Clone, Debug)]
pub struct RecipeDensitySource {
    recipe: TerrainRecipe,
    river_carve: Option<RiverCarveContext>,
    atlas: Option<Arc<IslandAtlas>>,
    world_bounds: Option<WorldVolumeBounds>,
    detail_noise: ValueNoise,
    field_stack: FieldStackParams,
}

/// Coastal inland factor: 0 at the shore, 1 deep inland.
pub fn coastal_inland_factor(recipe: &TerrainRecipe, x: f32, z: f32) -> f32 {
    if let Some((center, radius_m, falloff_m, domain_warp, _)) = island_mask_params(recipe) {
        let (wx, wz) = warp_xz_for_mask(recipe.seed, x, z, domain_warp);
        let dx = wx - center[0];
        let dz = wz - center[1];
        let dist = (dx * dx + dz * dz).sqrt();
        let inland_band_m = (radius_m * 0.5).max(falloff_m);
        return ((radius_m - dist) / inland_band_m).clamp(0.0, 1.0);
    }

    for op in &recipe.ops {
        if let RecipeOp::CoastalSurface { origin, scale, .. } = op {
            let sx = scale[0].max(f32::EPSILON);
            let sz = scale[1].max(f32::EPSILON);
            let dx = (x - origin[0]) / (sx * 0.5);
            let dz = (z - origin[1]) / (sz * 0.5);
            let radial = (dx * dx + dz * dz).sqrt();
            return (1.0 - radial).clamp(0.0, 1.0);
        }
    }
    1.0
}

/// Approximate horizontal distance to the recipe coastline in meters.
pub fn distance_to_water_m(recipe: &TerrainRecipe, x: f32, z: f32) -> f32 {
    let inland = coastal_inland_factor(recipe, x, z);
    let max_distance = if let Some((_, radius_m, falloff_m, _, _)) = island_mask_params(recipe) {
        (radius_m * 0.5).max(falloff_m)
    } else {
        recipe
            .ops
            .iter()
            .find_map(|op| {
                if let RecipeOp::CoastalSurface { scale, .. } = op {
                    Some(scale[0].max(scale[1]))
                } else {
                    None
                }
            })
            .unwrap_or(96.0)
    };
    (1.0 - inland) * max_distance
}

/// Horizontal distance to the nearest river channel centerline in recipe space.
pub fn distance_to_river_m(source: &RecipeDensitySource, recipe_x: f32, recipe_z: f32) -> f32 {
    if let Some(ref river) = source.river_carve {
        let (dist, _, _) = river_channel_at(&river.spline, recipe_x, recipe_z);
        dist
    } else {
        f32::MAX
    }
}

fn river_spline_to_recipe_space(spline: &RiverSpline, coord_offset: [f32; 3]) -> RiverSpline {
    RiverSpline {
        points: spline
            .points
            .iter()
            .map(|pt| RiverControlPoint {
                position_xz: [
                    pt.position_xz[0] + coord_offset[0],
                    pt.position_xz[1] + coord_offset[2],
                ],
                bed_elevation: pt.bed_elevation,
                water_elevation: pt.water_elevation,
                width: pt.width,
                depth: pt.depth,
                discharge: pt.discharge,
            })
            .collect(),
    }
}

impl RecipeDensitySource {
    pub fn new(recipe: TerrainRecipe) -> Self {
        let detail_noise = ValueNoise::new(recipe.seed);
        Self {
            recipe,
            river_carve: None,
            atlas: None,
            world_bounds: None,
            detail_noise,
            field_stack: FieldStackParams::default(),
        }
    }

    pub fn with_field_stack(mut self, stack: FieldStackParams) -> Self {
        self.field_stack = stack;
        self
    }

    pub fn with_world_bounds(mut self, bounds: WorldVolumeBounds) -> Self {
        self.world_bounds = Some(bounds);
        self
    }

    pub fn world_bounds(&self) -> Option<WorldVolumeBounds> {
        self.world_bounds
    }

    fn clamp_surface_for_world(&self, surface_y: f32) -> f32 {
        self.world_bounds
            .map(|bounds| bounds.clamp_surface_y(surface_y))
            .unwrap_or(surface_y)
    }

    pub fn with_atlas(mut self, atlas: IslandAtlas, bank_width_m: f32) -> Self {
        if let Some(ref river) = atlas.river_graph {
            self.river_carve = Some(RiverCarveContext {
                spline: river_spline_to_recipe_space(river, self.recipe.coord_offset),
                bank_width_m,
            });
        }
        self.atlas = Some(Arc::new(atlas));
        self
    }

    pub fn atlas(&self) -> Option<&IslandAtlas> {
        self.atlas.as_deref()
    }

    /// Horizontal extent used to scale climate noise wavelengths (≈ island diameter).
    pub fn climate_extent_m(&self) -> f32 {
        if let Some(atlas) = self.atlas() {
            (atlas.width().saturating_sub(1)) as f32 * atlas.spacing_m()
        } else {
            256.0
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
        debug_assert!(
            self.recipe.coord_offset[1].abs() < f32::EPSILON,
            "coord_offset Y must be zero until vertical offset is supported"
        );
        if let Some(ref atlas) = self.atlas {
            return self.density_at_recipe_with_atlas(atlas, x, y, z, true);
        }
        self.density_at_recipe_without_atlas(x, y, z, true)
    }

    /// Natural terrain density excluding union recipe objects (pads, platforms).
    pub fn terrain_density_at(&self, world_x: f32, world_y: f32, world_z: f32) -> f32 {
        self.terrain_density_at_recipe(
            world_x + self.recipe.coord_offset[0],
            world_y + self.recipe.coord_offset[1],
            world_z + self.recipe.coord_offset[2],
        )
    }

    fn terrain_density_at_recipe(&self, x: f32, y: f32, z: f32) -> f32 {
        if let Some(ref atlas) = self.atlas {
            return self.density_at_recipe_with_atlas(atlas, x, y, z, false);
        }
        self.density_at_recipe_without_atlas(x, y, z, false)
    }

    fn density_at_recipe_without_atlas(&self, x: f32, y: f32, z: f32, include_union_objects: bool) -> f32 {
        let noise = &self.detail_noise;
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
        let land_surface = land_surface_height(&self.recipe, x, z);

        for op in &self.recipe.ops {
            match op {
                RecipeOp::CoastalSurface { .. } => {
                    let surface_y = match land_factor {
                        Some(f) if f >= 1.0 => land_surface,
                        Some(f) if f <= 0.0 => compose_ocean_elevation(
                            x,
                            z,
                            &noise,
                            self.recipe.seed,
                            self.recipe.sea_level,
                            ocean_floor,
                            island,
                        ),
                        Some(f) => {
                            let ocean_y = compose_ocean_elevation(
                                x,
                                z,
                                &noise,
                                self.recipe.seed,
                                self.recipe.sea_level,
                                ocean_floor,
                                island,
                            );
                            land_surface * f + ocean_y * (1.0 - f)
                        }
                        None => land_surface,
                    };
                    density = solid_union(density, plane_density(y, surface_y));
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
                    if !include_union_objects && *combine == CombineOp::Union {
                        continue;
                    }
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
                    if !include_union_objects && *combine == CombineOp::Union {
                        continue;
                    }
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
                    let scaled_amp = amplitude * self.field_stack.ridge_amplitude.max(0.01);
                    let perturb =
                        (noise.sample(x * scale, y * scale, z * scale) - 0.5) * scaled_amp;
                    let band = perturb_band_weight(density, *density_min, *density_max);
                    density += perturb * band;
                }
            }
        }

        if let Some(ref river) = self.river_carve {
            let (dist, half_width, depth) = river_channel_at(&river.spline, x, z);
            let carve = river_carve_offset(dist, half_width, river.bank_width_m, depth);
            density += carve;
        }

        apply_foundation_seal_at(&self.recipe, x, y, z, density, land_surface)
    }

    fn density_at_recipe_with_atlas(
        &self,
        atlas: &IslandAtlas,
        x: f32,
        y: f32,
        z: f32,
        include_union_objects: bool,
    ) -> f32 {
        let noise = &self.detail_noise;
        let wx = x - self.recipe.coord_offset[0];
        let wz = z - self.recipe.coord_offset[2];
        let surface = self.clamp_surface_for_world(sample_atlas_surface(atlas, wx, wz));
        let mut density = y - surface;

        for op in &self.recipe.ops {
            match op {
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
                    if !include_union_objects && *combine == CombineOp::Union {
                        continue;
                    }
                    let mut cy = center[1];
                    if let Some((freq, amp)) = peak_noise {
                        cy += (noise.sample(x * freq, 0.0, z * freq) - 0.5) * amp;
                    }
                    let sdf = ellipsoid_sdf(
                        x, y, z, center[0], cy, center[2], radii[0], radii[1], radii[2],
                    );
                    density = apply_combine(density, sdf, *combine);
                }
                RecipeOp::Capsule {
                    start,
                    end,
                    radius,
                    combine,
                } => {
                    if !include_union_objects && *combine == CombineOp::Union {
                        continue;
                    }
                    let sdf = capsule_sdf(
                        x, y, z, start[0], start[1], start[2], end[0], end[1], end[2], *radius,
                    );
                    density = apply_combine(density, sdf, *combine);
                }
                RecipeOp::NoisePerturb {
                    scale,
                    amplitude,
                    density_min,
                    density_max,
                } => {
                    let scaled_amp = amplitude * self.field_stack.ridge_amplitude.max(0.01);
                    let perturb =
                        (noise.sample(x * scale, y * scale, z * scale) - 0.5) * scaled_amp;
                    let band = perturb_band_weight(density, *density_min, *density_max);
                    density += perturb * band;
                }
                RecipeOp::IslandMask { .. }
                | RecipeOp::OceanFloor { .. }
                | RecipeOp::CoastalSurface { .. }
                | RecipeOp::ValleyBasin { .. }
                | RecipeOp::CoastModifier { .. } => {}
            }
        }

        // River channels are already carved into the island atlas elevation field.

        apply_foundation_seal_at(&self.recipe, x, y, z, density, surface)
    }

    pub fn terrain_surface_height_at(&self, world_x: f32, world_z: f32) -> f32 {
        let mut lo = self.recipe.sea_level - 10.0;
        let mut hi = self.surface_search_upper_bound();
        for _ in 0..32 {
            let mid = (lo + hi) * 0.5;
            if self.terrain_density_at(world_x, mid, world_z) <= 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        hi
    }

    /// Surface height used as the foundation seal reference at `(world_x, world_z)`.
    ///
    /// Atlas-backed sources use the clamped atlas elevation field (matching
    /// [`Self::density_at`]); recipe-only sources use the coastal surface height.
    /// Prefer this over [`Self::terrain_surface_height_at`] when probing bedrock
    /// depth — binary search can sit one float ULP above the seal plane near
    /// cave margins.
    pub fn foundation_surface_at(&self, world_x: f32, world_z: f32) -> f32 {
        if let Some(ref atlas) = self.atlas {
            return self
                .clamp_surface_for_world(sample_atlas_surface(atlas, world_x, world_z));
        }
        let rx = world_x + self.recipe.coord_offset[0];
        let rz = world_z + self.recipe.coord_offset[2];
        land_surface_height(&self.recipe, rx, rz)
    }

    /// Surface slope in degrees; uses the local-tier atlas slope field when available.
    pub fn terrain_slope_at(&self, world_x: f32, world_z: f32) -> f32 {
        if let Some(atlas) = self.atlas.as_ref() {
            return atlas.slope_at(world_x, world_z);
        }
        let eps = 4.0;
        let hx = self.terrain_surface_height_at(world_x + eps, world_z)
            - self.terrain_surface_height_at(world_x - eps, world_z);
        let hz = self.terrain_surface_height_at(world_x, world_z + eps)
            - self.terrain_surface_height_at(world_x, world_z - eps);
        let gradient = (hx * hx + hz * hz).sqrt() / (2.0 * eps);
        gradient.atan().to_degrees()
    }

    pub fn surface_height_at(&self, world_x: f32, world_z: f32) -> f32 {
        let mut lo = self.recipe.sea_level - 10.0;
        let mut hi = self.surface_search_upper_bound();
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

    fn surface_search_upper_bound(&self) -> f32 {
        if let Some(atlas) = self.atlas.as_ref() {
            let regional_max = atlas
                .elevation_regional
                .samples
                .iter()
                .copied()
                .fold(self.recipe.sea_level, f32::max);
            let local_max = atlas
                .elevation_local
                .samples
                .iter()
                .copied()
                .fold(0.0, f32::max);
            return regional_max + local_max + 25.0;
        }
        self.recipe.sea_level + 85.0
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
        let (x, y, z, _) =
            self.resolve_player_spawn(crate::spawn::PLAYER_SPAWN_MIN_CLEARANCE_M, 48.0);
        (x, y, z)
    }

    /// Lowest walkable natural terrain floor with clearance above.
    pub fn walkable_terrain_floor_at(
        &self,
        x: f32,
        z: f32,
        max_y: f32,
        min_clearance: f32,
    ) -> Option<f32> {
        let mut y = max_y.floor();
        while y >= self.recipe.sea_level - 32.0 {
            let here = self.terrain_density_at(x, y, z);
            let above = self.terrain_density_at(x, y + 0.5, z);
            if here <= 0.0
                && above > 0.0
                && self.terrain_clearance_above_floor(x, y, z) >= min_clearance
            {
                return Some(y);
            }
            y -= 0.5;
        }
        None
    }

    pub(crate) fn terrain_clearance_above_floor(&self, x: f32, floor_y: f32, z: f32) -> f32 {
        let mut y = floor_y + 0.5;
        while y < floor_y + 24.0 {
            if self.terrain_density_at(x, y, z) <= 0.0 {
                return y - floor_y - 0.5;
            }
            y += 0.5;
        }
        24.0
    }

    /// Whether natural terrain exists within `max_gap` meters below `foot_y`.
    pub fn has_terrain_support_below(&self, x: f32, foot_y: f32, z: f32, max_gap: f32) -> bool {
        let mut y = foot_y - 0.5;
        let limit = foot_y - max_gap;
        while y >= limit {
            if self.terrain_density_at(x, y, z) <= 0.0 {
                return true;
            }
            y -= 0.5;
        }
        false
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
        let mut density = self.density_at(world_x, world_y, world_z);
        if let Some(atlas) = self.atlas.as_ref() {
            if atlas.voxel_amplitude_m > 0.0 {
                let wx = world_x;
                let wz = world_z;
                let land = atlas.island_mask.sample_bilinear(wx, wz);
                if land > 0.01 && density.abs() < 2.0 {
                    let noise = &self.detail_noise;
                    let micro = (noise.fbm_2d(wx * 0.35, wz * 0.35, 2) - 0.5)
                        * atlas.voxel_amplitude_m;
                    density += micro * land;
                }
            }
        }
        density
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

fn island_mask_params(
    recipe: &TerrainRecipe,
) -> Option<([f32; 2], f32, f32, f32, f32)> {
    recipe.ops.iter().find_map(|op| {
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
    })
}

fn warp_xz_for_mask(seed: u64, x: f32, z: f32, domain_warp: f32) -> (f32, f32) {
    if domain_warp <= 0.0 {
        return (x, z);
    }
    let noise = ValueNoise::new(seed);
    let ox = noise.fbm(x * domain_warp, 0.0, z * domain_warp, 2, 2.0, 0.5) - 0.5;
    let oz = noise.fbm(x * domain_warp + 100.0, 0.0, z * domain_warp, 2, 2.0, 0.5) - 0.5;
    (
        x + ox * 30.0 * domain_warp,
        z + oz * 30.0 * domain_warp,
    )
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn perturb_band_weight(density: f32, density_min: f32, density_max: f32) -> f32 {
    let width = (density_max - density_min).max(0.5);
    smoothstep(density_min, density_min + width, density)
        * (1.0 - smoothstep(density_max - width, density_max, density))
}

fn compose_ocean_elevation(
    x: f32,
    z: f32,
    noise: &ValueNoise,
    seed: u64,
    sea_level: f32,
    ocean_floor: Option<([f32; 2], [f32; 2], f32, f32, f32, u32)>,
    island: Option<([f32; 2], f32, f32, f32, f32)>,
) -> f32 {
    let shelf = ocean_surface_y(x, z, noise, sea_level, ocean_floor);
    let Some((center, radius_m, falloff_m, domain_warp, basin_y)) = island else {
        return shelf;
    };
    let (wx, wz) = warp_xz_for_mask(seed, x, z, domain_warp);
    let dx = wx - center[0];
    let dz = wz - center[1];
    let dist = (dx * dx + dz * dz).sqrt();
    let beyond_shore = (dist - radius_m - falloff_m).max(0.0);
    let blend_depth = radius_m.max(falloff_m * 2.0);
    let t = (beyond_shore / blend_depth).clamp(0.0, 1.0);
    shelf + (basin_y - shelf) * t
}

fn ocean_surface_y(
    x: f32,
    z: f32,
    noise: &ValueNoise,
    sea_level: f32,
    ocean_floor: Option<([f32; 2], [f32; 2], f32, f32, f32, u32)>,
) -> f32 {
    if let Some((origin, scale, base_depth_m, variation_m, detail_frequency, detail_octaves)) =
        ocean_floor
    {
        let sx = scale[0].max(f32::EPSILON);
        let sz = scale[1].max(f32::EPSILON);
        let lx = (x - origin[0]) / sx;
        let lz = (z - origin[1]) / sz;
        let detail = (noise.fbm(
            lx * detail_frequency,
            0.0,
            lz * detail_frequency,
            detail_octaves,
            2.0,
            0.5,
        ) - 0.5)
            * variation_m
            * 2.0;
        sea_level - base_depth_m + detail
    } else {
        sea_level - 8.0
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

#[cfg(test)]
mod tests {
    use super::*;

    fn expanded_island_recipe() -> TerrainRecipe {
        TerrainRecipe {
            seed: 48129,
            sea_level: 2.0,
            spawn_x: 70.0,
            spawn_z: 160.0,
            coord_offset: [128.0, 0.0, 128.0],
            ops: vec![
                RecipeOp::IslandMask {
                    center: [128.0, 128.0],
                    radius_m: 92.0,
                    falloff_m: 24.0,
                    ocean_floor_y: -28.0,
                    domain_warp: 0.012,
                },
                RecipeOp::OceanFloor {
                    origin: [128.0, 128.0],
                    scale: [256.0, 256.0],
                    base_depth_m: 18.0,
                    variation_m: 8.0,
                    detail_frequency: 0.018,
                    detail_octaves: 4,
                },
                RecipeOp::CoastalSurface {
                    origin: [128.0, 128.0],
                    scale: [256.0, 256.0],
                    base_height: 6.0,
                    height_range: 16.0,
                    ridge_origin: [180.0, 196.0],
                    ridge_scale: [48.0, 56.0],
                    ridge_amplitude: 12.0,
                    detail_frequency: 0.025,
                    detail_amplitude: 2.5,
                    detail_octaves: 4,
                    regional_frequency: 0.006,
                    regional_amplitude: 5.0,
                    local_frequency: 0.035,
                    local_amplitude: 2.0,
                    ridged_amplitude: 2.5,
                    domain_warp: 0.008,
                },
            ],
        }
    }

    #[test]
    fn coastal_inland_factor_is_radially_symmetric_around_mask() {
        let recipe = expanded_island_recipe();
        let inset = 85.0;
        let center = [128.0, 128.0];
        let north = coastal_inland_factor(&recipe, center[0], center[1] + inset);
        let south = coastal_inland_factor(&recipe, center[0], center[1] - inset);
        let east = coastal_inland_factor(&recipe, center[0] + inset, center[1]);
        let west = coastal_inland_factor(&recipe, center[0] - inset, center[1]);
        let spread = [north, south, east, west]
            .iter()
            .fold(0.0_f32, |acc, v| acc.max((v - north).abs()));
        assert!(
            spread < 0.05,
            "cardinal inland factors should match (N={north}, S={south}, E={east}, W={west})"
        );
    }

    #[test]
    fn ocean_floor_variation_reaches_offshore_samples() {
        let recipe = expanded_island_recipe();
        let noise = ValueNoise::new(recipe.seed);
        let ocean_floor = recipe.ops.iter().find_map(|op| {
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
        let island = island_mask_params(&recipe);
        let mut heights = Vec::new();
        for i in 0..100 {
            let angle = i as f32 * 0.21;
            let dist = 118.0 + (i as f32 * 0.03);
            let x = 128.0 + dist * angle.cos();
            let z = 128.0 + dist * angle.sin();
            heights.push(compose_ocean_elevation(
                x,
                z,
                &noise,
                recipe.seed,
                recipe.sea_level,
                ocean_floor,
                island,
            ));
        }
        let min = heights.iter().copied().fold(f32::INFINITY, f32::min);
        let max = heights.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            max - min > 0.4,
            "ocean shelf should vary with ocean_floor detail (range={:.2} m, was flat before fix)",
            max - min
        );
    }

    #[test]
    fn coastal_surface_unions_with_prior_ellipsoid() {
        let recipe = TerrainRecipe {
            seed: 1,
            sea_level: 2.0,
            spawn_x: 0.0,
            spawn_z: 0.0,
            coord_offset: [0.0; 3],
            ops: vec![
                RecipeOp::Ellipsoid {
                    center: [32.0, 14.0, 32.0],
                    radii: [10.0, 8.0, 10.0],
                    peak_noise: None,
                    combine: CombineOp::Union,
                },
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
            ],
        };
        let source = RecipeDensitySource::new(recipe);
        let bulge = source.density_at_recipe(32.0, 16.0, 32.0);
        assert!(
            bulge <= 0.0,
            "ellipsoid authored before coastal_surface should remain solid (density={bulge})"
        );
    }
}
