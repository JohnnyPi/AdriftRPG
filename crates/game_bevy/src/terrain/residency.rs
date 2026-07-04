// crates/game_bevy/src/terrain/residency.rs
//! Interest-based chunk residency (VS2 §3.3).
//!
//! Radii come from [`WorldTweaks`] (seeded from YAML via
//! [`WorldTweaks::apply_authored_residency`](crate::ui::WorldTweaks::apply_authored_residency)).
//! Use [`ChunkResidencyRadius`] + [`within_radius`] for generic checks.

use avian3d::prelude::Collider;
use bevy::prelude::*;
use voxel_core::{CHUNK_CELLS, ChunkCoord, WorldCell};

use crate::player::Player;
use crate::ui::WorldTweaks;

#[derive(Resource, Clone, Debug)]
pub struct TerrainWorldRuntime {
    pub seed: u64,
    pub revision: u64,
    pub interest_center: ChunkCoord,
    pub cell_size_m: f32,
}

impl Default for TerrainWorldRuntime {
    fn default() -> Self {
        Self {
            seed: 0,
            revision: 1,
            interest_center: ChunkCoord::new(0, 0, 0),
            cell_size_m: 1.0,
        }
    }
}

pub struct ChunkResidencyPlugin;

impl Plugin for ChunkResidencyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainWorldRuntime>()
            .add_systems(Update, update_interest_center);
    }
}

fn update_interest_center(
    mut runtime: ResMut<TerrainWorldRuntime>,
    players: Query<&Transform, With<Player>>,
) {
    let Ok(tf) = players.single() else {
        return;
    };
    let cell = WorldCell::new(
        tf.translation.x.floor() as i32,
        tf.translation.y.floor() as i32,
        tf.translation.z.floor() as i32,
    );
    runtime.interest_center = cell.chunk_coord();
}

pub fn chunk_chebyshev_distance(a: ChunkCoord, b: ChunkCoord) -> i32 {
    (a.x - b.x)
        .abs()
        .max((a.y - b.y).abs())
        .max((a.z - b.z).abs())
}

/// True when a chunk has finished mesh upload and is visible in the world.
pub fn chunk_has_uploaded_mesh(
    pipeline: &crate::terrain::TerrainPipelineState,
    coord: ChunkCoord,
) -> bool {
    use crate::terrain::ChunkState;
    pipeline
        .chunks
        .get(&coord)
        .is_some_and(|chunk| chunk.state == ChunkState::Ready && chunk.entity.is_some())
}

/// Spawn terrain is ready when the spawn chunk or an adjacent vertical neighbor has uploaded geometry.
pub fn spawn_terrain_uploaded(
    pipeline: &crate::terrain::TerrainPipelineState,
    spawn: ChunkCoord,
) -> bool {
    if chunk_has_uploaded_mesh(pipeline, spawn) {
        return true;
    }
    for dy in -1..=1i32 {
        if dy == 0 {
            continue;
        }
        let neighbor = ChunkCoord::new(spawn.x, spawn.y + dy, spawn.z);
        if chunk_has_uploaded_mesh(pipeline, neighbor) {
            return true;
        }
    }
    false
}

/// True when the spawn chunk's terrain mesh has a physics collider attached.
/// Neighbor chunks are not accepted — placement must hit the spawn chunk trimesh.
pub fn spawn_terrain_collider_ready(
    pipeline: &crate::terrain::TerrainPipelineState,
    spawn: ChunkCoord,
    colliders: &Query<Entity, With<Collider>>,
) -> bool {
    let Some(entity) = pipeline.chunks.get(&spawn).and_then(|chunk| chunk.entity) else {
        return false;
    };
    colliders.get(entity).is_ok()
}

/// Chunk interest radii indexed from [`WorldTweaks`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkResidencyRadius {
    Density,
    Render,
    Physics,
    Decoration,
    HighDetail,
}

impl ChunkResidencyRadius {
    pub fn limit(self, tweaks: &WorldTweaks) -> i32 {
        match self {
            Self::Density => tweaks.density_radius,
            Self::Render => tweaks.render_radius,
            Self::Physics => tweaks.physics_radius,
            Self::Decoration => tweaks.decoration_radius,
            Self::HighDetail => tweaks.high_detail_radius,
        }
    }
}

pub fn within_radius(
    center: ChunkCoord,
    coord: ChunkCoord,
    radius: ChunkResidencyRadius,
    tweaks: &WorldTweaks,
) -> bool {
    chunk_chebyshev_distance(center, coord) <= radius.limit(tweaks)
}

pub fn within_density_radius(center: ChunkCoord, coord: ChunkCoord, tweaks: &WorldTweaks) -> bool {
    within_radius(center, coord, ChunkResidencyRadius::Density, tweaks)
}

pub fn within_render_radius(center: ChunkCoord, coord: ChunkCoord, tweaks: &WorldTweaks) -> bool {
    within_radius(center, coord, ChunkResidencyRadius::Render, tweaks)
}

