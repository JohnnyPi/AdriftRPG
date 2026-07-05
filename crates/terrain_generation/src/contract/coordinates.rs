//! World-space coordinate conventions for the terrain compiler.

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

/// Horizontal world position (+X east, +Z north). World origin is at the center.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldXZ(pub DVec2);

impl WorldXZ {
    pub fn new(x: f64, z: f64) -> Self {
        Self(DVec2::new(x, z))
    }

    pub fn x(&self) -> f64 {
        self.0.x
    }

    pub fn z(&self) -> f64 {
        self.0.y
    }
}

/// Full world position (+Y up).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldPosition(pub DVec3);

impl WorldPosition {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self(DVec3::new(x, y, z))
    }

    pub fn horizontal(&self) -> WorldXZ {
        WorldXZ(DVec2::new(self.0.x, self.0.z))
    }
}

/// Elevation in meters above sea level (sea level = 0).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ElevationMeters(pub f32);

/// Field grid cell size in meters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CellSizeMeters(pub f64);

/// Integer tile coordinate for deterministic seed derivation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileCoord {
    pub x: i32,
    pub z: i32,
}

/// Convert grid cell center to world XZ given descriptor origin and cell size.
pub fn grid_cell_to_world(col: u32, row: u32, origin: WorldXZ, cell_size_m: f64) -> WorldXZ {
    WorldXZ(DVec2::new(
        origin.x() + col as f64 * cell_size_m,
        origin.z() + row as f64 * cell_size_m,
    ))
}

/// Convert world XZ to continuous grid coordinates (column, row).
pub fn world_to_grid_coords(world: WorldXZ, origin: WorldXZ, cell_size_m: f64) -> (f64, f64) {
    (
        (world.x() - origin.x()) / cell_size_m,
        (world.z() - origin.z()) / cell_size_m,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_world_round_trip() {
        let origin = WorldXZ::new(-100.0, -100.0);
        let cell = CellSizeMeters(8.0);
        let col = 5u32;
        let row = 7u32;
        let world = grid_cell_to_world(col, row, origin, cell.0);
        let (gc, gr) = world_to_grid_coords(world, origin, cell.0);
        assert!((gc - col as f64).abs() < 1e-9);
        assert!((gr - row as f64).abs() < 1e-9);
    }
}
