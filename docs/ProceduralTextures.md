# Native Bevy 0.19 Procedural Terrain Material Pipeline

## 1. Objective

Build a Bevy 0.19-native terrain-material system with:

* CPU-generated, seamless PBR textures based on Symbios generators;
* no runtime dependency on `bevy_symbios_texture`;
* texture arrays for albedo, normals, and packed PBR data;
* world-space triplanar projection;
* up to four materials blended per terrain vertex;
* a custom surface classifier driven by biome, geology, slope, moisture, water, caves, and coast data;
* one shared material for all terrain chunks;
* an offline baking path for release builds;
* an optional asynchronous generation path for development and mods.

The final flow is:

```text
Existing terrain generator
    ↓
Surface mesh positions and normals
    ↓
Sample biome/geology/environment fields
    ↓
Custom surface classifier
    ↓
Four material IDs and weights per vertex
    ↓
Custom Bevy mesh attributes
    ↓
Native Bevy 0.19 TerrainMaterial
    ↓
Albedo, normal, and PBR texture arrays
    ↓
Triplanar WGSL sampling
    ↓
Bevy PBR lighting
```

Bevy 0.19 supports custom materials through `Material`, `MaterialPlugin`, and `AsBindGroup`. Its official array-texture example demonstrates `texture_2d_array`, array-image loading, custom material bindings, and reuse of Bevy’s PBR shader functions.

---

# 2. Why extract Symbios instead of porting its Bevy plugin?

The useful part of Symbios is mostly engine-independent:

* procedural noise;
* toroidal seamless sampling;
* surface height generation;
* albedo generation;
* tangent-space normal generation;
* ORM generation;
* deterministic seeding;
* parallel row generation;
* mipmap algorithms.

Its current Bevy wrapper targets Bevy 0.18, but the underlying texture algorithms do not inherently require Bevy. Symbios generates CPU-side albedo, normal, ORM, and optional emissive buffers, with surface textures designed to tile seamlessly.

The recommended split is:

```text
symbios source algorithms
    → procedural_textures crate with no Bevy dependency

Bevy 0.19 adapter
    → creates Images and array textures

terrain material crate
    → custom Material and WGSL

surface classifier
    → chooses texture layers
```

This limits future Bevy upgrade work to the rendering adapter.

---

# 3. Recommended workspace

```text
crates/
  terrain_core/
    Existing density, voxel, biome, geology and field data

  terrain_meshing/
    Surface Nets, Dual Contouring, mesh buffers

  terrain_surface/
    Surface classifier and material identities

  procedural_textures/
    Extracted Symbios CPU generators

  terrain_material_bevy/
    Bevy 0.19 images, texture arrays, custom material and shader

  terrain_tools/
    Optional offline material baker
```

Dependency direction:

```text
terrain_core
       ↑
terrain_surface
       ↑
terrain_meshing

procedural_textures
       ↑
terrain_material_bevy
       ↑
game application
```

`procedural_textures` should not depend on:

* Bevy;
* ECS;
* rendering;
* asset handles;
* WGSL;
* your terrain engine.

## Your broader terrain design already separates field generation, voxelization, meshing, and Bevy integration; this material pipeline preserves that separation.

# 4. Cargo configuration

At workspace level:

```toml
[workspace]
resolver = "3"
members = [
    "crates/terrain_core",
    "crates/terrain_surface",
    "crates/terrain_meshing",
    "crates/procedural_textures",
    "crates/terrain_material_bevy",
    "crates/terrain_tools",
]

[workspace.dependencies]
bevy = "0.19"
serde = { version = "1", features = ["derive"] }
thiserror = "2"
rayon = "1"
blake3 = "1"
bytemuck = { version = "1", features = ["derive"] }
```

`procedural_textures/Cargo.toml`:

```toml
[package]
name = "procedural_textures"
version = "0.1.0"
edition = "2024"

[dependencies]
serde.workspace = true
thiserror.workspace = true
rayon.workspace = true
blake3.workspace = true

# Add the exact noise/math dependencies used by the extracted
# Symbios modules.
```

`terrain_material_bevy/Cargo.toml`:

```toml
[package]
name = "terrain_material_bevy"
version = "0.1.0"
edition = "2024"

[dependencies]
bevy.workspace = true
serde.workspace = true
thiserror.workspace = true
bytemuck.workspace = true

procedural_textures = {
    path = "../procedural_textures"
}

terrain_surface = {
    path = "../terrain_surface"
}

terrain_meshing = {
    path = "../terrain_meshing"
}
```

---

# 5. Extracting the Symbios generators

The source repository uses an MIT license, so its code can be reused and modified subject to preserving the license notice.

## 5.1 Copy only CPU-side modules

Retain or adapt modules responsible for:

```text
generator traits
noise helpers
toroidal coordinate mapping
normal derivation
mipmap generation
config fingerprints
surface generators:
    rock
    ground
    sand
    cobblestone
    lava
    snow
    ice
```

Exclude Bevy-specific modules:

```text
SymbiosTexturePlugin
PendingTexture
TextureReady
Assets<Image>
Handle<Image>
StandardMaterial helpers
egui editor widgets
Bevy task integration
```

## 5.2 Replace the public result type

Create a neutral texture result:

```rust
#[derive(Clone, Debug)]
pub struct GeneratedPbrMaps {
    pub width: u32,
    pub height: u32,

    /// RGBA8, color data stored in linear values converted to
    /// display-ready sRGB bytes.
    pub albedo_rgba8: Vec<u8>,

    /// RGBA8 tangent-space normal map.
    /// R=X, G=Y, B=Z, A=255.
    pub normal_rgba8: Vec<u8>,

    /// RGBA8 packed map.
    /// R=ambient occlusion
    /// G=roughness
    /// B=metallic
    /// A=height or 255 when unavailable
    pub ormh_rgba8: Vec<u8>,

    pub emissive_rgba8: Option<Vec<u8>>,

    pub mip_level_count: u32,
}
```

Symbios currently defines its core surface maps as:

```text
albedo: sRGB color
normal: tangent-space RGB
ORM:
    R = occlusion
    G = roughness
    B = metallic
```

That channel layout should remain explicit in your extracted crate.

## 5.3 Define a Bevy-independent generator trait

```rust
use std::error::Error;

pub trait ProceduralTextureGenerator:
    Send + Sync + 'static
{
    fn generate(
        &self,
        width: u32,
        height: u32,
    ) -> Result<GeneratedPbrMaps, Box<dyn Error + Send + Sync>>;

    fn fingerprint(&self) -> [u8; 32];
}
```

Prefer a typed error in production:

```rust
#[derive(Debug, thiserror::Error)]
pub enum TextureGenerationError {
    #[error("texture dimensions must be non-zero")]
    ZeroDimension,

    #[error("texture dimensions exceed configured limit")]
    DimensionsTooLarge,

    #[error("generated buffer had invalid length")]
    InvalidBufferLength,

    #[error("invalid generator configuration: {0}")]
    InvalidConfig(String),
}
```

