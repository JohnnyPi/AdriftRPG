// crates/terrain_material_bevy/src/material.rs
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::prelude::*;
use bevy::render::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderType};
use bevy::render::render_resource::SpecializedMeshPipelineError;
use bevy::shader::ShaderRef;
use terrain_surface::{CHUNK_LOCAL_SLOT_COUNT, ChunkSlotPalette, UNUSED_SLOT};

#[derive(Clone, Copy, Debug, ShaderType, Default)]
pub struct TerrainMaterialSettings {
    pub triplanar_sharpness: f32,
    pub global_texture_scale: f32,
    pub normal_strength: f32,
    pub height_blend_strength: f32,
    pub layer_count: u32,
    pub debug_mode: u32,
    pub macro_variation_scale: f32,
    pub macro_variation_strength: f32,
    pub global_wetness: f32,
    pub global_moss: f32,
}

/// Per-layer repeat scale in world meters (up to 64 layers).
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct TerrainLayerScales {
    pub count: u32,
    pub _padding0: u32,
    pub _padding1: u32,
    pub _padding2: u32,
    pub scales: [Vec4; 16],
}

impl Default for TerrainLayerScales {
    fn default() -> Self {
        Self {
            count: 0,
            _padding0: 0,
            _padding1: 0,
            _padding2: 0,
            scales: [Vec4::ONE; 16],
        }
    }
}

#[derive(Clone, Copy, Debug, ShaderType, Default)]
pub struct ChunkSlotPaletteUniform {
    pub local_to_global: [UVec4; 2],
}

fn pack_chunk_slots(slots: &[u32; CHUNK_LOCAL_SLOT_COUNT]) -> [UVec4; 2] {
    [
        UVec4::new(slots[0], slots[1], slots[2], slots[3]),
        UVec4::new(slots[4], slots[5], slots[6], slots[7]),
    ]
}

impl From<ChunkSlotPalette> for ChunkSlotPaletteUniform {
    fn from(palette: ChunkSlotPalette) -> Self {
        Self {
            local_to_global: pack_chunk_slots(palette.local_to_global()),
        }
    }
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

    #[uniform(8)]
    pub chunk_slots: ChunkSlotPaletteUniform,
}

impl Material for TerrainPbrMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/terrain_material.wgsl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "shaders/terrain_material.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Cave interiors and overhangs expose back-facing terrain shells; default
        // back-face culling made surface geometry vanish at certain view angles.
        descriptor.primitive.cull_mode = None;
        Ok(())
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
            chunk_slots: ChunkSlotPaletteUniform::default(),
        })
    }

    pub fn with_chunk_palette(&self, palette: ChunkSlotPalette) -> Self {
        let mut clone = self.clone();
        clone.chunk_slots = palette.into();
        clone
    }
}

pub fn layer_scales_from_recipes(
    recipes: &[procedural_textures::TerrainMaterialRecipe],
) -> TerrainLayerScales {
    let mut values = [0.0f32; 64];
    for (i, recipe) in recipes.iter().take(64).enumerate() {
        values[i] = recipe.meters_per_repeat.max(0.01);
    }
    let mut scales = [Vec4::ONE; 16];
    for (chunk, values_chunk) in scales.iter_mut().zip(values.chunks_exact(4)) {
        *chunk = Vec4::new(
            values_chunk[0],
            values_chunk[1],
            values_chunk[2],
            values_chunk[3],
        );
    }
    TerrainLayerScales {
        count: recipes.len().min(64) as u32,
        _padding0: 0,
        _padding1: 0,
        _padding2: 0,
        scales,
    }
}

pub fn default_chunk_slots() -> ChunkSlotPaletteUniform {
    let mut slots = [UNUSED_SLOT; CHUNK_LOCAL_SLOT_COUNT];
    for (i, slot) in slots.iter_mut().enumerate() {
        *slot = i as u32;
    }
    ChunkSlotPaletteUniform {
        local_to_global: pack_chunk_slots(&slots),
    }
}
