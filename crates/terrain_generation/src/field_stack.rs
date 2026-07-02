// crates/terrain_generation/src/field_stack.rs
//! Composable 2D terrain fields (VS2 §5).

use serde::Serialize;

use crate::field2d::Field2D;

#[derive(Clone, Debug, Default, Serialize)]
pub struct FieldStackParams {
    pub ridge_amplitude: f32,
    pub valley_depth: f32,
    pub coast_blend: f32,
}

pub fn ridge_field(x: f32, z: f32, origin: [f32; 2], scale: [f32; 2], amplitude: f32) -> f32 {
    let dx = (x - origin[0]) / scale[0];
    let dz = (z - origin[1]) / scale[1];
    let r2 = dx * dx + dz * dz;
    if r2 > 1.0 {
        return 0.0;
    }
    let t = 1.0 - r2;
    amplitude * t * t
}

pub fn valley_field(x: f32, z: f32, origin: [f32; 2], scale: [f32; 2], depth: f32) -> f32 {
    let dx = (x - origin[0]) / scale[0].max(f32::EPSILON);
    let dz = (z - origin[1]) / scale[1].max(f32::EPSILON);
    let r2 = dx * dx + dz * dz;
    if r2 > 1.0 {
        return 0.0;
    }
    let t = 1.0 - r2;
    -depth * t * t
}

pub fn build_coast_mask(width: u32, height: u32, origin: [f32; 2], spacing: f32) -> Field2D<f32> {
    let mut field = Field2D::new(width, height, origin, spacing);
    let cx = origin[0] + (width.saturating_sub(1)) as f32 * spacing * 0.5;
    let cz = origin[1] + (height.saturating_sub(1)) as f32 * spacing * 0.5;
    let max_r = ((width.max(height).saturating_sub(1)) as f32 * spacing * 0.5).max(spacing);
    for z in 0..height {
        for x in 0..width {
            let wx = origin[0] + x as f32 * spacing;
            let wz = origin[1] + z as f32 * spacing;
            let dist = ((wx - cx).powi(2) + (wz - cz).powi(2)).sqrt();
            let coast = 1.0 - (dist / max_r).min(1.0);
            field.set(x, z, coast.max(0.0));
        }
    }
    field
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valley_field_has_smooth_rim() {
        let inner = valley_field(128.0, 128.0, [128.0, 128.0], [80.0, 80.0], 6.0);
        let edge = valley_field(208.0, 128.0, [128.0, 128.0], [80.0, 80.0], 6.0);
        assert!(inner < -1.0);
        assert_eq!(edge, 0.0);
    }
}
