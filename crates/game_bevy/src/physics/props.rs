// crates/game_bevy/src/physics/props.rs
//! Dynamic physics props (crates) for water interaction demos.

use avian3d::prelude::*;
use bevy::prelude::*;
use physics_bridge::{layers_for_profile, water_sensor_layers, PhysicsBodySpec, PhysicsBodyType};
use terrain_generation::RecipeDensitySource;

use crate::data::{ConfigRegistryResource, UserSetupPrefs};
use crate::state::AppState;
use crate::terrain::{TerrainPipelineState, TerrainSpawnPoint, TerrainWorldInitSet};
use crate::ui::PhysicsTweaks;

#[derive(Component)]
pub struct DynamicCrate;

pub struct DynamicPropPlugin;

impl Plugin for DynamicPropPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::Running),
            spawn_physics_demos.after(TerrainWorldInitSet),
        )
            .add_systems(
                Update,
                apply_physics_tweaks.run_if(in_state(AppState::Running)),
            );
    }
}

fn spawn_physics_demos(
    mut commands: Commands,
    spawn_point: Res<TerrainSpawnPoint>,
    pipeline: Res<TerrainPipelineState>,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(source) = pipeline.density_source.as_ref() else {
        return;
    };
    let world_id = crate::world::requested_world_id(&prefs);
    let world = registry
        .0
        .effective_world(Some(&world_id))
        .expect("world");
    let pool = Vec3::from_array(world.recipe_to_world([82.0, 33.0, 196.0]));
    let pool_sensor = Vec3::from_array(world.recipe_to_world([82.0, 30.0, 196.0]));
    let sea_sensor = Vec3::from_array(world.recipe_to_world([128.0, 0.0, 128.0]));

    let crate_half = 0.45;
    for (i, xz) in [(8.0, 4.0), (9.2, 4.0)].iter().enumerate() {
        let wx = spawn_point.0.x + xz.0;
        let wz = spawn_point.0.z + xz.1;
        let Some(cy) = snap_prop_center(source, wx, wz, crate_half, spawn_point.0.y + 8.0) else {
            continue;
        };
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
            Vec3::new(wx, cy, wz),
            spec,
        );
    }

    if let Some(cy) = snap_prop_center(source, pool.x, pool.z, crate_half, pool.y + 4.0) {
        spawn_crate(
            &mut commands,
            &mut meshes,
            &mut materials,
            Vec3::new(pool.x, cy, pool.z),
            PhysicsBodySpec {
                body_type: PhysicsBodyType::Dynamic,
                mass: 18.0,
                friction: 0.5,
                ..default()
            },
        );
    }

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
}

fn snap_prop_center(
    source: &RecipeDensitySource,
    wx: f32,
    wz: f32,
    half_height: f32,
    search_y: f32,
) -> Option<f32> {
    source.snap_object_center_to_terrain(wx, wz, half_height, search_y)
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
        crate::physics::water_physics::RiverFlowCache::default(),
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
