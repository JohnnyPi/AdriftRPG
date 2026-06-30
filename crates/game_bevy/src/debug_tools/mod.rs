mod bindings;

use bevy::prelude::*;
use bevy::pbr::wireframe::{Wireframe, WireframePlugin};
use avian3d::prelude::*;

use bindings::{init_debug_bindings, DebugKeyBindings};
use crate::data::ConfigRegistryResource;
use crate::environment::biomes::{biome_color, classify_biome, BiomeCatalog, BiomeKind};
use crate::environment::materials::assign_material_color;
use crate::state::AppState;
use crate::terrain::{
    regen_terrain_with_seed, TerrainChunkEntity, TerrainPipelineMetrics, TerrainPipelineState,
    TerrainRecipeRevision, TerrainRevision, TerrainSpawnPoint, WorldSeedOverride,
};

#[derive(Resource, Default)]
pub struct DebugOverlayState {
    pub chunk_bounds: bool,
    pub wireframe: bool,
    pub show_biomes: bool,
    pub show_materials: bool,
    pub show_colliders: bool,
    pub show_density: bool,
    pub show_normals: bool,
    pub debug_panel: bool,
}

#[derive(Component)]
struct DebugPanelText;

pub struct DebugToolsPlugin;

impl Plugin for DebugToolsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugOverlayState>()
            .add_plugins(WireframePlugin::default())
            .add_systems(OnEnter(AppState::Running), init_debug_bindings)
            .add_systems(Update, handle_debug_keys.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_chunk_bounds.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_colliders.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_vertex_normals.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_biome_labels.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_material_gizmos.run_if(in_state(AppState::Running)))
            .add_systems(Update, toggle_wireframe.run_if(in_state(AppState::Running)))
            .add_systems(Update, update_debug_panel.run_if(in_state(AppState::Running)));
    }
}

fn handle_debug_keys(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    bindings: Res<DebugKeyBindings>,
    mut debug: ResMut<DebugOverlayState>,
    mut revision: ResMut<TerrainRevision>,
    mut recipe_revision: ResMut<TerrainRecipeRevision>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut seed_override: ResMut<WorldSeedOverride>,
    mut spawn_point: ResMut<TerrainSpawnPoint>,
    registry: Res<ConfigRegistryResource>,
) {
    if keyboard.just_pressed(bindings.panel) {
        debug.debug_panel = !debug.debug_panel;
    }
    if keyboard.just_pressed(bindings.chunk_bounds) {
        debug.chunk_bounds = !debug.chunk_bounds;
    }
    if keyboard.just_pressed(bindings.wireframe) {
        debug.wireframe = !debug.wireframe;
    }
    if keyboard.just_pressed(bindings.biome) {
        debug.show_biomes = !debug.show_biomes;
    }
    if keyboard.just_pressed(bindings.material) {
        debug.show_materials = !debug.show_materials;
    }
    if keyboard.just_pressed(bindings.collider) {
        debug.show_colliders = !debug.show_colliders;
    }
    if keyboard.just_pressed(bindings.density) {
        debug.show_density = !debug.show_density;
    }
    if keyboard.just_pressed(bindings.normals) {
        debug.show_normals = !debug.show_normals;
    }
    if keyboard.just_pressed(bindings.regen) {
        regen_terrain_with_seed(
            &mut commands,
            &registry,
            &mut pipeline,
            &mut recipe_revision,
            &mut revision,
            &seed_override,
            &mut spawn_point,
        );
    }
    if keyboard.just_pressed(bindings.next_seed) {
        seed_override.seed = seed_override.seed.wrapping_add(1);
        regen_terrain_with_seed(
            &mut commands,
            &registry,
            &mut pipeline,
            &mut recipe_revision,
            &mut revision,
            &seed_override,
            &mut spawn_point,
        );
    }
    if keyboard.just_pressed(bindings.freeze_pipeline) {
        pipeline.frozen = !pipeline.frozen;
    }
}

fn draw_colliders(
    debug: Res<DebugOverlayState>,
    mut gizmos: Gizmos,
    colliders: Query<(&Collider, &GlobalTransform)>,
) {
    if !debug.show_colliders {
        return;
    }

    for (collider, transform) in &colliders {
        let aabb = collider.aabb(transform.translation(), transform.rotation());
        let center = Vec3::from(aabb.center());
        let size = Vec3::from(aabb.size());
        if size.length_squared() <= f32::EPSILON {
            continue;
        }
        gizmos.cube(
            Transform::from_translation(center).with_scale(size),
            Color::srgba(1.0, 0.45, 0.1, 0.25),
        );
    }
}

fn draw_chunk_bounds(
    debug: Res<DebugOverlayState>,
    mut gizmos: Gizmos,
    chunks: Query<(&TerrainChunkEntity, &Transform)>,
) {
    if !debug.chunk_bounds {
        return;
    }
    for (chunk, transform) in &chunks {
        let origin = transform.translation;
        let size = 16.0;
        let mins = origin;
        let maxs = origin + Vec3::splat(size);
        gizmos.cube(
            Transform::from_translation((mins + maxs) * 0.5).with_scale(Vec3::splat(size)),
            Color::srgba(0.2, 0.8, 1.0, 0.15),
        );
        let _ = chunk.coord;
    }
}

