// crates/procedural_textures/src/normal.rs
pub fn normals_from_height_field(
    width: u32,
    height: u32,
    height_field: &[f32],
    strength: f32,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let mut normal = vec![0u8; w * h * 4];

    for y in 0..h {
        for x in 0..w {
            let left = height_field[y * w + (x + w - 1) % w];
            let right = height_field[y * w + (x + 1) % w];
            let down = height_field[((y + h - 1) % h) * w + x];
            let up = height_field[((y + 1) % h) * w + x];

            let dx = (right - left) * strength;
            let dy = (up - down) * strength;
            let mut n = [-dx, -dy, 1.0f32];
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(f32::EPSILON);
            n[0] /= len;
            n[1] /= len;
            n[2] /= len;

            let idx = (y * w + x) * 4;
            normal[idx] = ((n[0] * 0.5 + 0.5) * 255.0).round() as u8;
            normal[idx + 1] = ((n[1] * 0.5 + 0.5) * 255.0).round() as u8;
            normal[idx + 2] = ((n[2] * 0.5 + 0.5) * 255.0).round() as u8;
            normal[idx + 3] = 255;
        }
    }
    normal
}
