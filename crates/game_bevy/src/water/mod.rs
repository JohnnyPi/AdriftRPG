// crates/game_bevy/src/water/mod.rs
use bevy::prelude::*;
use bevy::pbr::Material;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use crate::data::ConfigRegistryResource;
use crate::state::AppState;
use crate::terrain::{TerrainFeatureRegistry, TerrainPipelineState, TerrainWorldInitSet};
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
}

#[derive(Component)]
pub struct WaterSurface;

#[derive(Component)]
pub struct OceanSurface;

#[derive(Component)]
pub struct RiverWaterSurface;

pub struct WaterPlugin;

/// VS2 §20 render-side water surfaces.
#[allow(dead_code)]
pub type WaterRenderingPlugin = WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<WaterMaterial>::default())
            .add_systems(OnEnter(AppState::Running), spawn_water_bodies.after(TerrainWorldInitSet))
            .add_systems(
                Update,
                (
                    animate_water,
                    sync_ocean_plane_with_profile,
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
    tweaks: Res<WaterTweaks>,
    prefs: Res<UserSetupPrefs>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
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

    let ocean_extent = registry.0.world_ocean_extent_m(world);

    let sea_mat = make_water_material(
        &mut materials,
        water_def,
        registry.0.water_body_material(&shared::StableId::new("waterbody.sea")),
        sea_level,
        &tweaks,
    );
    commands.spawn((
        WaterSurface,
        OceanSurface,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(ocean_extent, ocean_extent))),
        MeshMaterial3d(sea_mat),
        Transform::from_xyz(0.0, sea_level + 0.02, 0.0),
    ));

    let pool_elevation = if tweaks.use_overrides {
        tweaks.pool_elevation_m
    } else {
        registry
            .0
            .upland_pool_hydrology()
            .map(|h| h.elevation_m)
            .unwrap_or(31.5)
    };
    let pool_world = world.recipe_to_world([82.0, 0.0, 196.0]);
    let pool_mat = make_water_material(
        &mut materials,
        water_def,
        registry.0.water_body_material(&shared::StableId::new("waterbody.upland_pool")),
        pool_elevation,
        &tweaks,
    );
    commands.spawn((
        WaterSurface,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(48.0, 48.0))),
        MeshMaterial3d(pool_mat),
        Transform::from_xyz(pool_world[0], pool_elevation + 0.02, pool_world[2]),
    ));

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

    for body in features.water_bodies.values() {
        let _ = body;
    }
}

fn sync_ocean_plane_with_profile(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    tweaks: Res<WaterTweaks>,
    mut ocean: Query<(&mut Transform, &mut Mesh3d), (With<WaterSurface>, With<OceanSurface>)>,
    mut meshes: ResMut<Assets<Mesh>>,
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
    let ocean_extent = registry.0.world_ocean_extent_m(world);

    for (mut transform, mesh) in &mut ocean {
        transform.translation = Vec3::new(0.0, sea_level + 0.02, 0.0);
        if let Some(mut mesh_asset) = meshes.get_mut(&mesh.0) {
            *mesh_asset = Plane3d::default().mesh().size(ocean_extent, ocean_extent).into();
        }
    }
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
    registry: Res<ConfigRegistryResource>,
    mut water: Query<&mut MeshMaterial3d<WaterMaterial>>,
    mut materials: ResMut<Assets<WaterMaterial>>,
) {
    let Ok(world) = registry.0.active_world() else {
        return;
    };
    let Some(water_def) = registry.0.water.get(&world.water) else {
        return;
    };
    for mat_handle in &mut water {
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            mat.params.animation.x = time.elapsed_secs();
            mat.params.wave.y = water_def.wave_speed;
            mat.params.wave.z = water_def.wave_amplitude;
        }
    }
}