fn draw_vertex_normals(
    debug: Res<DebugOverlayState>,
    mut gizmos: Gizmos,
    meshes: Res<Assets<Mesh>>,
    chunks: Query<(&Mesh3d, &GlobalTransform), With<TerrainChunkEntity>>,
) {
    if !debug.show_normals {
        return;
    }

    for (mesh3d, transform) in &chunks {
        let Some(mesh) = meshes.get(&mesh3d.0) else {
            continue;
        };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            continue;
        };
        let step = (positions.len() / 64).max(1);
        for (position, normal) in positions.iter().zip(normals.iter()).step_by(step) {
            let origin = transform.transform_point(Vec3::from_array(*position));
            let tip = origin + Vec3::from_array(*normal) * 0.35;
            gizmos.line(origin, tip, Color::srgb(0.2, 1.0, 0.35));
        }
    }
}

fn draw_biome_labels(
    debug: Res<DebugOverlayState>,
    pipeline: Res<TerrainPipelineState>,
    biomes: Res<BiomeCatalog>,
    mut gizmos: Gizmos,
) {
    if !debug.show_biomes && !debug.show_density {
        return;
    }
    let Some(source) = pipeline.density_source.as_ref() else {
        return;
    };
    let sea = source.recipe().sea_level;
    for x in (-20..20).step_by(5) {
        for z in (-20..20).step_by(5) {
            let wx = x as f32;
            let wz = z as f32;
            let y = source.surface_height_at(wx, wz);
            let density = source.density_at(wx, y, wz);
            if debug.show_biomes {
                let biome = classify_biome(biomes.as_ref(), sea, wx, y, wz, density);
                let color = biome_color(biomes.as_ref(), biome);
                gizmos.sphere(
                    Isometry3d::from_translation(Vec3::new(wx, y + 0.5, wz)),
                    0.25,
                    color,
                );
            }
            if debug.show_density && density <= 0.0 {
                gizmos.sphere(
                    Isometry3d::from_translation(Vec3::new(wx, y, wz)),
                    0.15,
                    Color::srgba(0.9, 0.2, 0.2, 0.5),
                );
            }
        }
    }
}

fn draw_material_gizmos(
    debug: Res<DebugOverlayState>,
    pipeline: Res<TerrainPipelineState>,
    biomes: Res<BiomeCatalog>,
    mut gizmos: Gizmos,
) {
    if !debug.show_materials {
        return;
    }
    let Some(source) = pipeline.density_source.as_ref() else {
        return;
    };
    let sea = source.recipe().sea_level;
    for x in (-20..20).step_by(4) {
        for z in (-20..20).step_by(4) {
            let wx = x as f32;
            let wz = z as f32;
            let y = source.surface_height_at(wx, wz);
            let density = source.density_at(wx, y, wz);
            if density > 0.0 {
                continue;
            }
            let biome = classify_biome(biomes.as_ref(), sea, wx, y, wz, density);
            let material_id = match biome {
                BiomeKind::Beach => 1,
                BiomeKind::Grassland => 0,
                BiomeKind::RockyUpland => 2,
                BiomeKind::Cave => 3,
                BiomeKind::ShallowWater => 1,
            };
            let color = assign_material_color(biomes.as_ref(), material_id);
            gizmos.cube(
                Transform::from_translation(Vec3::new(wx, y + 0.2, wz))
                    .with_scale(Vec3::new(0.35, 0.08, 0.35)),
                color,
            );
        }
    }
}

fn update_debug_panel(
    debug: Res<DebugOverlayState>,
    pipeline: Res<TerrainPipelineState>,
    metrics: Res<TerrainPipelineMetrics>,
    seed_override: Res<WorldSeedOverride>,
    registry: Res<ConfigRegistryResource>,
    time: Res<Time>,
    mut panel: Local<Option<Entity>>,
    mut commands: Commands,
    mut text_query: Query<&mut Text, With<DebugPanelText>>,
) {
    if !debug.debug_panel {
        if let Some(entity) = panel.take() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let ready = pipeline
        .chunks
        .iter()
        .filter(|c| c.state == crate::terrain::ChunkState::Ready)
        .count();
    let world_seed = registry.0.active_world().map(|w| w.seed).unwrap_or(0);
    let fps = 1.0 / time.delta_secs().max(0.0001);
    let body = format!(
        "Debug Panel\n\
         Seed: {} (override)\n\
         Chunks ready: {ready}/{}\n\
         Queues  D:{:?} M:{:?} U:{:?} C:{:?}\n\
         Last ms  density:{:.1} mesh:{:.1} upload:{:.1}\n\
         Colliders/frame: {}  Frozen: {}\n\
         FPS: {fps:.0}\n\
         N=normals  F8=regen  F9=next seed",
        seed_override.seed,
        pipeline.chunks.len(),
        pipeline.density_queue_len(),
        pipeline.mesh_queue_len(),
        pipeline.upload_queue_len(),
        pipeline.collider_queue_len(),
        metrics.last_density_ms,
        metrics.last_mesh_ms,
        metrics.last_upload_ms,
        metrics.colliders_built_this_frame,
        pipeline.frozen,
    );
    let _ = world_seed;

    if panel.is_none() {
        let entity = commands
            .spawn((
                DebugPanelText,
                Text::new(body.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(12.0),
                    left: Val::Px(12.0),
                    ..default()
                },
            ))
            .id();
        *panel = Some(entity);
    } else if let Ok(mut text) = text_query.single_mut() {
        **text = body;
    }
}

fn toggle_wireframe(
    debug: Res<DebugOverlayState>,
    mut commands: Commands,
    chunks: Query<Entity, With<TerrainChunkEntity>>,
    wireframes: Query<Entity, With<Wireframe>>,
) {
    let want = debug.wireframe;
    for entity in wireframes.iter() {
        if !want {
            commands.entity(entity).remove::<Wireframe>();
        }
    }
    if want {
        for entity in chunks.iter() {
            commands.entity(entity).insert(Wireframe);
        }
    }
}
