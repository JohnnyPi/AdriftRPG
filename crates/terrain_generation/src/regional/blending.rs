//! Raised-cosine window blending for regional residuals.

pub fn raised_cosine_weight(t: f32) -> f32 {
    0.5 - 0.5 * (std::f32::consts::PI * t).cos()
}

pub fn window_weight_2d(fx: f32, fz: f32) -> f32 {
    let wx = raised_cosine_weight(fx.clamp(0.0, 1.0));
    let wz = raised_cosine_weight(fz.clamp(0.0, 1.0));
    wx * wz
}