## 5.4 Wrap individual generators

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum TextureRecipe {
    Rock(RockConfig),
    Ground(GroundConfig),
    Sand(SandConfig),
    Cobblestone(CobblestoneConfig),
    Lava(LavaConfig),
}
```

```rust
impl TextureRecipe {
    pub fn generate(
        &self,
        width: u32,
        height: u32,
    ) -> Result<GeneratedPbrMaps, TextureGenerationError> {
        match self {
            Self::Rock(config) => {
                RockGenerator::new(config.clone())
                    .generate(width, height)
            }
            Self::Ground(config) => {
                GroundGenerator::new(config.clone())
                    .generate(width, height)
            }
            Self::Sand(config) => {
                SandGenerator::new(config.clone())
                    .generate(width, height)
            }
            Self::Cobblestone(config) => {
                CobblestoneGenerator::new(config.clone())
                    .generate(width, height)
            }
            Self::Lava(config) => {
                LavaGenerator::new(config.clone())
                    .generate(width, height)
            }
        }
    }
}
```

Rock, ground, and sand are useful initial bases: Symbios describes rock as ridged multifractal stone, ground as dual-scale organic soil, and sand as directional ripples plus grain detail.

---

# 6. Expose height data

Normal maps are derived from an implicit scalar surface. Preserve that scalar field before converting it to normals.

Change generators internally from:

```text
height field
    ↓
normal generation
    ↓
discard height
```

to:

```text
height field
    ├── normal generation
    └── grayscale height channel
```

Example helper:

```rust
pub fn encode_height_u8(
    height: &[f32],
    min_value: f32,
    max_value: f32,
) -> Vec<u8> {
    let range = (max_value - min_value).max(f32::EPSILON);

    height
        .iter()
        .map(|value| {
            let normalized =
                ((*value - min_value) / range).clamp(0.0, 1.0);

            (normalized * 255.0).round() as u8
        })
        .collect()
}
```

Pack height into alpha:

```rust
pub fn pack_ormh(
    ao: &[u8],
    roughness: &[u8],
    metallic: &[u8],
    height: &[u8],
) -> Vec<u8> {
    let count = ao.len();

    assert_eq!(roughness.len(), count);
    assert_eq!(metallic.len(), count);
    assert_eq!(height.len(), count);

    let mut output = Vec::with_capacity(count * 4);

    for index in 0..count {
        output.extend_from_slice(&[
            ao[index],
            roughness[index],
            metallic[index],
            height[index],
        ]);
    }

    output
}
```

Height is not required for the first version, but retaining it now makes later height-aware blending possible.

---

# 7. Stable terrain-material identities

Do not use array indices as persistent identity.

```rust
#[repr(u16)]
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum TerrainMaterialId {
    FreshBasalt = 0,
    WeatheredBasalt = 1,
    CaveBasalt = 2,
    TropicalRedSoil = 3,
    JungleLoam = 4,
    JungleMoss = 5,
    LeafLitter = 6,
    CoralSand = 7,
    BlackSand = 8,
    CoralRubble = 9,
    RiverGravel = 10,
    RiverSilt = 11,
    Mud = 12,
    Limestone = 13,
    Flowstone = 14,
    VolcanicAsh = 15,
}
```

Create a stable runtime layer registry:

```rust
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct MaterialLayerRegistry {
    by_material: BTreeMap<TerrainMaterialId, u32>,
    by_layer: Vec<TerrainMaterialId>,
}

impl MaterialLayerRegistry {
    pub fn build(
        ordered_materials: impl IntoIterator<Item = TerrainMaterialId>,
    ) -> Self {
        let mut registry = Self::default();

        for material in ordered_materials {
            let layer = registry.by_layer.len() as u32;

            assert!(
                registry.by_material.insert(material, layer).is_none(),
                "duplicate material {material:?}"
            );

            registry.by_layer.push(material);
        }

        registry
    }

    pub fn layer(&self, material: TerrainMaterialId) -> u32 {
        *self
            .by_material
            .get(&material)
            .expect("material missing from texture array")
    }

    pub fn layer_count(&self) -> u32 {
        self.by_layer.len() as u32
    }
}
```

Use a fixed order:

```rust
pub const CORE_TERRAIN_MATERIALS: &[TerrainMaterialId] = &[
    TerrainMaterialId::FreshBasalt,
    TerrainMaterialId::WeatheredBasalt,
    TerrainMaterialId::CaveBasalt,
    TerrainMaterialId::TropicalRedSoil,
    TerrainMaterialId::JungleLoam,
    TerrainMaterialId::JungleMoss,
    TerrainMaterialId::LeafLitter,
    TerrainMaterialId::CoralSand,
    TerrainMaterialId::BlackSand,
    TerrainMaterialId::CoralRubble,
    TerrainMaterialId::RiverGravel,
    TerrainMaterialId::RiverSilt,
    TerrainMaterialId::Mud,
    TerrainMaterialId::Limestone,
    TerrainMaterialId::Flowstone,
    TerrainMaterialId::VolcanicAsh,
];
```

---

# 8. Material recipes

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TerrainMaterialRecipe {
    pub id: TerrainMaterialId,
    pub resolution: u32,
    pub meters_per_repeat: f32,
    pub generator: TextureRecipe,

    #[serde(default = "default_normal_strength")]
    pub normal_strength: f32,

    #[serde(default)]
    pub tint: [f32; 3],
}

fn default_normal_strength() -> f32 {
    1.0
}
```

Example YAML:

```yaml
schema_version: 1

materials:
  - id: WeatheredBasalt
    resolution: 1024
    meters_per_repeat: 3.5
    normal_strength: 1.15

    generator:
      Rock:
        seed: 1002
        scale: 3.0
        octaves: 7
        attenuation: 2.0
        color_light: [0.25, 0.22, 0.18]
        color_dark: [0.07, 0.06, 0.055]
        normal_strength: 3.7

  - id: TropicalRedSoil
    resolution: 1024
    meters_per_repeat: 2.0
    normal_strength: 0.9

    generator:
      Ground:
        seed: 2001
        macro_scale: 2.2
        macro_octaves: 5
        micro_scale: 10.0
        micro_octaves: 4
        micro_weight: 0.38
        color_dry: [0.48, 0.17, 0.07]
        color_moist: [0.19, 0.055, 0.025]
        normal_strength: 2.0
```

All textures placed in one array must have identical:

* width;
* height;
* texture format;
* mip count;
* layer layout.

Validate this before GPU upload.

---

# 9. Building CPU texture arrays

Store all layers contiguously:

```text
layer 0 mip 0
layer 1 mip 0
layer 2 mip 0
...
```

For a base-level-only first implementation:

```rust
pub struct CpuTextureArrays {
    pub width: u32,
    pub height: u32,
    pub layers: u32,

    pub albedo: Vec<u8>,
    pub normal: Vec<u8>,
    pub ormh: Vec<u8>,
}
```

