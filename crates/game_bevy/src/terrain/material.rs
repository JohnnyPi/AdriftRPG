use bevy::prelude::*;
use bevy::pbr::Material;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use crate::state::AppState;

#[derive(Resource, Clone)]
pub struct TerrainMaterialHandle(pub Handle<TerrainTriplanarMaterial>);

/// Flat vec4 fields avoid WGSL uniform-array storage class mismatches.
#[derive(ShaderType, Clone, Copy, Debug)]
pub struct TerrainParams {
    pub color0: Vec4,
    pub color1: Vec4,
    pub color2: Vec4,
    pub color3: Vec4,
    pub color4: Vec4,
    /// `.x` = triplanar scale, `.y` = roughness
    pub props0: Vec4,
    pub props1: Vec4,
    pub props2: Vec4,
    pub props3: Vec4,
    pub props4: Vec4,
}

impl TerrainParams {
    pub fn set_color(&mut self, idx: usize, color: Vec4) {
        match idx {
            0 => self.color0 = color,
            1 => self.color1 = color,
            2 => self.color2 = color,
            3 => self.color3 = color,
            _ => self.color4 = color,
        }
    }

    pub fn set_props(&mut self, idx: usize, props: Vec4) {
        match idx {
            0 => self.props0 = props,
            1 => self.props1 = props,
            2 => self.props2 = props,
            3 => self.props3 = props,
            _ => self.props4 = props,
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TerrainTriplanarMaterial {
    #[uniform(0)]
    pub params: TerrainParams,
}

impl Material for TerrainTriplanarMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain_triplanar.wgsl".into()
    }
}

impl TerrainTriplanarMaterial {
    pub fn default_catalog() -> Self {
        Self {
            params: TerrainParams {
                color0: Vec4::new(0.34, 0.52, 0.28, 1.0),
                color1: Vec4::new(0.86, 0.78, 0.58, 1.0),
                color2: Vec4::new(0.45, 0.44, 0.42, 1.0),
                color3: Vec4::new(0.28, 0.26, 0.30, 1.0),
                color4: Vec4::new(0.32, 0.38, 0.36, 1.0),
                props0: Vec4::new(0.5, 0.9, 0.0, 0.0),
                props1: Vec4::new(0.35, 0.95, 0.0, 0.0),
                props2: Vec4::new(0.25, 0.85, 0.0, 0.0),
                props3: Vec4::new(0.2, 0.95, 0.0, 0.0),
                props4: Vec4::new(0.25, 0.4, 0.0, 0.0),
            },
        }
    }

    pub fn from_registry(registry: &game_data::ConfigRegistry) -> Self {
        let world = registry.active_world().expect("world");
        let materials = registry.materials.get(&world.materials).expect("materials");
        let mut mat = Self::default_catalog();
        for entry in &materials.materials {
            let idx = entry.id as usize;
            if idx < 5 {
                mat.params.set_color(
                    idx,
                    Vec4::new(entry.albedo[0], entry.albedo[1], entry.albedo[2], 1.0),
                );
                mat.params.set_props(
                    idx,
                    Vec4::new(entry.triplanar_scale, entry.roughness, 0.0, 0.0),
                );
            }
        }
        mat
    }
}

pub struct TerrainMaterialPlugin;

impl Plugin for TerrainMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<TerrainTriplanarMaterial>::default())
            .add_systems(OnEnter(AppState::Running), init_terrain_material);
    }
}

fn init_terrain_material(
    registry: Res<crate::data::ConfigRegistryResource>,
    mut materials: ResMut<Assets<TerrainTriplanarMaterial>>,
    mut commands: Commands,
) {
    let handle = materials.add(TerrainTriplanarMaterial::from_registry(&registry.0));
    commands.insert_resource(TerrainMaterialHandle(handle));
}
