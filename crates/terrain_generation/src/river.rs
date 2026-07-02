// crates/terrain_generation/src/river.rs
//! Small river routing and carving (VS2 §6).

use crate::field_stack::FieldStackParams;
use crate::surface_height::land_surface_height;
use crate::TerrainRecipe;
use crate::water_body::{RiverControlPoint, RiverSpline};

#[derive(Clone, Debug)]
pub struct RiverGenConfig {
    pub source_center: [f32; 2],
    pub source_radius_m: f32,
    pub grid_spacing_m: f32,
    pub mouth_width_m: f32,
    pub source_width_m: f32,
    pub source_depth_m: f32,
    pub mouth_depth_m: f32,
    pub bank_width_m: f32,
    pub minimum_depth_m: f32,
    pub depression_repair_radius_cells: u32,
    pub maximum_breach_depth_m: f32,
    pub seed: u64,
    pub field_stack: FieldStackParams,
    pub surface_recipe: Option<TerrainRecipe>,
}

impl Default for RiverGenConfig {
    fn default() -> Self {
        Self {
            source_center: [82.0, 196.0],
            source_radius_m: 24.0,
            grid_spacing_m: 2.0,
            mouth_width_m: 6.5,
            source_width_m: 1.8,
            source_depth_m: 0.4,
            mouth_depth_m: 1.6,
            bank_width_m: 3.5,
            minimum_depth_m: 0.25,
            depression_repair_radius_cells: 2,
            maximum_breach_depth_m: 1.5,
            seed: 48129,
            field_stack: FieldStackParams::default(),
            surface_recipe: Some(crate::default_vertical_slice_recipe(48129, 0.0)),
        }
    }
}

pub fn generate_river_spline(config: &RiverGenConfig, sea_level: f32) -> Option<RiverSpline> {
    let mut path = trace_downhill(config, sea_level)?;
    if path.len() < 4 {
        return None;
    }
    smooth_path(&mut path);
    repair_depressions(&mut path, config);
    let mut points = Vec::new();
    let n = path.len();
    for (i, (x, z, bed)) in path.iter().enumerate() {
        let t = i as f32 / (n - 1) as f32;
        let width = config.source_width_m + (config.mouth_width_m - config.source_width_m) * t;
        let depth = config.source_depth_m + (config.mouth_depth_m - config.source_depth_m) * t;
        points.push(RiverControlPoint {
            position_xz: [*x, *z],
            bed_elevation: *bed,
            water_elevation: (*bed + config.minimum_depth_m).max(sea_level),
            width,
            depth,
            discharge: 1.0 - t * 0.3,
        });
    }
    for i in (1..points.len()).rev() {
        let downstream = points[i].water_elevation;
        let bed = points[i - 1].bed_elevation;
        points[i - 1].water_elevation = points[i - 1]
            .water_elevation
            .max(bed + config.minimum_depth_m)
            .max(downstream);
    }
    for i in 1..points.len() {
        if points[i].water_elevation > points[i - 1].water_elevation {
            points[i].water_elevation = points[i - 1].water_elevation;
        }
        points[i].water_elevation = points[i].water_elevation.max(points[i].bed_elevation + config.minimum_depth_m);
    }
    Some(RiverSpline { points })
}

fn trace_downhill(config: &RiverGenConfig, sea_level: f32) -> Option<Vec<(f32, f32, f32)>> {
    let (sx, sz) = find_source(config, sea_level)?;
    let mut path = vec![(sx, sz, surface_at(config, sx, sz, sea_level))];
    let mut visited = std::collections::HashSet::new();
    visited.insert(grid_key(sx, sz, config.grid_spacing_m));

    for _ in 0..200 {
        let (x, z, _) = *path.last()?;
        if surface_at(config, x, z, sea_level) <= sea_level + 0.5 {
            break;
        }
        let neighbors = [
            (config.grid_spacing_m, 0.0),
            (-config.grid_spacing_m, 0.0),
            (0.0, config.grid_spacing_m),
            (0.0, -config.grid_spacing_m),
        ];
        let mut best: Option<(f32, f32, f32)> = None;
        for (dx, dz) in neighbors {
            let nx = x + dx;
            let nz = z + dz;
            let key = grid_key(nx, nz, config.grid_spacing_m);
            if visited.contains(&key) {
                continue;
            }
            let h = surface_at(config, nx, nz, sea_level);
            if best.map(|(_, _, bh)| h < bh).unwrap_or(true) {
                best = Some((nx, nz, h));
            }
        }
        let Some((nx, nz, nh)) = best else {
            break;
        };
        visited.insert(grid_key(nx, nz, config.grid_spacing_m));
        path.push((nx, nz, nh));
    }
    if let Some((x, z, h)) = path.last().copied() {
        if h > sea_level + 0.5 {
            path.push((x, z, sea_level));
        }
    }
    if path.len() < 4 {
        return None;
    }
    Some(path)
}

