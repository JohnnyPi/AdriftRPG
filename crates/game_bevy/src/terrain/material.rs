// crates/game_bevy/src/terrain/material.rs
use bevy::prelude::*;
use game_data::{
    CompiledSurfaceRegistry, CompiledTerrainMaterials, TerrainMaterialEntryDefinition,
    resolve_entry_generator,
};
use procedural_textures::{
    CobblestoneConfig, GroundConfig, RockConfig, SandConfig, TerrainMaterialRecipe, TextureRecipe,
    texture_recipe_from_definition, texture_recipe_from_yaml_value,
};
use terrain_material_bevy::{
    FallbackTerrainMaterialSet, PendingTextureBake, ProceduralMaterialRecipeOverride,
    ProceduralTerrainMaterialPlugin, TerrainProceduralMaterialState,
};

use crate::camera::MainGameCamera;
use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;
use crate::lod::{LodPolicy, render_lod_tier_for_distance};
use crate::state::AppState;
use crate::terrain::TerrainWorldRuntime;
use crate::terrain::TerrainChunkEntity;
use crate::ui::TerrainMaterialTweaks;
use crate::world::requested_world_id;

pub use terrain_material_bevy::TerrainPbrMaterial;

/// Queue a rebake from the active world's compiled terrain-material palette.
pub fn queue_compiled_palette_reload(
    registry: &game_data::ConfigRegistry,
    world_id: &shared::StableId,
    override_recipes: &mut ProceduralMaterialRecipeOverride,
    pending: &mut PendingTextureBake,
    state: &mut TerrainProceduralMaterialState,
) -> bool {
    let Ok(world) = registry.effective_world(Some(world_id)) else {
        return false;
    };
    let Some(compiled) = registry.materials.get(&world.materials) else {
        return false;
    };
    override_recipes.recipes = Some(recipes_from_compiled_palette(
        compiled,
        surface_registry_for_world(registry, world_id),
    ));
    override_recipes.layer_order = Some(
        compiled
            .layer_order
            .iter()
            .map(|id| id.as_str().to_string())
            .collect(),
    );
    state.ready = false;
    pending.task = None;
    true
}

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
                sync_render_distance_lod,
                sync_catalog_material_defaults,
                sync_terrain_material_tweaks,
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
    mut last_fingerprint: Local<Option<[u8; 32]>>,
    mut chunks: Query<&mut MeshMaterial3d<TerrainPbrMaterial>, With<TerrainChunkEntity>>,
) {
    if !state.ready {
        *last_fingerprint = None;
        return;
    }
    if last_fingerprint.as_ref() == Some(&state.recipe_fingerprint) {
        return;
    }
    *last_fingerprint = Some(state.recipe_fingerprint);
    for mut mat in &mut chunks {
        mat.0 = handle.0.clone();
    }
}

fn init_terrain_material_handle(
    state: Res<TerrainProceduralMaterialState>,
    mut commands: Commands,
) {
    commands.insert_resource(TerrainMaterialHandle(state.material.clone()));
}

fn sync_render_distance_lod(
    policy: Res<LodPolicy>,
    runtime: Res<TerrainWorldRuntime>,
    camera: Query<&Transform, With<MainGameCamera>>,
    handle: Res<TerrainMaterialHandle>,
    state: Res<TerrainProceduralMaterialState>,
    mut materials: ResMut<Assets<TerrainPbrMaterial>>,
) {
    if !state.ready {
        return;
    }
    let Ok(cam) = camera.single() else {
        return;
    };
    let interest = Vec3::new(
        runtime.interest_center.x as f32 * 16.0,
        cam.translation.y,
        runtime.interest_center.z as f32 * 16.0,
    );
    let distance = cam.translation.distance(interest);
    let Some(tier) = render_lod_tier_for_distance(&policy, distance) else {
        return;
    };
    let Some(mut mat) = materials.get_mut(&handle.0) else {
        return;
    };
    mat.settings.layer_count = tier.active_layers.min(mat.settings.layer_count.max(1));
    mat.settings.triplanar_sharpness = match tier.projection_axes {
        1 => 8.0,
        2 => 6.0,
        _ => 4.0,
    };
}

fn sync_catalog_material_defaults(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    handle: Res<TerrainMaterialHandle>,
    state: Res<TerrainProceduralMaterialState>,
    mut materials: ResMut<Assets<TerrainPbrMaterial>>,
    mut last_hash: Local<Option<String>>,
) {
    if !state.ready {
        return;
    }
    let hash = registry.0.hash.clone();
    if last_hash.as_ref() == Some(&hash) {
        return;
    }
    *last_hash = Some(hash);
    let Ok(world) = registry.0.effective_world(Some(&requested_world_id(&prefs))) else {
        return;
    };
    let Some(render_profile) = registry
        .0
        .render_profiles
        .get(&world.lod.materials.render_profile)
    else {
        return;
    };
    let Some(mut material) = materials.get_mut(&handle.0) else {
        return;
    };
    if render_profile.macro_variation {
        material.settings.macro_variation_scale = 42.0;
        material.settings.macro_variation_strength = 0.10;
    } else {
        material.settings.macro_variation_strength = 0.0;
    }
}

