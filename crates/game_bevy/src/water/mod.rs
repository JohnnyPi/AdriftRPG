// crates/game_bevy/src/water/mod.rs
use bevy::prelude::*;
use bevy::pbr::Material;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use std::collections::HashSet;
use terrain_generation::water_body::{
    HorizontalFootprint, WaterBodyId, WaterBodyKind, WaterSurfaceDefinition,
};

use crate::data::ConfigRegistryResource;
use crate::state::AppState;
use crate::terrain::{TerrainFeatureRegistry, TerrainPipelineState, TerrainWorldInitSet, TerrainWorldRuntime};
use crate::ui::WaterTweaks;
use crate::data::UserSetupPrefs;
use crate::world::requested_world_id;

#[derive(ShaderType, Clone, Copy, Debug)]
pub struct WaterParams {
    pub shallow_color: Vec4,
    pub deep_color: Vec4,
    pub wave: Vec4,
    pub animation: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct WaterMaterial {
    #[uniform(0)]
    pub params: WaterParams,
}

impl Material for WaterMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/water.wgsl".into()
    }

    // Water must render in the transparent pass: as an opaque material the
    // plane wrote flat unlit-looking color over everything at sea level and
    // hid the terrain shelf below it.
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
}

#[derive(Component)]
pub struct WaterSurface;

#[derive(Component)]
pub struct OceanSurface;

#[derive(Component)]
struct OceanTile {
    #[allow(dead_code)]
    grid_x: i32,
    #[allow(dead_code)]
    grid_z: i32,
}

const OCEAN_TILE_SIZE_M: f32 = 256.0;
const OCEAN_TILE_RADIUS: i32 = 1;

#[derive(Resource, Default)]
struct OceanTileGrid {
    snap_x: i32,
    snap_z: i32,
    sea_level: f32,
    initialized: bool,
}

#[derive(Component)]
pub struct RiverWaterSurface;

#[derive(Component)]
pub struct LakeWaterSurface {
    pub body_id: u32,
}

#[derive(Resource, Default)]
struct InlandWaterSync {
    hydrology_epoch: u32,
    spawned: bool,
}

pub struct WaterPlugin;

