use bevy::prelude::*;
use game_data::{CompiledTerrainMaterials, TerrainMaterialEntryDefinition};
use procedural_textures::{
    CobblestoneConfig, GroundConfig, RockConfig, SandConfig, TerrainMaterialIdName,
    TerrainMaterialRecipe, TextureRecipe,
};
use terrain_material_bevy::{
    FallbackTerrainMaterialSet, PendingTextureBake, ProceduralMaterialRecipeOverride,
    ProceduralTerrainMaterialPlugin, TerrainProceduralMaterialState,
};

use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;
use crate::state::AppState;
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
                sync_chunk_terrain_materials,
            )
                .chain()
                .run_if(in_state(AppState::Running)),
        );
    }
}

fn sync_chunk_terrain_materials(
    handle: Res<TerrainMaterialHandle>,
    mut chunks: Query<
        &mut MeshMaterial3d<TerrainPbrMaterial>,
        With<crate::terrain::TerrainChunkEntity>,
    >,
) {
    for mut material in &mut chunks {
        if material.0 != handle.0 {
            material.0 = handle.0.clone();
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
    override_recipes.0 = Some(recipes_from_compiled_materials(compiled));
    state.ready = false;
    pending.task = None;
}

fn recipes_from_compiled_materials(
    compiled: &CompiledTerrainMaterials,
) -> Vec<TerrainMaterialRecipe> {
    let max_id = compiled
        .materials
        .iter()
        .map(|entry| entry.id as usize)
        .max()
        .unwrap_or(0);
    let mut recipes = Vec::with_capacity(max_id + 1);
    for id in 0..=max_id {
        let recipe = compiled
            .materials
            .iter()
            .find(|entry| entry.id as usize == id)
            .map(recipe_from_entry)
            .unwrap_or_else(|| placeholder_recipe(id as u16));
        recipes.push(recipe);
    }
    recipes
}

fn recipe_from_entry(entry: &TerrainMaterialEntryDefinition) -> TerrainMaterialRecipe {
    let semantic = classify_material_name(&entry.name);
    let albedo = entry.albedo;
    let roughness = entry.roughness.clamp(0.02, 1.0);
    let generator = match semantic {
        MaterialSemantic::Sand => TextureRecipe::Sand(SandConfig {
            seed: 3_000 + entry.id as u32,
            ripple_scale: 6.0,
            grain_scale: 24.0,
            color_light: brighten(albedo, 1.08),
            color_dark: darken(albedo, 0.78),
            normal_strength: 1.2,
            roughness,
        }),
        MaterialSemantic::Gravel => TextureRecipe::Cobblestone(CobblestoneConfig {
            seed: 4_000 + entry.id as u32,
            scale: 5.0,
            octaves: 5,
            color_light: brighten(albedo, 1.04),
            color_dark: darken(albedo, 0.58),
            normal_strength: 3.2,
            roughness,
        }),
        MaterialSemantic::Rock | MaterialSemantic::Cave => TextureRecipe::Rock(RockConfig {
            seed: 1_000 + entry.id as u32,
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
            seed: 2_000 + entry.id as u32,
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
    };

    TerrainMaterialRecipe {
        id: recipe_id_for_name(&entry.name),
        resolution: 256,
        meters_per_repeat: (1.0 / entry.triplanar_scale.max(0.12)).clamp(0.8, 8.0),
        generator,
        normal_strength: normal_strength_for_semantic(semantic),
        tint: [1.0, 1.0, 1.0],
    }
}

fn placeholder_recipe(id: u16) -> TerrainMaterialRecipe {
    recipe_from_entry(&TerrainMaterialEntryDefinition {
        id,
        name: format!("material_{id}"),
        albedo: [0.34, 0.52, 0.28],
        triplanar_scale: 0.5,
        roughness: 0.9,
    })
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
    if lower.contains("cave") {
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

fn recipe_id_for_name(name: &str) -> TerrainMaterialIdName {
    match classify_material_name(name) {
        MaterialSemantic::Cave => TerrainMaterialIdName::CaveBasalt,
        MaterialSemantic::Gravel => TerrainMaterialIdName::RiverGravel,
        MaterialSemantic::Sand => TerrainMaterialIdName::CoralSand,
        MaterialSemantic::Wet => TerrainMaterialIdName::Mud,
        MaterialSemantic::Rock => TerrainMaterialIdName::WeatheredBasalt,
        MaterialSemantic::Ground => TerrainMaterialIdName::JungleLoam,
    }
}

fn normal_strength_for_semantic(semantic: MaterialSemantic) -> f32 {
    match semantic {
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
    fn compiled_world_materials_become_dense_recipe_layers() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry");
        let compiled = registry
            .materials
            .get(&shared::StableId::new("materials.expanded_slice"))
            .expect("materials");
        let recipes = recipes_from_compiled_materials(compiled);
        assert_eq!(recipes.len(), 7);
        assert!(recipes[0].meters_per_repeat > recipes[1].meters_per_repeat);
        assert!(recipes.iter().all(|recipe| recipe.resolution == 256));
        assert!(recipes.iter().any(|recipe| recipe.normal_strength > 0.0));
    }
}
