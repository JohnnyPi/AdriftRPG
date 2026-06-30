//! Traversal waypoint smoke test for expanded slice routes.

#[cfg(test)]
mod traversal_tests {
    use crate::{generate_river_spline, stack_surface_height, FieldStackParams, RiverGenConfig};

  #[test]
    fn drainage_basin_has_traversable_elevation_range() {
        let params = FieldStackParams::default();
        let samples = [
            (-30.0, -25.0),
            (82.0, 196.0),
            (128.0, 128.0),
            (200.0, 180.0),
        ];
        for (x, z) in samples {
            let h = stack_surface_height(x, z, 0.0, 48129, &params);
            assert!(h > -2.0 && h < 40.0, "landmark ({x},{z}) height {h} out of range");
        }
    }

    #[test]
    fn river_connects_upland_to_coast() {
        let spline = generate_river_spline(&RiverGenConfig::default(), 0.0).expect("river");
        let first = spline.points.first().unwrap();
        let last = spline.points.last().unwrap();
        assert!(first.bed_elevation > 8.0);
        assert!(last.bed_elevation < 2.0);
    }
}
