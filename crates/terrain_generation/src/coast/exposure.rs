//! Coastal wave exposure from climate and bathymetry.

use crate::fields::scalar::ScalarField;

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 >= edge1 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn compute_coastal_wave_exposure(
    wind_exposure: &ScalarField,
    coast_distance: &ScalarField,
    bathymetry: &ScalarField,
    land_mask: &ScalarField,
    sea_level_m: f32,
) -> ScalarField {
    let mut out = ScalarField::zeros(wind_exposure.descriptor.clone());
    for z in 0..out.descriptor.height {
        for x in 0..out.descriptor.width {
            let wind = wind_exposure.get(x, z);
            let coast = coast_distance.get(x, z);
            let depth = (sea_level_m - bathymetry.get(x, z)).max(0.0);
            let land = land_mask.get(x, z);

            let near_coast = if land > 0.5 {
                1.0 - smoothstep(0.0, 400.0, coast.max(0.0))
            } else {
                1.0 - smoothstep(0.0, 800.0, coast.abs())
            };
            let shallow = 1.0 - smoothstep(5.0, 40.0, depth);
            out.set(
                x,
                z,
                (wind * 0.7 + near_coast * 0.2 + shallow * 0.1).clamp(0.0, 1.0),
            );
        }
    }
    out
}
