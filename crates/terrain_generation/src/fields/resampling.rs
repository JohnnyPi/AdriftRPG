//! Field resampling between resolution tiers.

use crate::contract::coordinates::WorldXZ;

use super::dense::DenseField2D;
use super::descriptor::FieldDescriptor;
use super::sampling::ScalarSampling;
use super::sampling::sample_bilinear;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OutOfBoundsPolicy {
    Clamp,
    Constant(f32),
}

pub fn resample_scalar(
    source: &DenseField2D<f32>,
    target_desc: FieldDescriptor,
    oob: OutOfBoundsPolicy,
) -> DenseField2D<f32> {
    let mut out = DenseField2D::zeros(target_desc.clone());
    for z in 0..target_desc.height {
        for x in 0..target_desc.width {
            let wx = target_desc.origin_x() + x as f64 * target_desc.cell_size_m;
            let wz = target_desc.origin_z() + z as f64 * target_desc.cell_size_m;
            let world = WorldXZ::new(wx, wz);
            let v = match oob {
                OutOfBoundsPolicy::Clamp => {
                    sample_bilinear(source, world, ScalarSampling::Bilinear)
                }
                OutOfBoundsPolicy::Constant(c) => {
                    if in_source_bounds(source, world) {
                        sample_bilinear(source, world, ScalarSampling::Bilinear)
                    } else {
                        c
                    }
                }
            };
            out.set(x, z, v);
        }
    }
    out
}

fn in_source_bounds(source: &DenseField2D<f32>, world: WorldXZ) -> bool {
    let d = &source.descriptor;
    let max_x = d.origin_x() + d.extent_x_m();
    let max_z = d.origin_z() + d.extent_z_m();
    world.x() >= d.origin_x()
        && world.x() <= max_x
        && world.z() >= d.origin_z()
        && world.z() <= max_z
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::descriptor::FieldDescriptor;
    use crate::fields::key::FieldKey;

    #[test]
    fn resample_constant_field_stays_constant() {
        let desc = FieldDescriptor::new(FieldKey::BaseElevation, WorldXZ::new(0.0, 0.0), 4.0, 4, 4);
        let mut src = DenseField2D::zeros(desc.clone());
        for z in 0..4 {
            for x in 0..4 {
                src.set(x, z, 42.0);
            }
        }
        let target =
            FieldDescriptor::new(FieldKey::BaseElevation, WorldXZ::new(0.0, 0.0), 2.0, 8, 8);
        let out = resample_scalar(&src, target, OutOfBoundsPolicy::Clamp);
        for v in &out.values {
            assert!((*v - 42.0).abs() < 0.01);
        }
    }
}
