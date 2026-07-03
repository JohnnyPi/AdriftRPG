// crates/game_data/src/surface_registry.rs
//! Compiled surface registry and material dependency index.

use std::collections::BTreeMap;

use serde::Serialize;
use shared::{DataError, DataResult, StableId};

use crate::definitions::{RawDefinition, TerrainMaterialEntryDefinition, TerrainMaterialsDefinition};
use crate::material_catalog::{
    MaterialCatalogDefinition, OverlayDefinition, SurfaceMaterialDefinition,
    TextureRecipeDefinition,
};

pub type SurfaceIndex = u32;
pub type TextureSetIndex = u32;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledTextureRecipe {
    pub id: StableId,
    pub resolution: u32,
    pub seed: Option<u32>,
    pub tileable: bool,
    pub generator: Option<serde_yaml::Value>,
    pub graph: Option<serde_yaml::Value>,
    pub content_hash: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledSurface {
    pub id: StableId,
    pub texture: StableId,
    pub texture_layer: TextureSetIndex,
    pub rendering: crate::material_catalog::TerrainMaterialRenderingDefinition,
    pub responses: crate::material_catalog::TerrainMaterialResponsesDefinition,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledSurfaceRegistry {
    pub catalog_id: Option<StableId>,
    pub content_hash: String,
    pub textures: Vec<CompiledTextureRecipe>,
    pub texture_by_id: BTreeMap<StableId, TextureSetIndex>,
    pub surfaces: Vec<CompiledSurface>,
    pub surface_by_id: BTreeMap<StableId, SurfaceIndex>,
    pub overlays: BTreeMap<StableId, OverlayDefinition>,
    pub region_chunks: u32,
    /// Maps palette material key → resolved surface id (if any).
    pub material_key_to_surface: BTreeMap<StableId, StableId>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct MaterialDependencyIndex {
    pub texture_to_surfaces: BTreeMap<StableId, Vec<StableId>>,
    pub surface_to_material_keys: BTreeMap<StableId, Vec<StableId>>,
    pub catalog_textures: Vec<StableId>,
    pub catalog_surfaces: Vec<StableId>,
}

impl CompiledSurfaceRegistry {
    pub fn texture_for_surface(&self, surface_id: &StableId) -> Option<&CompiledTextureRecipe> {
        let surface = self.surface_by_id.get(surface_id)?;
        let compiled = self.surfaces.get(*surface as usize)?;
        self.textures.get(compiled.texture_layer as usize)
    }

    pub fn surface_for_material_key(&self, key: &StableId) -> Option<&CompiledSurface> {
        let surface_id = self.material_key_to_surface.get(key)?;
        let idx = *self.surface_by_id.get(surface_id)? as usize;
        self.surfaces.get(idx)
    }
}

pub fn build_surface_registry(
    definitions: &[RawDefinition],
    catalog_id: Option<&StableId>,
    palette: Option<&TerrainMaterialsDefinition>,
) -> DataResult<(CompiledSurfaceRegistry, MaterialDependencyIndex)> {
    let mut texture_defs: BTreeMap<StableId, TextureRecipeDefinition> = BTreeMap::new();
    let mut surface_defs: BTreeMap<StableId, SurfaceMaterialDefinition> = BTreeMap::new();
    let mut overlay_defs: BTreeMap<StableId, OverlayDefinition> = BTreeMap::new();
    let mut catalog: Option<MaterialCatalogDefinition> = None;

    for def in definitions {
        match def {
            RawDefinition::TextureRecipe(tex) => {
                texture_defs.insert(tex.header.id.clone(), tex.clone());
            }
            RawDefinition::SurfaceMaterial(surf) => {
                surface_defs.insert(surf.header.id.clone(), surf.clone());
            }
            RawDefinition::Overlay(ov) => {
                overlay_defs.insert(ov.header.id.clone(), ov.clone());
            }
            RawDefinition::MaterialCatalog(cat) => {
                if catalog_id.is_some_and(|id| id == &cat.header.id) {
                    catalog = Some(cat.clone());
                }
            }
            _ => {}
        }
    }

    if let Some(cat_id) = catalog_id {
        if catalog.is_none() {
            for def in definitions {
                if let RawDefinition::MaterialCatalog(cat) = def {
                    if &cat.header.id == cat_id {
                        catalog = Some(cat.clone());
                        break;
                    }
                }
            }
        }
        if catalog.is_none() {
            return Err(DataError::InvalidValue {
                context: format!("catalog `{cat_id}`"),
                message: "material catalog not found in loaded definitions".to_string(),
            });
        }
    }

    let region_chunks = catalog.as_ref().map(|c| c.region_chunks).unwrap_or(4);

    if let Some(ref cat) = catalog {
        for tex_id in &cat.textures {
            if !texture_defs.contains_key(tex_id) {
                return Err(DataError::InvalidValue {
                    context: format!("catalog `{}`", cat.header.id),
                    message: format!("references unknown texture `{tex_id}`"),
                });
            }
        }
        for surf_id in &cat.surfaces {
            if !surface_defs.contains_key(surf_id) {
                return Err(DataError::InvalidValue {
                    context: format!("catalog `{}`", cat.header.id),
                    message: format!("references unknown surface `{surf_id}`"),
                });
            }
        }
        for ov_id in &cat.overlays {
            if !overlay_defs.contains_key(ov_id) {
                return Err(DataError::InvalidValue {
                    context: format!("catalog `{}`", cat.header.id),
                    message: format!("references unknown overlay `{ov_id}`"),
                });
            }
        }
    }

    let mut sorted_texture_ids: Vec<StableId> = texture_defs.keys().cloned().collect();
    sorted_texture_ids.sort();

    let mut textures = Vec::with_capacity(sorted_texture_ids.len());
    let mut texture_by_id = BTreeMap::new();
    for (index, id) in sorted_texture_ids.iter().enumerate() {
        let def = &texture_defs[id];
        if def.generator.is_none() && def.graph.is_none() {
            return Err(DataError::InvalidValue {
                context: format!("texture `{id}`"),
                message: "must declare generator or graph".to_string(),
            });
        }
        texture_by_id.insert(id.clone(), index as TextureSetIndex);
        textures.push(CompiledTextureRecipe {
            id: id.clone(),
            resolution: def.resolution,
            seed: def.seed,
            tileable: def.tileable,
            generator: def.generator.clone(),
            graph: def.graph.clone(),
            content_hash: hash_yaml_value(def),
        });
    }

    let mut sorted_surface_ids: Vec<StableId> = surface_defs.keys().cloned().collect();
    sorted_surface_ids.sort();

    let mut surfaces = Vec::with_capacity(sorted_surface_ids.len());
    let mut surface_by_id = BTreeMap::new();
    for (index, id) in sorted_surface_ids.iter().enumerate() {
        let def = &surface_defs[id];
        let texture_layer = texture_by_id.get(&def.texture).copied().ok_or_else(|| {
            DataError::InvalidValue {
                context: format!("surface `{id}`"),
                message: format!("references unknown texture `{}`", def.texture),
            }
        })?;
        surface_by_id.insert(id.clone(), index as SurfaceIndex);
        surfaces.push(CompiledSurface {
            id: id.clone(),
            texture: def.texture.clone(),
            texture_layer,
            rendering: def.rendering.clone(),
            responses: def.responses.clone(),
            tags: def.tags.clone(),
        });
    }

    let mut material_key_to_surface = BTreeMap::new();
    if let Some(palette) = palette {
        for entry in &palette.materials {
            let key = entry.resolved_key();
            if let Some(ref surface_ref) = entry.surface {
                if !surface_by_id.contains_key(surface_ref) {
                    return Err(DataError::InvalidValue {
                        context: format!("material `{}`", key),
                        message: format!("references unknown surface `{surface_ref}`"),
                    });
                }
                material_key_to_surface.insert(key, surface_ref.clone());
            } else if let Some(ref tex_ref) = entry.texture {
                let surface_id = StableId::new(&format!("surfaces.{}", key.as_str()));
                if surface_by_id.contains_key(&surface_id) {
                    material_key_to_surface.insert(key, surface_id);
                } else if texture_by_id.contains_key(tex_ref) {
                    let auto_id = StableId::new(&format!("surface.auto_{}", key.as_str()));
                    material_key_to_surface.insert(key, auto_id);
                }
            } else {
                let auto_surface = StableId::new(&format!("surfaces.{}", key.as_str()));
                if surface_by_id.contains_key(&auto_surface) {
                    material_key_to_surface.insert(key, auto_surface);
                }
            }
        }
    }

    let mut texture_to_surfaces: BTreeMap<StableId, Vec<StableId>> = BTreeMap::new();
    for surface in &surfaces {
        texture_to_surfaces
            .entry(surface.texture.clone())
            .or_default()
            .push(surface.id.clone());
    }

    let mut surface_to_material_keys: BTreeMap<StableId, Vec<StableId>> = BTreeMap::new();
    for (key, surface_id) in &material_key_to_surface {
        surface_to_material_keys
            .entry(surface_id.clone())
            .or_default()
            .push(key.clone());
    }

    let dep_index = MaterialDependencyIndex {
        texture_to_surfaces,
        surface_to_material_keys,
        catalog_textures: catalog.as_ref().map(|c| c.textures.clone()).unwrap_or_default(),
        catalog_surfaces: catalog.as_ref().map(|c| c.surfaces.clone()).unwrap_or_default(),
    };

    let registry = CompiledSurfaceRegistry {
        catalog_id: catalog_id.cloned(),
        content_hash: hash_registry(&textures, &surfaces),
        textures,
        texture_by_id,
        surfaces,
        surface_by_id,
        overlays: overlay_defs,
        region_chunks,
        material_key_to_surface,
    };

    Ok((registry, dep_index))
}

use sha2::{Digest, Sha256};

fn hash_yaml_value<T: serde::Serialize>(value: &T) -> String {
    let json = serde_json::to_string(value).unwrap_or_default();
    format!("{:x}", Sha256::digest(json.as_bytes()))
}

fn hash_registry(textures: &[CompiledTextureRecipe], surfaces: &[CompiledSurface]) -> String {
    let json = serde_json::to_string(&(textures, surfaces)).unwrap_or_default();
    format!("{:x}", Sha256::digest(json.as_bytes()))
}

/// Resolve a material entry's texture generator YAML from the compiled registry.
pub fn resolve_entry_generator(
    entry: &TerrainMaterialEntryDefinition,
    registry: Option<&CompiledSurfaceRegistry>,
) -> Option<serde_yaml::Value> {
    if let Some(generator) = &entry.generator {
        return Some(generator.clone());
    }
    if let Some(reg) = registry {
        if let Some(ref tex_id) = entry.texture {
            if let Some(&layer) = reg.texture_by_id.get(tex_id) {
                if let Some(tex) = reg.textures.get(layer as usize) {
                    if let Some(ref generator) = tex.generator {
                        return Some(generator.clone());
                    }
                    if let Some(ref graph) = tex.graph {
                        return Some(graph.clone());
                    }
                }
            }
        }
        if let Some(ref surface_id) = entry.surface {
            if let Some(surface) = reg.surface_by_id.get(surface_id) {
                if let Some(compiled) = reg.surfaces.get(*surface as usize) {
                    if let Some(tex) = reg.textures.get(compiled.texture_layer as usize) {
                        if let Some(ref generator) = tex.generator {
                            return Some(generator.clone());
                        }
                        if let Some(ref graph) = tex.graph {
                            return Some(graph.clone());
                        }
                    }
                }
            }
        }
        if let Some(surface_id) = reg.material_key_to_surface.get(&entry.resolved_key()) {
            if let Some(surface_idx) = reg.surface_by_id.get(surface_id) {
                if let Some(compiled) = reg.surfaces.get(*surface_idx as usize) {
                    if let Some(tex) = reg.textures.get(compiled.texture_layer as usize) {
                        if let Some(ref generator) = tex.generator {
                            return Some(generator.clone());
                        }
                        if let Some(ref graph) = tex.graph {
                            return Some(graph.clone());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Keys that should be replaced by dynamic overlays instead of separate baked textures.
pub const DEPRECATED_OVERLAY_MATERIAL_KEYS: &[&str] = &["wet_rock"];

pub fn is_deprecated_overlay_material(key: &str) -> bool {
    DEPRECATED_OVERLAY_MATERIAL_KEYS.contains(&key)
}

pub fn deprecated_overlay_warnings(palette: &TerrainMaterialsDefinition) -> Vec<String> {
    let mut warnings = Vec::new();
    for entry in &palette.materials {
        let key = entry.resolved_key();
        if is_deprecated_overlay_material(key.as_str()) {
            warnings.push(format!(
                "material `{}` is deprecated; use base surface + wetness overlay",
                key
            ));
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::DefinitionHeader;

    fn header(id: &str) -> DefinitionHeader {
        DefinitionHeader {
            schema_version: 1,
            id: StableId::new(id),
        }
    }

    #[test]
    fn builds_registry_from_texture_and_surface() {
        let definitions = vec![
            RawDefinition::TextureRecipe(TextureRecipeDefinition {
                header: header("textures.rock"),
                resolution: 256,
                seed: Some(42),
                tileable: true,
                generator: Some(serde_yaml::from_str("Rock: { seed: 42 }").unwrap()),
                graph: None,
            }),
            RawDefinition::SurfaceMaterial(SurfaceMaterialDefinition {
                header: header("surfaces.rock"),
                texture: StableId::new("textures.rock"),
                tags: vec!["rock".into()],
                rendering: Default::default(),
                physical: Default::default(),
                responses: Default::default(),
            }),
        ];
        let (registry, _) =
            build_surface_registry(&definitions, None, None).expect("build");
        assert_eq!(registry.textures.len(), 1);
        assert_eq!(registry.surfaces.len(), 1);
        assert_eq!(registry.surfaces[0].texture_layer, 0);
    }
}
