// crates/terrain_generation/src/island_gen/bathymetry.rs
//! Bathymetry and shelf profiles (VS3 §10).

use crate::field2d::{smoothstep, Field2D};
use crate::island_gen::params::IslandGenParams;
use crate::noise::ValueNoise;

pub const SALT_SHELF_WIDTH: u64 = 0xC04A_7D15_5E1F_0001;
pub const SALT_SHELF_DEPTH: u64 = 0xC04A_7D15_5E1F_0002;
pub const SALT_DEEP_SLOPE: u64 = 0xC04A_7D15_D33F_0003;

fn seeded_unit(noise: &ValueNoise, wx: f32, wz: f32) -> f32 {
    noise.sample(wx * 0.0025, 0.0, wz * 0.0025)
}

fn range_mix(min: f32, max: f32, t: f32) -> f32 {
    min + (max - min) * t.clamp(0.0, 1.0)
}

pub fn bathymetry_height(
    params: &IslandGenParams,
    wx: f32,
    wz: f32,
    coast_distance: f32,
    shelf_width_noise: &ValueNoise,
    shelf_depth_noise: &ValueNoise,
    deep_slope_noise: &ValueNoise,
) -> f32 {
    let sea = params.island.sea_level_m;
    if coast_distance <= 0.0 {
        return sea;
    }
    let coast = &params.coast;
    let shelf_width = range_mix(
        coast.shelf_width_min_m,
        coast.shelf_width_max_m,
        seeded_unit(shelf_width_noise, wx, wz),
    );
    let shelf_depth = range_mix(
        coast.shelf_depth_min_m,
        coast.shelf_depth_max_m,
        seeded_unit(shelf_depth_noise, wx, wz),
    );
    let deep_slope = range_mix(
        coast.deep_slope_min,
        coast.deep_slope_max,
        seeded_unit(deep_slope_noise, wx, wz),
    );

    if coast_distance < shelf_width {
        let t = (coast_distance / shelf_width).clamp(0.0, 1.0);
        return sea - shelf_depth * smoothstep(0.0, 1.0, t);
    }
    let beyond = coast_distance - shelf_width;
    sea - shelf_depth - beyond * deep_slope
}

fn is_coast_cell(mask: &Field2D<f32>, x: u32, z: u32) -> bool {
    let land = mask.get(x, z) > 0.5;
    let w = mask.width;
    let h = mask.height;
    for (dx, dz) in [
        (-1, 0),
        (1, 0),
        (0, -1),
        (0, 1),
        (-1, -1),
        (-1, 1),
        (1, -1),
        (1, 1),
    ] {
        let nx = x as i32 + dx;
        let nz = z as i32 + dz;
        if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
            continue;
        }
        let neighbor_land = mask.get(nx as u32, nz as u32) > 0.5;
        if land != neighbor_land {
            return true;
        }
    }
    false
}

