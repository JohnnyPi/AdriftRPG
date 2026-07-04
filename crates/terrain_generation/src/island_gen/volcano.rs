// crates/terrain_generation/src/island_gen/volcano.rs
//! Volcanic cone stack (VS3 §4).

use std::f32::consts::{PI, TAU};

use crate::field2d::smoothstep;
use crate::island_gen::params::IslandGenParams;
use crate::noise::ValueNoise;

/// Radial ridge relief as a fraction of the composed edifice height
/// (shield + summit). Ridge amplitude must scale with the authored island —
/// a fixed meter value here is what previously buried a 46 m edifice under
/// 28 m ridges and clamped the whole summit into a plateau.
const RIDGE_RELIEF_FRACTION: f32 = 0.12;

/// Ridges fade in over this normalized-radius band so they emerge from the
/// flanks instead of stacking on the summit/caldera.
const RIDGE_INNER_FADE: (f32, f32) = (0.06, 0.22);

/// Minimum freeboard (meters above sea level) enforced where the island
/// footprint is fully supported. Carving ops (caldera, sector collapse,
/// inter-ridge troughs) may sculpt the interior but can never split the
/// landmass below sea level — that is what made the island non-contiguous.
const INTERIOR_FREEBOARD_M: f32 = 1.5;

fn cone_profile(r: f32, exponent: f32) -> f32 {
    (1.0 - r).max(0.0).powf(exponent.max(0.1))
}

fn annular_crater(r: f32, center_r: f32, width: f32) -> f32 {
    let x = ((r - center_r) / width.max(0.01)).abs();
    (1.0 - smoothstep(0.0, 1.0, x)).max(0.0)
}

/// Smallest signed angular difference between `a` and `b`, wrapped to [-PI, PI].
fn wrapped_angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (a - b) % TAU;
    if d > PI {
        d -= TAU;
    } else if d < -PI {
        d += TAU;
    }
    d
}

pub fn volcanic_height(params: &IslandGenParams, wx: f32, wz: f32, land_mask: f32) -> f32 {
    if land_mask <= 0.0 {
        return params.island.sea_level_m;
    }
    let v = &params.volcano;
    let sea = params.island.sea_level_m;
    let dx = wx - v.center[0];
    let dz = wz - v.center[1];
    let dist = (dx * dx + dz * dz).sqrt();
    let r = (dist / v.shield_radius_m.max(1.0)).clamp(0.0, 1.2);

    let coastal_lift = land_mask.clamp(0.0, 1.0).powf(1.6);
    let shield = cone_profile(r, v.shield_exponent) * v.shield_height_m;
    let summit_r = (dist / v.summit_radius_m.max(1.0)).clamp(0.0, 1.0);
    let summit = cone_profile(summit_r, v.summit_exponent) * v.summit_height_m;
    let mut height = sea + (shield + summit) * coastal_lift;

    // Caldera annulus derived from the authored caldera radius rather than
    // fixed normalized constants, so it tracks re-scaled islands.
    let crater_center_r = (v.caldera_radius_m / v.shield_radius_m.max(1.0)).clamp(0.02, 0.6);
    let crater_width_r = (crater_center_r * 0.65).max(0.02);
    let crater = annular_crater(r, crater_center_r, crater_width_r);
    height -= crater * v.caldera_depth_m * coastal_lift;
    height += crater * v.caldera_rim_height_m * 0.35 * coastal_lift;

    let theta = dz.atan2(dx);
    let ridges = v.radial_ridge_count.max(1);
    let ridge_amplitude_m =
        (v.shield_height_m + v.summit_height_m).max(0.0) * RIDGE_RELIEF_FRACTION;
    let ridge_radial = cone_profile(r, 1.2) * smoothstep(RIDGE_INNER_FADE.0, RIDGE_INNER_FADE.1, r);
    for i in 0..ridges {
        let ridge_angle = (i as f32 / ridges as f32) * TAU;
        let angle_diff = wrapped_angle_diff(theta, ridge_angle);
        let angular = (-(angle_diff * angle_diff) / 0.15).exp();
        height += angular * ridge_radial * ridge_amplitude_m * coastal_lift;
    }

    let collapse_dir = v.collapse_direction_deg.to_radians();
    let angular_match = (-((theta - collapse_dir).cos() - 0.4).powi(2) / 0.12).exp();
    let radial_band = smoothstep(0.25, 0.85, r);
    height -= angular_match * radial_band * v.collapse_depth_m * coastal_lift;

    // Contiguity floor: fully supported interior stays above sea level; the
    // floor fades to sea level toward the coast so the shoreline still forms
    // from the natural mask falloff.
    let interior = smoothstep(0.45, 0.85, land_mask);
    height = height.max(sea + INTERIOR_FREEBOARD_M * interior);

    height.min(sea + params.island.maximum_height_m)
}

