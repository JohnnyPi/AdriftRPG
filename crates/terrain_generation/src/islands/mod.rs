//! Island generation module.

pub mod footprint;
pub mod pass;
pub mod seed;
pub mod skeleton;
pub mod validation;

pub use footprint::{generate_age_field, generate_influence_field, generate_island_id_field};
pub use pass::IslandSkeletonPass;
pub use seed::{IslandBlueprint, IslandSeed};
pub use skeleton::{IslandSkeleton, build_skeleton};