```rust
pub fn build_cpu_arrays(
    recipes: &[TerrainMaterialRecipe],
) -> Result<CpuTextureArrays, TextureGenerationError> {
    let first = recipes.first().ok_or_else(|| {
        TextureGenerationError::InvalidConfig(
            "no terrain material recipes".to_owned(),
        )
    })?;

    let width = first.resolution;
    let height = first.resolution;

    let pixels_per_layer = width as usize * height as usize;
    let bytes_per_layer = pixels_per_layer * 4;

    let mut albedo =
        Vec::with_capacity(bytes_per_layer * recipes.len());
    let mut normal =
        Vec::with_capacity(bytes_per_layer * recipes.len());
    let mut ormh =
        Vec::with_capacity(bytes_per_layer * recipes.len());

    for recipe in recipes {
        if recipe.resolution != width {
            return Err(TextureGenerationError::InvalidConfig(
                format!(
                    "{:?} uses resolution {}, expected {}",
                    recipe.id,
                    recipe.resolution,
                    width,
                ),
            ));
        }

        let maps = recipe.generator.generate(width, height)?;

        validate_map_lengths(&maps)?;

        albedo.extend_from_slice(&maps.albedo_rgba8);
        normal.extend_from_slice(&maps.normal_rgba8);
        ormh.extend_from_slice(&maps.ormh_rgba8);
    }

    Ok(CpuTextureArrays {
        width,
        height,
        layers: recipes.len() as u32,
        albedo,
        normal,
        ormh,
    })
}
```

```rust
fn validate_map_lengths(
    maps: &GeneratedPbrMaps,
) -> Result<(), TextureGenerationError> {
    let expected =
        maps.width as usize * maps.height as usize * 4;

    for buffer in [
        &maps.albedo_rgba8,
        &maps.normal_rgba8,
        &maps.ormh_rgba8,
    ] {
        if buffer.len() != expected {
            return Err(
                TextureGenerationError::InvalidBufferLength
            );
        }
    }

    Ok(())
}
```

---

# 10. Creating Bevy 0.19 array images

Bevy’s official material example binds array images with:

```rust
#[texture(0, dimension = "2d_array")]
#[sampler(1)]
```

and samples them in WGSL through `texture_2d_array<f32>`.

For runtime-generated arrays:

```rust
use bevy::{
    asset::RenderAssetUsages,
    image::{Image, ImageAddressMode, ImageFilterMode, ImageSampler},
    render::render_resource::{
        Extent3d,
        TextureDimension,
        TextureFormat,
        TextureUsages,
    },
};
```

```rust
pub fn create_array_image(
    width: u32,
    height: u32,
    layers: u32,
    data: Vec<u8>,
    format: TextureFormat,
) -> Image {
    let expected =
        width as usize
        * height as usize
        * layers as usize
        * 4;

    assert_eq!(data.len(), expected);

    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: layers,
        },
        TextureDimension::D2,
        data,
        format,
        RenderAssetUsages::RENDER_WORLD
            | RenderAssetUsages::MAIN_WORLD,
    );

    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING
        | TextureUsages::COPY_DST;

    image.sampler = ImageSampler::Descriptor(
        bevy::image::ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            address_mode_w: ImageAddressMode::ClampToEdge,
            mag_filter: ImageFilterMode::Linear,
            min_filter: ImageFilterMode::Linear,
            mipmap_filter: ImageFilterMode::Linear,
            ..Default::default()
        },
    );

    image
}
```

Create the three arrays:

```rust
pub struct TerrainArrayHandles {
    pub albedo: Handle<Image>,
    pub normal: Handle<Image>,
    pub ormh: Handle<Image>,
}
```

```rust
pub fn upload_texture_arrays(
    cpu: CpuTextureArrays,
    images: &mut Assets<Image>,
) -> TerrainArrayHandles {
    let albedo = create_array_image(
        cpu.width,
        cpu.height,
        cpu.layers,
        cpu.albedo,
        TextureFormat::Rgba8UnormSrgb,
    );

    let normal = create_array_image(
        cpu.width,
        cpu.height,
        cpu.layers,
        cpu.normal,
        TextureFormat::Rgba8Unorm,
    );

    let ormh = create_array_image(
        cpu.width,
        cpu.height,
        cpu.layers,
        cpu.ormh,
        TextureFormat::Rgba8Unorm,
    );

    TerrainArrayHandles {
        albedo: images.add(albedo),
        normal: images.add(normal),
        ormh: images.add(ormh),
    }
}
```

The exact `ImageSamplerDescriptor` import path can shift between Bevy patch releases; keep this helper isolated in `terrain_material_bevy`.

---

# 11. Mipmaps

Base-level-only textures are acceptable for the first proof, but terrain requires mipmaps to reduce:

* shimmering;
* moiré;
* distant noise;
* aliasing;
* texture crawl.

The extracted Symbios implementation already has type-aware mip generation and stores a mip count in `TextureMap`. Its 0.6 release computes mipmaps on worker threads.

Preserve three separate mip algorithms:

## Albedo

Average in linear color space, then encode back to sRGB.

## Normal

Decode vectors, average them, normalize, and repack.

## ORMH

Average each scalar channel independently.

Array mip data must be ordered according to the GPU texture upload layout expected by wgpu/Bevy. Implement and test mip upload separately from base-level array construction.

A sensible staged approach:

```text
Milestone 1:
base level only

Milestone 2:
CPU mip chain for each individual layer

Milestone 3:
pack all layers and mip levels

Milestone 4:
compressed baked arrays for release
```

---

# 12. Native Bevy terrain material

```rust
use bevy::{
    pbr::Material,
    prelude::*,
    reflect::TypePath,
    render::render_resource::{
        AsBindGroup,
        ShaderType,
    },
    shader::ShaderRef,
};
```

```rust
#[derive(Clone, Copy, Debug, ShaderType)]
pub struct TerrainMaterialSettings {
    pub triplanar_sharpness: f32,
    pub global_texture_scale: f32,
    pub normal_strength: f32,
    pub height_blend_strength: f32,

    pub layer_count: u32,
    pub debug_mode: u32,
    pub _padding: Vec2,
}
```

```rust
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TerrainMaterial {
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
}
```

```rust
impl Material for TerrainMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain_material.wgsl".into()
    }
}
```

Register it:

```rust
pub struct TerrainMaterialPlugin;

impl Plugin for TerrainMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            MaterialPlugin::<TerrainMaterial>::default()
        );
    }
}
```

Bevy’s `Material` trait is intended for custom high-level mesh rendering and derives GPU bindings through `AsBindGroup`. Materials are attached through `Mesh3d` and `MeshMaterial3d`.

---

# 13. Optional per-layer parameters

A single global scale is limiting. Add fixed-size arrays to the uniform for a small initial library:

```rust
pub const MAX_TERRAIN_LAYERS: usize = 32;

#[derive(Clone, Copy, Debug, ShaderType)]
pub struct TerrainLayerParameters {
    pub meters_per_repeat: f32,
    pub normal_strength: f32,
    pub roughness_multiplier: f32,
    pub tint_strength: f32,
}
```

Large fixed uniform arrays can become awkward. Alternatives are:

1. a small parameter texture;
2. a storage buffer;
3. a binding array;
4. grouping layers into several materials.

For 16–32 layers, a parameter texture is simple:

```text
one texel per layer

R = reciprocal meters per repeat
G = normal strength
B = roughness multiplier
A = reserved
```

That avoids custom render-world storage-buffer plumbing.

---

# 14. Custom terrain mesh attributes

Each vertex needs:

* four material layer indices;
* four weights.

Use integer indices and normalized float weights.

