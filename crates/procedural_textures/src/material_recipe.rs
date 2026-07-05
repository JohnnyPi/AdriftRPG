// crates/procedural_textures/src/material_recipe.rs
use serde::{Deserialize, Serialize};

use crate::recipe::{TextureRecipe, texture_recipe_from_yaml_value};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "PascalCase")]
pub enum TerrainMaterialIdName {
    FreshBasalt,
    WeatheredBasalt,
    CaveBasalt,
    TropicalRedSoil,
    JungleLoam,
    JungleMoss,
    LeafLitter,
    CoralSand,
    BlackSand,
    CoralRubble,
    RiverGravel,
    RiverSilt,
    Mud,
    Limestone,
    Flowstone,
    VolcanicAsh,
}

impl TerrainMaterialIdName {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FreshBasalt => "FreshBasalt",
            Self::WeatheredBasalt => "WeatheredBasalt",
            Self::CaveBasalt => "CaveBasalt",
            Self::TropicalRedSoil => "TropicalRedSoil",
            Self::JungleLoam => "JungleLoam",
            Self::JungleMoss => "JungleMoss",
            Self::LeafLitter => "LeafLitter",
            Self::CoralSand => "CoralSand",
            Self::BlackSand => "BlackSand",
            Self::CoralRubble => "CoralRubble",
            Self::RiverGravel => "RiverGravel",
            Self::RiverSilt => "RiverSilt",
            Self::Mud => "Mud",
            Self::Limestone => "Limestone",
            Self::Flowstone => "Flowstone",
            Self::VolcanicAsh => "VolcanicAsh",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerrainMaterialRecipe {
    #[serde(deserialize_with = "deserialize_material_id")]
    pub id: String,
    pub resolution: u32,
    pub meters_per_repeat: f32,
    #[serde(deserialize_with = "deserialize_generator")]
    pub generator: TextureRecipe,
    #[serde(default = "default_normal_strength")]
    pub normal_strength: f32,
    #[serde(default = "default_tint")]
    pub tint: [f32; 3],
}

fn deserialize_material_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_yaml::Value::deserialize(deserializer)?;
    match value {
        serde_yaml::Value::String(s) => Ok(s),
        serde_yaml::Value::Number(n) => Ok(n.to_string()),
        other => Err(serde::de::Error::custom(format!(
            "expected string material id, got {other:?}"
        ))),
    }
}

fn deserialize_generator<'de, D>(deserializer: D) -> Result<TextureRecipe, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_yaml::Value::deserialize(deserializer)?;
    texture_recipe_from_yaml_value(&value).map_err(serde::de::Error::custom)
}

fn default_normal_strength() -> f32 {
    1.0
}

fn default_tint() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

/// Layer order as declared in a procedural materials document (YAML list order).
pub fn document_layer_order(doc: &ProceduralMaterialsDocument) -> Vec<String> {
    doc.materials
        .iter()
        .map(|recipe| recipe.id.clone())
        .collect()
}

pub fn order_recipes_for_document(
    doc: &ProceduralMaterialsDocument,
) -> Result<Vec<TerrainMaterialRecipe>, crate::error::TextureGenerationError> {
    order_recipes_for_palette(&document_layer_order(doc), &doc.materials)
}

/// Canonical legacy island layer order (deprecated — prefer `document_layer_order`).
pub const CORE_ISLAND_LAYER_ORDER: &[TerrainMaterialIdName] = &[
    TerrainMaterialIdName::FreshBasalt,
    TerrainMaterialIdName::WeatheredBasalt,
    TerrainMaterialIdName::TropicalRedSoil,
    TerrainMaterialIdName::JungleLoam,
    TerrainMaterialIdName::JungleMoss,
    TerrainMaterialIdName::CoralSand,
    TerrainMaterialIdName::RiverGravel,
    TerrainMaterialIdName::RiverSilt,
];

pub fn order_recipes_for_palette(
    layer_order: &[impl AsRef<str>],
    materials: &[TerrainMaterialRecipe],
) -> Result<Vec<TerrainMaterialRecipe>, crate::error::TextureGenerationError> {
    use crate::error::TextureGenerationError;
    use std::collections::HashMap;

    let by_id: HashMap<_, _> = materials
        .iter()
        .map(|recipe| (recipe.id.clone(), recipe.clone()))
        .collect();
    let mut ordered = Vec::with_capacity(layer_order.len());
    for key in layer_order {
        let key = key.as_ref();
        let recipe = by_id.get(key).ok_or_else(|| {
            TextureGenerationError::InvalidConfig(format!(
                "missing terrain material `{key}` in procedural materials document"
            ))
        })?;
        ordered.push(recipe.clone());
    }
    Ok(ordered)
}

/// Reorder YAML recipes to match the legacy core layer table.
pub fn order_recipes_for_core_layers(
    materials: &[TerrainMaterialRecipe],
) -> Result<Vec<TerrainMaterialRecipe>, crate::error::TextureGenerationError> {
    let keys: Vec<String> = CORE_ISLAND_LAYER_ORDER
        .iter()
        .map(|name| name.as_str().to_string())
        .collect();
    order_recipes_for_palette(&keys, materials)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProceduralMaterialsDocument {
    pub schema_version: u32,
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub materials: Vec<TerrainMaterialRecipe>,
}

pub fn default_island_recipes() -> Vec<TerrainMaterialRecipe> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/procedural/terrain/procedural_island.yaml");
    let text = std::fs::read_to_string(&path).expect("read embedded island yaml");
    let doc: ProceduralMaterialsDocument =
        serde_yaml::from_str(strip_utf8_bom(&text)).expect("parse embedded island yaml");
    order_recipes_for_document(&doc).expect("order embedded island recipes")
}

pub fn document_fingerprint(doc: &ProceduralMaterialsDocument) -> [u8; 32] {
    use crate::texture_graph::GENERATOR_VERSION;
    let json = serde_json::to_string(doc).unwrap_or_default();
    let mut hasher = blake3::Hasher::new();
    hasher.update(&GENERATOR_VERSION.to_le_bytes());
    hasher.update(json.as_bytes());
    *hasher.finalize().as_bytes()
}

/// Strip UTF-8 BOM sometimes added by Windows editors.
pub fn strip_utf8_bom(text: &str) -> &str {
    text.strip_prefix('\u{FEFF}').unwrap_or(text)
}