fn surface_at(config: &RiverGenConfig, x: f32, z: f32, sea_level: f32) -> f32 {
    if let Some(recipe) = &config.surface_recipe {
        land_surface_height(recipe, x, z)
    } else {
        sea_level
    }
}

fn repair_depressions(path: &mut Vec<(f32, f32, f32)>, config: &RiverGenConfig) {
    if path.len() < 3 {
        return;
    }
    let radius = config.depression_repair_radius_cells as usize;
    let max_breach = config.maximum_breach_depth_m;
    for i in 1..path.len() - 1 {
        let downstream_min = path[i + 1..]
            .iter()
            .take(radius.max(1))
            .map(|(_, _, h)| *h)
            .fold(f32::INFINITY, f32::min);
        let (_, _, h) = &mut path[i];
        if *h > downstream_min + 0.05 {
            *h = (*h - max_breach).max(downstream_min);
        }
    }
}

fn find_source(config: &RiverGenConfig, sea_level: f32) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32, f32)> = None;
    let r = config.source_radius_m;
    let step = config.grid_spacing_m;
    let mut x = config.source_center[0] - r;
    while x <= config.source_center[0] + r {
        let mut z = config.source_center[1] - r;
        while z <= config.source_center[1] + r {
            let dx = x - config.source_center[0];
            let dz = z - config.source_center[1];
            if dx * dx + dz * dz <= r * r {
                let h = surface_at(config, x, z, sea_level);
                if h > sea_level + 8.0 {
                    if best.map(|(_, _, bh)| h > bh).unwrap_or(true) {
                        best = Some((x, z, h));
                    }
                }
            }
            z += step;
        }
        x += step;
    }
    best.map(|(x, z, _)| (x, z))
}

fn grid_key(x: f32, z: f32, spacing: f32) -> (i32, i32) {
    ((x / spacing).round() as i32, (z / spacing).round() as i32)
}

fn smooth_path(path: &mut Vec<(f32, f32, f32)>) {
    if path.len() < 3 {
        return;
    }
    let mut smoothed = path.clone();
    for i in 1..path.len() - 1 {
        let (x0, z0, h0) = path[i - 1];
        let (x1, z1, h1) = path[i];
        let (x2, z2, h2) = path[i + 1];
        smoothed[i] = (
            (x0 + 2.0 * x1 + x2) / 4.0,
            (z0 + 2.0 * z1 + z2) / 4.0,
            (h0 + 2.0 * h1 + h2) / 4.0,
        );
    }
    *path = smoothed;
}

