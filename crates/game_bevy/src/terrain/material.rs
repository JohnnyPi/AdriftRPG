// crates/game_bevy/src/terrain/material.rs
use bevy::prelude::*;
use game_data::{CompiledTerrainMaterials, TerrainMaterialEntryDefinition};
use procedural_textures::{
    texture_recipe_from_yaml_value, CobblestoneConfig, GroundConfig, RockConfig, SandConfig,
    TerrainMaterialRecipe, TextureRecipe,
};
use terrain_material_bevy::{
    FallbackTerrainMaterialSet, PendingTextureBake, ProceduralMaterialRecipeOverride,
    ProceduralTerrainMaterialPlugin, TerrainProceduralMaterialState,
};

use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;
use crate::state::AppState;
use crate::terrain::{TerrainChunkEntity, TerrainChunkPalette};
use crate::world::requested_world_id;

pub use terrain_material_bevy::TerrainPbrMaterial;

#[derive(Resource, Clone)]
pub struct TerrainMaterialHandle(pub Handle<TerrainPbrMaterial>);

pub struct TerrainMaterialPlugin;

impl Plugin for TerrainMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ProceduralTerrainMaterialPlugin {
            materials_yaml: None,
        })
        .add_systems(
            Startup,
            init_terrain_material_handle.after(FallbackTerrainMaterialSet),
        )
        .add_systems(
            Update,
            (
                sync_world_terrain_material_recipes,
                sync_terrain_material_handle,
                refresh_chunk_terrain_materials,
            )
                .chain()
                .run_if(in_state(AppState::Running)),
        );
    }
}

fn refresh_chunk_terrain_materials(
    handle: Res<TerrainMaterialHandle>,
    state: Res<TerrainProceduralMaterialState>,
    mut materials: ResMut<Assets<TerrainPbrMaterial>>,
    mut last_fingerprint: Local<Option<[u8; 32]>>,
    mut last_chunk_count: Local<usize>,
    chunks: Query<(&MeshMaterial3d<TerrainPbrMaterial>, &TerrainChunkPalette), With<TerrainChunkEntity>>,
) {
    if !state.ready {
        *last_fingerprint = None;
        *last_chunk_count = 0;
        return;
    }

    let chunk_count = chunks.iter().count();
    let fingerprint_changed = last_fingerprint.as_ref() != Some(&state.recipe_fingerprint);
    let new_chunks = chunk_count > *last_chunk_count;
    if !fingerprint_changed && !new_chunks {
        return;
    }
    *last_fingerprint = Some(state.recipe_fingerprint);
    *last_chunk_count = chunk_count;

    let Some(template) = materials.get(&handle.0).cloned() else {
        return;
    };

    for (mat_handle, palette) in &chunks {
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            let mut updated = template.with_chunk_palette(palette.0);
            updated.settings.debug_mode = 0;
            *mat = updated;
        }
    }
}

fn init_terrain_material_handle(
    state: Res<TerrainProceduralMaterialState>,
    mut commands: Commands,
) {
    commands.insert_resource(TerrainMaterialHandle(state.material.clone()));
}

fn sync_terrain_material_handle(
    state: Res<TerrainProceduralMaterialState>,
    mut handle: ResMut<TerrainMaterialHandle>,
) {
    if state.ready && handle.0 != state.material {
        handle.0 = state.material.clone();
    }
}

fn sync_world_terrain_material_recipes(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    mut override_recipes: ResMut<ProceduralMaterialRecipeOverride>,
    mut pending: ResMut<PendingTextureBake>,
    mut state: ResMut<TerrainProceduralMaterialState>,
    mut last_key: Local<Option<(String, String)>>,
) {
    let world_id = requested_world_id(&prefs);
    let Ok(world) = registry.0.effective_world(Some(&world_id)) else {
        return;
    };
    let key = (registry.0.hash.clone(), world.materials.as_str().to_owned());
    if last_key.as_ref() == Some(&key) {
        return;
    }
    *last_key = Some(key);

    let Some(compiled) = registry.0.materials.get(&world.materials) else {
        return;
    };
    override_recipes.recipes = Some(recipes_from_compiled_palette(compiled));
    override_recipes.layer_order = Some(
        compiled
            .layer_order
            .iter()
            .map(|id| id.as_str().to_string())
            .collect(),
    );
    state.ready = false;
    pending.task = None;
}

pub fn recipes_from_compiled_palette(
    compiled: &CompiledTerrainMaterials,
) -> Vec<TerrainMaterialRecipe> {
    let mut recipes = Vec::with_capacity(compiled.materials.len());
    for entry in &compiled.materials {
        recipes.push(recipe_from_entry(entry));
    }
    recipes
}

fn recipe_from_entry(entry: &TerrainMaterialEntryDefinition) -> TerrainMaterialRecipe {
    let key = entry.resolved_key();
    let albedo = entry.albedo;
    let roughness = entry.roughness.clamp(0.02, 1.0);
    let legacy_id = entry.resolved_legacy_id();
    let generator = if let Some(ref value) = entry.generator {
        texture_recipe_from_yaml_value(value).unwrap_or_else(|_| {
            synthesized_generator(&entry.name, legacy_id, albedo, roughness)
        })
    } else {
        synthesized_generator(&entry.name, legacy_id, albedo, roughness)
    };

    TerrainMaterialRecipe {
        id: key.as_str().to_string(),
        resolution: 256,
        meters_per_repeat: (1.0 / entry.triplanar_scale.max(0.12)).clamp(0.8, 8.0),
        generator,
        normal_strength: normal_strength_for_name(&entry.name),
        tint: [1.0, 1.0, 1.0],
    }
}

