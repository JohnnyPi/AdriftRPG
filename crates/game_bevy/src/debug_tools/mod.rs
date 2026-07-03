// crates/game_bevy/src/debug_tools/mod.rs
mod bindings;

pub use bindings::DebugKeyBindings;

use bevy::prelude::*;
use bevy::pbr::wireframe::{Wireframe, WireframePlugin};
use avian3d::prelude::*;

use bindings::init_debug_bindings;
use crate::camera::{CameraDebugSnapshot, MainGameCamera, MmoCamera};
use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;
use crate::environment::biome_context::BiomeSampleContext;
use crate::environment::biomes::{
    biome_color, biome_discrete_debug_color, biome_scalar_debug_value, classify_biome,
};
use crate::environment::BiomeCatalog;
use crate::environment::materials::{assign_material_color, material_for_world};
use crate::state::AppState;
use crate::terrain::{
    regen_terrain_with_seed, TerrainChunkEntity, TerrainEditStore, TerrainFeatureRegistry,
    TerrainMaterialHandle, TerrainPipelineMetrics, TerrainPipelineState, TerrainRecipeRevision,
    TerrainRegenPending, TerrainRevision, TerrainSpawnPoint,
    TerrainWorldRuntime, WorldSeedOverride,
};
use terrain_material_bevy::TerrainPbrMaterial;
use crate::terrain::draw_residency_rings;
use crate::terrain::chunk_world_center;
use crate::lod::LodPolicy;
use crate::staging::{AssetStagingQueue, StagingGate};
use crate::world::{effective_world_from_prefs, semantic_tag_color, WorldSemanticRegistry, WorldSemanticTag};
use crate::environment::fog::FogStack;
use crate::ui::{EcologyTweaks, RiverTweaks, TerrainTweaks, WorldTweaks};
use terrain_generation::build_coast_mask;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BiomeDebugView {
    #[default]
    Normal,
    ConstantGreen,
    HeatmapGrayscale,
    DiscreteRegions,
}

#[derive(Resource, Default)]
pub struct DebugOverlayState {
    pub chunk_bounds: bool,
    pub wireframe: bool,
    pub show_biomes: bool,
    pub show_materials: bool,
    pub show_colliders: bool,
    pub show_density: bool,
    /// VS3 island atlas field overlay: elevation, flow_accumulation, river_mask, beach_suitability, cliff_suitability
    pub island_field_view: String,
    pub show_normals: bool,
    pub show_camera_cast: bool,
    pub debug_panel: bool,
    pub biome_debug_view: BiomeDebugView,
    pub show_fog_contributors: bool,
    pub show_lod_tiers: bool,
    pub show_staging_queue: bool,
}

#[derive(Component)]
struct DebugPanelText;

#[derive(Resource, Default, Clone)]
struct DebugPanelLodStagingText {
    lod_line: String,
    staging_line: String,
}

pub struct DebugToolsPlugin;

