// crates/game_bevy/src/terrain/horizon_skirt.rs
//! Low-detail horizon ring sampling the island atlas so chunk despawn has cover.

use bevy::prelude::*;
use terrain_generation::RecipeDensitySource;
use terrain_surface::SurfaceMeshResolver;
use voxel_core::CHUNK_CELLS;

use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;
use crate::environment::atmosphere::PlanetAtmosphereMedium;
use crate::environment::biome_context::ChunkColumnCache;
use crate::environment::surface::ChunkSurfaceResolver;
use crate::environment::BiomeCatalog;
use crate::state::AppState;
use crate::terrain::{
    insert_terrain_material_attributes, TerrainChunkPalette, TerrainEditStore,
    TerrainMaterialHandle, TerrainPipelineState, TerrainRegenPending, TerrainWorldRuntime,
};
use crate::ui::WorldTweaks;
use crate::world::requested_world_id;
use terrain_material_bevy::{TerrainPbrMaterial, TerrainProceduralMaterialState};

#[derive(Resource, Default)]
struct HorizonSkirtSpawned(bool);

#[derive(Component)]
pub struct HorizonSkirt;

pub struct HorizonSkirtPlugin;

impl Plugin for HorizonSkirtPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HorizonSkirtSpawned>()
            .add_systems(
                Update,
                (
                    despawn_horizon_skirt_on_reset,
                    rebuild_horizon_skirt_on_radius_change,
                    spawn_horizon_skirt_once,
                    refresh_horizon_skirt_material,
                    update_horizon_skirt_follow,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn despawn_horizon_skirt_on_reset(
    mut commands: Commands,
    mut pending: ResMut<TerrainRegenPending>,
    mut spawned: ResMut<HorizonSkirtSpawned>,
    skirts: Query<Entity, With<HorizonSkirt>>,
) {
    if !pending.horizon_skirt_reset {
        return;
    }
    pending.horizon_skirt_reset = false;
    spawned.0 = false;
    for entity in skirts.iter() {
        commands.entity(entity).despawn();
    }
}

fn rebuild_horizon_skirt_on_radius_change(
    world_tweaks: Res<WorldTweaks>,
    planet_atmosphere: Option<Res<PlanetAtmosphereMedium>>,
    mut spawned: ResMut<HorizonSkirtSpawned>,
    mut last_radius: Local<Option<i32>>,
    mut commands: Commands,
    skirts: Query<Entity, With<HorizonSkirt>>,
) {
    if planet_atmosphere.is_some() {
        return;
    }
    let radius = world_tweaks.render_radius;
    if last_radius.as_ref() == Some(&radius) {
        return;
    }
    if last_radius.is_some() {
        spawned.0 = false;
        for entity in skirts.iter() {
            commands.entity(entity).despawn();
        }
    }
    *last_radius = Some(radius);
}

fn spawn_horizon_skirt_once(
    mut commands: Commands,
    mut spawned: ResMut<HorizonSkirtSpawned>,
    planet_atmosphere: Option<Res<PlanetAtmosphereMedium>>,
    skirts: Query<Entity, With<HorizonSkirt>>,
    pipeline: Res<TerrainPipelineState>,
    runtime: Res<TerrainWorldRuntime>,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    biomes: Res<BiomeCatalog>,
    edit_store: Res<TerrainEditStore>,
    world_tweaks: Res<WorldTweaks>,
    terrain_state: Res<TerrainProceduralMaterialState>,
    material_handle: Res<TerrainMaterialHandle>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainPbrMaterial>>,
) {
    // Procedural atmosphere renders the sky only where the depth buffer is still clear
    // (depth == 0). The opaque horizon skirt fills those pixels at low camera pitch and
    // blocks the sky pass — the old inverted sky sphere avoided this via negative scale
    // on an inner surface; Bevy atmosphere needs unobstructed depth instead.
    if planet_atmosphere.is_some() {
        if spawned.0 {
            for entity in skirts.iter() {
                commands.entity(entity).despawn();
            }
            spawned.0 = false;
        }
        return;
    }

    if spawned.0 {
        return;
    }
    let Some(source) = pipeline.density_source.as_ref().map(|s| s.as_ref().clone()) else {
        return;
    };
    if source.atlas().is_none() {
        return;
    }
    if !terrain_state.ready {
        return;
    }

    let world_id = requested_world_id(&prefs);
    let Ok(world) = registry.0.effective_world(Some(&world_id)) else {
        return;
    };
    let Some(compiled_materials) = registry.0.materials.get(&world.materials) else {
        return;
    };
    let Some(compiled_surface) = registry.0.surface_rules.get(&world.surface) else {
        return;
    };

    let cell_size_m = runtime.cell_size_m;
    let chunk_m = cell_size_m * CHUNK_CELLS as f32;
    let center = runtime.interest_center;
    let center_wx = center.x as f32 * chunk_m + chunk_m * 0.5;
    let center_wz = center.z as f32 * chunk_m + chunk_m * 0.5;
    let origin_x = (center_wx / cell_size_m).floor() as i32;
    let origin_z = (center_wz / cell_size_m).floor() as i32;
    let radius_m = world_tweaks.render_radius as f32 * chunk_m + chunk_m * 0.5;
    let cache_side = (radius_m / cell_size_m).ceil() as usize + 6;
    let column_cache = ChunkColumnCache::build(&source, origin_x - 1, origin_z - 1, cache_side);
    let resolver = ChunkSurfaceResolver::from_compiled(
        source.clone(),
        column_cache,
        origin_x,
        0,
        origin_z,
        cell_size_m,
        edit_store.clone(),
        compiled_materials,
        compiled_surface,
        biomes.clone(),
    );

    let (mesh, palette) = build_skirt_mesh(
        &source,
        &resolver,
        center_wx,
        center_wz,
        cell_size_m,
        origin_x,
        origin_z,
        world_tweaks.render_radius,
    );
    let skirt_material = materials
        .get(&material_handle.0)
        .cloned()
        .map(|template| materials.add(template.with_chunk_palette(palette.0)))
        .unwrap_or_else(|| material_handle.0.clone());

    commands.spawn((
        HorizonSkirt,
        TerrainChunkPalette(palette.0),
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(skirt_material),
        Transform::from_xyz(center_wx, 0.0, center_wz),
        Visibility::default(),
    ));
    spawned.0 = true;
}

fn refresh_horizon_skirt_material(
    handle: Res<TerrainMaterialHandle>,
    state: Res<TerrainProceduralMaterialState>,
    mut materials: ResMut<Assets<TerrainPbrMaterial>>,
    mut last_fingerprint: Local<Option<[u8; 32]>>,
    mut skirts: Query<(&mut MeshMaterial3d<TerrainPbrMaterial>, &TerrainChunkPalette), With<HorizonSkirt>>,
) {
    if !state.ready {
        *last_fingerprint = None;
        return;
    }
    if last_fingerprint.as_ref() == Some(&state.recipe_fingerprint) {
        return;
    }
    *last_fingerprint = Some(state.recipe_fingerprint);
    let Some(template) = materials.get(&handle.0).cloned() else {
        return;
    };
    for (mut mat_handle, palette) in &mut skirts {
        let mut updated = template.with_chunk_palette(palette.0);
        updated.settings.debug_mode = 0;
        mat_handle.0 = materials.add(updated);
    }
}

fn update_horizon_skirt_follow(
    runtime: Res<TerrainWorldRuntime>,
    mut skirts: Query<&mut Transform, With<HorizonSkirt>>,
) {
    let center = runtime.interest_center;
    let chunk_m = runtime.cell_size_m * CHUNK_CELLS as f32;
    let world = Vec3::new(
        center.x as f32 * chunk_m + chunk_m * 0.5,
        0.0,
        center.z as f32 * chunk_m + chunk_m * 0.5,
    );
    for mut transform in &mut skirts {
        transform.translation = world;
    }
}

fn build_skirt_mesh(
    source: &RecipeDensitySource,
    resolver: &ChunkSurfaceResolver,
    center_wx: f32,
    center_wz: f32,
    cell_size_m: f32,
    origin_x: i32,
    origin_z: i32,
    render_radius: i32,
) -> (Mesh, TerrainChunkPalette) {
    let chunk_m = CHUNK_CELLS as f32 * cell_size_m;
    let radius_m = render_radius as f32 * chunk_m + chunk_m * 0.5;
    let segments = 64usize;
    let mut positions = Vec::with_capacity(segments * 2);
    let mut normals = Vec::with_capacity(segments * 2);
    let mut material_vertices = Vec::with_capacity(segments * 2);

    let center_y = source
        .atlas()
        .map(|a| a.sea_level_m)
        .unwrap_or(0.0);

    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        let angle = t * std::f32::consts::TAU;
        let lx = angle.cos() * radius_m;
        let lz = angle.sin() * radius_m;
        let wx = center_wx + lx;
        let wz = center_wz + lz;
        let top_y = if let Some(atlas) = source.atlas() {
            atlas.surface_height_at(wx, wz).max(center_y - 12.0)
        } else {
            source
                .terrain_surface_height_at(wx, wz)
                .max(center_y - 12.0)
        };
        let bottom_y = center_y - 18.0;
        let top_normal = [angle.cos(), 0.15, angle.sin()];
        let bottom_normal = [angle.cos(), -0.2, angle.sin()];

        for (y, normal) in [(top_y, top_normal), (bottom_y, bottom_normal)] {
            positions.push([lx, y, lz]);
            normals.push(normal);
            let local = [
                wx / cell_size_m - origin_x as f32,
                y / cell_size_m,
                wz / cell_size_m - origin_z as f32,
            ];
            material_vertices.push(resolver.vertex_blend(local, normal));
        }
    }

    let mut indices = Vec::with_capacity(segments * 6);
    for i in 0..segments {
        let i0 = (i * 2) as u32;
        let i1 = i0 + 1;
        let i2 = i0 + 2;
        let i3 = i0 + 3;
        indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
    }

    let palette = TerrainChunkPalette(resolver.chunk_palette());
    let mut mesh = Mesh::new(
        bevy::render::render_resource::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    insert_terrain_material_attributes(&mut mesh, &material_vertices);
    mesh.insert_indices(bevy::render::mesh::Indices::U32(indices));
    (mesh, palette)
}
