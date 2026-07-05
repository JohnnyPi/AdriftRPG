// crates/physics_bridge/src/character.rs
use avian3d::math::AdjustPrecision;
use avian3d::prelude::*;
use bevy::prelude::*;

use crate::collision::{CharacterCollisionQuery, terrain_ground_filter};

#[derive(Component, Debug, Default)]
pub struct GroundedState {
    pub grounded: bool,
    pub ground_normal: Vec3,
    /// Gap between the capsule and the ground from the most recent probe.
    /// `f32::INFINITY` when no ground was found within the search range.
    pub ground_distance: f32,
}

#[derive(Component, Debug)]
pub struct CharacterController {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub jump_speed: f32,
    pub max_slope_deg: f32,
    pub step_height: f32,
    pub ground_snap_m: f32,
}

#[derive(Bundle)]
pub struct CharacterControllerBundle {
    pub rigid_body: RigidBody,
    pub locked_axes: LockedAxes,
    pub custom_integration: CustomPositionIntegration,
    pub custom_velocity: CustomVelocityIntegration,
    pub speculative_margin: SpeculativeMargin,
    pub controller: CharacterController,
    pub grounded: GroundedState,
    pub linear_velocity: LinearVelocity,
    pub collider: Collider,
    pub friction: Friction,
}

impl CharacterControllerBundle {
    pub fn new(
        radius: f32,
        half_height: f32,
        jump_height: f32,
        gravity: f32,
        ground_snap_m: f32,
        max_slope_deg: f32,
        step_height: f32,
    ) -> Self {
        let jump_speed = (2.0 * gravity * jump_height).sqrt();
        Self {
            rigid_body: RigidBody::Kinematic,
            locked_axes: LockedAxes::ROTATION_LOCKED,
            custom_integration: CustomPositionIntegration,
            custom_velocity: CustomVelocityIntegration,
            speculative_margin: SpeculativeMargin(0.0),
            controller: CharacterController {
                walk_speed: 4.8,
                run_speed: 7.5,
                jump_speed,
                max_slope_deg,
                step_height,
                ground_snap_m,
            },
            grounded: GroundedState::default(),
            linear_velocity: LinearVelocity::default(),
            collider: Collider::capsule(radius, half_height),
            friction: Friction::new(0.0).with_combine_rule(CoefficientCombine::Min),
        }
    }
}

pub struct CharacterControllerPlugin;

/// Character controller systems run in this set (gravity → move → probe → snap).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct CharacterPhysicsSystems;

impl Plugin for CharacterControllerPlugin {
    fn build(&self, app: &mut App) {
        // FixedUpdate avoids query conflicts with Avian's FixedPostUpdate transform sync.
        //
        // Order matters: gravity uses the previous step's grounded flag (one
        // step of latency is imperceptible), the character then moves, we
        // re-probe the ground at the *new* position, and finally snap using
        // that fresh distance. Probing before snapping is what keeps ground
        // following stable — the old order snapped against a stale distance
        // captured before the horizontal move.
        app.add_systems(
            FixedUpdate,
            (apply_gravity, move_character, probe_ground, snap_to_ground)
                .chain()
                .in_set(CharacterPhysicsSystems),
        );
    }
}

// --- Ground-contact tuning -------------------------------------------------

/// Minimum clearance ever kept between the capsule and the ground. The
/// controller never lets the capsule sit closer than this, which prevents the
/// collider from clipping into terrain triangles at chunk seams.
const GROUND_CONTACT_SKIN_M: f32 = 0.015;

/// Re-exported for callers that align other colliders to the capsule's floor.
pub const GROUND_CONTACT_SKIN: f32 = GROUND_CONTACT_SKIN_M;

/// Target clearance the capsule settles at when grounded. Kept a hair above the
/// skin so the capsule visibly floats slightly over the surface instead of
/// intersecting it, which reads as much smoother footing over uneven terrain.
const GROUND_HOVER_M: f32 = 0.06;

