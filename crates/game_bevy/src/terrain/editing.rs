use avian3d::prelude::*;
use bevy::prelude::*;
use tracing::info;
use voxel_core::{MaterialId, TerrainEditCommand};

use crate::camera::{MainGameCamera, MmoCamera};
use crate::debug_tools::DebugKeyBindings;
use crate::environment::biomes::BiomeCatalog;
use crate::environment::materials::material_for_world;
use crate::player::Player;
use crate::state::AppState;
use crate::terrain::{TerrainEditStore, TerrainPipelineState, TerrainRevision};

const EDIT_SPHERE_RADIUS_M: f32 = 2.5;
const EDIT_RAYCAST_DISTANCE_M: f32 = 24.0;

pub struct TerrainEditingPlugin;

impl Plugin for TerrainEditingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_edit_keys.run_if(in_state(AppState::Running)));
    }
}

fn handle_edit_keys(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    bindings: Res<DebugKeyBindings>,
    biomes: Res<BiomeCatalog>,
    player: Query<&Transform, With<Player>>,
    camera: Query<&MmoCamera, With<MainGameCamera>>,
    spatial: SpatialQuery,
    mut edit_store: ResMut<TerrainEditStore>,
    mut revision: ResMut<TerrainRevision>,
    mut pipeline: ResMut<TerrainPipelineState>,
) {
    let command = if keyboard.just_pressed(bindings.subtract) {
        Some(TerrainEditCommand::SubtractSphere {
            center: [0.0; 3],
            radius_m: EDIT_SPHERE_RADIUS_M,
        })
    } else if keyboard.just_pressed(bindings.add) {
        Some(TerrainEditCommand::AddSphere {
            center: [0.0; 3],
            radius_m: EDIT_SPHERE_RADIUS_M,
        })
    } else if keyboard.just_pressed(bindings.paint) {
        Some(TerrainEditCommand::PaintMaterial {
            center: [0.0; 3],
            radius_m: EDIT_SPHERE_RADIUS_M,
            material: MaterialId(2),
        })
    } else {
        None
    };

    let Some(mut command) = command else {
        return;
    };

    let Ok(player_tf) = player.single() else {
        return;
    };
    let Ok(cam) = camera.single() else {
        return;
    };

    let center = edit_target_world(
        player_tf.translation,
        cam,
        &spatial,
        cam.player,
    );
    match &mut command {
        TerrainEditCommand::SubtractSphere { center: c, .. }
        | TerrainEditCommand::AddSphere { center: c, .. }
        | TerrainEditCommand::PaintMaterial { center: c, .. } => {
            *c = center.to_array();
        }
    }

    let Some(source) = pipeline.density_source.clone() else {
        return;
    };

    let affected = edit_store.0.apply_command(
        &command,
        |wx, wy, wz| source.density_at(wx as f32, wy as f32, wz as f32),
        |wx, wy, wz, density| {
            material_for_world(
                &biomes,
                &source,
                wx as f32,
                wy as f32,
                wz as f32,
                density,
            )
        },
    );

    if affected.is_empty() {
        return;
    }

    revision.value += 1;
    let coords: Vec<_> = affected.into_iter().collect();
    let to_despawn = pipeline.invalidate_chunks(&coords, revision.value);
    for entity in to_despawn {
        commands.entity(entity).despawn();
    }

    info!(
        ?command,
        chunk_count = coords.len(),
        revision = revision.value,
        "terrain edit applied"
    );
}

fn edit_target_world(
    player_pos: Vec3,
    camera: &MmoCamera,
    spatial: &SpatialQuery,
    player_entity: Entity,
) -> Vec3 {
    let focus = camera.current_focus;
    let yaw = camera.current_yaw;
    let pitch = camera.current_pitch;
    let direction = Vec3::new(
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        yaw.cos() * pitch.cos(),
    )
    .normalize_or_zero();

    let filter = SpatialQueryFilter::default().with_excluded_entities([player_entity]);
    if let Some(hit) = spatial.cast_ray(
        focus,
        Dir3::new(direction).unwrap_or(Dir3::NEG_Z),
        EDIT_RAYCAST_DISTANCE_M,
        true,
        &filter,
    ) {
        return focus + direction * hit.distance;
    }

    player_pos + direction * 4.0 + Vec3::Y * 0.5
}
