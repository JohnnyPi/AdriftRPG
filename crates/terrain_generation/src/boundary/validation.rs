//! Boundary perimeter validation.

use crate::fields::scalar::ScalarField;

pub struct BoundaryValidationReport {
    pub perimeter_below_threshold: bool,
    pub max_perimeter_elevation: f32,
    pub passed: bool,
    pub messages: Vec<String>,
}

pub fn validate_boundary_perimeter(
    ocean_basin: &ScalarField,
    deep_ocean_threshold_m: f32,
) -> BoundaryValidationReport {
    let w = ocean_basin.descriptor.width;
    let h = ocean_basin.descriptor.height;
    let mut max_elev = f32::NEG_INFINITY;
    let mut all_deep = true;
    let mut messages = Vec::new();

    for x in 0..w {
        for z in [0, h - 1] {
            let v = ocean_basin.get(x, z);
            max_elev = max_elev.max(v);
            if v > deep_ocean_threshold_m {
                all_deep = false;
            }
        }
    }
    for z in 0..h {
        for x in [0, w - 1] {
            let v = ocean_basin.get(x, z);
            max_elev = max_elev.max(v);
            if v > deep_ocean_threshold_m {
                all_deep = false;
            }
        }
    }

    if !all_deep {
        messages.push(format!(
            "perimeter elevation {max_elev} exceeds deep ocean threshold {deep_ocean_threshold_m}"
        ));
    }

    BoundaryValidationReport {
        perimeter_below_threshold: all_deep,
        max_perimeter_elevation: max_elev,
        passed: all_deep,
        messages,
    }
}
