// crates/procedural_textures/src/seam.rs
//! Seamless texture edge validation.

use crate::maps::GeneratedPbrMaps;

pub const DEFAULT_SEAM_TOLERANCE: f32 = 0.02;

pub fn maximum_texture_seam_error(maps: &GeneratedPbrMaps) -> f32 {
    let w = maps.width as usize;
    let h = maps.height as usize;
    if w < 2 || h < 2 {
        return 0.0;
    }

    let albedo_h = compare_left_right_edges(&maps.albedo_rgba8, w, h);
    let albedo_v = compare_top_bottom_edges(&maps.albedo_rgba8, w, h);
    let normal_h = compare_left_right_edges(&maps.normal_rgba8, w, h);
    let normal_v = compare_top_bottom_edges(&maps.normal_rgba8, w, h);
    let ormh_h = compare_left_right_edges(&maps.ormh_rgba8, w, h);
    let ormh_v = compare_top_bottom_edges(&maps.ormh_rgba8, w, h);
    albedo_h
        .max(albedo_v)
        .max(normal_h)
        .max(normal_v)
        .max(ormh_h)
        .max(ormh_v)
}

fn compare_left_right_edges(buffer: &[u8], width: usize, height: usize) -> f32 {
    let mut max_err = 0.0f32;
    for y in 0..height {
        let left = y * width * 4;
        let right = (y * width + (width - 1)) * 4;
        for c in 0..3 {
            let a = buffer[left + c] as f32 / 255.0;
            let b = buffer[right + c] as f32 / 255.0;
            max_err = max_err.max((a - b).abs());
        }
    }
    max_err
}

fn compare_top_bottom_edges(buffer: &[u8], width: usize, height: usize) -> f32 {
    let mut max_err = 0.0f32;
    for x in 0..width {
        let top = x * 4;
        let bottom = ((height - 1) * width + x) * 4;
        for c in 0..3 {
            let a = buffer[top + c] as f32 / 255.0;
            let b = buffer[bottom + c] as f32 / 255.0;
            max_err = max_err.max((a - b).abs());
        }
    }
    max_err
}

pub fn assert_seamless(maps: &GeneratedPbrMaps, tolerance: f32) -> Result<(), String> {
    let err = maximum_texture_seam_error(maps);
    if err > tolerance {
        return Err(format!(
            "texture seam error {err:.4} exceeds tolerance {tolerance:.4}"
        ));
    }
    Ok(())
}