```rust
use bevy::{
    mesh::{
        MeshVertexAttribute,
        MeshVertexBufferLayoutRef,
    },
    render::render_resource::VertexFormat,
};

pub const ATTRIBUTE_TERRAIN_MATERIAL_INDICES:
    MeshVertexAttribute =
    MeshVertexAttribute::new(
        "TerrainMaterialIndices",
        0xA100_0001,
        VertexFormat::Uint32x4,
    );

pub const ATTRIBUTE_TERRAIN_MATERIAL_WEIGHTS:
    MeshVertexAttribute =
    MeshVertexAttribute::new(
        "TerrainMaterialWeights",
        0xA100_0002,
        VertexFormat::Float32x4,
    );
```

The numeric IDs must be globally unique inside your project.

Mesh-side record:

```rust
#[derive(Clone, Copy, Debug)]
pub struct TerrainMaterialVertex {
    pub indices: [u32; 4],
    pub weights: [f32; 4],
}
```

---

# 15. Surface classifier input

```rust
#[derive(Clone, Copy, Debug)]
pub struct SurfaceContext {
    pub world_position: Vec3,
    pub world_normal: Vec3,

    pub elevation_m: f32,
    pub sea_level_m: f32,
    pub water_depth_m: f32,

    pub slope_degrees: f32,
    pub moisture: f32,
    pub soil_depth_m: f32,

    pub coast_distance_m: f32,
    pub river_distance_m: f32,
    pub wave_exposure: f32,

    pub cave_exposure: f32,
    pub mineral_deposition: f32,

    pub biome: BiomeId,
    pub geology: GeologyId,
}
```

```rust
pub fn slope_degrees(normal: Vec3) -> f32 {
    normal
        .normalize_or(Vec3::Y)
        .dot(Vec3::Y)
        .clamp(-1.0, 1.0)
        .acos()
        .to_degrees()
}
```

Sample these fields while constructing the terrain mesh. Do not make the render shader query simulation structures.

---

# 16. Classifier output

```rust
#[derive(Clone, Copy, Debug)]
pub struct SurfaceMaterialBlend {
    pub materials: [TerrainMaterialId; 4],
    pub weights: [f32; 4],
}
```

```rust
impl SurfaceMaterialBlend {
    pub fn single(material: TerrainMaterialId) -> Self {
        Self {
            materials: [material; 4],
            weights: [1.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn normalize(mut self) -> Self {
        for weight in &mut self.weights {
            *weight = weight.max(0.0);
        }

        let sum: f32 = self.weights.iter().sum();

        if sum <= f32::EPSILON {
            self.weights = [1.0, 0.0, 0.0, 0.0];
        } else {
            for weight in &mut self.weights {
                *weight /= sum;
            }
        }

        self
    }
}
```

Classifier trait:

```rust
pub trait SurfaceClassifier: Send + Sync {
    fn classify(
        &self,
        context: &SurfaceContext,
    ) -> SurfaceMaterialBlend;
}
```

---

# 17. Initial tropical-island classifier

```rust
#[derive(Default)]
pub struct TropicalIslandClassifier;

impl SurfaceClassifier for TropicalIslandClassifier {
    fn classify(
        &self,
        c: &SurfaceContext,
    ) -> SurfaceMaterialBlend {
        if c.cave_exposure > 0.55 {
            return classify_cave(c);
        }

        if c.water_depth_m > 0.05 {
            return classify_underwater(c);
        }

        if c.coast_distance_m < 20.0
            && c.elevation_m < c.sea_level_m + 4.0
        {
            return classify_coast(c);
        }

        if c.river_distance_m < 5.0 {
            return classify_river(c);
        }

        if c.slope_degrees > 48.0 {
            return classify_cliff(c);
        }

        classify_land(c)
    }
}
```

Utilities:

```rust
#[inline]
pub fn saturate(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

#[inline]
pub fn smoothstep(
    start: f32,
    end: f32,
    value: f32,
) -> f32 {
    if (end - start).abs() < f32::EPSILON {
        return if value >= end { 1.0 } else { 0.0 };
    }

    let t = saturate((value - start) / (end - start));
    t * t * (3.0 - 2.0 * t)
}
```

## Cliff

```rust
fn classify_cliff(
    c: &SurfaceContext,
) -> SurfaceMaterialBlend {
    let moss =
        smoothstep(0.45, 0.95, c.moisture)
        * (1.0 - smoothstep(
            70.0,
            88.0,
            c.slope_degrees,
        ));

    let fresh_rock =
        smoothstep(68.0, 88.0, c.slope_degrees);

    SurfaceMaterialBlend {
        materials: [
            TerrainMaterialId::WeatheredBasalt,
            TerrainMaterialId::FreshBasalt,
            TerrainMaterialId::JungleMoss,
            TerrainMaterialId::TropicalRedSoil,
        ],
        weights: [
            0.70 * (1.0 - fresh_rock),
            0.30 + fresh_rock,
            moss * 0.25,
            saturate(c.soil_depth_m / 1.5) * 0.10,
        ],
    }
    .normalize()
}
```

## Coast

```rust
fn classify_coast(
    c: &SurfaceContext,
) -> SurfaceMaterialBlend {
    let rock =
        smoothstep(25.0, 50.0, c.slope_degrees);

    let wave_rubble =
        saturate(c.wave_exposure) * (1.0 - rock);

    let shoreline =
        smoothstep(
            c.sea_level_m + 2.0,
            c.sea_level_m,
            c.elevation_m,
        );

    SurfaceMaterialBlend {
        materials: [
            TerrainMaterialId::CoralSand,
            TerrainMaterialId::BlackSand,
            TerrainMaterialId::CoralRubble,
            TerrainMaterialId::WeatheredBasalt,
        ],
        weights: [
            (1.0 - rock)
                * (1.0 - wave_rubble)
                * 0.70,
            shoreline * 0.20,
            wave_rubble * 0.45,
            rock,
        ],
    }
    .normalize()
}
```

## River

```rust
fn classify_river(
    c: &SurfaceContext,
) -> SurfaceMaterialBlend {
    let channel =
        1.0 - smoothstep(
            0.0,
            5.0,
            c.river_distance_m,
        );

    let mud =
        channel * smoothstep(0.55, 0.95, c.moisture);

    SurfaceMaterialBlend {
        materials: [
            TerrainMaterialId::RiverGravel,
            TerrainMaterialId::Mud,
            TerrainMaterialId::RiverSilt,
            TerrainMaterialId::JungleLoam,
        ],
        weights: [
            channel * (1.0 - mud),
            mud * 0.55,
            mud * 0.45,
            1.0 - channel,
        ],
    }
    .normalize()
}
```

## Cave

```rust
fn classify_cave(
    c: &SurfaceContext,
) -> SurfaceMaterialBlend {
    let flowstone =
        smoothstep(
            0.35,
            0.85,
            c.mineral_deposition,
        );

    let moss =
        smoothstep(0.60, 0.95, c.moisture)
        * smoothstep(0.45, 0.70, c.cave_exposure);

    SurfaceMaterialBlend {
        materials: [
            TerrainMaterialId::CaveBasalt,
            TerrainMaterialId::Flowstone,
            TerrainMaterialId::JungleMoss,
            TerrainMaterialId::Limestone,
        ],
        weights: [
            1.0 - flowstone,
            flowstone,
            moss * 0.15,
            match c.geology {
                GeologyId::Limestone => 0.65,
                _ => 0.0,
            },
        ],
    }
    .normalize()
}
```