/// Extra band below the hover height over which the character is still counted
/// as grounded once it already was (coyote / hysteresis band). Stops grounded
/// from flickering when crossing small dips or chunk-seam normal changes.
const GROUNDED_HYSTERESIS_M: f32 = 0.12;

/// Floor on how fast the capsule is eased toward the hover height while
/// following the ground. The actual cap scales up with horizontal speed (see
/// [`snap_to_ground`]) so the capsule stays glued to walkable descents without
/// launching, while small bumps still resolve smoothly instead of teleporting.
const GROUND_FOLLOW_SPEED_MPS: f32 = 8.0;

/// Multiplier applied to horizontal speed when deriving the follow-speed cap.
/// A little over 1.0 covers the steepest walkable slope (tan(47°) ≈ 1.07) with
/// headroom, guaranteeing the follower can always keep up on ground the
/// character is allowed to walk on.
const GROUND_FOLLOW_SPEED_FACTOR: f32 = 1.3;

/// Upward speed above which ground snapping is suppressed. When the capsule is
/// clearly rising (a jump) we never want the follower to yank it back down.
const RISING_SUPPRESS_SNAP_MPS: f32 = 0.2;

/// Downward speed cap while airborne so long falls stay controllable and the
/// interpolated render position never jumps by a huge amount in a single step.
const TERMINAL_FALL_MPS: f32 = 55.0;

// --- Systems ---------------------------------------------------------------

/// Probe the ground beneath the capsule and classify grounded state.
///
/// Runs *after* the move so it reports the capsule's post-move footing. The
/// grounded band is deliberately the same reach the snap step can act on, so we
/// never mark the capsule grounded at a height snapping cannot resolve (the old
/// float bug). A small hysteresis band widens the reach once already grounded.
fn probe_ground(
    spatial: SpatialQuery,
    mut query: Query<(
        Entity,
        &Transform,
        &CharacterController,
        &Collider,
        &mut GroundedState,
    )>,
) {
    for (entity, transform, controller, collider, mut grounded) in &mut query {
        let base_band = controller.ground_snap_m + GROUND_HOVER_M;
        let band = base_band
            + if grounded.grounded {
                GROUNDED_HYSTERESIS_M
            } else {
                0.0
            };
        // Cast slightly past the grounded band so we still learn the distance
        // right at the edge of the band rather than reporting a miss.
        let search = band + GROUND_CONTACT_SKIN_M;
        let filter = terrain_ground_filter(entity);
        if let Some(hit) = CharacterCollisionQuery::ground_cast(
            &spatial,
            collider,
            transform.translation,
            transform.rotation,
            search,
            &filter,
        ) {
            let angle = hit.normal.angle_between(Vec3::Y);
            let walkable = angle <= controller.max_slope_deg.to_radians();
            grounded.ground_distance = hit.distance;
            // Only walkable ground within the band counts as footing. Standing
            // on a too-steep face reports not-grounded so gravity + slide let
            // the capsule slip down it instead of sticking to the cliff.
            grounded.grounded = walkable && hit.distance <= band;
            grounded.ground_normal = if grounded.grounded {
                hit.normal
            } else {
                Vec3::Y
            };
        } else {
            grounded.grounded = false;
            grounded.ground_normal = Vec3::Y;
            grounded.ground_distance = f32::INFINITY;
        }
    }
}

fn apply_gravity(
    time: Res<Time<Fixed>>,
    registry_gravity: Option<Res<Gravity>>,
    mut query: Query<(&GroundedState, &mut LinearVelocity), With<CharacterController>>,
) {
    let gravity_y = registry_gravity.map(|g| g.0.y).unwrap_or(-18.0);
    let dt = time.delta_secs();
    for (grounded, mut velocity) in &mut query {
        if grounded.grounded && velocity.y <= 0.0 {
            // Resting on ground: cancel accumulated downward speed so the hover
            // follower has sole control over vertical placement. Upward speed
            // (a jump launched this step) is preserved.
            velocity.y = 0.0;
        } else {
            velocity.y += gravity_y * dt;
            if velocity.y < -TERMINAL_FALL_MPS {
                velocity.y = -TERMINAL_FALL_MPS;
            }
        }
    }
}

