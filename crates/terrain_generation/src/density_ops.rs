// crates/terrain_generation/src/density_ops.rs
/// Solid union: keep the more-interior (more negative) value.
#[inline]
pub fn solid_union(a: f32, b: f32) -> f32 {
    a.min(b)
}

/// Subtract a cavity from solid terrain.
#[inline]
pub fn solid_subtract(solid: f32, cavity: f32) -> f32 {
    solid.max(-cavity)
}

/// Signed distance to a plane `y = height` (negative below = solid).
#[inline]
pub fn plane_density(y: f32, height: f32) -> f32 {
    y - height
}

/// Sphere SDF: negative inside, positive outside.
#[inline]
pub fn sphere_density(x: f32, y: f32, z: f32, cx: f32, cy: f32, cz: f32, radius: f32) -> f32 {
    let dx = x - cx;
    let dy = y - cy;
    let dz = z - cz;
    (dx * dx + dy * dy + dz * dz).sqrt() - radius
}

/// Capsule SDF between endpoints `a` and `b`.
pub fn capsule_sdf(
    px: f32,
    py: f32,
    pz: f32,
    ax: f32,
    ay: f32,
    az: f32,
    bx: f32,
    by: f32,
    bz: f32,
    radius: f32,
) -> f32 {
    let abx = bx - ax;
    let aby = by - ay;
    let abz = bz - az;
    let apx = px - ax;
    let apy = py - ay;
    let apz = pz - az;
    let ab_len_sq = abx * abx + aby * aby + abz * abz;
    let t = if ab_len_sq <= f32::EPSILON {
        0.0
    } else {
        ((apx * abx + apy * aby + apz * abz) / ab_len_sq).clamp(0.0, 1.0)
    };
    let cx = ax + abx * t;
    let cy = ay + aby * t;
    let cz = az + abz * t;
    let dx = px - cx;
    let dy = py - cy;
    let dz = pz - cz;
    (dx * dx + dy * dy + dz * dz).sqrt() - radius
}

/// Ellipsoid SDF (axis-aligned). Returns approximate metric distance: the zero-set is
/// exact, but gradient magnitude varies with axis radii before scaling.
pub fn ellipsoid_sdf(
    px: f32,
    py: f32,
    pz: f32,
    cx: f32,
    cy: f32,
    cz: f32,
    rx: f32,
    ry: f32,
    rz: f32,
) -> f32 {
    let scale = rx.min(ry).min(rz);
    let dx = (px - cx) / rx;
    let dy = (py - cy) / ry;
    let dz = (pz - cz) / rz;
    ((dx * dx + dy * dy + dz * dz).sqrt() - 1.0) * scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_convention_negative_is_solid() {
        assert!(plane_density(0.0, 5.0) < 0.0);
        assert!(plane_density(10.0, 5.0) > 0.0);
    }

    #[test]
    fn solid_union_keeps_interior() {
        assert_eq!(solid_union(-2.0, -5.0), -5.0);
    }

    #[test]
    fn solid_subtract_creates_cavity() {
        let base = plane_density(5.0, 10.0);
        let cavity = sphere_density(0.0, 5.0, 0.0, 0.0, 5.0, 0.0, 3.0);
        let result = solid_subtract(base, cavity);
        assert!(result > base);
    }
}