## General land

```rust
fn classify_land(
    c: &SurfaceContext,
) -> SurfaceMaterialBlend {
    let exposed_rock =
        smoothstep(10.0, 34.0, c.slope_degrees)
        * (1.0 - saturate(c.soil_depth_m / 2.0));

    let moss =
        smoothstep(0.55, 0.95, c.moisture)
        * (1.0 - exposed_rock);

    let litter = match c.biome {
        BiomeId::Rainforest => 0.40,
        BiomeId::CloudForest => 0.35,
        BiomeId::DryForest => 0.15,
        _ => 0.05,
    };

    SurfaceMaterialBlend {
        materials: [
            TerrainMaterialId::JungleLoam,
            TerrainMaterialId::LeafLitter,
            TerrainMaterialId::JungleMoss,
            TerrainMaterialId::WeatheredBasalt,
        ],
        weights: [
            1.0 - litter - moss - exposed_rock,
            litter,
            moss,
            exposed_rock,
        ],
    }
    .normalize()
}
```

---

# 18. Convert material identities into array layers

```rust
pub fn resolve_blend(
    blend: SurfaceMaterialBlend,
    layers: &MaterialLayerRegistry,
) -> TerrainMaterialVertex {
    TerrainMaterialVertex {
        indices: blend.materials.map(|id| layers.layer(id)),
        weights: blend.weights,
    }
}
```

Never let the classifier directly emit texture-array layer numbers.

---

# 19. Terrain mesh construction

```rust
pub struct TerrainMeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,

    pub material_indices: Vec<[u32; 4]>,
    pub material_weights: Vec<[f32; 4]>,
}
```

During meshing:

```rust
pub fn emit_surface_vertex(
    position: Vec3,
    normal: Vec3,
    fields: &TerrainFieldSampler,
    classifier: &dyn SurfaceClassifier,
    layer_registry: &MaterialLayerRegistry,
    mesh: &mut TerrainMeshData,
) {
    let context =
        fields.sample_surface_context(position, normal);

    let blend = classifier.classify(&context);
    let resolved = resolve_blend(blend, layer_registry);

    mesh.positions.push(position.to_array());
    mesh.normals.push(normal.to_array());

    mesh.material_indices.push(resolved.indices);
    mesh.material_weights.push(resolved.weights);
}
```

Convert to Bevy:

```rust
use bevy::{
    asset::RenderAssetUsages,
    mesh::Indices,
    prelude::*,
    render::render_resource::PrimitiveTopology,
};

pub fn into_bevy_mesh(
    data: TerrainMeshData,
) -> Mesh {
    assert_eq!(
        data.positions.len(),
        data.normals.len()
    );

    assert_eq!(
        data.positions.len(),
        data.material_indices.len()
    );

    assert_eq!(
        data.positions.len(),
        data.material_weights.len()
    );

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD
            | RenderAssetUsages::MAIN_WORLD,
    );

    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        data.positions,
    );

    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        data.normals,
    );

    mesh.insert_attribute(
        ATTRIBUTE_TERRAIN_MATERIAL_INDICES,
        data.material_indices,
    );

    mesh.insert_attribute(
        ATTRIBUTE_TERRAIN_MATERIAL_WEIGHTS,
        data.material_weights,
    );

    mesh.insert_indices(Indices::U32(data.indices));

    mesh
}
```

---

# 20. Vertex-shader specialization

The default Bevy mesh shader does not know about your custom attributes. Provide a custom vertex shader or specialize the material pipeline to expose them.

Recommended route:

```text
custom terrain vertex shader
    reads:
        position
        normal
        material indices
        material weights

    outputs:
        world position
        world normal
        flat/no-perspective material indices
        material weights
```

Material indices should not be interpolated numerically. In WGSL, use `@interpolate(flat)` for integer indices.

Conceptual vertex output:

```wgsl
struct TerrainVertexOutput {
    @builtin(position)
    clip_position: vec4<f32>,

    @location(0)
    world_position: vec3<f32>,

    @location(1)
    world_normal: vec3<f32>,

    @location(2)
    @interpolate(flat)
    material_indices: vec4<u32>,

    @location(3)
    material_weights: vec4<f32>,

    @builtin(instance_index)
    instance_index: u32,
}
```

Because Bevy’s material pipeline APIs are version-sensitive, isolate `specialize()` and vertex layout code in one module:

```text
terrain_material_bevy/src/pipeline.rs
```

---

# 21. WGSL material bindings

```wgsl
#import bevy_pbr::{
    mesh_view_bindings::view,
    pbr_types::{
        STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT,
        PbrInput,
        pbr_input_new,
    },
    pbr_functions as pbr,
}

#import bevy_core_pipeline::tonemapping::tone_mapping

struct TerrainSettings {
    triplanar_sharpness: f32,
    global_texture_scale: f32,
    normal_strength: f32,
    height_blend_strength: f32,

    layer_count: u32,
    debug_mode: u32,
    _padding: vec2<f32>,
}

@group(#{MATERIAL_BIND_GROUP})
@binding(0)
var terrain_albedo: texture_2d_array<f32>;

@group(#{MATERIAL_BIND_GROUP})
@binding(1)
var terrain_albedo_sampler: sampler;

@group(#{MATERIAL_BIND_GROUP})
@binding(2)
var terrain_normal: texture_2d_array<f32>;

@group(#{MATERIAL_BIND_GROUP})
@binding(3)
var terrain_normal_sampler: sampler;

@group(#{MATERIAL_BIND_GROUP})
@binding(4)
var terrain_ormh: texture_2d_array<f32>;

@group(#{MATERIAL_BIND_GROUP})
@binding(5)
var terrain_ormh_sampler: sampler;

@group(#{MATERIAL_BIND_GROUP})
@binding(6)
var<uniform> settings: TerrainSettings;
```

Bevy 0.19’s official array-texture shader uses the same material bind-group syntax and PBR imports.

---

# 22. Triplanar weights

```wgsl
fn triplanar_weights(
    world_normal: vec3<f32>
) -> vec3<f32> {
    let powered = pow(
        abs(world_normal),
        vec3<f32>(settings.triplanar_sharpness)
    );

    let total =
        max(powered.x + powered.y + powered.z, 0.0001);

    return powered / total;
}
```

Coordinates:

```wgsl
fn projection_uvs(
    world_position: vec3<f32>,
    scale: f32
) -> mat3x2<f32> {
    let p = world_position * scale;

    return mat3x2<f32>(
        p.zy,  // X projection
        p.xz,  // Y projection
        p.xy,  // Z projection
    );
}
```

---

# 23. Sampling one material layer

```wgsl
struct TerrainSample {
    albedo: vec4<f32>,
    normal: vec3<f32>,
    ao: f32,
    roughness: f32,
    metallic: f32,
    height: f32,
}
```

```wgsl
fn unpack_normal(
    encoded: vec3<f32>
) -> vec3<f32> {
    return normalize(encoded * 2.0 - 1.0);
}
```

```wgsl
fn sample_layer_albedo(
    uv: vec2<f32>,
    layer: u32
) -> vec4<f32> {
    return textureSample(
        terrain_albedo,
        terrain_albedo_sampler,
        uv,
        layer
    );
}
```

