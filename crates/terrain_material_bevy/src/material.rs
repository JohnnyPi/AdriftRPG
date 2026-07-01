use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

#[derive(Clone, Copy, Debug, ShaderType, Default)]
pub struct TerrainMaterialSettings {
    pub triplanar_sharpness: f32,
    pub global_texture_scale: f32,
    pub normal_strength: f32,
    pub height_blend_strength: f32,
    pub layer_count: u32,
    pub debug_mode: u32,
    pub _padding: Vec2,
}

/// Per-layer repeat scale in world meters (up to 8 layers in two vec4 uniforms).
#[derive(Clone, Copy, Debug, ShaderType, Default)]
pub struct TerrainLayerScales {
    pub scales0: Vec4,
    pub scales1: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TerrainPbrMaterial {
    #[texture(0, dimension = "2d_array")]
    #[sampler(1)]
    pub albedo_array: Handle<Image>,

    #[texture(2, dimension = "2d_array")]
    #[sampler(3)]
    pub normal_array: Handle<Image>,

    #[texture(4, dimension = "2d_array")]
    #[sampler(5)]
    pub ormh_array: Handle<Image>,

    #[uniform(6)]
    pub settings: TerrainMaterialSettings,

    #[uniform(7)]
    pub layer_scales: TerrainLayerScales,
}

impl Material for TerrainPbrMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain_material.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}

impl TerrainPbrMaterial {
    pub fn fallback(
        images: &mut Assets<Image>,
        materials: &mut Assets<Self>,
        layers: u32,
    ) -> Handle<Self> {
        let handles = crate::arrays::create_placeholder_array_images(images, layers);
        materials.add(Self {
            albedo_array: handles.albedo,
            normal_array: handles.normal,
            ormh_array: handles.ormh,
            settings: TerrainMaterialSettings {
                layer_count: layers,
                debug_mode: 2,
                ..Default::default()
            },
            layer_scales: TerrainLayerScales::default(),
        })
    }
}

pub fn layer_scales_from_recipes(
    recipes: &[procedural_textures::TerrainMaterialRecipe],
) -> TerrainLayerScales {
    let mut values = [1.0f32; 8];
    for (i, recipe) in recipes.iter().take(8).enumerate() {
        values[i] = recipe.meters_per_repeat.max(0.01);
    }
    TerrainLayerScales {
        scales0: Vec4::new(values[0], values[1], values[2], values[3]),
        scales1: Vec4::new(values[4], values[5], values[6], values[7]),
    }
}
