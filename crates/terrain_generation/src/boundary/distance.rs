//! Distance to rectangular world edge.

use glam::DVec2;

pub fn distance_to_rect_edge(p: DVec2, half_extent: DVec2) -> f64 {
    let dx = half_extent.x - p.x.abs();
    let dz = half_extent.y - p.y.abs();
    dx.min(dz).max(0.0)
}

pub fn normalized_interior_mask(edge_distance: f64, half_extent: f64, start_fraction: f32) -> f32 {
    let start = half_extent * start_fraction as f64;
    if edge_distance >= start {
        return 1.0;
    }
    smoothstep(0.0, start, edge_distance) as f32
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    if edge1 <= edge0 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
