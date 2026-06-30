//! Shared land-surface height evaluation for coastal terrain and foundation sealing.

use crate::field_stack::valley_field;
use crate::noise::ValueNoise;
use crate::recipe::{coastal_inland_factor, island_land_factor, RecipeOp, TerrainRecipe};

#[derive(Clone, Copy, Debug)]
pub struct CoastalSurfaceParams {
    pub origin: [f32; 2],
    pub scale: [f32; 2],
    pub base_height: f32,
    pub height_range: f32,
    pub ridge_origin: [f32; 2],
    pub ridge_scale: [f32; 2],
    pub ridge_amplitude: f32,
    pub detail_frequency: f32,
    pub detail_amplitude: f32,
    pub detail_octaves: u32,
    pub regional_frequency: f32,
    pub regional_amplitude: f32,
    pub local_frequency: f32,
    pub local_amplitude: f32,
    pub ridged_amplitude: f32,
    pub domain_warp: f32,
}

fn warp_xz(noise: &ValueNoise, x: f32, z: f32, strength: f32) -> (f32, f32) {
    if strength <= 0.0 {
        return (x, z);
    }
    let ox = noise.fbm(x * strength, 0.0, z * strength, 2, 2.0, 0.5) - 0.5;
    let oz = noise.fbm(x * strength + 100.0, 0.0, z * strength, 2, 2.0, 0.5) - 0.5;
    (x + ox * 30.0 * strength, z + oz * 30.0 * strength)
}

fn cove_depression(x: f32, z: f32, center: [f32; 2], radius_m: f32, depth_m: f32) -> f32 {
    let dx = x - center[0];
    let dz = z - center[1];
    let dist = (dx * dx + dz * dz).sqrt();
    if dist >= radius_m {
        return 0.0;
    }
    let t = 1.0 - dist / radius_m;
    depth_m * t * t
}

fn harbor_depression(
    noise: &ValueNoise,
    x: f32,
    z: f32,
    center: [f32; 2],
    radius_m: f32,
    depth_m: f32,
    arc_frequency: f32,
) -> f32 {
    let dx = x - center[0];
    let dz = z - center[1];
    let dist = (dx * dx + dz * dz).sqrt();
    if dist >= radius_m {
        return 0.0;
    }
    let angle = dz.atan2(dx);
    let arc = (noise.sample(angle * arc_frequency, 0.0, dist * 0.05) - 0.3).max(0.0);
    let t = 1.0 - dist / radius_m;
    depth_m * t * t * arc
}

pub fn sample_coastal_surface(
    recipe: &TerrainRecipe,
    params: &CoastalSurfaceParams,
    x: f32,
    z: f32,
    noise: &ValueNoise,
) -> f32 {
    let (wx, wz) = warp_xz(noise, x, z, params.domain_warp);
    let coast = coastal_inland_factor(recipe, wx, wz);
    let broad = params.base_height + coast * params.height_range;
    let ridge_bump = ((wx - params.ridge_origin[0]) / params.ridge_scale[0]).clamp(0.0, 1.0)
        * ((wz + params.ridge_origin[1]) / params.ridge_scale[1]).clamp(0.0, 1.0)
        * params.ridge_amplitude;

    let regional = if params.regional_amplitude > 0.0 && params.regional_frequency > 0.0 {
        (noise.fbm(
            wx * params.regional_frequency,
            0.0,
            wz * params.regional_frequency,
            4,
            2.0,
            0.5,
        ) - 0.5)
            * params.regional_amplitude
    } else {
        0.0
    };

    let local_freq = if params.local_frequency > 0.0 {
        params.local_frequency
    } else {
        params.detail_frequency
    };
    let local_amp = if params.local_amplitude > 0.0 {
        params.local_amplitude
    } else {
        params.detail_amplitude
    };
    let local = (noise.fbm(
        wx * local_freq,
        0.0,
        wz * local_freq,
        params.detail_octaves,
        2.0,
        0.5,
    ) - 0.5)
        * local_amp;

    let ridged = if params.ridged_amplitude > 0.0 {
        let r = noise.fbm(
            wx * local_freq * 1.5,
            0.0,
            wz * local_freq * 1.5,
            3,
            2.0,
            0.5,
        );
        let ridged_val = 1.0 - (2.0 * r - 1.0).abs();
        ridged_val * params.ridged_amplitude * coast
    } else {
        0.0
    };

    broad + ridge_bump + regional + local + ridged
}

pub fn land_surface_height(recipe: &TerrainRecipe, x: f32, z: f32) -> f32 {
    let noise = ValueNoise::new(recipe.seed);
    let mut height = recipe.sea_level;

    for op in &recipe.ops {
        if let RecipeOp::CoastalSurface {
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
            regional_frequency,
            regional_amplitude,
            local_frequency,
            local_amplitude,
            ridged_amplitude,
            domain_warp,
        } = op
        {
            let _ = (origin, scale);
            height = sample_coastal_surface(
                recipe,
                &CoastalSurfaceParams {
                    origin: *origin,
                    scale: *scale,
                    base_height: *base_height,
                    height_range: *height_range,
                    ridge_origin: *ridge_origin,
                    ridge_scale: *ridge_scale,
                    ridge_amplitude: *ridge_amplitude,
                    detail_frequency: *detail_frequency,
                    detail_amplitude: *detail_amplitude,
                    detail_octaves: *detail_octaves,
                    regional_frequency: *regional_frequency,
                    regional_amplitude: *regional_amplitude,
                    local_frequency: *local_frequency,
                    local_amplitude: *local_amplitude,
                    ridged_amplitude: *ridged_amplitude,
                    domain_warp: *domain_warp,
                },
                x,
                z,
                &noise,
            );
            break;
        }
    }

    for op in &recipe.ops {
        match op {
            RecipeOp::ValleyBasin {
                origin,
                scale,
                depth_m,
            } => {
                height += valley_field(x, z, *origin, *scale, *depth_m);
            }
            RecipeOp::CoastModifier {
                center,
                radius_m,
                depth_m,
                min_land_factor,
                max_land_factor,
                kind,
            } => {
                let land_factor = island_land_factor_warped(recipe, x, z, &noise);
                if land_factor >= *min_land_factor && land_factor <= *max_land_factor {
                    let depression = match kind {
                        CoastModifierKind::Cove => {
                            cove_depression(x, z, *center, *radius_m, *depth_m)
                        }
                        CoastModifierKind::Harbor => harbor_depression(
                            &noise,
                            x,
                            z,
                            *center,
                            *radius_m,
                            *depth_m,
                            2.5,
                        ),
                        CoastModifierKind::CliffShelf => 0.0,
                    };
                    height -= depression;
                }
            }
            _ => {}
        }
    }

    height
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoastModifierKind {
    Cove,
    Harbor,
    CliffShelf,
}

pub fn island_land_factor_warped(
    recipe: &TerrainRecipe,
    x: f32,
    z: f32,
    noise: &ValueNoise,
) -> f32 {
    for op in &recipe.ops {
        if let RecipeOp::IslandMask {
            center,
            radius_m,
            falloff_m,
            domain_warp,
            ..
        } = op
        {
            let (wx, wz) = warp_xz(noise, x, z, *domain_warp);
            return island_land_factor(wx, wz, *center, *radius_m, *falloff_m);
        }
    }
    1.0
}