impl Plugin for DebugToolsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugOverlayState>()
            .init_resource::<DebugPanelLodStagingText>()
            .add_plugins(WireframePlugin::default())
            .add_systems(OnEnter(AppState::Running), init_debug_bindings)
            .add_systems(Update, handle_debug_keys.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_chunk_bounds.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_colliders.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_vertex_normals.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_biome_labels.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_material_gizmos.run_if(in_state(AppState::Running)))
            .add_systems(Update, sync_terrain_debug_shader.run_if(in_state(AppState::Running)))
            .add_systems(Update, toggle_wireframe.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_camera_cast.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_residency_and_river.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_terrain_masks.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_island_atlas_fields.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_fog_contributors.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_semantic_landmarks.run_if(in_state(AppState::Running)))
            .add_systems(Update, draw_lod_tier_overlay.run_if(in_state(AppState::Running)))
            .add_systems(
                Update,
                (
                    sync_debug_panel_lod_staging,
                    update_debug_panel,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn sync_debug_panel_lod_staging(
    debug: Res<DebugOverlayState>,
    policy: Res<LodPolicy>,
    staging: Res<AssetStagingQueue>,
    gate: Res<StagingGate>,
    mut lines: ResMut<DebugPanelLodStagingText>,
) {
    lines.lod_line = if debug.show_lod_tiers {
        format!("LOD profile: {}\n", policy.render_profile_id.as_str())
    } else {
        String::new()
    };
    lines.staging_line = if debug.show_staging_queue {
        format!(
            "Staging: pending={} gate={}\n",
            staging.pending_count(),
            if gate.spawn_allowed { "open" } else { "closed" }
        )
    } else {
        String::new()
    };
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
    mut pending: ResMut<TerrainRegenPending>,
    mut edit_store: ResMut<TerrainEditStore>,
    mut runtime: ResMut<crate::terrain::TerrainWorldRuntime>,
    registry: Res<ConfigRegistryResource>,
    mut prefs: ResMut<crate::data::UserSetupPrefs>,
    terrain_tweaks: Res<TerrainTweaks>,
    ecology: Res<EcologyTweaks>,
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
        if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
            debug.biome_debug_view = cycle_biome_debug_view(debug.biome_debug_view);
        } else {
            debug.show_biomes = !debug.show_biomes;
        }
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
    if keyboard.just_pressed(KeyCode::F6) && keyboard.pressed(KeyCode::ControlLeft) {
        debug.island_field_view = cycle_island_field_view(&debug.island_field_view);
    }
    if ecology.show_wetness_heatmap {
        debug.biome_debug_view = BiomeDebugView::HeatmapGrayscale;
    }
    if keyboard.just_pressed(bindings.regen) {
        regen_terrain_with_seed(
            &mut commands,
            &registry,
            &prefs,
            &terrain_tweaks,
            &mut pipeline,
            &mut recipe_revision,
            &mut revision,
            &seed_override,
            &mut spawn_point,
            &mut pending,
            &mut edit_store,
            &mut runtime,
        );
    }
    if keyboard.just_pressed(bindings.next_seed) {
        seed_override.seed = seed_override.seed.wrapping_add(1);
        prefs.seed = seed_override.seed;
        regen_terrain_with_seed(
            &mut commands,
            &registry,
            &prefs,
            &terrain_tweaks,
            &mut pipeline,
            &mut recipe_revision,
            &mut revision,
            &seed_override,
            &mut spawn_point,
            &mut pending,
            &mut edit_store,
            &mut runtime,
        );
    }
    if keyboard.just_pressed(bindings.freeze_pipeline) {
        pipeline.frozen = !pipeline.frozen;
    }
    if keyboard.just_pressed(KeyCode::F7)
        && (keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight))
    {
        debug.show_staging_queue = !debug.show_staging_queue;
    } else if keyboard.just_pressed(KeyCode::F7) {
        debug.show_lod_tiers = !debug.show_lod_tiers;
    }
}

fn cycle_biome_debug_view(current: BiomeDebugView) -> BiomeDebugView {
    match current {
        BiomeDebugView::Normal => BiomeDebugView::ConstantGreen,
        BiomeDebugView::ConstantGreen => BiomeDebugView::HeatmapGrayscale,
        BiomeDebugView::HeatmapGrayscale => BiomeDebugView::DiscreteRegions,
        BiomeDebugView::DiscreteRegions => BiomeDebugView::Normal,
    }
}

