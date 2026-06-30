use bevy::prelude::*;
use voxel_core::ChunkCoord;

use crate::state::AppState;
use crate::terrain::{ChunkState, TerrainPipelineState, TerrainRevision};

#[derive(Resource, Default, Debug)]
pub struct TerrainEditLayer {
    pub dirty_chunks: Vec<ChunkCoord>,
}

pub struct TerrainEditingPlugin;

impl Plugin for TerrainEditingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainEditLayer>()
            .add_systems(Update, handle_edit_keys.run_if(in_state(AppState::Running)));
    }
}

fn handle_edit_keys(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut edit: ResMut<TerrainEditLayer>,
    mut revision: ResMut<TerrainRevision>,
    mut pipeline: ResMut<TerrainPipelineState>,
) {
    let pressed = |key: KeyCode| keyboard.just_pressed(key);
    if !(pressed(KeyCode::Digit1) || pressed(KeyCode::Digit2) || pressed(KeyCode::Digit3)) {
        return;
    }

    let mode = if pressed(KeyCode::Digit1) {
        "subtract_sphere"
    } else if pressed(KeyCode::Digit2) {
        "add_sphere"
    } else {
        "paint_material"
    };
    let _ = mode;

    if let Some(spawn_chunk) = pipeline.spawn_chunk {
        if !edit.dirty_chunks.contains(&spawn_chunk) {
            edit.dirty_chunks.push(spawn_chunk);
        }
        if let Some(chunk) = pipeline.chunks.iter_mut().find(|c| c.coord == spawn_chunk) {
            chunk.state = ChunkState::Unrequested;
        }
        revision.value += 1;
        let to_despawn = pipeline.reset_for_revision(revision.value);
        for entity in to_despawn {
            commands.entity(entity).despawn();
        }
    }
}
