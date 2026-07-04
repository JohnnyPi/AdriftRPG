//! Nighttime starfield dome rendered above the terrain, faded by sun elevation.

use bevy::mesh::SphereKind;
use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use super::celestial::CelestialState;
use super::config_init::EnvironmentInitSet;
use super::sky_config::{SkyEffectsRevision, SkyPresentationConfig};
use crate::camera::MainGameCamera;
use crate::state::AppState;

#[derive(ShaderType, Clone, Copy, Debug)]
pub struct StarParams {
    pub density: f32,
    pub sun_elevation_deg: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct StarMaterial {
    #[uniform(0)]
    pub params: StarParams,
}

impl Material for StarMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/stars.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

#[derive(Component)]
struct StarfieldDome;

#[derive(Resource, Default)]
struct StarfieldSpawned(bool);

pub struct StarfieldPlugin;

impl Plugin for StarfieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<StarMaterial>::default())
            .init_resource::<StarfieldSpawned>()
            .add_systems(
                OnEnter(AppState::Running),
                spawn_starfield.after(EnvironmentInitSet),
            )
            .add_systems(
                Update,
                (
                    sync_starfield_on_revision,
                    follow_starfield_camera,
                    update_starfield_params,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn spawn_starfield(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StarMaterial>>,
    sky: Res<SkyPresentationConfig>,
    mut spawned: ResMut<StarfieldSpawned>,
    existing: Query<Entity, With<StarfieldDome>>,
) {
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }
    spawned.0 = false;

    if !sky.stars_enabled {
        return;
    }
    let mesh = meshes.add(Sphere::new(4000.0).mesh().kind(SphereKind::Uv {
        sectors: 32,
        stacks: 16,
    }));
    let material = materials.add(StarMaterial {
        params: StarParams {
            density: sky.stars_density,
            sun_elevation_deg: 45.0,
            _pad0: 0.0,
            _pad1: 0.0,
        },
    });
    commands.spawn((
        StarfieldDome,
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        Visibility::default(),
    ));
    spawned.0 = true;
}

fn sync_starfield_on_revision(
    revision: Res<SkyEffectsRevision>,
    mut last: Local<Option<u32>>,
    commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<StarMaterial>>,
    sky: Res<SkyPresentationConfig>,
    spawned: ResMut<StarfieldSpawned>,
    existing: Query<Entity, With<StarfieldDome>>,
) {
    if last.is_none() {
        *last = Some(revision.0);
        return;
    }
    if *last == Some(revision.0) {
        return;
    }
    *last = Some(revision.0);
    spawn_starfield(commands, meshes, materials, sky, spawned, existing);
}

fn follow_starfield_camera(
    camera: Query<&Transform, (With<MainGameCamera>, Without<StarfieldDome>)>,
    mut domes: Query<&mut Transform, With<StarfieldDome>>,
) {
    let Ok(cam) = camera.single() else {
        return;
    };
    for mut tf in &mut domes {
        tf.translation = cam.translation;
    }
}

fn update_starfield_params(
    sky: Res<SkyPresentationConfig>,
    celestial: Res<CelestialState>,
    mut materials: ResMut<Assets<StarMaterial>>,
    domes: Query<&MeshMaterial3d<StarMaterial>, With<StarfieldDome>>,
) {
    for mat_handle in &domes {
        let Some(mut mat) = materials.get_mut(&mat_handle.0) else {
            continue;
        };
        mat.params.density = sky.stars_density;
        mat.params.sun_elevation_deg = celestial.sun_elevation_deg;
    }
}
