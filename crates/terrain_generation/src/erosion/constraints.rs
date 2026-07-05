//! Reapply elevation constraints after erosion.

use crate::fields::scalar::ScalarField;

pub fn reapply_constraints(
    eroded: &mut ScalarField,
    original: &ScalarField,
    value_constraint: &ScalarField,
    land_mask: &ScalarField,
) {
    for z in 0..eroded.descriptor.height {
        for x in 0..eroded.descriptor.width {
            if land_mask.get(x, z) < 0.3 {
                continue;
            }
            let c = value_constraint.get(x, z).clamp(0.0, 1.0);
            if c > 0.01 {
                let orig = original.get(x, z);
                let er = eroded.get(x, z);
                eroded.set(x, z, orig * c + er * (1.0 - c));
            }
        }
    }
}
