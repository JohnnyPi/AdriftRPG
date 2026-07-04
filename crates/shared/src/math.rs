//! Canonical scalar math helpers shared across engine crates.

/// Hermite interpolation on the unit interval after clamping `value` into `[start, end]`.
#[inline]
pub fn smoothstep(start: f32, end: f32, value: f32) -> f32 {
    if (end - start).abs() < f32::EPSILON {
        return if value >= end { 1.0 } else { 0.0 };
    }
    let t = saturate((value - start) / (end - start));
    t * t * (3.0 - 2.0 * t)
}

/// Linear interpolation.
#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Component-wise RGB linear interpolation.
#[inline]
pub fn lerp_rgb(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
    ]
}

/// Clamp `value` to `[0, 1]`.
#[inline]
pub fn saturate(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

/// Smooth band weight: 1 inside `[min, max]` with `fade` falloff at edges.
#[inline]
pub fn range_weight(value: f32, min: f32, max: f32, fade: f32) -> f32 {
    smoothstep(min - fade, min, value) * (1.0 - smoothstep(max, max + fade, value))
}

/// Remap `value` from `[from_min, from_max]` into `[to_min, to_max]` with clamped normalization.
#[inline]
pub fn remap(value: f32, from_min: f32, from_max: f32, to_min: f32, to_max: f32) -> f32 {
    let t = if (from_max - from_min).abs() < f32::EPSILON {
        0.0
    } else {
        ((value - from_min) / (from_max - from_min)).clamp(0.0, 1.0)
    };
    to_min + t * (to_max - to_min)
}

/// Surface slope in degrees from a (not necessarily unit) world-space normal.
#[inline]
pub fn slope_degrees(normal: [f32; 3]) -> f32 {
    let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if len <= f32::EPSILON {
        return 0.0;
    }
    let ny = (normal[1] / len).clamp(-1.0, 1.0);
    ny.acos().to_degrees()
}

/// Deterministic unit float in `[0, 1)` from a seed and index (Murmur-style mixing).
#[inline]
pub fn hash_unit(seed: u64, index: u32) -> f32 {
    let mut value = seed
        ^ (index as u64)
            .wrapping_add(1)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    ((value >> 40) as u32) as f32 / u32::MAX as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothstep_boundaries() {
        assert_eq!(smoothstep(0.0, 1.0, -0.5), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 1.5), 1.0);
        assert!((smoothstep(0.0, 1.0, 0.5) - 0.5).abs() < f32::EPSILON);
        assert_eq!(smoothstep(1.0, 1.0, 1.5), 1.0);
        assert_eq!(smoothstep(1.0, 1.0, 0.0), 0.0);
        assert_eq!(smoothstep(1.0, 1.0, 0.5), 0.0);
    }

    #[test]
    fn lerp_endpoints() {
        assert_eq!(lerp(2.0, 8.0, 0.0), 2.0);
        assert_eq!(lerp(2.0, 8.0, 1.0), 8.0);
        assert_eq!(lerp(2.0, 8.0, 0.5), 5.0);
    }

    #[test]
    fn saturate_clamps() {
        assert_eq!(saturate(-1.0), 0.0);
        assert_eq!(saturate(0.5), 0.5);
        assert_eq!(saturate(2.0), 1.0);
    }

    #[test]
    fn remap_linear() {
        assert_eq!(remap(5.0, 0.0, 10.0, 0.0, 100.0), 50.0);
        assert_eq!(remap(-1.0, 0.0, 10.0, 0.0, 100.0), 0.0);
        assert_eq!(remap(20.0, 0.0, 10.0, 0.0, 100.0), 100.0);
    }

    #[test]
    fn slope_degrees_up_and_flat() {
        assert!((slope_degrees([0.0, 1.0, 0.0]) - 0.0).abs() < f32::EPSILON);
        assert!((slope_degrees([0.0, 0.0, 1.0]) - 90.0).abs() < f32::EPSILON);
    }

    #[test]
    fn range_weight_band() {
        assert_eq!(range_weight(5.0, 10.0, 20.0, 2.0), 0.0);
        assert!((range_weight(15.0, 10.0, 20.0, 2.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn hash_unit_is_deterministic_and_bounded() {
        let a = hash_unit(42, 7);
        let b = hash_unit(42, 7);
        assert_eq!(a, b);
        assert!((0.0..1.0).contains(&a));
        assert_ne!(hash_unit(42, 8), a);
    }
}
