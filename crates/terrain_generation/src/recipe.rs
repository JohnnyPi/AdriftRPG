use crate::density_ops::{capsule_sdf, ellipsoid_sdf, plane_density, solid_subtract, solid_union};
use crate::noise::ValueNoise;
use crate::DensitySource;

/// Portable terrain recipe evaluated at runtime (may originate from YAML).
#[derive(Clone, Debug)]
pub struct TerrainRecipe {
    pub seed: u64,
    pub sea_level: f32,
    pub spawn_x: f32,
    pub spawn_z: f32,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombineOp {
    Union,
    Subtract,
}

#[derive(Clone, Debug)]
pub struct RecipeDensitySource {
    recipe: TerrainRecipe,
}

impl RecipeDensitySource {
    pub fn new(recipe: TerrainRecipe) -> Self {
        Self { recipe }
    }

    pub fn recipe(&self) -> &TerrainRecipe {
        &self.recipe
    }

    pub fn density_at(&self, x: f32, y: f32, z: f32) -> f32 {
        let noise = ValueNoise::new(self.recipe.seed);
        let mut density = f32::MAX;

        for op in &self.recipe.ops {
            match op {
                RecipeOp::CoastalSurface {
                    origin,
                    scale,
                    base_height,
                    height_range,
                    ridge_origin,
                    ridge_scale,
                    ridge_amplitude,
                    detail_frequency,
                    detail_amplitude,
                    detail_octaves,
                } => {
                    let nx = (x + origin[0]) / scale[0];
                    let nz = (z + origin[1]) / scale[1];
                    let coast = 1.0 - (nx * 0.6 + (1.0 - nz) * 0.4).clamp(0.0, 1.0);
                    let broad = *base_height + coast * *height_range;
                    let ridge_bump = ((x - ridge_origin[0]) / ridge_scale[0]).clamp(0.0, 1.0)
                        * ((z + ridge_origin[1]) / ridge_scale[1]).clamp(0.0, 1.0)
                        * *ridge_amplitude;
                    let detail = (noise.fbm(
                        x * detail_frequency,
                        0.0,
                        z * detail_frequency,
                        *detail_octaves,
                        2.0,
                        0.5,
                    ) - 0.5)
                        * *detail_amplitude;
                    density = plane_density(y, broad + ridge_bump + detail);
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

        density
    }

    pub fn surface_height_at(&self, x: f32, z: f32) -> f32 {
        let mut lo = self.recipe.sea_level - 10.0;
        let mut hi = 60.0;
        for _ in 0..32 {
            let mid = (lo + hi) * 0.5;
            if self.density_at(x, mid, z) <= 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        hi
    }

    pub fn spawn_position(&self) -> (f32, f32, f32) {
        let surface_y = self.surface_height_at(self.recipe.spawn_x, self.recipe.spawn_z);
        (
            self.recipe.spawn_x,
            surface_y + 0.05,
            self.recipe.spawn_z,
        )
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

/// Default vertical-slice recipe matching legacy hardcoded generator.
pub fn default_vertical_slice_recipe(seed: u64, sea_level: f32) -> TerrainRecipe {
    TerrainRecipe {
        seed,
        sea_level,
        spawn_x: -30.0,
        spawn_z: -25.0,
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
            },
            RecipeOp::Ellipsoid {
                center: [35.0, 11.0, 15.0],
                radii: [18.0, 22.0, 12.0],
                peak_noise: Some((0.1, 3.0)),
                combine: CombineOp::Union,
            },
            RecipeOp::Ellipsoid {
                center: [28.0, 16.0, 8.0],
                radii: [8.0, 14.0, 6.0],
                peak_noise: None,
                combine: CombineOp::Union,
            },
            RecipeOp::Ellipsoid {
                center: [26.0, 10.0, 8.0],
                radii: [10.0, 8.0, 8.0],
                peak_noise: None,
                combine: CombineOp::Subtract,
            },
            RecipeOp::Ellipsoid {
                center: [30.0, 8.0, 5.0],
                radii: [5.0, 4.0, 4.0],
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
        ],
    }
}