/// Distance to the nearest coastline cell (land/water boundary), valid on both sides.
pub fn compute_coast_distance(mask: &Field2D<f32>, spacing: f32) -> Field2D<f32> {
    let w = mask.width;
    let h = mask.height;
    let n = (w * h) as usize;
    let diag = spacing * std::f32::consts::SQRT_2;
    let mut dist = vec![f32::MAX; n];

    for z in 0..h {
        for x in 0..w {
            if is_coast_cell(mask, x, z) {
                dist[mask.index(x, z)] = 0.0;
            }
        }
    }

    for z in 0..h {
        for x in 0..w {
            let i = mask.index(x, z);
            let mut d = dist[i];
            if x > 0 {
                d = d.min(dist[mask.index(x - 1, z)] + spacing);
            }
            if z > 0 {
                d = d.min(dist[mask.index(x, z - 1)] + spacing);
            }
            if x > 0 && z > 0 {
                d = d.min(dist[mask.index(x - 1, z - 1)] + diag);
            }
            if x + 1 < w && z > 0 {
                d = d.min(dist[mask.index(x + 1, z - 1)] + diag);
            }
            dist[i] = d;
        }
    }

    for z in (0..h).rev() {
        for x in (0..w).rev() {
            let i = mask.index(x, z);
            let mut d = dist[i];
            if x + 1 < w {
                d = d.min(dist[mask.index(x + 1, z)] + spacing);
            }
            if z + 1 < h {
                d = d.min(dist[mask.index(x, z + 1)] + spacing);
            }
            if x + 1 < w && z + 1 < h {
                d = d.min(dist[mask.index(x + 1, z + 1)] + diag);
            }
            if x > 0 && z + 1 < h {
                d = d.min(dist[mask.index(x - 1, z + 1)] + diag);
            }
            dist[i] = d;
        }
    }

    let mut field = Field2D::new(w, h, mask.origin, spacing);
    for (i, d) in dist.into_iter().enumerate() {
        field.samples[i] = if d.is_finite() { d } else { 0.0 };
    }
    field
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::island_gen::params::IslandGenParams;

    fn circular_mask(radius_cells: u32, spacing: f32) -> Field2D<f32> {
        let diameter = radius_cells * 2 + 1;
        let origin = [
            -(radius_cells as f32) * spacing,
            -(radius_cells as f32) * spacing,
        ];
        let mut mask = Field2D::<f32>::new(diameter, diameter, origin, spacing);
        let cx = radius_cells as f32;
        let cz = radius_cells as f32;
        for z in 0..diameter {
            for x in 0..diameter {
                let dx = x as f32 - cx;
                let dz = z as f32 - cz;
                let d = (dx * dx + dz * dz).sqrt() * spacing;
                let land = if d < radius_cells as f32 * spacing * 0.95 {
                    1.0
                } else if d > radius_cells as f32 * spacing * 1.05 {
                    0.0
                } else {
                    1.0 - (d / (radius_cells as f32 * spacing) - 0.95) / 0.1
                };
                mask.set(x, z, land.clamp(0.0, 1.0));
            }
        }
        mask
    }

    #[test]
    fn coast_distance_at_center_approaches_island_radius() {
        let spacing = 4.0;
        let radius_cells = 20;
        let mask = circular_mask(radius_cells, spacing);
        let dist = compute_coast_distance(&mask, spacing);
        let center = dist.get(radius_cells, radius_cells);
        let expected_radius = radius_cells as f32 * spacing * 0.95;
        assert!(
            (center - expected_radius).abs() < spacing * 3.0,
            "center dist {center} should be near radius {expected_radius}"
        );
    }

    #[test]
    fn coast_distance_increases_monotonically_inland() {
        let spacing = 4.0;
        let radius_cells = 20;
        let mask = circular_mask(radius_cells, spacing);
        let dist = compute_coast_distance(&mask, spacing);
        let cx = radius_cells;
        let mut prev = 0.0f32;
        for r in (1..=radius_cells.saturating_sub(2)).rev() {
            let d = dist.get(cx - r, cx);
            assert!(
                d + 0.01 >= prev,
                "inland distance should increase toward center: r={r} d={d} prev={prev}"
            );
            prev = d;
        }
    }

    #[test]
    fn coast_distance_has_no_offshore_jumps() {
        let spacing = 4.0;
        let radius_cells = 25;
        let mask = circular_mask(radius_cells, spacing);
        let dist = compute_coast_distance(&mask, spacing);
        let cx = radius_cells;
        let max_step = spacing * 2.0;
        let mut prev = dist.get(cx + 1, cx);
        for r in 2..radius_cells {
            let d = dist.get(cx + r, cx);
            assert!(
                (d - prev).abs() <= max_step + 0.01,
                "offshore jump at r={r}: {prev} -> {d}"
            );
            prev = d;
        }
    }

    fn bathymetry_noises(params: &IslandGenParams) -> (ValueNoise, ValueNoise, ValueNoise) {
        (
            ValueNoise::new(params.seed.wrapping_add(SALT_SHELF_WIDTH)),
            ValueNoise::new(params.seed.wrapping_add(SALT_SHELF_DEPTH)),
            ValueNoise::new(params.seed.wrapping_add(SALT_DEEP_SLOPE)),
        )
    }

    #[test]
    fn bathymetry_deepens_past_shelf_width() {
        let params = IslandGenParams::default();
        let (w, d, s) = bathymetry_noises(&params);
        let shelf_width = params.coast.shelf_width_min_m
            + (params.coast.shelf_width_max_m - params.coast.shelf_width_min_m) * 0.5;
        let shallow = bathymetry_height(&params, 0.0, 0.0, shelf_width, &w, &d, &s);
        let deep = bathymetry_height(&params, 0.0, 0.0, shelf_width * 2.0, &w, &d, &s);
        assert!(
            deep < shallow,
            "depth at 2x shelf ({deep}) should exceed 1x ({shallow})"
        );
    }

    #[test]
    fn shelf_width_varies_by_seed() {
        let mut a = IslandGenParams::default();
        a.seed = 1;
        let mut b = IslandGenParams::default();
        b.seed = 99_999;
        let wx = 120.0;
        let wz = -40.0;
        let dist = 80.0;
        let (wa, da, sa) = bathymetry_noises(&a);
        let (wb, db, sb) = bathymetry_noises(&b);
        let ha = bathymetry_height(&a, wx, wz, dist, &wa, &da, &sa);
        let hb = bathymetry_height(&b, wx, wz, dist, &wb, &db, &sb);
        assert!(
            (ha - hb).abs() > 0.01,
            "different seeds should change shelf profile at the same point"
        );
    }
}