/// VS2 §20 render-side water surfaces.
#[allow(dead_code)]
pub type WaterRenderingPlugin = WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<WaterMaterial>::default())
            .init_resource::<InlandWaterSync>()
            .init_resource::<OceanTileGrid>()
            .add_systems(OnEnter(AppState::Running), spawn_water_bodies.after(TerrainWorldInitSet))
            .add_systems(
                Update,
                (
                    animate_water,
                    sync_ocean_tiles,
                    sync_ocean_plane_with_profile,
                    sync_inland_lakes_on_hydrology_change,
                    sync_lake_surface_transforms,
                )
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn spawn_water_bodies(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    features: Res<TerrainFeatureRegistry>,
    pipeline: Res<TerrainPipelineState>,
    runtime: Res<TerrainWorldRuntime>,
    tweaks: Res<WaterTweaks>,
    prefs: Res<UserSetupPrefs>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    mut inland_sync: ResMut<InlandWaterSync>,
    mut ocean_grid: ResMut<OceanTileGrid>,
) {
    let world_id = requested_world_id(&prefs);
    let world = registry
        .0
        .effective_world(Some(&world_id))
        .expect("world");
    let water_def = registry.0.water.get(&world.water).expect("water");
    let sea_level = if tweaks.use_overrides {
        tweaks.sea_level_m
    } else {
        water_def.sea_level_m
    };

    let sea_mat = make_water_material(
        &mut materials,
        water_def,
        registry.0.water_body_material(&shared::StableId::new("waterbody.sea")),
        sea_level,
        &tweaks,
    );
    let tile_mesh = meshes.add(Plane3d::default().mesh().size(OCEAN_TILE_SIZE_M, OCEAN_TILE_SIZE_M));
    let (snap_x, snap_z) = ocean_snap_indices(&runtime);
    spawn_ocean_tile_grid(
        &mut commands,
        snap_x,
        snap_z,
        sea_level,
        tile_mesh,
        sea_mat.clone(),
    );
    ocean_grid.snap_x = snap_x;
    ocean_grid.snap_z = snap_z;
    ocean_grid.sea_level = sea_level;
    ocean_grid.initialized = true;

    let river_spline = pipeline
        .density_source
        .as_ref()
        .and_then(|s| s.atlas())
        .and_then(|a| a.river_graph.clone())
        .or_else(|| features.rivers.get(&1).cloned());

    if let Some(river) = river_spline {
        if let Some(mesh) = build_river_ribbon_mesh(&river) {
            let river_mat = make_water_material(
                &mut materials,
                water_def,
                registry.0.water_body_material(&shared::StableId::new("waterbody.river")),
                sea_level,
                &tweaks,
            );
            commands.spawn((
                RiverWaterSurface,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(river_mat),
                Transform::IDENTITY,
            ));
        }
    }

    spawn_inland_lake_surfaces(
        &mut commands,
        &registry,
        &features,
        water_def,
        &tweaks,
        &mut meshes,
        &mut materials,
        &HashSet::new(),
    );
    inland_sync.hydrology_epoch = features.hydrology_epoch;
    inland_sync.spawned = true;
}

fn sync_inland_lakes_on_hydrology_change(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    features: Res<TerrainFeatureRegistry>,
    tweaks: Res<WaterTweaks>,
    prefs: Res<UserSetupPrefs>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    mut inland_sync: ResMut<InlandWaterSync>,
    lakes: Query<(Entity, &LakeWaterSurface)>,
) {
    let current_ids: HashSet<u32> = features.water_bodies.values().map(|body| body.id.0).collect();
    let mut existing_ids = HashSet::new();
    for (entity, lake) in &lakes {
        if current_ids.contains(&lake.body_id) {
            existing_ids.insert(lake.body_id);
        } else {
            commands.entity(entity).despawn();
        }
    }

    let needs_spawn =
        !inland_sync.spawned || inland_sync.hydrology_epoch != features.hydrology_epoch;
    if !needs_spawn {
        return;
    }

    let world_id = requested_world_id(&prefs);
    let world = registry
        .0
        .effective_world(Some(&world_id))
        .expect("world");
    let water_def = registry.0.water.get(&world.water).expect("water");
    spawn_inland_lake_surfaces(
        &mut commands,
        &registry,
        &features,
        water_def,
        &tweaks,
        &mut meshes,
        &mut materials,
        &existing_ids,
    );
    inland_sync.hydrology_epoch = features.hydrology_epoch;
    inland_sync.spawned = true;
}

fn sync_lake_surface_transforms(
    features: Res<TerrainFeatureRegistry>,
    mut lakes: Query<(&LakeWaterSurface, &mut Transform, &mut Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (lake, mut transform, mesh) in &mut lakes {
        let Some(body) = features.water_bodies.get(&WaterBodyId(lake.body_id)) else {
            continue;
        };
        let WaterSurfaceDefinition::Horizontal {
            elevation,
            footprint: Some(HorizontalFootprint::Disc {
                center_xz,
                radius_m,
            }),
        } = &body.surface
        else {
            continue;
        };
        transform.translation = Vec3::new(center_xz[0], *elevation + 0.02, center_xz[1]);
        let diameter = radius_m.max(1.0) * 2.0;
        if let Some(mut mesh_asset) = meshes.get_mut(&mesh.0) {
            *mesh_asset = Plane3d::default().mesh().size(diameter, diameter).into();
        }
    }
}

fn spawn_inland_lake_surfaces(
    commands: &mut Commands,
    registry: &ConfigRegistryResource,
    features: &TerrainFeatureRegistry,
    water_def: &game_data::CompiledWater,
    tweaks: &WaterTweaks,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<WaterMaterial>,
    existing_ids: &HashSet<u32>,
) {
    for body in features.water_bodies.values() {
        if existing_ids.contains(&body.id.0) {
            continue;
        }
        let WaterSurfaceDefinition::Horizontal {
            elevation,
            footprint: Some(HorizontalFootprint::Disc {
                center_xz,
                radius_m,
            }),
        } = &body.surface
        else {
            continue;
        };
        if !matches!(
            body.kind,
            WaterBodyKind::Lake | WaterBodyKind::Pond | WaterBodyKind::Spring | WaterBodyKind::CavePool
        ) {
            continue;
        }
        let diameter = radius_m.max(1.0) * 2.0;
        let material_key = if body.material_id.as_str().starts_with("waterbody.") {
            body.material_id.clone()
        } else {
            shared::StableId::new("waterbody.upland_pool")
        };
        let lake_mat = make_water_material(
            materials,
            water_def,
            registry.0.water_body_material(&material_key),
            *elevation,
            tweaks,
        );
        commands.spawn((
            LakeWaterSurface {
                body_id: body.id.0,
            },
            Mesh3d(meshes.add(Plane3d::default().mesh().size(diameter, diameter))),
            MeshMaterial3d(lake_mat),
            Transform::from_xyz(center_xz[0], *elevation + 0.02, center_xz[1]),
        ));
    }
}

fn ocean_snap_indices(runtime: &TerrainWorldRuntime) -> (i32, i32) {
    let center = runtime.interest_center;
    let chunk_m = runtime.cell_size_m * voxel_core::CHUNK_CELLS as f32;
    let world_x = center.x as f32 * chunk_m + chunk_m * 0.5;
    let world_z = center.z as f32 * chunk_m + chunk_m * 0.5;
    let snap_x = (world_x / OCEAN_TILE_SIZE_M).floor() as i32;
    let snap_z = (world_z / OCEAN_TILE_SIZE_M).floor() as i32;
    (snap_x, snap_z)
}

fn spawn_ocean_tile_grid(
    commands: &mut Commands,
    snap_x: i32,
    snap_z: i32,
    sea_level: f32,
    tile_mesh: Handle<Mesh>,
    sea_mat: Handle<WaterMaterial>,
) {
    for dz in -OCEAN_TILE_RADIUS..=OCEAN_TILE_RADIUS {
        for dx in -OCEAN_TILE_RADIUS..=OCEAN_TILE_RADIUS {
            let grid_x = snap_x + dx;
            let grid_z = snap_z + dz;
            let x = grid_x as f32 * OCEAN_TILE_SIZE_M + OCEAN_TILE_SIZE_M * 0.5;
            let z = grid_z as f32 * OCEAN_TILE_SIZE_M + OCEAN_TILE_SIZE_M * 0.5;
            commands.spawn((
                WaterSurface,
                OceanSurface,
                OceanTile {
                    grid_x,
                    grid_z,
                },
                Mesh3d(tile_mesh.clone()),
                MeshMaterial3d(sea_mat.clone()),
                Transform::from_xyz(x, sea_level + 0.02, z),
            ));
        }
    }
}

fn sync_ocean_tiles(
    mut commands: Commands,
    runtime: Res<TerrainWorldRuntime>,
    tweaks: Res<WaterTweaks>,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    mut ocean_grid: ResMut<OceanTileGrid>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
    tiles: Query<(Entity, &OceanTile)>,
    mut transforms: Query<&mut Transform, With<OceanSurface>>,
) {
    if !ocean_grid.initialized {
        return;
    }
    let world_id = requested_world_id(&prefs);
    let world = registry
        .0
        .effective_world(Some(&world_id))
        .expect("world");
    let water_def = registry.0.water.get(&world.water).expect("water");
    let sea_level = if tweaks.use_overrides {
        tweaks.sea_level_m
    } else {
        water_def.sea_level_m
    };
    if (sea_level - ocean_grid.sea_level).abs() > 0.001 {
        ocean_grid.sea_level = sea_level;
        for mut transform in &mut transforms {
            transform.translation.y = sea_level + 0.02;
        }
    }

    let (snap_x, snap_z) = ocean_snap_indices(&runtime);
    if snap_x == ocean_grid.snap_x && snap_z == ocean_grid.snap_z {
        return;
    }
    ocean_grid.snap_x = snap_x;
    ocean_grid.snap_z = snap_z;

    for (entity, _) in &tiles {
        commands.entity(entity).despawn();
    }

    let sea_mat = make_water_material(
        &mut materials,
        water_def,
        registry.0.water_body_material(&shared::StableId::new("waterbody.sea")),
        sea_level,
        &tweaks,
    );
    let tile_mesh = meshes.add(Plane3d::default().mesh().size(OCEAN_TILE_SIZE_M, OCEAN_TILE_SIZE_M));
    spawn_ocean_tile_grid(
        &mut commands,
        snap_x,
        snap_z,
        sea_level,
        tile_mesh,
        sea_mat,
    );
}

fn sync_ocean_plane_with_profile(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    tweaks: Res<WaterTweaks>,
    mut ocean_grid: ResMut<OceanTileGrid>,
    mut ocean: Query<&mut Transform, (With<WaterSurface>, With<OceanSurface>)>,
    mut last: Local<Option<String>>,
) {
    let Some(ref previous) = *last else {
        *last = Some(prefs.world_id.clone());
        return;
    };
    if *previous == prefs.world_id {
        return;
    }
    *last = Some(prefs.world_id.clone());

    let world_id = requested_world_id(&prefs);
    let world = registry
        .0
        .effective_world(Some(&world_id))
        .expect("world");
    let water_def = registry.0.water.get(&world.water).expect("water");
    let sea_level = if tweaks.use_overrides {
        tweaks.sea_level_m
    } else {
        water_def.sea_level_m
    };

    for mut transform in &mut ocean {
        transform.translation.y = sea_level + 0.02;
    }
    ocean_grid.initialized = false;
}

fn make_water_material(
    materials: &mut Assets<WaterMaterial>,
    water_def: &game_data::CompiledWater,
    body_material: Option<&game_data::CompiledWaterBodyMaterial>,
    elevation: f32,
    tweaks: &WaterTweaks,
) -> Handle<WaterMaterial> {
    let mut shallow = body_material
        .map(|m| m.shallow_color)
        .unwrap_or(water_def.shallow_color);
    let mut deep = body_material
        .map(|m| m.deep_color)
        .unwrap_or(water_def.deep_color);
    if tweaks.use_overrides {
        shallow = tweaks.shallow_color;
        deep = tweaks.deep_color;
    }
    let transparency = body_material
        .map(|m| m.transparency)
        .unwrap_or(water_def.transparency);
    let wave_speed = body_material
        .map(|m| m.wave_speed)
        .unwrap_or(water_def.wave_speed);
    let wave_amplitude = body_material
        .map(|m| m.wave_amplitude)
        .unwrap_or(water_def.wave_amplitude);
    materials.add(WaterMaterial {
        params: WaterParams {
            shallow_color: Vec4::new(shallow[0], shallow[1], shallow[2], transparency),
            deep_color: Vec4::new(deep[0], deep[1], deep[2], 1.0),
            wave: Vec4::new(
                elevation,
                wave_speed,
                wave_amplitude * 0.6,
                transparency,
            ),
            animation: Vec4::ZERO,
        },
    })
}

fn build_river_ribbon_mesh(
    river: &terrain_generation::RiverSpline,
) -> Option<Mesh> {
    if river.points.len() < 2 {
        return None;
    }
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for (i, pt) in river.points.iter().enumerate() {
        let half_w = pt.width * 0.5;
        let y = pt.water_elevation;
        let x = pt.position_xz[0];
        let z = pt.position_xz[1];
        let tangent = if i + 1 < river.points.len() {
            let n = &river.points[i + 1];
            Vec2::new(n.position_xz[0] - x, n.position_xz[1] - z).normalize_or_zero()
        } else {
            Vec2::new(1.0, 0.0)
        };
        let perp = Vec2::new(-tangent.y, tangent.x);
        let left = Vec3::new(x + perp.x * half_w, y, z + perp.y * half_w);
        let right = Vec3::new(x - perp.x * half_w, y, z - perp.y * half_w);
        positions.push(left.to_array());
        positions.push(right.to_array());
        normals.push([0.0, 1.0, 0.0]);
        normals.push([0.0, 1.0, 0.0]);
        let t = i as f32 / river.points.len() as f32;
        uvs.push([t, 0.0]);
        uvs.push([t, 1.0]);
        if i > 0 {
            let base = (i * 2) as u32;
            indices.extend([base - 2, base - 1, base, base - 1, base + 1, base]);
        }
    }

    let front_count = indices.len();
    for i in (0..front_count).step_by(3) {
        indices.push(indices[i]);
        indices.push(indices[i + 2]);
        indices.push(indices[i + 1]);
    }

    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(bevy::mesh::Indices::U32(indices));
    Some(mesh)
}

fn animate_water(
    time: Res<Time>,
    mut water: Query<&mut MeshMaterial3d<WaterMaterial>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    for mat_handle in &mut water {
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            mat.params.animation.x = time.elapsed_secs();
        }
    }
}