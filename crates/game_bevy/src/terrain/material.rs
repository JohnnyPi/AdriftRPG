use bevy::prelude::*;
use terrain_material_bevy::{
    FallbackTerrainMaterialSet, ProceduralTerrainMaterialPlugin, TerrainProceduralMaterialState,
};

use crate::data::assets_root;
use crate::state::AppState;

pub use terrain_material_bevy::TerrainPbrMaterial;

#[derive(Resource, Clone)]
pub struct TerrainMaterialHandle(pub Handle<TerrainPbrMaterial>);

pub struct TerrainMaterialPlugin;

impl Plugin for TerrainMaterialPlugin {
    fn build(&self, app: &mut App) {
        let yaml = assets_root().join("procedural/terrain/procedural_island.yaml");
        app.add_plugins(ProceduralTerrainMaterialPlugin {
            materials_yaml: Some(yaml),
        })
        .add_systems(
            Startup,
            init_terrain_material_handle.after(FallbackTerrainMaterialSet),
        )
        .add_systems(Update, (
            sync_terrain_material_handle,
            sync_chunk_terrain_materials,
        ).chain().run_if(in_state(AppState::Running)));
    }
}

fn sync_chunk_terrain_materials(
    handle: Res<TerrainMaterialHandle>,
    mut chunks: Query<&mut MeshMaterial3d<TerrainPbrMaterial>, With<crate::terrain::TerrainChunkEntity>>,
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