pub fn river_carve_offset(distance: f32, half_width: f32, bank_width: f32, depth: f32) -> f32 {
    let bed_factor = 1.0 - smoothstep(0.0, half_width, distance);
    let bank_factor = 1.0 - smoothstep(half_width, half_width + bank_width, distance);
    bed_factor * depth + bank_factor * depth * 0.35
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 >= edge1 {
        return 0.0;
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn distance_to_river_centerline(spline: &RiverSpline, x: f32, z: f32) -> f32 {
    let mut min_dist = f32::MAX;
    for i in 0..spline.points.len().saturating_sub(1) {
        let a = &spline.points[i];
        let b = &spline.points[i + 1];
        let d = dist_point_segment(
            x,
            z,
            a.position_xz[0],
            a.position_xz[1],
            b.position_xz[0],
            b.position_xz[1],
        );
        min_dist = min_dist.min(d);
    }
    min_dist
}

pub fn river_channel_at(spline: &RiverSpline, x: f32, z: f32) -> (f32, f32, f32) {
    let mut best_dist = f32::MAX;
    let mut half_width = 1.0;
    let mut depth = 0.5;
    for i in 0..spline.points.len().saturating_sub(1) {
        let a = &spline.points[i];
        let b = &spline.points[i + 1];
        let (dist, t) = dist_point_segment_t(
            x,
            z,
            a.position_xz[0],
            a.position_xz[1],
            b.position_xz[0],
            b.position_xz[1],
        );
        if dist < best_dist {
            best_dist = dist;
            half_width = (a.width + (b.width - a.width) * t) * 0.5;
            depth = a.depth + (b.depth - a.depth) * t;
        }
    }
    (best_dist, half_width, depth)
}

fn dist_point_segment_t(px: f32, pz: f32, ax: f32, az: f32, bx: f32, bz: f32) -> (f32, f32) {
    let abx = bx - ax;
    let abz = bz - az;
    let apx = px - ax;
    let apz = pz - az;
    let ab_len2 = abx * abx + abz * abz;
    if ab_len2 < 1e-6 {
        return ((apx * apx + apz * apz).sqrt(), 0.0);
    }
    let t = ((apx * abx + apz * abz) / ab_len2).clamp(0.0, 1.0);
    let cx = ax + abx * t;
    let cz = az + abz * t;
    let dx = px - cx;
    let dz = pz - cz;
    ((dx * dx + dz * dz).sqrt(), t)
}

fn dist_point_segment(px: f32, pz: f32, ax: f32, az: f32, bx: f32, bz: f32) -> f32 {
    let abx = bx - ax;
    let abz = bz - az;
    let apx = px - ax;
    let apz = pz - az;
    let ab_len2 = abx * abx + abz * abz;
    if ab_len2 < 1e-6 {
        return (apx * apx + apz * apz).sqrt();
    }
    let t = ((apx * abx + apz * abz) / ab_len2).clamp(0.0, 1.0);
    let cx = ax + abx * t;
    let cz = az + abz * t;
    let dx = px - cx;
    let dz = pz - cz;
    (dx * dx + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn river_reaches_sea_with_monotonic_water() {
        let config = RiverGenConfig::default();
        let spline = generate_river_spline(&config, 0.0).expect("river");
        assert!(spline.points.len() >= 4);
        let last = spline.points.last().unwrap();
        assert!(last.bed_elevation <= 1.0);
        for w in spline.points.windows(2) {
            assert!(w[0].water_elevation + 0.01 >= w[1].water_elevation);
            assert!(w[0].water_elevation >= w[0].bed_elevation);
        }
    }

    #[test]
    fn river_is_deterministic() {
        let a = generate_river_spline(&RiverGenConfig::default(), 0.0).unwrap();
        let b = generate_river_spline(&RiverGenConfig::default(), 0.0).unwrap();
        assert_eq!(a.points.len(), b.points.len());
    }

    #[test]
    fn depression_repair_lowers_local_high_points() {
        let mut path = vec![
            (0.0, 0.0, 10.0),
            (2.0, 0.0, 12.0),
            (4.0, 0.0, 8.0),
            (6.0, 0.0, 7.0),
        ];
        let config = RiverGenConfig::default();
        repair_depressions(&mut path, &config);
        assert!(path[1].2 < 12.0, "repair should lower depression peak");
        assert!(path[1].2 >= path[2].2);
    }

    #[test]
    fn channel_width_grows_toward_mouth() {
        let config = RiverGenConfig::default();
        let spline = generate_river_spline(&config, 0.0).expect("river");
        let source_w = spline.points.first().unwrap().width;
        let mouth_w = spline.points.last().unwrap().width;
        assert!(mouth_w > source_w);
    }

    #[test]
    fn river_carve_lowers_surface_near_channel() {
        use crate::recipe::{default_vertical_slice_recipe, RecipeDensitySource, RiverCarveContext};

        let config = RiverGenConfig::default();
        let spline = generate_river_spline(&config, 0.0).expect("river");
        let mid = &spline.points[spline.points.len() / 2];
        let (x, z) = (mid.position_xz[0], mid.position_xz[1]);
        let (dist, half_w, depth) = river_channel_at(&spline, x, z);
        let carve = river_carve_offset(dist, half_w, config.bank_width_m, depth);
        assert!(carve > 0.05, "expected meaningful carve at river centerline");

        let base = RecipeDensitySource::new(default_vertical_slice_recipe(42, 0.0));
        let carved = base.clone().with_river_carve(RiverCarveContext {
            spline,
            bank_width_m: config.bank_width_m,
        });
        let base_height = base.terrain_surface_height_at(x, z);
        let carved_height = carved.terrain_surface_height_at(x, z);
        assert!(
            carved_height < base_height - 0.05,
            "centerline surface should drop (base={base_height}, carved={carved_height})"
        );

        let bank_x = x + half_w * 0.5;
        let bank_base = base.terrain_surface_height_at(bank_x, z);
        let bank_carved = carved.terrain_surface_height_at(bank_x, z);
        assert!(
            bank_carved < bank_base - 0.02,
            "bank surface should also drop (base={bank_base}, carved={bank_carved})"
        );
    }
}
