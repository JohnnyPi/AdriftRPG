//! Field sampling policies.

use crate::contract::coordinates::WorldXZ;

use super::dense::DenseField2D;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScalarSampling {
    Nearest,
    Bilinear,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CategoricalSampling {
    Nearest,
}

pub fn sample_bilinear(field: &DenseField2D<f32>, world: WorldXZ, _policy: ScalarSampling) -> f32 {
    let d = &field.descriptor;
    if d.width == 0 || d.height == 0 {
        return 0.0;
    }
    let (lx, lz) = field.world_to_grid(world);
    let max_x = (d.width.saturating_sub(1)) as f64;
    let max_z = (d.height.saturating_sub(1)) as f64;
    let lx = lx.clamp(0.0, max_x);
    let lz = lz.clamp(0.0, max_z);

    let x0 = (lx.floor() as u32).min(d.width.saturating_sub(1));
    let z0 = (lz.floor() as u32).min(d.height.saturating_sub(1));
    let x1 = (x0 + 1).min(d.width.saturating_sub(1));
    let z1 = (z0 + 1).min(d.height.saturating_sub(1));
    let fx = (lx - x0 as f64) as f32;
    let fz = (lz - z0 as f64) as f32;

    let a = field.get(x0, z0);
    let b = field.get(x1, z0);
    let c = field.get(x0, z1);
    let d_val = field.get(x1, z1);
    let ab = a + (b - a) * fx;
    let cd = c + (d_val - c) * fx;
    ab + (cd - ab) * fz
}

pub fn sample_nearest_u32(field: &DenseField2D<u32>, world: WorldXZ) -> u32 {
    let d = &field.descriptor;
    let (lx, lz) = (
        (world.x() - d.origin_x()) / d.cell_size_m,
        (world.z() - d.origin_z()) / d.cell_size_m,
    );
    let x = (lx.round() as i64).clamp(0, d.width.saturating_sub(1) as i64) as u32;
    let z = (lz.round() as i64).clamp(0, d.height.saturating_sub(1) as i64) as u32;
    field.get(x, z)
}
