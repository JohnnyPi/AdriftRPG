// crates/physics_bridge/src/collision.rs
use avian3d::prelude::*;
use bevy::prelude::*;

/// Spatial query helpers for character controllers.
pub struct CharacterCollisionQuery;

pub struct GroundHit {
    pub normal: Vec3,
    pub distance: f32,
}

impl CharacterCollisionQuery {
    pub fn ground_cast(
        spatial: &SpatialQuery,
        collider: &Collider,
        origin: Vec3,
        rotation: Quat,
        max_distance: f32,
        filter: &SpatialQueryFilter,
    ) -> Option<GroundHit> {
        let hit = spatial.cast_shape(
            collider,
            origin,
            rotation,
            Dir3::NEG_Y,
            &ShapeCastConfig::from_max_distance(max_distance),
            filter,
        )?;
        Some(GroundHit {
            normal: Vec3::from(hit.normal1),
            distance: hit.distance,
        })
    }
}
