// crates/game_data/src/material_catalog.rs
//! Terrain material catalog definitions: textures, surfaces, overlays, and catalogs.

use serde::{Deserialize, Serialize};
use shared::{DefinitionHeader, StableId};

/// `textures.*.texture.yaml` — procedural texture recipe (preset generator or graph).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TextureRecipeDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    #[serde(default = "default_texture_resolution")]
    pub resolution: u32,
    #[serde(default)]
    pub seed: Option<u32>,
    #[serde(default = "default_true")]
    pub tileable: bool,
    /// Preset generator block (`Rock`, `Ground`, `Sand`, `Cobblestone`) or future graph.
    #[serde(default)]
    pub generator: Option<serde_yaml::Value>,
    #[serde(default)]
    pub graph: Option<serde_yaml::Value>,
}

fn default_texture_resolution() -> u32 {
    512
}

fn default_true() -> bool {
    true
}

/// `surfaces.*.surface.yaml` — rendering identity separate from classifier rules.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceMaterialDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub texture: StableId,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub rendering: TerrainMaterialRenderingDefinition,
    #[serde(default)]
    pub physical: SurfacePhysicalDefinition,
    #[serde(default)]
    pub responses: TerrainMaterialResponsesDefinition,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct TerrainMaterialRenderingDefinition {
    #[serde(default = "default_projection")]
    pub projection: String,
    #[serde(default)]
    pub meters_per_repeat: Option<f32>,
    #[serde(default)]
    pub triplanar_sharpness: Option<f32>,
    #[serde(default)]
    pub normal_strength: Option<f32>,
    #[serde(default)]
    pub height_blend_strength: Option<f32>,
    #[serde(default)]
    pub macro_variation: Option<MacroVariationDefinition>,
}

fn default_projection() -> String {
    "triplanar".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MacroVariationDefinition {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_macro_scale")]
    pub scale_m: f32,
    #[serde(default = "default_macro_color")]
    pub color_strength: f32,
    #[serde(default = "default_macro_roughness")]
    pub roughness_strength: f32,
}

fn default_macro_scale() -> f32 {
    42.0
}

fn default_macro_color() -> f32 {
    0.10
}

fn default_macro_roughness() -> f32 {
    0.07
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct SurfacePhysicalDefinition {
    #[serde(default)]
    pub hardness: Option<f32>,
    #[serde(default)]
    pub permeability: Option<f32>,
    #[serde(default)]
    pub friction: Option<f32>,
    #[serde(default)]
    pub porosity: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct TerrainMaterialResponsesDefinition {
    #[serde(default)]
    pub wetness: Option<WetnessResponseDefinition>,
    #[serde(default)]
    pub moss: Option<MossResponseDefinition>,
    #[serde(default)]
    pub snow: Option<SnowResponseDefinition>,
    #[serde(default)]
    pub scorch: Option<ScorchResponseDefinition>,
    #[serde(default)]
    pub mud: Option<MudResponseDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WetnessResponseDefinition {
    #[serde(default = "default_wetness_darkening")]
    pub darkening: f32,
    #[serde(default = "default_wetness_roughness")]
    pub roughness_reduction: f32,
    #[serde(default)]
    pub normal_flattening: f32,
}

fn default_wetness_darkening() -> f32 {
    0.28
}

fn default_wetness_roughness() -> f32 {
    0.32
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MossResponseDefinition {
    #[serde(default = "default_moss_affinity")]
    pub affinity: f32,
}

fn default_moss_affinity() -> f32 {
    0.44
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SnowResponseDefinition {
    #[serde(default)]
    pub retention: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScorchResponseDefinition {
    #[serde(default = "default_scorch_visibility")]
    pub visibility: f32,
}

fn default_scorch_visibility() -> f32 {
    0.68
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MudResponseDefinition {
    #[serde(default)]
    pub affinity: f32,
}

/// `overlays.*.overlay.yaml` — global overlay response curves.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OverlayDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    #[serde(default)]
    pub wetness: Option<WetnessResponseDefinition>,
    #[serde(default)]
    pub moss: Option<MossResponseDefinition>,
    #[serde(default)]
    pub snow: Option<SnowResponseDefinition>,
    #[serde(default)]
    pub scorch: Option<ScorchResponseDefinition>,
    #[serde(default)]
    pub mud: Option<MudResponseDefinition>,
}

/// `catalogs.*.material_catalog.yaml` — bundles texture, surface, and overlay refs.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MaterialCatalogDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub textures: Vec<StableId>,
    #[serde(default)]
    pub surfaces: Vec<StableId>,
    #[serde(default)]
    pub overlays: Vec<StableId>,
    #[serde(default)]
    pub palettes: Vec<StableId>,
    #[serde(default)]
    pub classifiers: Vec<StableId>,
    /// Material-region size in horizontal chunks (default 4).
    #[serde(default = "default_region_chunks")]
    pub region_chunks: u32,
}

fn default_region_chunks() -> u32 {
    4
}

/// Per-entry rendering overrides on `materials.*` schema v3 entries.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct MaterialEntryRenderingDefinition {
    #[serde(default)]
    pub meters_per_repeat: Option<f32>,
    #[serde(default)]
    pub normal_strength: Option<f32>,
    #[serde(default)]
    pub triplanar_sharpness: Option<f32>,
    #[serde(default)]
    pub height_blend_strength: Option<f32>,
    #[serde(default)]
    pub macro_variation: Option<MacroVariationDefinition>,
}