pub fn within_physics_radius(center: ChunkCoord, coord: ChunkCoord, tweaks: &WorldTweaks) -> bool {
    within_radius(center, coord, ChunkResidencyRadius::Physics, tweaks)
}

pub fn within_decoration_radius(
    center: ChunkCoord,
    coord: ChunkCoord,
    tweaks: &WorldTweaks,
) -> bool {
    within_radius(center, coord, ChunkResidencyRadius::Decoration, tweaks)
}

pub fn within_high_detail_radius(
    center: ChunkCoord,
    coord: ChunkCoord,
    tweaks: &WorldTweaks,
) -> bool {
    within_radius(center, coord, ChunkResidencyRadius::HighDetail, tweaks)
}

pub fn world_position_in_radius(
    center: ChunkCoord,
    position: Vec3,
    radius: ChunkResidencyRadius,
    tweaks: &WorldTweaks,
) -> bool {
    let cell = WorldCell::new(
        position.x.floor() as i32,
        position.y.floor() as i32,
        position.z.floor() as i32,
    );
    within_radius(center, cell.chunk_coord(), radius, tweaks)
}

pub fn world_position_in_high_detail_radius(
    center: ChunkCoord,
    position: Vec3,
    tweaks: &WorldTweaks,
) -> bool {
    world_position_in_radius(center, position, ChunkResidencyRadius::HighDetail, tweaks)
}

pub fn world_position_in_decoration_radius(
    center: ChunkCoord,
    position: Vec3,
    tweaks: &WorldTweaks,
) -> bool {
    world_position_in_radius(center, position, ChunkResidencyRadius::Decoration, tweaks)
}

/// Chunk center in world meters.
pub fn chunk_world_center(coord: ChunkCoord, cell_size_m: f32) -> Vec3 {
    let extent = CHUNK_CELLS as f32 * cell_size_m;
    Vec3::new(
        coord.x as f32 * extent + extent * 0.5,
        coord.y as f32 * extent + extent * 0.5,
        coord.z as f32 * extent + extent * 0.5,
    )
}

pub fn draw_residency_rings(
    gizmos: &mut Gizmos,
    center: ChunkCoord,
    cell_size_m: f32,
    tweaks: &WorldTweaks,
) {
    let origin = chunk_world_center(center, cell_size_m);
    let cells = CHUNK_CELLS as f32 * cell_size_m;
    for (radius, color) in [
        (
            ChunkResidencyRadius::Render.limit(tweaks),
            Color::srgba(0.2, 0.8, 1.0, 0.35),
        ),
        (
            ChunkResidencyRadius::Physics.limit(tweaks),
            Color::srgba(1.0, 0.6, 0.2, 0.35),
        ),
        (
            ChunkResidencyRadius::Decoration.limit(tweaks),
            Color::srgba(0.9, 0.4, 0.9, 0.25),
        ),
        (
            ChunkResidencyRadius::HighDetail.limit(tweaks),
            Color::srgba(0.4, 1.0, 0.4, 0.35),
        ),
        (
            ChunkResidencyRadius::Density.limit(tweaks),
            Color::srgba(0.4, 1.0, 0.4, 0.25),
        ),
    ] {
        let size = (radius as f32 * 2.0 + 1.0) * cells;
        gizmos.cube(
            Transform::from_translation(origin).with_scale(Vec3::splat(size)),
            color,
        );
    }
}

#[cfg(test)]
mod residency_tests {
    use super::*;
    use crate::ui::WorldTweaks;
    use voxel_core::ChunkCoord;

    #[test]
    fn physics_radius_is_subset_of_render_radius() {
        let tweaks = WorldTweaks::default();
        let center = ChunkCoord::new(0, 0, 0);
        let near = ChunkCoord::new(tweaks.physics_radius, 0, 0);
        assert!(within_physics_radius(center, near, &tweaks));
        assert!(within_render_radius(center, near, &tweaks));
        let far = ChunkCoord::new(tweaks.render_radius + 1, 0, 0);
        assert!(!within_render_radius(center, far, &tweaks));
        assert!(!within_physics_radius(center, far, &tweaks));
    }

    #[test]
    fn high_detail_radius_matches_chebyshev_threshold() {
        let tweaks = WorldTweaks::default();
        let center = ChunkCoord::new(0, 0, 0);
        let near = ChunkCoord::new(tweaks.high_detail_radius, 0, 0);
        assert!(within_high_detail_radius(center, near, &tweaks));
        let far = ChunkCoord::new(tweaks.high_detail_radius + 1, 0, 0);
        assert!(!within_high_detail_radius(center, far, &tweaks));
    }

    #[test]
    fn chebyshev_distance_is_symmetric() {
        let a = ChunkCoord::new(1, 2, 3);
        let b = ChunkCoord::new(4, 0, 1);
        assert_eq!(
            chunk_chebyshev_distance(a, b),
            chunk_chebyshev_distance(b, a)
        );
    }
}
