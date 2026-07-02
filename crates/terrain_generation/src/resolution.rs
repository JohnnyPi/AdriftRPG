// crates/terrain_generation/src/resolution.rs
//! Generation resolution tiers (PhasedExpansionPlan §2.2).

use std::fmt;

/// Spacing for each generation tier in meters per sample.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenerationResolution {
    /// Island chains, ocean basin, climate regions (250–2,000 m). Zero = disabled for single-island worlds.
    pub world_control_m: f32,
    /// Mountains, valleys, watersheds, coastline (8–128 m).
    pub regional_m: f32,
    /// Beaches, gullies, cliffs, river channels (1–8 m).
    pub local_m: f32,
    /// Caves, overhangs, materials — analytical at sample time (always 1 m).
    pub voxel_m: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolutionError {
    VoxelMustBeOneMeter,
    TiersNotCoarseToFine,
    TiersNotIntegerRatio,
    GridTooLarge { tier: &'static str, cells: u32 },
    WorldControlBelowMinimum,
    RegionalOutOfRange,
    LocalOutOfRange,
}

impl fmt::Display for ResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VoxelMustBeOneMeter => write!(f, "voxel_m must equal 1.0"),
            Self::TiersNotCoarseToFine => {
                write!(f, "resolution tiers must be coarse to fine: world >= regional >= local >= voxel")
            }
            Self::TiersNotIntegerRatio => {
                write!(f, "each resolution tier must divide evenly into the next")
            }
            Self::GridTooLarge { tier, cells } => {
                write!(f, "grid for {tier} exceeds maximum ({cells} cells per axis)")
            }
            Self::WorldControlBelowMinimum => {
                write!(f, "world_control_m must be at least 250 m when enabled")
            }
            Self::RegionalOutOfRange => write!(f, "regional_m must be between 8 and 128 m"),
            Self::LocalOutOfRange => write!(f, "local_m must be between 1 and 8 m"),
        }
    }
}

impl std::error::Error for ResolutionError {}

/// Maximum monolithic grid dimension per axis (tiled storage deferred).
pub const MAX_GRID_AXIS: u32 = 4096;

impl GenerationResolution {
    pub const VOXEL_M: f32 = 1.0;

    /// Extent-aware defaults for vertical slice and archipelago scales.
    pub fn for_extent(extent_m: f32) -> Self {
        if extent_m <= 512.0 {
            Self {
                world_control_m: 0.0,
                regional_m: 8.0,
                local_m: 4.0,
                voxel_m: Self::VOXEL_M,
            }
        } else if extent_m <= 8_000.0 {
            Self {
                world_control_m: 512.0,
                regional_m: 32.0,
                local_m: 4.0,
                voxel_m: Self::VOXEL_M,
            }
        } else {
            Self {
                world_control_m: 1024.0,
                regional_m: 64.0,
                local_m: 8.0,
                voxel_m: Self::VOXEL_M,
            }
        }
    }

    pub fn validate(&self, extent_m: f32) -> Result<(), ResolutionError> {
        if (self.voxel_m - Self::VOXEL_M).abs() > f32::EPSILON {
            return Err(ResolutionError::VoxelMustBeOneMeter);
        }

        if self.world_control_m > 0.0 && self.world_control_m < 250.0 {
            return Err(ResolutionError::WorldControlBelowMinimum);
        }

        if !(8.0..=128.0).contains(&self.regional_m) {
            return Err(ResolutionError::RegionalOutOfRange);
        }

        if !(1.0..=8.0).contains(&self.local_m) {
            return Err(ResolutionError::LocalOutOfRange);
        }

        let wc = if self.world_control_m > 0.0 {
            self.world_control_m
        } else {
            f32::MAX
        };

        if wc + f32::EPSILON < self.regional_m
            || self.regional_m + f32::EPSILON < self.local_m
            || self.local_m + f32::EPSILON < self.voxel_m
        {
            return Err(ResolutionError::TiersNotCoarseToFine);
        }

        if self.world_control_m > 0.0 && !integer_ratio(self.world_control_m, self.regional_m) {
            return Err(ResolutionError::TiersNotIntegerRatio);
        }
        if !integer_ratio(self.regional_m, self.local_m) {
            return Err(ResolutionError::TiersNotIntegerRatio);
        }
        if !integer_ratio(self.local_m, self.voxel_m) {
            return Err(ResolutionError::TiersNotIntegerRatio);
        }

        check_grid_size(extent_m, self.regional_m, "regional")?;
        check_grid_size(extent_m, self.local_m, "local")?;
        if self.world_control_m > 0.0 {
            check_grid_size(extent_m, self.world_control_m, "world_control")?;
        }

        Ok(())
    }

    /// Deprecated alias: finest rasterized field spacing.
    pub fn macro_spacing_m(&self) -> f32 {
        self.local_m
    }
}

impl Default for GenerationResolution {
    fn default() -> Self {
        Self::for_extent(288.0)
    }
}

fn integer_ratio(coarse: f32, fine: f32) -> bool {
    if fine <= f32::EPSILON {
        return false;
    }
    let ratio = coarse / fine;
    (ratio - ratio.round()).abs() < 0.001 && ratio >= 1.0
}

fn check_grid_size(extent_m: f32, spacing_m: f32, tier: &'static str) -> Result<(), ResolutionError> {
    let dims = grid_dims(extent_m, spacing_m);
    let cells = dims.0.max(dims.1);
    if cells > MAX_GRID_AXIS {
        return Err(ResolutionError::GridTooLarge { tier, cells });
    }
    Ok(())
}

/// Grid dimensions for a square atlas covering `extent_m` at `spacing_m`.
pub fn grid_dims(extent_m: f32, spacing_m: f32) -> (u32, u32) {
    let n = (extent_m / spacing_m).ceil() as u32 + 1;
    (n, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_vs3_resolution_validates() {
        let res = GenerationResolution::for_extent(288.0);
        assert_eq!(res.regional_m, 8.0);
        assert_eq!(res.local_m, 4.0);
        assert!(res.validate(288.0).is_ok());
    }

    #[test]
    fn rejects_non_integer_tier_ratio() {
        let res = GenerationResolution {
            world_control_m: 0.0,
            regional_m: 16.0,
            local_m: 3.0,
            voxel_m: 1.0,
        };
        assert_eq!(res.validate(288.0), Err(ResolutionError::TiersNotIntegerRatio));
    }

    #[test]
    fn rejects_voxel_not_one_meter() {
        let res = GenerationResolution {
            world_control_m: 0.0,
            regional_m: 16.0,
            local_m: 4.0,
            voxel_m: 0.5,
        };
        assert_eq!(res.validate(288.0), Err(ResolutionError::VoxelMustBeOneMeter));
    }
}