fn move_character(
    time: Res<Time<Fixed>>,
    move_and_slide: MoveAndSlide,
    mut query: Query<
        (
            Entity,
            &CharacterController,
            &GroundedState,
            &Collider,
            &mut Transform,
            &mut LinearVelocity,
        ),
        With<CharacterController>,
    >,
) {
    let dt = time.delta();
    let up = Vec3::Y;

    for (entity, controller, _grounded, collider, mut transform, mut velocity) in &mut query {
        let max_slope = controller.max_slope_deg.to_radians();
        let shape_position = transform.translation.adjust_precision();
        let shape_rotation = transform.rotation.adjust_precision();
        let mut hit_ground_or_ceiling = false;

        let output = move_and_slide.move_and_slide(
            collider,
            shape_position,
            shape_rotation,
            velocity.0,
            dt,
            &MoveAndSlideConfig::default(),
            &SpatialQueryFilter::from_excluded_entities([entity]),
            |hit| {
                let normal = Vec3::from(*hit.normal);
                let angle = up.angle_between(normal);
                let is_floor = normal.y > 0.5 && angle <= max_slope;
                let is_ceiling = normal.y < -0.5;
                // Flag contacts that constrain vertical velocity (walkable
                // floor or ceiling); steep-wall slides are handled purely by
                // the projected velocity below.
                if is_floor || is_ceiling {
                    hit_ground_or_ceiling = true;
                }
                MoveAndSlideHitResponse::Accept
            },
        );

        transform.translation = Vec3::from(output.position);

        if hit_ground_or_ceiling {
            let up = up.adjust_precision();
            let velocity_along_up = velocity.0.dot(up);
            let new_velocity_along_up = output.projected_velocity.dot(up);
            velocity.0 += (new_velocity_along_up - velocity_along_up) * up;
            if new_velocity_along_up < 0.0 && velocity.0.y > 0.0 {
                velocity.0.y = 0.0;
            }
        } else {
            velocity.0 = output.projected_velocity;
        }
    }
}

/// Ease the capsule toward the hover height above walkable ground.
///
/// Unlike a hard snap, the downward correction is bounded by a speed cap that
/// scales with horizontal speed, so walking down bumps and slopes stays smooth
/// and the capsule never teleports vertically within a step. Penetration (or
/// terrain rising into the capsule, e.g. climbing a slope) is relieved
/// immediately so the collider never intersects the ground.
fn snap_to_ground(
    time: Res<Time<Fixed>>,
    mut query: Query<(&mut Transform, &GroundedState, &LinearVelocity), With<CharacterController>>,
) {
    let dt = time.delta_secs();
    for (mut transform, grounded, velocity) in &mut query {
        // Never fight an active jump / upward launch.
        if velocity.y > RISING_SUPPRESS_SNAP_MPS {
            continue;
        }
        // Only walkable footing is snapped; airborne and steep-slope cases fall
        // through to gravity + slide. `grounded` already guarantees a finite,
        // in-range ground distance from the fresh probe above.
        if !grounded.grounded || !grounded.ground_distance.is_finite() {
            continue;
        }

        // Signed distance from the desired hover height (positive => too high).
        let error = grounded.ground_distance - GROUND_HOVER_M;
        if error.abs() <= 1e-4 {
            continue;
        }

        if error > 0.0 {
            // Above the hover height: ease down, capped so descents and bumps
            // read smoothly. The cap scales with horizontal speed to keep the
            // capsule glued to any walkable downslope without launching.
            let horizontal_speed = Vec2::new(velocity.x, velocity.z).length();
            let follow_cap =
                (horizontal_speed * GROUND_FOLLOW_SPEED_FACTOR).max(GROUND_FOLLOW_SPEED_MPS);
            transform.translation.y -= error.min(follow_cap * dt);
        } else {
            // Closer than the hover height (terrain rose into us / climbing a
            // slope): lift straight to the hover height immediately. Moving
            // away from the ground can never cause penetration, so no cap.
            transform.translation.y -= error;
        }
    }
}
