// crates/terrain_generation/src/island_gen/volcano.rs
//! Volcanic cone stack (VS3 §4).

use crate::field2d::smoothstep;
use crate::island_gen::params::IslandGenParams;
use crate::noise::ValueNoise;

fn cone_profile(r: f32, exponent: f32) -> f32 {
    (1.0 - r).max(0.0).powf(exponent.max(0.1))
}

fn annular_crater(r: f32, center_r: f32, width: f32) -> f32 {
    let x = ((r - center_r) / width.max(0.01)).abs();
    (1.0 - smoothstep(0.0, 1.0, x)).max(0.0)
}

pub fn volcanic_height(params: &IslandGenParams, wx: f32, wz: f32, land_mask: f32) -> f32 {
    if land_mask <= 0.0 {
        return params.island.sea_level_m;
    }
    let v = &params.volcano;
    let dx = wx - v.center[0];
    let dz = wz - v.center[1];
    let dist = (dx * dx + dz * dz).sqrt();
    let r = (dist / v.shield_radius_m.max(1.0)).clamp(0.0, 1.2);

    let coastal_lift = land_mask.clamp(0.0, 1.0).powf(1.6);
    let shield = cone_profile(r, v.shield_exponent) * v.shield_height_m;
    let summit_r = (dist / v.summit_radius_m.max(1.0)).clamp(0.0, 1.0);
    let summit = cone_profile(summit_r, v.summit_exponent) * v.summit_height_m;
    let mut height = params.island.sea_level_m + (shield + summit) * coastal_lift;

    let crater = annular_crater(r, 0.12, 0.08);
    height -= crater * v.caldera_depth_m * coastal_lift;
    height += crater * v.caldera_rim_height_m * 0.35 * coastal_lift;

    let theta = dz.atan2(dx);
    let ridges = v.radial_ridge_count.max(1);
    for i in 0..ridges {
        let ridge_angle = (i as f32 / ridges as f32) * std::f32::consts::TAU;
        let angle_diff = (theta - ridge_angle).abs();
        let angular = (-angle_diff.powi(2) / 0.15).exp();
        let ridge = angular * cone_profile(r, 1.2) * 28.0;
        height += ridge * coastal_lift;
    }

    let collapse_dir = v.collapse_direction_deg.to_radians();
    let angular_match = (-((theta - collapse_dir).cos() - 0.4).powi(2) / 0.12).exp();
    let radial_band = smoothstep(0.25, 0.85, r);
    height -= angular_match * radial_band * v.collapse_depth_m * coastal_lift;

    height.min(params.island.sea_level_m + params.island.maximum_height_m)
}

/// Regional-tier surface noise applied during regional elevation build.
pub fn regional_detail_at(params: &IslandGenParams, wx: f32, wz: f32) -> f32 {
    let noise = ValueNoise::new(params.seed.wrapping_add(17));
    (noise.fbm_2d(wx * 0.002, wz * 0.002, 3) - 0.5) * params.surface_noise.regional_amplitude_m
}

/// Local-tier surface noise applied during local elevation build.
pub fn local_detail_at(params: &IslandGenParams, wx: f32, wz: f32) -> f32 {
    let noise = ValueNoise::new(params.seed.wrapping_add(23));
    (noise.fbm_2d(wx * 0.035, wz * 0.035, 2) - 0.5) * params.surface_noise.local_amplitude_m
}