fn synthesized_generator(
    name: &str,
    legacy_id: u16,
    albedo: [f32; 3],
    roughness: f32,
) -> TextureRecipe {
    let semantic = classify_material_name(name);
    match semantic {
        MaterialSemantic::Sand => TextureRecipe::Sand(SandConfig {
            seed: 3_000 + legacy_id as u32,
            ripple_scale: 6.0,
            grain_scale: 24.0,
            color_light: brighten(albedo, 1.08),
            color_dark: darken(albedo, 0.78),
            normal_strength: 1.2,
            roughness,
        }),
        MaterialSemantic::Gravel => TextureRecipe::Cobblestone(CobblestoneConfig {
            seed: 4_000 + legacy_id as u32,
            scale: 5.0,
            octaves: 5,
            color_light: brighten(albedo, 1.04),
            color_dark: darken(albedo, 0.58),
            normal_strength: 3.2,
            roughness,
        }),
        MaterialSemantic::Rock | MaterialSemantic::Cave => TextureRecipe::Rock(RockConfig {
            seed: 1_000 + legacy_id as u32,
            scale: 3.0,
            octaves: 6,
            attenuation: 2.0,
            color_light: brighten(albedo, 1.05),
            color_dark: darken(albedo, 0.45),
            normal_strength: if matches!(semantic, MaterialSemantic::Cave) {
                2.8
            } else {
                3.4
            },
            roughness,
            metallic: 0.0,
        }),
        MaterialSemantic::Wet | MaterialSemantic::Ground => TextureRecipe::Ground(GroundConfig {
            seed: 2_000 + legacy_id as u32,
            macro_scale: 2.0,
            macro_octaves: 5,
            micro_scale: 10.0,
            micro_octaves: 4,
            micro_weight: if matches!(semantic, MaterialSemantic::Wet) {
                0.48
            } else {
                0.38
            },
            color_dry: brighten(albedo, 1.02),
            color_moist: darken(
                albedo,
                if matches!(semantic, MaterialSemantic::Wet) {
                    0.62
                } else {
                    0.72
                },
            ),
            normal_strength: if matches!(semantic, MaterialSemantic::Wet) {
                1.1
            } else {
                1.8
            },
            roughness,
        }),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MaterialSemantic {
    Ground,
    Sand,
    Rock,
    Cave,
    Wet,
    Gravel,
}

fn classify_material_name(name: &str) -> MaterialSemantic {
    let lower = name.to_ascii_lowercase();
    if lower.contains("cave") || lower.contains("flowstone") || lower.contains("limestone") {
        MaterialSemantic::Cave
    } else if lower.contains("gravel") || lower.contains("cobble") {
        MaterialSemantic::Gravel
    } else if lower.contains("sand") || lower.contains("silt") || lower.contains("shore") {
        MaterialSemantic::Sand
    } else if lower.contains("wet") || lower.contains("mud") || lower.contains("water") {
        MaterialSemantic::Wet
    } else if lower.contains("rock") || lower.contains("stone") || lower.contains("basalt") {
        MaterialSemantic::Rock
    } else {
        MaterialSemantic::Ground
    }
}

fn normal_strength_for_name(name: &str) -> f32 {
    match classify_material_name(name) {
        MaterialSemantic::Sand => 0.7,
        MaterialSemantic::Wet => 0.85,
        MaterialSemantic::Ground => 0.95,
        MaterialSemantic::Rock => 1.1,
        MaterialSemantic::Cave => 1.0,
        MaterialSemantic::Gravel => 1.15,
    }
}

fn brighten(color: [f32; 3], factor: f32) -> [f32; 3] {
    [
        (color[0] * factor).clamp(0.0, 1.0),
        (color[1] * factor).clamp(0.0, 1.0),
        (color[2] * factor).clamp(0.0, 1.0),
    ]
}

fn darken(color: [f32; 3], factor: f32) -> [f32; 3] {
    [
        (color[0] * factor).clamp(0.0, 1.0),
        (color[1] * factor).clamp(0.0, 1.0),
        (color[2] * factor).clamp(0.0, 1.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_data::load_registry_from_directory;
    use std::path::PathBuf;

    fn workspace_assets() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .canonicalize()
            .expect("assets")
    }

    #[test]
    fn compiled_world_materials_become_palette_recipes() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry");
        let compiled = registry
            .materials
            .get(&shared::StableId::new("materials.expanded_slice"))
            .expect("materials");
        let recipes = recipes_from_compiled_palette(compiled);
        assert_eq!(recipes.len(), compiled.materials.len());
        assert!(recipes.iter().all(|recipe| recipe.resolution == 256));
        assert!(recipes.iter().any(|recipe| recipe.normal_strength > 0.0));
        let keys: Vec<_> = recipes.iter().map(|r| r.id.as_str()).collect();
        assert!(keys.contains(&"grass"));
        assert!(keys.contains(&"flowstone"));
    }
}