fn sync_terrain_debug_shader(
    debug: Res<DebugOverlayState>,
    handle: Res<TerrainMaterialHandle>,
    mut materials: ResMut<Assets<TerrainPbrMaterial>>,
    mut last_mode: Local<Option<u32>>,
) {
    let mode = match debug.biome_debug_view {
        BiomeDebugView::ConstantGreen => 1u32,
        BiomeDebugView::DiscreteRegions => 1u32,
        _ => 0u32,
    };
    if last_mode.as_ref() == Some(&mode) {
        return;
    }
    *last_mode = Some(mode);
    let Some(mut material) = materials.get_mut(&handle.0) else {
        return;
    };
    material.settings.debug_mode = mode;
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
    if !debug.show_biomes && !debug.show_density && debug.biome_debug_view == BiomeDebugView::Normal {
        return;
    }
    let Some(source) = pipeline.density_source.as_ref() else {
        return;
    };
    for x in (-20..20).step_by(5) {
        for z in (-20..20).step_by(5) {
            let wx = x as f32;
            let wz = z as f32;
            let y = source.surface_height_at(wx, wz);
            let density = source.density_at(wx, y, wz);
            if debug.show_biomes || debug.biome_debug_view != BiomeDebugView::Normal {
                let ctx = BiomeSampleContext::sample(source, wx, y, wz);
                let color = match debug.biome_debug_view {
                    BiomeDebugView::HeatmapGrayscale => {
                        let v = biome_scalar_debug_value(&ctx);
                        Color::srgb(v, v, v)
                    }
                    BiomeDebugView::DiscreteRegions => {
                        biome_discrete_debug_color(biome_scalar_debug_value(&ctx))
                    }
                    _ => {
                        let biome = classify_biome(biomes.as_ref(), source, wx, y, wz, density);
                        biome_color(biomes.as_ref(), biome)
                    }
                };
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
    for x in (-20..20).step_by(4) {
        for z in (-20..20).step_by(4) {
            let wx = x as f32;
            let wz = z as f32;
            let y = source.surface_height_at(wx, wz);
            let density = source.density_at(wx, y, wz);
            let material = material_for_world(biomes.as_ref(), source, wx, y, wz, density);
            let color = assign_material_color(biomes.as_ref(), material.0);
            gizmos.cube(
                Transform::from_translation(Vec3::new(wx, y + 0.2, wz))
                    .with_scale(Vec3::new(0.35, 0.08, 0.35)),
                color,
            );
        }
    }
}

fn draw_camera_cast(
    debug: Res<DebugOverlayState>,
    cameras: Query<&MmoCamera, With<MainGameCamera>>,
    snapshot: Res<CameraDebugSnapshot>,
    mut gizmos: Gizmos,
) {
    if !debug.show_camera_cast {
        return;
    }
    let Ok(camera) = cameras.single() else {
        return;
    };
    let focus = camera.current_focus;
    let yaw = camera.current_yaw;
    let pitch = camera.current_pitch;
    let dir = Vec3::new(
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        yaw.cos() * pitch.cos(),
    )
    .normalize_or_zero();
    let end = focus + dir * camera.current_distance;
    gizmos.line(focus, end, Color::srgb(0.2, 0.8, 1.0));
    if let Some(hit) = snapshot.hit_position {
        gizmos.sphere(Isometry3d::from_translation(hit), 0.2, Color::srgb(1.0, 0.3, 0.2));
        gizmos.line(focus, hit, Color::srgb(1.0, 0.5, 0.2));
    }
}

fn update_debug_panel(
    debug: Res<DebugOverlayState>,
    pipeline: Res<TerrainPipelineState>,
    metrics: Res<TerrainPipelineMetrics>,
    lod_staging: Res<DebugPanelLodStagingText>,
    seed_override: Res<WorldSeedOverride>,
    pending: Res<TerrainRegenPending>,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    world_tweaks: Res<WorldTweaks>,
    semantic: Res<WorldSemanticRegistry>,
    runtime: Res<TerrainWorldRuntime>,
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
        .values()
        .filter(|c| c.state == crate::terrain::ChunkState::Ready)
        .count();
    let world_seed = effective_world_from_prefs(&registry.0, &prefs)
        .map(|w| w.seed)
        .unwrap_or(0);
    let fps = 1.0 / time.delta_secs().max(0.0001);
    let regen_line = if pending.pending {
        format!("Terrain regen PENDING (F8) hash={}", pending.recipe_hash)
    } else {
        String::from("Terrain regen: idle")
    };
    let budget_ok = metrics.within_vs_budget(ready.max(1));
    let landmark_lines = if world_tweaks.show_semantic_landmarks {
        let count = semantic.facts.len();
        let focus = chunk_world_center(runtime.interest_center);
        let nearest = semantic
            .nearest_fact_any(focus)
            .map(|(fact, dist)| format!("Nearest: {} ({dist:.0} m)", fact.label));
        match nearest {
            Some(line) => format!("Landmarks: {count}\n{line}\n"),
            None => format!("Landmarks: {count}\n"),
        }
    } else {
        String::new()
    };
    let staging_line = &lod_staging.staging_line;
    let lod_line = &lod_staging.lod_line;
    let body = format!(
        "Debug Panel\n\
         Seed: {} (override)\n\
         {regen_line}\n\
         {staging_line}\
         {lod_line}\
         Chunks ready: {ready}/{}\n\
         Pipeline budget: {}\n\
         Queues  D:{:?} M:{:?} U:{:?} C:{:?}\n\
         Last ms  density:{:.1} mesh:{:.1} upload:{:.1}\n\
         Colliders/frame: {}  Frozen: {}\n\
         Biome view: {:?} (Shift+F4 cycle)\n\
         {landmark_lines}\
         FPS: {fps:.0}\n\
         N=normals  F7=LOD tiers  Shift+F7=staging  F8=regen  F9=next seed",
        seed_override.seed,
        pipeline.chunks.len(),
        if budget_ok { "PASS" } else { "REVIEW" },
        pipeline.density_queue_len(),
        pipeline.mesh_queue_len(),
        pipeline.upload_queue_len(),
        pipeline.collider_queue_len(),
        metrics.last_density_ms,
        metrics.last_mesh_ms,
        metrics.last_upload_ms,
        metrics.colliders_built_this_frame,
        pipeline.frozen,
        debug.biome_debug_view,
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

fn draw_lod_tier_overlay(
    debug: Res<DebugOverlayState>,
    pipeline: Res<TerrainPipelineState>,
    mut gizmos: Gizmos,
) {
    if !debug.show_lod_tiers {
        return;
    }
    for chunk in pipeline.chunks.values() {
        if chunk.entity.is_none() {
            continue;
        }
        let center = chunk_world_center(chunk.coord);
        let color = match chunk.lod_tier {
            0 => Color::srgba(0.2, 1.0, 0.3, 0.35),
            1 => Color::srgba(1.0, 0.85, 0.2, 0.35),
            2 => Color::srgba(1.0, 0.35, 0.2, 0.35),
            _ => Color::srgba(0.5, 0.5, 0.5, 0.25),
        };
        gizmos.cube(
            Transform::from_translation(center + Vec3::Y * 8.0).with_scale(Vec3::splat(14.0)),
            color,
        );
    }
}

fn draw_residency_and_river(
    mut gizmos: Gizmos,
    world_tweaks: Res<WorldTweaks>,
    river_tweaks: Res<RiverTweaks>,
    runtime: Res<TerrainWorldRuntime>,
    features: Res<TerrainFeatureRegistry>,
) {
    if world_tweaks.show_residency_rings {
        draw_residency_rings(&mut gizmos, runtime.interest_center, &world_tweaks);
    }
    if river_tweaks.show_spline {
        if let Some(river) = features.rivers.get(&1) {
            for i in 0..river.points.len().saturating_sub(1) {
                let a = &river.points[i];
                let b = &river.points[i + 1];
                let from = Vec3::new(a.position_xz[0], a.water_elevation, a.position_xz[1]);
                let to = Vec3::new(b.position_xz[0], b.water_elevation, b.position_xz[1]);
                gizmos.line(from, to, Color::srgb(0.2, 0.6, 1.0));
                if river_tweaks.show_flow_arrows && i % 3 == 0 {
                    let mid = from.lerp(to, 0.5);
                    let dir = (to - from).normalize_or_zero();
                    gizmos.arrow(mid, mid + dir * 2.0, Color::srgb(0.9, 0.9, 0.3));
                }
            }
        }
    }
}

fn cycle_island_field_view(current: &str) -> String {
    const MODES: &[&str] = &[
        "",
        "elevation",
        "flow_accumulation",
        "river_mask",
        "beach_suitability",
        "cliff_suitability",
    ];
    let idx = MODES.iter().position(|m| *m == current).unwrap_or(0);
    MODES[(idx + 1) % MODES.len()].to_string()
}

fn draw_island_atlas_fields(
    debug: Res<DebugOverlayState>,
    pipeline: Res<TerrainPipelineState>,
    mut gizmos: Gizmos,
) {
    if debug.island_field_view.is_empty() {
        return;
    }
    let Some(source) = pipeline.density_source.as_ref() else {
        return;
    };
    let Some(atlas) = source.atlas() else {
        return;
    };
    let step = (atlas.width() / 32).max(1);
    let spacing = atlas.spacing_m();
    for z in (0..atlas.height()).step_by(step as usize) {
        for x in (0..atlas.width()).step_by(step as usize) {
            let wx = atlas.origin[0] + x as f32 * spacing;
            let wz = atlas.origin[1] + z as f32 * spacing;
            let v = match debug.island_field_view.as_str() {
                "flow_accumulation" => atlas.flow_accumulation.sample_bilinear(wx, wz) / 2000.0,
                "river_mask" => atlas.river_mask.sample_bilinear(wx, wz),
                "beach_suitability" => atlas.beach_mask.get(x, z),
                "cliff_suitability" => atlas.cliff_mask.get(x, z),
                "elevation_regional" => {
                    (atlas.elevation_regional.sample_bilinear(wx, wz) + 20.0) / 120.0
                }
                "elevation_local" | "elevation_local_residual" => {
                    (atlas.elevation_local.sample_bilinear(wx, wz) + 5.0) / 10.0
                }
                _ => (atlas.composed_land_elevation_at(wx, wz) + 20.0) / 120.0,
            };
            if v < 0.02 {
                continue;
            }
            gizmos.cube(
                Transform::from_translation(Vec3::new(wx, 0.2, wz))
                    .with_scale(Vec3::new(spacing, 0.05, spacing)),
                Color::srgba(0.2, 0.7, 0.9, v.clamp(0.0, 1.0) * 0.4),
            );
        }
    }
}

fn draw_terrain_masks(
    terrain_tweaks: Res<TerrainTweaks>,
    mut gizmos: Gizmos,
) {
    if !terrain_tweaks.show_masks {
        return;
    }
    let mask = build_coast_mask(32, 32, [-32.0, -32.0], 4.0);
    for z in (0..32).step_by(4) {
        for x in (0..32).step_by(4) {
            let wx = mask.origin[0] + x as f32 * mask.spacing;
            let wz = mask.origin[1] + z as f32 * mask.spacing;
            let v = mask.sample_bilinear(wx, wz);
            if v < 0.05 {
                continue;
            }
            gizmos.cube(
                Transform::from_translation(Vec3::new(wx, 0.15, wz))
                    .with_scale(Vec3::new(2.0, 0.05, 2.0)),
                Color::srgba(0.9, 0.5, 0.1, v * 0.35),
            );
        }
    }
}

fn draw_fog_contributors(
    debug: Res<DebugOverlayState>,
    fog_stack: Res<FogStack>,
    mut gizmos: Gizmos,
) {
    if !debug.show_fog_contributors {
        return;
    }
    if let Some(height) = &fog_stack.height {
        gizmos.cube(
            Transform::from_translation(Vec3::new(128.0, height.base_height, 128.0))
                .with_scale(Vec3::new(200.0, 2.0, 200.0)),
            Color::srgba(height.color[0], height.color[1], height.color[2], 0.12),
        );
    }
    for volume in &fog_stack.local_volumes {
        gizmos.cube(
            Transform::from_translation(volume.center).with_scale(volume.half_extents * 2.0),
            Color::srgba(volume.color[0], volume.color[1], volume.color[2], volume.density),
        );
    }
}

fn draw_semantic_landmarks(
    world_tweaks: Res<WorldTweaks>,
    semantic: Res<WorldSemanticRegistry>,
    runtime: Res<TerrainWorldRuntime>,
    mut gizmos: Gizmos,
) {
    if !world_tweaks.show_semantic_landmarks {
        return;
    }
    let focus = chunk_world_center(runtime.interest_center);
    let nearest = semantic.nearest_fact(focus, 200.0);
    for fact in &semantic.facts {
        let color = semantic_tag_color(fact.tag);
        let marker = fact.position + Vec3::Y * 0.5;
        let radius = if nearest.is_some_and(|n| n.label == fact.label && n.position == fact.position) {
            0.55
        } else {
            0.35
        };
        let stem_height = if fact.tag == WorldSemanticTag::Shelter { 2.0 } else { 1.5 };
        gizmos.sphere(Isometry3d::from_translation(marker), radius, color);
        gizmos.line(
            fact.position,
            marker + Vec3::Y * stem_height,
            color.with_alpha(0.85),
        );
    }
}
