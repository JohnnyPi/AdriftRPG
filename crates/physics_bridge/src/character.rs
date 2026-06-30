use avian3d::math::AdjustPrecision;
use avian3d::prelude::*;
use bevy::prelude::*;

use crate::collision::CharacterCollisionQuery;

#[derive(Component, Debug, Default)]
pub struct GroundedState {
    pub grounded: bool,
    pub ground_normal: Vec3,
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

/// Character controller systems run in this set (gravity → move → ground probe).
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct CharacterPhysicsSystems;

impl Plugin for CharacterControllerPlugin {
    fn build(&self, app: &mut App) {
        // FixedUpdate avoids query conflicts with Avian's FixedPostUpdate transform sync.
        app.add_systems(
            FixedUpdate,
            (apply_gravity, move_character, probe_ground)
                .chain()
                .in_set(CharacterPhysicsSystems),
        );
    }
}

fn probe_ground(
    spatial: SpatialQuery,
    mut query: Query<(Entity, &Transform, &CharacterController, &Collider, &mut GroundedState)>,
) {
    for (entity, transform, controller, collider, mut grounded) in &mut query {
        let max_distance = controller.ground_snap_m + controller.step_height + 0.15;
        let filter = SpatialQueryFilter::from_excluded_entities([entity]);
        if let Some(hit) = CharacterCollisionQuery::ground_cast(
            &spatial,
            collider,
            transform.translation,
            transform.rotation,
            max_distance,
            &filter,
        ) {
            grounded.grounded = true;
            grounded.ground_normal = hit.normal;
        } else {
            grounded.grounded = false;
            grounded.ground_normal = Vec3::Y;
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
            velocity.y = 0.0;
        } else {
            velocity.y += gravity_y * dt;
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

    for (entity, controller, grounded, collider, mut transform, mut velocity) in &mut query {
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
                if grounded.grounded && angle > max_slope {
                    return MoveAndSlideHitResponse::Ignore;
                }
                if angle > max_slope && normal.y < 0.5 {
                    return MoveAndSlideHitResponse::Ignore;
                }
                let is_ground = angle <= max_slope;
                let is_ceiling = is_ground && up.dot(normal) < 0.0;
                if is_ground || is_ceiling {
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
        } else {
            velocity.0 = output.projected_velocity;
        }
    }
}