/// Regional-tier surface noise applied during regional elevation build.
pub fn regional_detail_at(params: &IslandGenParams, wx: f32, wz: f32, noise: &ValueNoise) -> f32 {
    (noise.fbm_2d(wx * 0.002, wz * 0.002, 3) - 0.5) * params.surface_noise.regional_amplitude_m
}

/// Local-tier surface noise applied during local elevation build.
pub fn local_detail_at(params: &IslandGenParams, wx: f32, wz: f32, noise: &ValueNoise) -> f32 {
    (noise.fbm_2d(wx * 0.035, wz * 0.035, 2) - 0.5) * params.surface_noise.local_amplitude_m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::island_gen::footprint::build_island_mask;
    use crate::island_gen::params::IslandGenParams;

    fn sample_ring(params: &IslandGenParams, radius: f32, samples: u32) -> Vec<f32> {
        (0..samples)
            .map(|i| {
                let a = (i as f32 / samples as f32) * TAU;
                let wx = params.volcano.center[0] + radius * a.cos();
                let wz = params.volcano.center[1] + radius * a.sin();
                volcanic_height(params, wx, wz, 1.0)
            })
            .collect()
    }

    #[test]
    fn wrapped_diff_stays_in_pi_range() {
        assert!((wrapped_angle_diff(-0.9, 5.4).abs()) < 0.1);
        assert!((wrapped_angle_diff(0.1, TAU - 0.1) - 0.2).abs() < 1e-4);
        assert!(wrapped_angle_diff(3.0, -3.0) < 0.0);
    }

    #[test]
    fn ridges_present_on_both_angular_halves() {
        // Pre-fix, |theta - ridge_angle| without wrapping suppressed every
        // ridge whose authored angle exceeded PI on the theta < 0 half.
        let params = IslandGenParams::default();
        let radius = params.volcano.shield_radius_m * 0.5;
        let heights = sample_ring(&params, radius, 72);
        let half = heights.len() / 2;
        let spread = |s: &[f32]| {
            s.iter().cloned().fold(f32::MIN, f32::max) - s.iter().cloned().fold(f32::MAX, f32::min)
        };
        let pos_spread = spread(&heights[..half]);
        let neg_spread = spread(&heights[half..]);
        assert!(pos_spread > 0.5, "expected ridge relief on theta>0 half");
        assert!(neg_spread > 0.5, "expected ridge relief on theta<0 half");
    }

    #[test]
    fn ridge_amplitude_scales_with_edifice_not_fixed_meters() {
        let params = IslandGenParams::default();
        let budget = params.volcano.shield_height_m + params.volcano.summit_height_m;
        let max_ridge = budget * RIDGE_RELIEF_FRACTION;
        let radius = params.volcano.shield_radius_m * 0.5;
        let heights = sample_ring(&params, radius, 72);
        let spread = heights.iter().cloned().fold(f32::MIN, f32::max)
            - heights.iter().cloned().fold(f32::MAX, f32::min);
        // Ring relief comes from ridges (+ collapse carve); it must stay in
        // the same order of magnitude as the proportional ridge budget.
        assert!(
            spread <= max_ridge + params.volcano.collapse_depth_m + 1.0,
            "ring relief {spread:.1} m exceeds proportional ridge budget"
        );
    }

    #[test]
    fn supported_interior_never_dips_below_sea() {
        let params = IslandGenParams::default();
        let sea = params.island.sea_level_m;
        let mask_noise = ValueNoise::new(params.seed);
        let extent = params.island.playable_diameter_m * 0.6;
        let step = extent / 48.0;
        let mut checked = 0u32;
        let mut z = -extent;
        while z <= extent {
            let mut x = -extent;
            while x <= extent {
                let wx = params.volcano.center[0] + x;
                let wz = params.volcano.center[1] + z;
                let mask = build_island_mask(&params, wx, wz, &mask_noise);
                if mask > 0.85 {
                    let h = volcanic_height(&params, wx, wz, mask);
                    assert!(
                        h >= sea + INTERIOR_FREEBOARD_M * 0.9,
                        "interior column ({wx:.0},{wz:.0}) mask={mask:.2} dipped to {h:.2} (sea {sea})"
                    );
                    checked += 1;
                }
                x += step;
            }
            z += step;
        }
        assert!(
            checked > 100,
            "expected to check interior columns, got {checked}"
        );
    }

    #[test]
    fn summit_respects_authored_peak_without_clamping_plateau() {
        let params = IslandGenParams::default();
        let sea = params.island.sea_level_m;
        let composed_budget = params.volcano.shield_height_m
            + params.volcano.summit_height_m
            + params.volcano.caldera_rim_height_m;
        let h = volcanic_height(
            &params,
            params.volcano.center[0] + 0.01,
            params.volcano.center[1] + 0.01,
            1.0,
        );
        assert!(
            h <= sea + composed_budget + 0.5,
            "summit {h:.1} exceeds authored composed budget {:.1}",
            sea + composed_budget
        );
    }
}
