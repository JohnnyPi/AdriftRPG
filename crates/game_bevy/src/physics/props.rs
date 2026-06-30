//! Dynamic props and moving platforms (VS2 Phase 2).

use avian3d::prelude::*;
use bevy::prelude::*;
use physics_bridge::{layers_for_profile, moving_platform_layers, water_sensor_layers, PhysicsBodySpec, PhysicsBodyType};

use crate::data::ConfigRegistryResource;
use crate::player::Player;
use crate::state::AppState;
use crate::terrain::TerrainSpawnPoint;
use crate::ui::PhysicsTweaks;

#[derive(Component)]
pub struct DynamicCrate;

#[derive(Component)]
pub struct MovingPlatform {
    pub start: Vec3,
    pub end: Vec3,
    pub speed: f32,
    pub phase: f32,
}

pub struct DynamicPropPlugin;

impl Plugin for DynamicPropPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Running), spawn_physics_demos)
            .add_systems(
                Update,
                (animate_moving_platform, apply_physics_tweaks).run_if(in_state(AppState::Running)),
            );
    }
}

fn spawn_physics_demos(
    mut commands: Commands,
    spawn_point: Res<TerrainSpawnPoint>,
    registry: Res<ConfigRegistryResource>,
    world_tweaks: Res<crate::ui::WorldTweaks>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let world_id = crate::world::requested_world_id(&registry, &world_tweaks);
    let world = registry
        .0
        .effective_world(Some(&world_id))
        .expect("world");
    let pool = Vec3::from_array(world.recipe_to_world([82.0, 33.0, 196.0]));
    let pool_sensor = Vec3::from_array(world.recipe_to_world([82.0, 30.0, 196.0]));
    let sea_sensor = Vec3::from_array(world.recipe_to_world([128.0, 0.0, 128.0]));

    let base = spawn_point.0 + Vec3::new(8.0, 2.0, 4.0);
    for (i, offset) in [(0.0, 0.0), (1.2, 0.0)].iter().enumerate() {
        let spec = PhysicsBodySpec {
            body_type: PhysicsBodyType::Dynamic,
            mass: 12.0 + i as f32 * 4.0,
            friction: 0.6,
            ..default()
        };
        spawn_crate(
            &mut commands,
            &mut meshes,
            &mut materials,
            base + Vec3::new(offset.0, offset.1, 0.0),
            spec,
        );
    }

    // Floating demo crate in upland pool (VS2 §11 polish).
    spawn_crate(
        &mut commands,
        &mut meshes,
        &mut materials,
        pool,
        PhysicsBodySpec {
            body_type: PhysicsBodyType::Dynamic,
            mass: 18.0,
            friction: 0.5,
            ..default()
        },
    );

    commands.spawn((
        crate::physics::water_physics::WaterSensor,
        Sensor,
        Collider::cuboid(24.0, 4.0, 24.0),
        CollisionLayers::from(water_sensor_layers()),
        Transform::from_translation(pool_sensor),
    ));
    commands.spawn((
        crate::physics::water_physics::WaterSensor,
        Sensor,
        Collider::cuboid(200.0, 6.0, 200.0),
        CollisionLayers::from(water_sensor_layers()),
        Transform::from_translation(sea_sensor),
    ));

    let platform_start = base + Vec3::new(-4.0, 0.5, 6.0);
    let platform_end = platform_start + Vec3::new(10.0, 0.0, 0.0);
    commands.spawn((
        MovingPlatform {
            start: platform_start,
            end: platform_end,
            speed: 2.5,
            phase: 0.0,
        },
        RigidBody::Kinematic,
        Collider::cuboid(2.0, 0.25, 1.5),
        CollisionLayers::from(moving_platform_layers()),
        Mesh3d(meshes.add(Cuboid::new(4.0, 0.5, 3.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.45, 0.38, 0.32),
            ..default()
        })),
        Transform::from_translation(platform_start),
        LinearVelocity::ZERO,
    ));
}

fn spawn_crate(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    position: Vec3,
    spec: PhysicsBodySpec,
) {
    commands.spawn((
        DynamicCrate,
        RigidBody::Dynamic,
        Collider::cuboid(0.45, 0.45, 0.45),
        Mass(spec.mass),
        Friction::new(spec.friction),
        Restitution::new(spec.restitution),
        LinearDamping(spec.linear_damping),
        AngularDamping(spec.angular_damping),
        CollisionLayers::from(layers_for_profile(spec.collision_profile)),
        Mesh3d(meshes.add(Cuboid::new(0.9, 0.9, 0.9))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.42, 0.28),
            ..default()
        })),
        Transform::from_translation(position),
    ));
}

fn animate_moving_platform(
    time: Res<Time>,
    tweaks: Res<PhysicsTweaks>,
    mut platforms: Query<(&MovingPlatform, &mut Transform, &mut LinearVelocity)>,
) {
    let speed = if tweaks.use_overrides {
        tweaks.platform_speed
    } else {
        2.5
    };
    for (platform, mut transform, mut velocity) in &mut platforms {
        let t = (time.elapsed_secs() * speed + platform.phase).sin() * 0.5 + 0.5;
        let pos = platform.start.lerp(platform.end, t);
        let prev = transform.translation;
        transform.translation = pos;
        let dt = time.delta_secs().max(0.0001);
        velocity.0 = (pos - prev) / dt;
    }
}

fn apply_physics_tweaks(
    tweaks: Res<PhysicsTweaks>,
    mut gravity: ResMut<Gravity>,
    mut crates: Query<&mut Friction, With<DynamicCrate>>,
) {
    if tweaks.use_overrides {
        gravity.0.y = -tweaks.gravity;
        for mut friction in &mut crates {
            *friction = Friction::new(tweaks.prop_friction);
        }
    }
}

#[derive(Component, Default)]
pub struct PlatformRider;

pub(crate) fn inherit_platform_velocity(
    mut commands: Commands,
    mut queries: ParamSet<(
        Query<(&Transform, &LinearVelocity), With<MovingPlatform>>,
        Query<(Entity, &Transform, &mut LinearVelocity), With<Player>>,
    )>,
) {
    let platform_samples: Vec<(Vec3, Vec3)> = queries
        .p0()
        .iter()
        .map(|(platform_tf, platform_vel)| (platform_tf.translation, platform_vel.0))
        .collect();

    for (entity, player_tf, mut player_vel) in queries.p1().iter_mut() {
        let mut riding = false;
        for (platform_pos, platform_vel) in &platform_samples {
            let half = Vec3::new(2.0, 0.5, 1.5);
            let min = *platform_pos - half;
            let max = *platform_pos + half;
            let p = player_tf.translation;
            if p.x >= min.x && p.x <= max.x && p.z >= min.z && p.z <= max.z && p.y < max.y + 1.2
            {
                riding = true;
                player_vel.x += platform_vel.x * 0.15;
                player_vel.z += platform_vel.z * 0.15;
            }
        }
        if riding {
            commands.entity(entity).insert(PlatformRider);
        } else {
            commands.entity(entity).remove::<PlatformRider>();
        }
    }
}
