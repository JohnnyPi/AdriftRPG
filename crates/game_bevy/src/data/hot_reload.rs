// crates/game_bevy/src/data/hot_reload.rs
use bevy::prelude::*;

use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;
use crate::environment::BiomeCatalog;
use crate::state::AppState;
use crate::terrain::queue_compiled_palette_reload;
use crate::water::{OceanSurface, WaterMaterial, WaterSurface};
use procedural_textures::ProceduralMaterialsDocument;
use terrain_material_bevy::{
    PendingTextureBake, ProceduralMaterialRecipeOverride, ProceduralMaterialYamlPath,
    TerrainProceduralMaterialState,
};

/// Applies live updates for visual YAML changes (materials, biomes, water).
pub struct VisualConfigHotReloadPlugin;

impl Plugin for VisualConfigHotReloadPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                reload_biome_catalog,
                reload_terrain_material,
                reload_procedural_terrain_material,
                reload_water_material,
            )
                .run_if(in_state(AppState::Running)),
        );
    }
}

fn reload_biome_catalog(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<crate::data::UserSetupPrefs>,
    mut last_hash: Local<Option<String>>,
    mut commands: Commands,
) {
    let hash = registry.0.hash.clone();
    if last_hash.as_ref() == Some(&hash) {
        return;
    }
    *last_hash = Some(hash);
    let world_id = crate::world::requested_world_id(&prefs);
    commands.insert_resource(BiomeCatalog::from_registry(&registry.0, Some(&world_id)));
}

fn reload_terrain_material(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    mut last_hash: Local<Option<String>>,
    mut override_recipes: ResMut<ProceduralMaterialRecipeOverride>,
    mut pending: ResMut<PendingTextureBake>,
    mut proc_state: ResMut<TerrainProceduralMaterialState>,
) {
    let hash = registry.0.hash.clone();
    if last_hash.as_ref() == Some(&hash) {
        return;
    }
    *last_hash = Some(hash);
    let world_id = crate::world::requested_world_id(&prefs);
    let _ = queue_compiled_palette_reload(
        &registry.0,
        &world_id,
        &mut override_recipes,
        &mut pending,
        &mut proc_state,
    );
}

fn reload_procedural_terrain_material(
    registry: Res<ConfigRegistryResource>,
    mut last_hash: Local<Option<String>>,
    recipe_override: Option<Res<ProceduralMaterialRecipeOverride>>,
    yaml_path: Option<Res<ProceduralMaterialYamlPath>>,
    mut proc_state: ResMut<TerrainProceduralMaterialState>,
    mut pending: ResMut<PendingTextureBake>,
) {
    let hash = registry.0.hash.clone();
    if last_hash.as_ref() == Some(&hash) {
        return;
    }
    *last_hash = Some(hash);
    proc_state.ready = false;
    pending.task = None;
    if let Some(override_recipes) = recipe_override
        .as_ref()
        .and_then(|override_recipes| override_recipes.recipes.as_ref())
    {
        proc_state.recipe_fingerprint =
            terrain_material_bevy::recipe_fingerprint_for(override_recipes);
        return;
    }
    let Some(yaml_path) = yaml_path else {
        return;
    };
    let Some(path) = yaml_path.0.as_ref() else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let text = procedural_textures::strip_utf8_bom(&text);
    let Ok(doc) = serde_yaml::from_str::<ProceduralMaterialsDocument>(text) else {
        return;
    };
    proc_state.recipe_fingerprint = procedural_textures::document_fingerprint(&doc);
}

fn reload_water_material(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    water_tweaks: Res<crate::ui::WaterTweaks>,
    mut last_hash: Local<Option<String>>,
    mut ocean_query: Query<
        (&mut Transform, &mut MeshMaterial3d<WaterMaterial>),
        With<OceanSurface>,
    >,
    mut other_water_query: Query<
        &mut MeshMaterial3d<WaterMaterial>,
        (With<WaterSurface>, Without<OceanSurface>),
    >,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let hash = registry.0.hash.clone();
    if last_hash.as_ref() == Some(&hash) {
        return;
    }
    *last_hash = Some(hash);
    let Ok(world) = crate::world::effective_world_from_prefs(&registry.0, &prefs) else {
        return;
    };
    let Some(water_def) = registry.0.water.get(&world.water) else {
        return;
    };
    let sea_level = if water_tweaks.use_overrides {
        water_tweaks.sea_level_m
    } else {
        water_def.sea_level_m
    };
    let update_mat = |mat: &mut WaterMaterial| {
        mat.params.shallow_color = Vec4::new(
            water_def.shallow_color[0],
            water_def.shallow_color[1],
            water_def.shallow_color[2],
            water_def.transparency,
        );
        mat.params.deep_color = Vec4::new(
            water_def.deep_color[0],
            water_def.deep_color[1],
            water_def.deep_color[2],
            1.0,
        );
        mat.params.wave = Vec4::new(
            sea_level,
            water_def.wave_speed,
            water_def.wave_amplitude,
            water_def.transparency,
        );
    };
    for (mut transform, mat_handle) in &mut ocean_query {
        transform.translation.y = sea_level + 0.02;
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            update_mat(&mut mat);
        }
    }
    for mat_handle in &mut other_water_query {
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            update_mat(&mut mat);
        }
    }
}
