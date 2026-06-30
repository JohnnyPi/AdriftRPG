use bevy::prelude::*;
use bevy::pbr::Material;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use crate::data::ConfigRegistryResource;
use crate::state::AppState;

#[derive(Component)]
pub struct WaterSurface;

#[derive(ShaderType, Clone, Copy, Debug)]
pub struct WaterParams {
    pub shallow_color: Vec4,
    pub deep_color: Vec4,
    /// x=sea_level, y=wave_speed, z=wave_amplitude, w=transparency
    pub wave: Vec4,
    /// x=time
    pub animation: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct WaterMaterial {
    #[uniform(0)]
    pub params: WaterParams,
}

impl Material for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }
}

pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<WaterMaterial>::default())
            .add_systems(OnEnter(AppState::Running), spawn_water)
            .add_systems(Update, animate_water.run_if(in_state(AppState::Running)));
    }
}

fn spawn_water(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let world = registry.0.active_world().expect("world");
    let water_def = registry.0.water.get(&world.water).expect("water");

    let material = materials.add(WaterMaterial {
        params: WaterParams {
            shallow_color: Vec4::new(
                water_def.shallow_color[0],
                water_def.shallow_color[1],
                water_def.shallow_color[2],
                water_def.transparency,
            ),
            deep_color: Vec4::new(
                water_def.deep_color[0],
                water_def.deep_color[1],
                water_def.deep_color[2],
                1.0,
            ),
            wave: Vec4::new(
                water_def.sea_level_m,
                water_def.wave_speed,
                water_def.wave_amplitude,
                water_def.transparency,
            ),
            animation: Vec4::ZERO,
        },
    });

    commands.spawn((
        WaterSurface,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(200.0, 200.0))),
        MeshMaterial3d(material),
        Transform::from_xyz(0.0, water_def.sea_level_m + 0.02, 0.0),
    ));
}

fn animate_water(
    time: Res<Time>,
    registry: Res<ConfigRegistryResource>,
    mut water: Query<&mut MeshMaterial3d<WaterMaterial>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let Ok(world) = registry.0.active_world() else {
        return;
    };
    let Some(water_def) = registry.0.water.get(&world.water) else {
        return;
    };
    for mat_handle in &mut water {
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            mat.params.animation.x = time.elapsed_secs();
            mat.params.wave.y = water_def.wave_speed;
            mat.params.wave.z = water_def.wave_amplitude;
        }
    }
}