```wgsl
fn sample_layer_ormh(
    uv: vec2<f32>,
    layer: u32
) -> vec4<f32> {
    return textureSample(
        terrain_ormh,
        terrain_ormh_sampler,
        uv,
        layer
    );
}
```

---

# 24. Correct triplanar normal orientation

Each projection’s tangent-space normal must be transformed into world space before blending.

Conceptual bases:

```text
X projection samples YZ:
    tangent   = +Z
    bitangent = +Y
    normal    = ±X

Y projection samples XZ:
    tangent   = +X
    bitangent = +Z
    normal    = ±Y

Z projection samples XY:
    tangent   = +X
    bitangent = +Y
    normal    = ±Z
```

WGSL:

```wgsl
fn normal_x_projection(
    n: vec3<f32>,
    geometric_normal: vec3<f32>
) -> vec3<f32> {
    let axis_sign = select(
        -1.0,
        1.0,
        geometric_normal.x >= 0.0
    );

    return normalize(vec3<f32>(
        n.z * axis_sign,
        n.y,
        n.x * axis_sign
    ));
}

fn normal_y_projection(
    n: vec3<f32>,
    geometric_normal: vec3<f32>
) -> vec3<f32> {
    let axis_sign = select(
        -1.0,
        1.0,
        geometric_normal.y >= 0.0
    );

    return normalize(vec3<f32>(
        n.x,
        n.z * axis_sign,
        n.y * axis_sign
    ));
}

fn normal_z_projection(
    n: vec3<f32>,
    geometric_normal: vec3<f32>
) -> vec3<f32> {
    let axis_sign = select(
        -1.0,
        1.0,
        geometric_normal.z >= 0.0
    );

    return normalize(vec3<f32>(
        n.x * axis_sign,
        n.y,
        n.z * axis_sign
    ));
}
```

Test these bases carefully. Incorrect signs cause visible lighting inversion on opposite-facing cliffs.

---

# 25. Full triplanar layer sample

```wgsl
fn sample_triplanar_layer(
    world_position: vec3<f32>,
    world_normal: vec3<f32>,
    layer: u32,
    texture_scale: f32
) -> TerrainSample {
    let blend = triplanar_weights(world_normal);
    let uv = projection_uvs(
        world_position,
        texture_scale
    );

    let albedo_x =
        sample_layer_albedo(uv[0], layer);
    let albedo_y =
        sample_layer_albedo(uv[1], layer);
    let albedo_z =
        sample_layer_albedo(uv[2], layer);

    let albedo =
        albedo_x * blend.x
        + albedo_y * blend.y
        + albedo_z * blend.z;

    let normal_x_raw = unpack_normal(
        textureSample(
            terrain_normal,
            terrain_normal_sampler,
            uv[0],
            layer
        ).xyz
    );

    let normal_y_raw = unpack_normal(
        textureSample(
            terrain_normal,
            terrain_normal_sampler,
            uv[1],
            layer
        ).xyz
    );

    let normal_z_raw = unpack_normal(
        textureSample(
            terrain_normal,
            terrain_normal_sampler,
            uv[2],
            layer
        ).xyz
    );

    let normal_x =
        normal_x_projection(
            normal_x_raw,
            world_normal
        );

    let normal_y =
        normal_y_projection(
            normal_y_raw,
            world_normal
        );

    let normal_z =
        normal_z_projection(
            normal_z_raw,
            world_normal
        );

    let mapped_normal = normalize(
        normal_x * blend.x
        + normal_y * blend.y
        + normal_z * blend.z
    );

    let ormh_x =
        sample_layer_ormh(uv[0], layer);
    let ormh_y =
        sample_layer_ormh(uv[1], layer);
    let ormh_z =
        sample_layer_ormh(uv[2], layer);

    let ormh =
        ormh_x * blend.x
        + ormh_y * blend.y
        + ormh_z * blend.z;

    return TerrainSample(
        albedo,
        mapped_normal,
        ormh.r,
        ormh.g,
        ormh.b,
        ormh.a
    );
}
```

This assumes:

```text
ORMH:
R = ambient occlusion
G = roughness
B = metallic
A = height
```

---

# 26. Blend four materials

```wgsl
fn normalize_material_weights(
    weights: vec4<f32>
) -> vec4<f32> {
    let positive = max(weights, vec4<f32>(0.0));

    let total = max(
        positive.x
        + positive.y
        + positive.z
        + positive.w,
        0.0001
    );

    return positive / total;
}
```

Optional height-aware adjustment:

```wgsl
fn height_adjust_weights(
    weights: vec4<f32>,
    heights: vec4<f32>
) -> vec4<f32> {
    let strength =
        settings.height_blend_strength;

    if strength <= 0.0 {
        return normalize_material_weights(weights);
    }

    let adjusted =
        weights
        * max(
            heights * strength
                + vec4<f32>(1.0 - strength),
            vec4<f32>(0.0001)
        );

    return normalize_material_weights(adjusted);
}
```

Sample four layers, then blend:

```wgsl
let sample_0 = sample_triplanar_layer(
    mesh.world_position,
    geometric_normal,
    mesh.material_indices.x,
    settings.global_texture_scale
);

let sample_1 = sample_triplanar_layer(
    mesh.world_position,
    geometric_normal,
    mesh.material_indices.y,
    settings.global_texture_scale
);

let sample_2 = sample_triplanar_layer(
    mesh.world_position,
    geometric_normal,
    mesh.material_indices.z,
    settings.global_texture_scale
);

let sample_3 = sample_triplanar_layer(
    mesh.world_position,
    geometric_normal,
    mesh.material_indices.w,
    settings.global_texture_scale
);
```

```wgsl
let heights = vec4<f32>(
    sample_0.height,
    sample_1.height,
    sample_2.height,
    sample_3.height
);

let weights = height_adjust_weights(
    mesh.material_weights,
    heights
);
```

```wgsl
let base_color =
    sample_0.albedo * weights.x
    + sample_1.albedo * weights.y
    + sample_2.albedo * weights.z
    + sample_3.albedo * weights.w;

let mapped_normal = normalize(
    sample_0.normal * weights.x
    + sample_1.normal * weights.y
    + sample_2.normal * weights.z
    + sample_3.normal * weights.w
);

let roughness =
    sample_0.roughness * weights.x
    + sample_1.roughness * weights.y
    + sample_2.roughness * weights.z
    + sample_3.roughness * weights.w;

let metallic =
    sample_0.metallic * weights.x
    + sample_1.metallic * weights.y
    + sample_2.metallic * weights.z
    + sample_3.metallic * weights.w;

let ao =
    sample_0.ao * weights.x
    + sample_1.ao * weights.y
    + sample_2.ao * weights.z
    + sample_3.ao * weights.w;
```

---

# 27. Feed Bevy’s PBR lighting

The official Bevy 0.19 array example creates a `PbrInput`, fills its material properties and geometry data, and calls Bevy’s PBR lighting function.

Conceptual fragment ending:

