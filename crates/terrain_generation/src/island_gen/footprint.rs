// crates/terrain_generation/src/island_gen/footprint.rs
//! Elliptical lobes and domain warp (VS3 §3).

use crate::field2d::{smooth_max, smoothstep};
use crate::island_gen::params::IslandGenParams;
use crate::noise::ValueNoise;

fn rotate(p: [f32; 2], angle: f32) -> [f32; 2] {
    let (s, c) = angle.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}

fn elliptical_distance(p: [f32; 2], center: [f32; 2], radii: [f32; 2], angle: f32) -> f32 {
    let q = rotate([p[0] - center[0], p[1] - center[1]], -angle);
    let rx = radii[0].max(1.0);
    let rz = radii[1].max(1.0);
    ((q[0] / rx).powi(2) + (q[1] / rz).powi(2)).sqrt()
}

fn island_support(d: f32, coast_start: f32, coast_end: f32) -> f32 {
    1.0 - smoothstep(coast_start, coast_end, d)
}

pub fn build_island_mask(params: &IslandGenParams, wx: f32, wz: f32) -> f32 {
    let noise = ValueNoise::new(params.seed);
    let warp = if params.island.warp_amplitude > 0.0 {
        let f = params.island.warp_frequency;
        [
            (noise.sample(wx * f, 0.0, wz * f) - 0.5) * 2.0 * params.island.warp_amplitude,
            (noise.sample(wx * f + 100.0, 0.0, wz * f) - 0.5) * 2.0 * params.island.warp_amplitude,
        ]
    } else {
        [0.0, 0.0]
    };
    let p = [wx + warp[0], wz + warp[1]];
    let radius = params.island.playable_diameter_m * 0.5;
    let mut support = 0.0f32;
    let lobes = params.island.lobe_count.max(1);
    for i in 0..lobes {
        let angle = (i as f32 / lobes as f32) * std::f32::consts::TAU;
        let offset = [
            angle.cos() * radius * 0.18,
            angle.sin() * radius * 0.18,
        ];
        let center = [
            params.center[0] + offset[0],
            params.center[1] + offset[1],
        ];
        let radii = [
            radius * (0.85 + 0.1 * (i as f32 * 0.7).sin()),
            radius * (0.75 + 0.12 * (i as f32 * 1.1).cos()),
        ];
        let d = elliptical_distance(p, center, radii, angle * 0.35);
        let s = island_support(d, 0.72, 1.05);
        support = if i == 0 {
            s
        } else {
            smooth_max(support, s, 0.15)
        };
    }
    support.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::island_gen::params::IslandGenParams;

    #[test]
    fn warped_mask_centroid_stays_near_configured_center() {
        let mut params = IslandGenParams::default();
        params.island.warp_amplitude = 24.0;
        params.island.warp_frequency = 0.006;
        params.center = [50.0, -30.0];
        let spacing = 8.0;
        let extent = 400.0;
        let origin = [
            params.center[0] - extent * 0.5,
            params.center[1] - extent * 0.5,
        ];
        let w = (extent / spacing).ceil() as u32 + 1;
        let h = w;
        let mut sum_x = 0.0f64;
        let mut sum_z = 0.0f64;
        let mut weight = 0.0f64;
        for z in 0..h {
            for x in 0..w {
                let wx = origin[0] + x as f32 * spacing;
                let wz = origin[1] + z as f32 * spacing;
                let m = build_island_mask(&params, wx, wz) as f64;
                sum_x += wx as f64 * m;
                sum_z += wz as f64 * m;
                weight += m;
            }
        }
        let cx = (sum_x / weight) as f32;
        let cz = (sum_z / weight) as f32;
        assert!(
            (cx - params.center[0]).abs() < spacing * 2.0,
            "centroid x {cx} should be near {}",
            params.center[0]
        );
        assert!(
            (cz - params.center[1]).abs() < spacing * 2.0,
            "centroid z {cz} should be near {}",
            params.center[1]
        );
    }
}
