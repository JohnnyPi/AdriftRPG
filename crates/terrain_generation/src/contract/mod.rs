//! Stable contracts between compiler, atlas, and runtime.

pub mod coordinates;
pub mod density;
pub mod manifest;
pub mod metadata;
pub mod version;

pub use coordinates::{
    CellSizeMeters, ElevationMeters, TileCoord, WorldPosition, WorldXZ, grid_cell_to_world,
    world_to_grid_coords,
};
pub use density::{
    ColumnSample, GeologySample, SurfaceSample, WorldDensityProvider, surface_density,
};
pub use manifest::WorldManifest;
pub use metadata::{RecipeHash, WorldExtent, WorldMetadata};
pub use version::{GENERATOR_VERSION, GeneratorVersion, derive_seed};
