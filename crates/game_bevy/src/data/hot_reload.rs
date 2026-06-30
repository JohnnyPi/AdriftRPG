use bevy::prelude::*;

use crate::data::ConfigRegistryResource;
use crate::environment::biomes::BiomeCatalog;
use crate::state::AppState;
use crate::terrain::{TerrainMaterialHandle, TerrainTriplanarMaterial};
use crate::water::WaterMaterial;

/// Applies live updates for visual YAML changes (materials, biomes, water).
pub struct VisualConfigHotReloadPlugin;

impl Plugin for VisualConfigHotReloadPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                reload_biome_catalog,
                reload_terrain_material,
                reload_water_material,
            )
                .run_if(in_state(AppState::Running)),
        );
    }
}

fn reload_biome_catalog(
    registry: Res<ConfigRegistryResource>,
    mut last_hash: Local<Option<String>>,
    mut commands: Commands,
) {
    let hash = registry.0.hash.clone();
    if last_hash.as_ref() == Some(&hash) {
        return;
    }
    *last_hash = Some(hash);
    commands.insert_resource(BiomeCatalog::from_registry(&registry.0));
}

fn reload_terrain_material(
    registry: Res<ConfigRegistryResource>,
    mut last_hash: Local<Option<String>>,
    handle: Option<Res<TerrainMaterialHandle>>,
    mut materials: ResMut<Assets<TerrainTriplanarMaterial>>,
) {
    let hash = registry.0.hash.clone();
    if last_hash.as_ref() == Some(&hash) {
        return;
    }
    *last_hash = Some(hash);
    let Some(handle) = handle else {
        return;
    };
    if let Some(mut mat) = materials.get_mut(&handle.0) {
        *mat = TerrainTriplanarMaterial::from_registry(&registry.0);
    }
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