fn sync_terrain_material_tweaks(
    tweaks: Res<TerrainMaterialTweaks>,
    handle: Res<TerrainMaterialHandle>,
    mut materials: ResMut<Assets<TerrainPbrMaterial>>,
    mut last_key: Local<Option<(bool, u32, f32, f32, f32)>>,
) {
    if !tweaks.use_overrides {
        return;
    }
    let key = (
        tweaks.use_overrides,
        tweaks.debug_mode,
        tweaks.global_wetness,
        tweaks.global_moss,
        tweaks.macro_variation_strength,
    );
    if last_key.as_ref() == Some(&key) {
        return;
    }
    *last_key = Some(key);
    let Some(mut material) = materials.get_mut(&handle.0) else {
        return;
    };
    material.settings.global_wetness = tweaks.global_wetness;
    material.settings.global_moss = tweaks.global_moss;
    material.settings.macro_variation_strength = tweaks.macro_variation_strength;
    material.settings.debug_mode = tweaks.debug_mode;
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

    let _ = queue_compiled_palette_reload(
        &registry.0,
        &world_id,
        &mut override_recipes,
        &mut pending,
        &mut state,
    );
}

fn surface_registry_for_world<'a>(
    registry: &'a game_data::ConfigRegistry,
    world_id: &'a shared::StableId,
) -> Option<&'a CompiledSurfaceRegistry> {
    let world = registry.effective_world(Some(world_id)).ok()?;
    let catalog_id = world.material_catalog.as_ref()?;
    registry.surface_registries.get(catalog_id)
}

pub fn recipes_from_compiled_palette(
    compiled: &CompiledTerrainMaterials,
    surface_registry: Option<&CompiledSurfaceRegistry>,
) -> Vec<TerrainMaterialRecipe> {
    let mut recipes = Vec::with_capacity(compiled.materials.len());
    for entry in &compiled.materials {
        recipes.push(recipe_from_entry(entry, surface_registry));
    }
    recipes
}

fn recipe_from_entry(
    entry: &TerrainMaterialEntryDefinition,
    surface_registry: Option<&CompiledSurfaceRegistry>,
) -> TerrainMaterialRecipe {
    let key = entry.resolved_key();
    let albedo = entry.albedo;
    let roughness = entry.roughness.clamp(0.02, 1.0);
    let legacy_id = entry.resolved_legacy_id();
    let generator = resolve_texture_recipe(entry, surface_registry, legacy_id, albedo, roughness);

    let mut meters_per_repeat = (1.0 / entry.triplanar_scale.max(0.12)).clamp(0.8, 8.0);
    let mut normal_strength = normal_strength_for_name(&entry.name);

    if let Some(ref rendering) = entry.rendering {
        if let Some(mpr) = rendering.meters_per_repeat {
            meters_per_repeat = mpr;
        }
        if let Some(ns) = rendering.normal_strength {
            normal_strength = ns;
        }
    }

    if let Some(reg) = surface_registry {
        if let Some(surface) = reg.surface_for_material_key(&key) {
            if let Some(mpr) = surface.rendering.meters_per_repeat {
                meters_per_repeat = mpr;
            }
            if let Some(ns) = surface.rendering.normal_strength {
                normal_strength = ns;
            }
        }
    }

    TerrainMaterialRecipe {
        id: key.as_str().to_string(),
        resolution: 256,
        meters_per_repeat,
        generator,
        normal_strength,
        tint: [1.0, 1.0, 1.0],
    }
}

fn resolve_texture_recipe(
    entry: &TerrainMaterialEntryDefinition,
    surface_registry: Option<&CompiledSurfaceRegistry>,
    legacy_id: u16,
    albedo: [f32; 3],
    roughness: f32,
) -> TextureRecipe {
    let seed = legacy_id as u32;
    if let Some(reg) = surface_registry {
        if let Some(ref tex_id) = entry.texture {
            if let Some(&layer) = reg.texture_by_id.get(tex_id) {
                if let Some(tex) = reg.textures.get(layer as usize) {
                    if let Ok(Some(recipe)) = texture_recipe_from_definition(
                        tex.generator.as_ref(),
                        tex.graph.as_ref(),
                        tex.seed.unwrap_or(seed),
                    ) {
                        return recipe;
                    }
                }
            }
        }
    }
    if let Some(generator_yaml) = resolve_entry_generator(entry, surface_registry) {
        if generator_yaml.get("nodes").is_some() {
            if let Ok(recipe) =
                procedural_textures::texture_graph_from_yaml_value(&generator_yaml, seed)
            {
                return TextureRecipe::Graph(recipe);
            }
        }
        if let Ok(recipe) = texture_recipe_from_yaml_value(&generator_yaml) {
            return recipe;
        }
    }
    synthesized_generator(&entry.name, legacy_id, albedo, roughness)
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
        let recipes = recipes_from_compiled_palette(compiled, None);
        assert_eq!(recipes.len(), compiled.materials.len());
        assert!(recipes.iter().all(|recipe| recipe.resolution == 256));
        assert!(recipes.iter().any(|recipe| recipe.normal_strength > 0.0));
        let keys: Vec<_> = recipes.iter().map(|r| r.id.as_str()).collect();
        assert!(keys.contains(&"grass"));
        assert!(keys.contains(&"flowstone"));
    }
}