```wgsl
var pbr_input: PbrInput = pbr_input_new();

pbr_input.material.base_color = base_color;
pbr_input.material.perceptual_roughness =
    clamp(roughness, 0.04, 1.0);
pbr_input.material.metallic =
    clamp(metallic, 0.0, 1.0);

pbr_input.frag_coord = mesh.position;
pbr_input.world_position = mesh.world_position;
pbr_input.world_normal = geometric_normal;
pbr_input.N = normalize(
    mix(
        geometric_normal,
        mapped_normal,
        settings.normal_strength
    )
);

pbr_input.V = pbr::calculate_view(
    mesh.world_position,
    pbr_input.is_orthographic
);

let lit = pbr::apply_pbr_lighting(pbr_input);

return tone_mapping(
    vec4<f32>(lit.rgb * ao, lit.a),
    view.color_grading
);
```

The exact field names in `PbrInput` should be checked against Bevy 0.19’s current WGSL modules while implementing. Copy the structure of the official 0.19 array-texture example rather than older third-party shaders.

---

# 28. Shared material asset

Create one `TerrainMaterial` and reuse it across all chunks:

```rust
#[derive(Resource, Clone)]
pub struct SharedTerrainMaterial {
    pub handle: Handle<TerrainMaterial>,
}
```

```rust
fn build_shared_material(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    let recipes = load_material_recipes();
    let cpu_arrays =
        build_cpu_arrays(&recipes)
            .expect("terrain textures should generate");

    let handles =
        upload_texture_arrays(cpu_arrays, &mut images);

    let material = materials.add(TerrainMaterial {
        albedo_array: handles.albedo,
        normal_array: handles.normal,
        ormh_array: handles.ormh,

        settings: TerrainMaterialSettings {
            triplanar_sharpness: 4.0,
            global_texture_scale: 0.4,
            normal_strength: 1.0,
            height_blend_strength: 0.0,
            layer_count: recipes.len() as u32,
            debug_mode: 0,
            _padding: Vec2::ZERO,
        },
    });

    commands.insert_resource(
        SharedTerrainMaterial {
            handle: material,
        },
    );
}
```

Chunk spawn:

```rust
commands.spawn((
    TerrainChunk { coord },
    Mesh3d(meshes.add(mesh)),
    MeshMaterial3d(
        shared_material.handle.clone()
    ),
    Transform::from_translation(chunk_origin),
));
```

---

# 29. Asynchronous texture generation

Do not block startup once the proof works.

Use Bevy’s async compute task pool or your own bounded Rayon pool:

```rust
#[derive(Resource, Default)]
pub struct TerrainTextureBuildState {
    pub task: Option<
        bevy::tasks::Task<
            Result<CpuTextureArrays, TextureGenerationError>
        >
    >,
}
```

Start:

```rust
fn begin_texture_generation(
    mut state: ResMut<TerrainTextureBuildState>,
    recipes: Res<TerrainMaterialRecipes>,
) {
    if state.task.is_some() {
        return;
    }

    let recipes = recipes.0.clone();

    state.task = Some(
        bevy::tasks::AsyncComputeTaskPool::get()
            .spawn(async move {
                build_cpu_arrays(&recipes)
            }),
    );
}
```

Poll:

```rust
fn finish_texture_generation(
    mut commands: Commands,
    mut state: ResMut<TerrainTextureBuildState>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<TerrainMaterial>>,
) {
    let Some(task) = &mut state.task else {
        return;
    };

    let Some(result) =
        bevy::tasks::block_on(
            bevy::tasks::poll_once(task)
        )
    else {
        return;
    };

    state.task = None;

    let arrays = match result {
        Ok(arrays) => arrays,
        Err(error) => {
            error!("terrain texture generation failed: {error}");
            return;
        }
    };

    let handles =
        upload_texture_arrays(arrays, &mut images);

    let material =
        materials.add(create_material(handles));

    commands.insert_resource(
        SharedTerrainMaterial {
            handle: material,
        },
    );
}
```

Use a loading state so terrain chunks do not appear before the material is ready.

---

# 30. Offline baking

Runtime generation is useful for:

* development;
* editor previews;
* mods;
* procedural worlds with custom visual recipes.

For release builds, bake arrays beforehand:

```text
terrain material YAML
    ↓
terrain_tools executable
    ↓
Symbios-derived generators
    ↓
array image data
    ↓
mip generation
    ↓
KTX2/Basis compression
    ↓
manifest
    ↓
packaged game assets
```

Manifest:

```rust
#[derive(serde::Serialize, serde::Deserialize)]
pub struct TerrainMaterialManifest {
    pub schema_version: u32,
    pub generator_version: u32,
    pub recipe_hash: [u8; 32],

    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub mip_levels: u32,

    pub layer_order: Vec<TerrainMaterialId>,

    pub albedo_asset: String,
    pub normal_asset: String,
    pub ormh_asset: String,
}
```

At startup:

```text
manifest recipe hash matches:
    load baked arrays

manifest missing or stale in development:
    regenerate

release build:
    fail loudly if baked assets are absent
```

Bevy 0.19’s image loader supports array layouts for stacked source images, and the official example uses `ImageLoaderSettings::array_layout`.

---

# 31. Caching

Cache key:

```text
generator source version
+ recipe schema version
+ complete serialized recipe
+ texture resolution
+ mip algorithm version
+ array layer order
+ output format
```

```rust
pub fn recipe_hash(
    serialized_recipe: &[u8],
    generator_version: u32,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();

    hasher.update(&generator_version.to_le_bytes());
    hasher.update(serialized_recipe);

    *hasher.finalize().as_bytes()
}
```

Store:

```text
cache/terrain_materials/<hash>/
  manifest.bin
  albedo.array.bin
  normal.array.bin
  ormh.array.bin
```

---

# 32. Debug rendering modes

Add a shader uniform:

```rust
pub enum TerrainDebugMode {
    None = 0,
    DominantMaterial = 1,
    BlendWeights = 2,
    WorldNormal = 3,
    TriplanarWeights = 4,
    Roughness = 5,
    Metallic = 6,
    AmbientOcclusion = 7,
    Height = 8,
}
```

Useful visualizations:

```wgsl
if settings.debug_mode == 1u {
    return debug_material_color(
        mesh.material_indices.x
    );
}

if settings.debug_mode == 2u {
    return vec4<f32>(
        mesh.material_weights.xyz,
        1.0
    );
}

if settings.debug_mode == 4u {
    let tri =
        triplanar_weights(geometric_normal);

    return vec4<f32>(tri, 1.0);
}
```

Also provide CPU debug maps for:

* biome;
* geology;
* slope;
* moisture;
* coast distance;
* river distance;
* cave exposure;
* selected dominant material.

---

# 33. Testing

## Generator tests

```rust
#[test]
fn rock_generation_is_deterministic() {
    let recipe = TextureRecipe::Rock(
        RockConfig {
            seed: 42,
            ..Default::default()
        }
    );

    let first =
        recipe.generate(128, 128).unwrap();
    let second =
        recipe.generate(128, 128).unwrap();

    assert_eq!(
        first.albedo_rgba8,
        second.albedo_rgba8
    );

    assert_eq!(
        first.normal_rgba8,
        second.normal_rgba8
    );

    assert_eq!(
        first.ormh_rgba8,
        second.ormh_rgba8
    );
}
```

## Seam tests

For every map:

```rust
#[test]
fn generated_texture_edges_match() {
    let map = generate_test_texture();

    for y in 0..map.height {
        assert_pixel_near(
            pixel(&map.albedo_rgba8, 0, y),
            pixel(
                &map.albedo_rgba8,
                map.width - 1,
                y,
            ),
        );
    }
}
```

