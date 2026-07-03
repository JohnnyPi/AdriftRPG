//! Level-1 animated cloud shell (VerticalSlice2 §13.5, SkyLightingGuide §15).
//!
//! Cloud ground shadows and volumetric clouds are deferred — see module docs.

use bevy::mesh::SphereKind;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{AsBindGroup, Face, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError};
use bevy::shader::ShaderRef;

use super::celestial::CelestialState;
use super::config_init::{sea_level_for_prefs, EnvironmentInitSet};
use super::sky_config::{SkyEffectsRevision, SkyPresentationConfig};
use crate::camera::MainGameCamera;
use crate::data::{ConfigRegistryResource, UserSetupPrefs};
use crate::state::AppState;

/// Base height scale for YAML `clouds_altitude` (0–1) → world meters.
const CLOUD_BASE_HEIGHT_M: f32 = 500.0;
const CLOUD_SHELL_RADIUS_M: f32 = 2800.0;

#[derive(ShaderType, Clone, Copy, Debug)]
pub struct CloudParams {
    pub coverage: f32,
    pub wind: Vec4,
    pub sun_dir: Vec4,
    pub sun_color: Vec4,
    pub horizon_color: Vec4,
    pub shell: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct CloudMaterial {
    #[uniform(0)]
    pub params: CloudParams,
}

impl Material for CloudMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/clouds.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = Some(Face::Front);
        Ok(())
    }
}

#[derive(Component)]
pub struct CloudLayer;

#[derive(Resource, Default)]
struct CloudLayerSpawned(bool);

pub struct CloudPlugin;

impl Plugin for CloudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<CloudMaterial>::default())
            .init_resource::<CloudLayerSpawned>()
            .add_systems(
                OnEnter(AppState::Running),
                spawn_cloud_layer.after(EnvironmentInitSet),
            )
            .add_systems(
                Update,
                (
                    sync_cloud_layer_on_revision,
                    follow_cloud_layer,
                    update_cloud_material,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

pub fn cloud_shell_world_y(sea_level_m: f32, clouds_altitude: f32) -> f32 {
    sea_level_m + CLOUD_BASE_HEIGHT_M * clouds_altitude.clamp(0.05, 1.0)
}

fn spawn_cloud_layer(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CloudMaterial>>,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    sky: Res<SkyPresentationConfig>,
    celestial: Res<CelestialState>,
    mut spawned: ResMut<CloudLayerSpawned>,
    existing: Query<Entity, With<CloudLayer>>,
) {
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }
    spawned.0 = false;

    if !sky.clouds_enabled {
        return;
    }

    let sea_level = sea_level_for_prefs(&registry, &prefs);
    let shell_y = cloud_shell_world_y(sea_level, sky.clouds_altitude);
    let material = materials.add(cloud_material_from_state(&sky, &celestial, shell_y, 0.0));

    let mesh = meshes.add(
        Sphere::new(CLOUD_SHELL_RADIUS_M)
            .mesh()
            .kind(SphereKind::Uv {
                sectors: 48,
                stacks: 32,
            })
            .build(),
    );
    commands.spawn((
        CloudLayer,
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, shell_y, 0.0),
        Visibility::default(),
    ));
    spawned.0 = true;
}

fn sync_cloud_layer_on_revision(
    revision: Res<SkyEffectsRevision>,
    mut last: Local<Option<u32>>,
    commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<CloudMaterial>>,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    sky: Res<SkyPresentationConfig>,
    celestial: Res<CelestialState>,
    spawned: ResMut<CloudLayerSpawned>,
    existing: Query<Entity, With<CloudLayer>>,
) {
    if last.is_none() {
        *last = Some(revision.0);
        return;
    }
    if *last == Some(revision.0) {
        return;
    }
    *last = Some(revision.0);
    spawn_cloud_layer(
        commands,
        meshes,
        materials,
        registry,
        prefs,
        sky,
        celestial,
        spawned,
        existing,
    );
}

fn follow_cloud_layer(
    cameras: Query<&Transform, (With<MainGameCamera>, Without<CloudLayer>)>,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    sky: Res<SkyPresentationConfig>,
    mut clouds: Query<&mut Transform, (With<CloudLayer>, Without<MainGameCamera>)>,
) {
    let Ok(camera_tf) = cameras.single() else {
        return;
    };
    let sea_level = sea_level_for_prefs(&registry, &prefs);
    let shell_y = cloud_shell_world_y(sea_level, sky.clouds_altitude);
    for mut transform in &mut clouds {
        transform.translation = Vec3::new(camera_tf.translation.x, shell_y, camera_tf.translation.z);
    }
}

fn update_cloud_material(
    time: Res<Time>,
    sky: Res<SkyPresentationConfig>,
    celestial: Res<CelestialState>,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    mut materials: ResMut<Assets<CloudMaterial>>,
    clouds: Query<&MeshMaterial3d<CloudMaterial>, With<CloudLayer>>,
) {
    let sea_level = sea_level_for_prefs(&registry, &prefs);
    let shell_y = cloud_shell_world_y(sea_level, sky.clouds_altitude);
    let elapsed = time.elapsed_secs();

    for mat_handle in clouds.iter() {
        let Some(mut mat) = materials.get_mut(&mat_handle.0) else {
            continue;
        };
        mat.params = cloud_material_from_state(&sky, &celestial, shell_y, elapsed).params;
    }
}

fn cloud_material_from_state(
    sky: &SkyPresentationConfig,
    celestial: &CelestialState,
    shell_y: f32,
    elapsed: f32,
) -> CloudMaterial {
    let dir_rad = sky.clouds_direction_deg.to_radians();
    let wind_dir = Vec2::new(dir_rad.cos(), dir_rad.sin());
    let coverage = celestial.cloud_cover;
    let sun = celestial.sun_direction;
    let sun_c = celestial.sun_color;
    let horizon = sky.horizon_color;

    CloudMaterial {
        params: CloudParams {
            coverage,
            wind: Vec4::new(
                wind_dir.x,
                wind_dir.y,
                sky.clouds_speed,
                elapsed,
            ),
            sun_dir: Vec4::new(sun.x, sun.y, sun.z, 0.0),
            sun_color: Vec4::new(sun_c[0], sun_c[1], sun_c[2], 1.0),
            horizon_color: Vec4::new(horizon[0], horizon[1], horizon[2], 1.0),
            shell: Vec4::new(shell_y, CLOUD_SHELL_RADIUS_M, 0.0, 0.0),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_shell_scales_with_altitude() {
        let low = cloud_shell_world_y(2.0, 0.2);
        let high = cloud_shell_world_y(2.0, 0.8);
        assert!(high > low);
    }
}
