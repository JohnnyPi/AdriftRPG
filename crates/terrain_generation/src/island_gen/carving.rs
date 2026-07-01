//! Channel and valley carving (VS3 §6).

use crate::field2d::Field2D;
use crate::island_gen::params::IslandGenParams;
use crate::river::{river_carve_offset, river_channel_at};
use crate::water_body::RiverSpline;

pub fn carve_river_channels(
    elevation: &mut Field2D<f32>,
    river: &RiverSpline,
    params: &IslandGenParams,
) {
    let bank_width = 3.5f32;
    elevation.for_each_world(|wx, wz, h| {
        let (dist, half_width, depth) = river_channel_at(river, wx, wz);
        let offset = river_carve_offset(dist, half_width, bank_width, depth);
        if offset > 0.0 {
            *h -= offset * 1.2;
        }
        let _ = params;
    });
}

pub fn compute_slope(elevation: &Field2D<f32>) -> Field2D<f32> {
    let mut slope = Field2D::<f32>::new(
        elevation.width,
        elevation.height,
        elevation.origin,
        elevation.spacing,
    );
    let s = elevation.spacing;
    for z in 1..elevation.height - 1 {
        for x in 1..elevation.width - 1 {
            let dx = (elevation.get(x + 1, z) - elevation.get(x - 1, z)) / (2.0 * s);
            let dz = (elevation.get(x, z + 1) - elevation.get(x, z - 1)) / (2.0 * s);
            let angle = (dx * dx + dz * dz).sqrt().atan().to_degrees();
            slope.set(x, z, angle);
        }
    }
    slope
}
