//! Bathymetry and shelf profiles (VS3 §10).

use crate::field2d::smoothstep;
use crate::island_gen::params::IslandGenParams;

pub fn bathymetry_height(params: &IslandGenParams, _wx: f32, _wz: f32, coast_distance: f32) -> f32 {
    let sea = params.island.sea_level_m;
    if coast_distance <= 0.0 {
        return sea;
    }
    let coast = &params.coast;
    let shelf_width = coast.shelf_width_min_m
        + (coast.shelf_width_max_m - coast.shelf_width_min_m) * 0.55;
    let shelf_depth = coast.shelf_depth_min_m
        + (coast.shelf_depth_max_m - coast.shelf_depth_min_m) * 0.5;
    let deep_slope = coast.deep_slope_min
        + (coast.deep_slope_max - coast.deep_slope_min) * 0.4;

    if coast_distance < shelf_width {
        let t = (coast_distance / shelf_width).clamp(0.0, 1.0);
        return sea - shelf_depth * smoothstep(0.0, 1.0, t);
    }
    let beyond = coast_distance - shelf_width;
    sea - shelf_depth - beyond * deep_slope
}

pub fn compute_coast_distance(mask: &crate::field2d::Field2D<f32>, spacing: f32) -> crate::field2d::Field2D<f32> {
    let mut dist = crate::field2d::Field2D::new(mask.width, mask.height, mask.origin, spacing);
    for z in 0..mask.height {
        for x in 0..mask.width {
            let land = mask.get(x, z);
            if land > 0.5 {
                dist.set(x, z, 0.0);
                continue;
            }
            let mut min_d = f32::MAX;
            let search = ((20.0 / spacing).ceil() as i32).min(40);
            for dz in -search..=search {
                for dx in -search..=search {
                    let nx = x as i32 + dx;
                    let nz = z as i32 + dz;
                    if nx < 0 || nz < 0 || nx >= mask.width as i32 || nz >= mask.height as i32 {
                        continue;
                    }
                    if mask.get(nx as u32, nz as u32) > 0.5 {
                        let d = ((dx * dx + dz * dz) as f32).sqrt() * spacing;
                        min_d = min_d.min(d);
                    }
                }
            }
            dist.set(x, z, if min_d.is_finite() { min_d } else { spacing * 40.0 });
        }
    }
    dist
}
