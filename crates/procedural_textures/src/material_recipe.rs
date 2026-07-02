// crates/procedural_textures/src/material_recipe.rs
use serde::{Deserialize, Serialize};

use crate::recipe::{texture_recipe_from_yaml_value, TextureRecipe};

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
    #[serde(default)]
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

/// Canonical legacy island layer order.
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
    vec![
        TerrainMaterialRecipe {
            id: TerrainMaterialIdName::FreshBasalt.as_str().to_string(),
            resolution: 512,
            meters_per_repeat: 2.5,
            generator: TextureRecipe::Rock(RockConfig {
                seed: 1001,
                color_light: [0.18, 0.17, 0.16],
                color_dark: [0.05, 0.05, 0.055],
                ..RockConfig::default()
            }),
            normal_strength: 1.15,
            tint: [1.0, 1.0, 1.0],
        },
        TerrainMaterialRecipe {
            id: TerrainMaterialIdName::WeatheredBasalt.as_str().to_string(),
            resolution: 512,
            meters_per_repeat: 3.5,
            generator: TextureRecipe::Rock(RockConfig {
                seed: 1002,
                color_light: [0.25, 0.22, 0.18],
                color_dark: [0.07, 0.06, 0.055],
                ..RockConfig::default()
            }),
            normal_strength: 1.15,
            tint: [1.0, 1.0, 1.0],
        },
        TerrainMaterialRecipe {
            id: TerrainMaterialIdName::TropicalRedSoil.as_str().to_string(),
            resolution: 512,
            meters_per_repeat: 2.0,
            generator: TextureRecipe::Ground(GroundConfig {
                seed: 2001,
                color_dry: [0.48, 0.17, 0.07],
                color_moist: [0.32, 0.12, 0.05],
                ..GroundConfig::default()
            }),
            normal_strength: 0.9,
            tint: [1.0, 1.0, 1.0],
        },
        TerrainMaterialRecipe {
            id: TerrainMaterialIdName::JungleLoam.as_str().to_string(),
            resolution: 512,
            meters_per_repeat: 2.0,
            generator: TextureRecipe::Ground(GroundConfig {
                seed: 2002,
                color_dry: [0.22, 0.14, 0.06],
                color_moist: [0.10, 0.08, 0.04],
                ..GroundConfig::default()
            }),
            normal_strength: 0.95,
            tint: [1.0, 1.0, 1.0],
        },
        TerrainMaterialRecipe {
            id: TerrainMaterialIdName::JungleMoss.as_str().to_string(),
            resolution: 512,
            meters_per_repeat: 1.8,
            generator: TextureRecipe::Ground(GroundConfig {
                seed: 2003,
                color_dry: [0.18, 0.28, 0.10],
                color_moist: [0.08, 0.18, 0.06],
                ..GroundConfig::default()
            }),
            normal_strength: 1.0,
            tint: [0.9, 1.05, 0.85],
        },
        TerrainMaterialRecipe {
            id: TerrainMaterialIdName::CoralSand.as_str().to_string(),
            resolution: 512,
            meters_per_repeat: 1.2,
            generator: TextureRecipe::Sand(SandConfig {
                seed: 3001,
                color_light: [0.92, 0.86, 0.72],
                color_dark: [0.78, 0.70, 0.55],
                ..SandConfig::default()
            }),
            normal_strength: 0.8,
            tint: [1.0, 1.0, 1.0],
        },
        TerrainMaterialRecipe {
            id: TerrainMaterialIdName::RiverGravel.as_str().to_string(),
            resolution: 512,
            meters_per_repeat: 1.5,
            generator: TextureRecipe::Cobblestone(CobblestoneConfig {
                seed: 4001,
                ..CobblestoneConfig::default()
            }),
            normal_strength: 1.2,
            tint: [1.0, 1.0, 1.0],
        },
        TerrainMaterialRecipe {
            id: TerrainMaterialIdName::RiverSilt.as_str().to_string(),
            resolution: 512,
            meters_per_repeat: 1.0,
            generator: TextureRecipe::Sand(SandConfig {
                seed: 3002,
                ripple_scale: 4.0,
                grain_scale: 24.0,
                color_light: [0.42, 0.38, 0.28],
                color_dark: [0.28, 0.24, 0.18],
                roughness: 0.75,
                ..SandConfig::default()
            }),
            normal_strength: 0.6,
            tint: [0.95, 0.92, 0.88],
        },
    ]
}

use crate::generators::{
    CobblestoneConfig, GroundConfig, RockConfig, SandConfig,
};

pub fn document_fingerprint(doc: &ProceduralMaterialsDocument) -> [u8; 32] {
    let json = serde_json::to_string(doc).unwrap_or_default();
    *blake3::hash(json.as_bytes()).as_bytes()
}

/// Strip UTF-8 BOM sometimes added by Windows editors.
pub fn strip_utf8_bom(text: &str) -> &str {
    text.strip_prefix('\u{FEFF}').unwrap_or(text)
}