Use a tolerance because sampled edges may represent adjacent points rather than duplicated texels, depending on generator convention.

## Classifier tests

```rust
#[test]
fn steep_basalt_surface_prefers_rock() {
    let classifier = TropicalIslandClassifier;

    let result = classifier.classify(
        &SurfaceContext {
            slope_degrees: 72.0,
            geology: GeologyId::Basalt,
            moisture: 0.2,
            soil_depth_m: 0.1,
            cave_exposure: 0.0,
            water_depth_m: 0.0,
            coast_distance_m: 100.0,
            river_distance_m: 100.0,
            elevation_m: 80.0,
            sea_level_m: 0.0,
            wave_exposure: 0.0,
            mineral_deposition: 0.0,
            biome: BiomeId::Rainforest,
            world_position: Vec3::ZERO,
            world_normal: Vec3::Y,
        },
    );

    assert!(
        result.materials.contains(
            &TerrainMaterialId::FreshBasalt
        )
        || result.materials.contains(
            &TerrainMaterialId::WeatheredBasalt
        )
    );
}
```

Validate every result:

```rust
pub fn validate_blend(
    blend: SurfaceMaterialBlend,
) {
    assert!(
        blend.weights.iter().all(
            |value| value.is_finite()
                && *value >= 0.0
        )
    );

    let sum: f32 = blend.weights.iter().sum();

    assert!((sum - 1.0).abs() < 0.001);
}
```

## GPU visual fixtures

Create a test scene with:

* horizontal plane;
* 45-degree ramp;
* vertical wall;
* inverted ceiling;
* sphere;
* cave arch;
* chunk seam;
* four-material transition.

This is more valuable than testing only the real terrain.

---

# 34. Performance budgets

## Generation

Generate textures:

* once during development startup;
* once during content baking;
* only when recipes change;
* never per terrain chunk.

## Rendering

All terrain chunks should share:

* one material asset;
* the same texture arrays;
* the same pipeline;
* compatible vertex layouts.

This allows Bevy to batch and organize terrain rendering more effectively. Bevy 0.19 includes substantial large-scene rendering improvements and reduced render overhead, which is one reason to keep the material native to 0.19.

## Memory

A 1024×1024 RGBA image is approximately 4 MiB before mipmaps.

For 16 layers and three maps:

```text
16 × 3 × 4 MiB
= 192 MiB base levels

with full mip chains:
approximately 256 MiB
```

Start with:

```text
8 layers at 512×512
```

Then profile:

```text
16 layers at 1024×1024
```

Compress baked release assets.

---

# 35. First material set

Begin with eight layers:

```text
0 Fresh basalt
1 Weathered basalt
2 Tropical red soil
3 Jungle loam
4 Jungle moss
5 Coral sand
6 River gravel
7 River silt
```

Add only after the base pipeline works:

```text
8 Cave basalt
9 Leaf litter
10 Black sand
11 Mud
12 Coral rubble
13 Limestone
14 Flowstone
15 Volcanic ash
```

---

# 36. Implementation phases

## Phase 1: extract procedural generation

Implement:

* Bevy-independent map structures;
* rock, ground, and sand generators;
* deterministic tests;
* seamless-edge tests;
* height-channel preservation.

Deliverable:

```text
A command-line test can generate raw PBR maps
without linking Bevy.
```

## Phase 2: basic Bevy 0.19 texture arrays

Implement:

* CPU array packing;
* array `Image` creation;
* albedo-only custom material;
* one layer selected globally.

Deliverable:

```text
A cube displays a procedurally generated
array-texture layer.
```

## Phase 3: custom terrain attributes

Implement:

* four indices;
* four weights;
* custom vertex shader;
* fragment selection;
* debug material colors.

Deliverable:

```text
One mesh displays four regions selected
by vertex material indices.
```

## Phase 4: triplanar albedo

Implement:

* world-space projections;
* blend weights;
* repeat scale;
* cliffs and ceilings.

Deliverable:

```text
Albedo does not stretch on vertical or inverted surfaces.
```

## Phase 5: PBR maps

Implement:

* ORMH array;
* roughness;
* metallic;
* ambient occlusion;
* Bevy PBR lighting integration.

Deliverable:

```text
Rock, soil, and sand respond differently to light.
```

## Phase 6: triplanar normal mapping

Implement:

* tangent-space normal unpacking;
* axis-space transforms;
* normal blending;
* front/back orientation tests.

Deliverable:

```text
Normals work correctly on all six axis directions.
```

## Phase 7: terrain classifier

Implement:

* `SurfaceContext`;
* slope;
* coast;
* river;
* cave;
* underwater;
* biome rules;
* deterministic top-four selection.

Deliverable:

```text
Generated terrain receives sensible biome materials.
```

## Phase 8: data-driven palettes

Implement:

* YAML material recipes;
* YAML biome palettes;
* validation;
* hot reload;
* stable layer manifest.

Deliverable:

```text
Terrain appearance can be altered without recompilation.
```

## Phase 9: asynchronous generation and cache

Implement:

* loading-state job;
* cache hashes;
* stale-cache rebuilding;
* progress reporting.

Deliverable:

```text
Development startup never blocks the primary frame loop.
```

## Phase 10: release baking

Implement:

* baker executable;
* mip chains;
* compressed arrays;
* packaged manifest;
* development/runtime fallback.

Deliverable:

```text
Release builds load precomputed compressed arrays.
```

## Phase 11: advanced rendering

Add:

* per-layer texture scale;
* height blending;
* dynamic wetness;
* macro color variation;
* stochastic tiling;
* underwater modifiers;
* snow, ash, or moss overlays.

---

# 37. Recommended first milestone

Build this exact vertical slice:

```text
4 materials:
    weathered basalt
    tropical soil
    coral sand
    jungle moss

512×512 maps
base mip only
one shared material
four material indices per vertex
four weights per vertex
triplanar albedo
roughness and metallic
no normal maps initially
no height blending
no stochastic tiling
```

Test it on:

```text
flat beach
steep cliff
cave entrance
cave ceiling
overhang underside
river bank
chunk boundary
```

Then add normals.

Normal mapping is the most error-prone portion, so it should not be introduced until array sampling, material IDs, weights, and albedo projection are already verified.

---

# Final architecture

```text
procedural_textures
    Extracted Symbios algorithms
    No Bevy dependency
    Deterministic tileable PBR generation

terrain_surface
    Stable material IDs
    SurfaceContext
    Custom classifier
    Four-way material blends

terrain_meshing
    Surface extraction
    Custom indices and weight attributes

terrain_material_bevy
    Bevy 0.19 array Images
    Custom TerrainMaterial
    Custom vertex shader
    Triplanar fragment shader
    Bevy PBR integration

terrain_tools
    Offline texture baking
    Mip generation
    Compression
    Cache and manifests
```

The central rule is:

> Procedural generation creates reusable material layers; terrain generation creates geometry and environmental fields; the classifier connects those fields to the material layers; and the Bevy shader handles projection, blending, and lighting.

This gives you the procedural flexibility of Symbios without tying the application to its Bevy 0.18 integration, while retaining a fully native Bevy 0.19 rendering path.
