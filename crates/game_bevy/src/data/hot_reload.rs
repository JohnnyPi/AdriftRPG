use bevy::prelude::*;

use crate::data::ConfigRegistryResource;
use crate::environment::biomes::BiomeCatalog;
use crate::state::AppState;
use crate::water::WaterMaterial;
use terrain_material_bevy::{
    PendingTextureBake, ProceduralMaterialYamlPath, TerrainProceduralMaterialState,
};
use procedural_textures::ProceduralMaterialsDocument;

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
    commands.insert_resource(BiomeCatalog::from_registry(
        &registry.0,
        Some(&world_id),
    ));
}

fn reload_terrain_material(
    registry: Res<ConfigRegistryResource>,
    mut last_hash: Local<Option<String>>,
    proc_state: Option<Res<TerrainProceduralMaterialState>>,
) {
    let hash = registry.0.hash.clone();
    if last_hash.as_ref() == Some(&hash) {
        return;
    }
    *last_hash = Some(hash);
    let _ = proc_state;
}

fn reload_procedural_terrain_material(
    registry: Res<ConfigRegistryResource>,
    mut last_hash: Local<Option<String>>,
    yaml_path: Option<Res<ProceduralMaterialYamlPath>>,
    mut proc_state: ResMut<TerrainProceduralMaterialState>,
    mut pending: ResMut<PendingTextureBake>,
) {
    let hash = registry.0.hash.clone();
    if last_hash.as_ref() == Some(&hash) {
        return;
    }
    *last_hash = Some(hash);
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
    proc_state.ready = false;
    pending.task = None;
    proc_state.recipe_fingerprint = procedural_textures::document_fingerprint(&doc);
    let _ = doc;
}

fn reload_water_material(
    registry: Res<ConfigRegistryResource>,
    mut last_hash: Local<Option<String>>,
    mut water_query: Query<(&mut Transform, &mut MeshMaterial3d<WaterMaterial>)>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let hash = registry.0.hash.clone();
    if last_hash.as_ref() == Some(&hash) {
        return;
    }
    *last_hash = Some(hash);
    let Ok(world) = registry.0.active_world() else {
        return;
    };
    let Some(water_def) = registry.0.water.get(&world.water) else {
        return;
    };
    for (mut transform, mat_handle) in &mut water_query {
        transform.translation.y = water_def.sea_level_m + 0.02;
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
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
                water_def.sea_level_m,
                water_def.wave_speed,
                water_def.wave_amplitude,
                water_def.transparency,
            );
        }
    }
}
