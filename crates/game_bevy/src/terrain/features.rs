// crates/game_bevy/src/terrain/features.rs
//! Terrain feature registry — rivers and water bodies (VS2 §19).
//!
//! Hydrology for the atlas island worlds consists of the global sea plus the
//! atlas-generated river (when the island produced one). The legacy demo
//! hydrology (`WaterBodyRegistry::demo_registry` + `demo_river.yaml`) was
//! removed with the op-based slice worlds: its "upland pool" was an
//! *unbounded* horizontal body at 31.5 m, which made `water_at` report every
//! point on the island below that elevation as deep water — driving swim
//! physics, submerged speed scaling, and the water floor clamp on dry land.

use bevy::prelude::*;
use shared::StableId;
use std::collections::BTreeMap;
use terrain_generation::water_body::{HorizontalFootprint, WaterBodyKind, WaterSurfaceDefinition};
use terrain_generation::{
    RiverHydrology, RiverSpline, WaterBody, WaterBodyId, WaterBodyRegistry, WaterQuery,
};

/// Axis-aligned bounds of the primary river spline with flow-radius padding.
#[derive(Clone, Copy, Debug)]
pub struct RiverFlowBounds {
    pub min_x: f32,
    pub min_z: f32,
    pub max_x: f32,
    pub max_z: f32,
    pub segment_count: usize,
}

impl RiverFlowBounds {
    pub fn contains_xz(&self, x: f32, z: f32) -> bool {
        x >= self.min_x && x <= self.max_x && z >= self.min_z && z <= self.max_z
    }

    pub fn clamp_segment_hint(&self, hint: usize) -> usize {
        hint.min(self.segment_count.saturating_sub(1))
    }
}

pub fn river_flow_bounds_from_spline(river: &RiverSpline, flow_radius_m: f32) -> RiverFlowBounds {
    let pad = flow_radius_m.max(0.0);
    let mut min_x = f32::MAX;
    let mut min_z = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_z = f32::MIN;
    for point in &river.points {
        min_x = min_x.min(point.position_xz[0]);
        min_z = min_z.min(point.position_xz[1]);
        max_x = max_x.max(point.position_xz[0]);
        max_z = max_z.max(point.position_xz[1]);
    }
    if river.points.is_empty() {
        return RiverFlowBounds {
            min_x: 0.0,
            min_z: 0.0,
            max_x: 0.0,
            max_z: 0.0,
            segment_count: 0,
        };
    }
    RiverFlowBounds {
        min_x: min_x - pad,
        min_z: min_z - pad,
        max_x: max_x + pad,
        max_z: max_z + pad,
        segment_count: river.points.len().saturating_sub(1),
    }
}

use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;
use crate::state::AppState;
use crate::terrain::{
    TerrainPipelineState, TerrainRecipeRevision, TerrainRegenPending, TerrainRevision,
    TerrainWorldInitSet,
};
use crate::ui::{TerrainTweaks, WaterTweaks};
use crate::world::requested_world_id;
use game_data::CompiledHydrologyBody;

#[derive(Resource, Clone, Debug, Default)]
pub struct TerrainFeatureRegistry {
    pub rivers: BTreeMap<u32, RiverSpline>,
    pub water_bodies: BTreeMap<WaterBodyId, WaterBody>,
    pub hydrology: Option<RiverHydrology>,
    pub river_flow_bounds: Option<RiverFlowBounds>,
    /// Bumped whenever hydrology is rebuilt; invalidates per-entity river caches.
    pub hydrology_epoch: u32,
}

impl TerrainFeatureRegistry {
    pub fn water_registry(&self) -> Option<&WaterBodyRegistry> {
        self.hydrology.as_ref().map(|h| &h.water)
    }
}

#[derive(Resource, Clone, Debug, Default)]
pub struct CameraWaterState {
    pub body: Option<WaterBodyId>,
    pub submerged_depth: f32,
}

pub struct TerrainFeaturePlugin;

impl Plugin for TerrainFeaturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainFeatureRegistry>()
            .init_resource::<CameraWaterState>()
            .add_systems(
                OnEnter(AppState::Running),
                init_terrain_features.after(TerrainWorldInitSet),
            )
            .add_systems(
                Update,
                (sync_hydrology_from_tweaks, update_camera_water_state)
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

#[derive(Clone, PartialEq, Debug)]
struct HydrologyTweakKey {
    ridge: f32,
    valley: f32,
    terrain_overrides: bool,
    water_sea_level: f32,
    water_overrides: bool,
    terrain_revision: u64,
    world_seed: u64,
    recipe_hash: String,
}

/// Effective sea level for hydrology and physics: options-panel water override,
/// then setup-screen island override, then the world's water definition.
pub fn effective_runtime_sea_level_m(
    registry: &ConfigRegistryResource,
    prefs: &UserSetupPrefs,
    water_tweaks: &WaterTweaks,
) -> f32 {
    if water_tweaks.use_overrides {
        return water_tweaks.sea_level_m;
    }
    let world_id = requested_world_id(prefs);
    let world = registry.0.world_by_id(&world_id).expect("world profile");
    let water = registry.0.water.get(&world.water).expect("water");
    registry
        .0
        .island_generation_for_world(world)
        .map(|base| prefs.apply_overrides(base).island.sea_level_m)
        .unwrap_or(water.sea_level_m)
}

/// Sea level of the requested world's water definition.
fn world_sea_level(
    registry: &ConfigRegistryResource,
    prefs: &UserSetupPrefs,
    water_tweaks: &WaterTweaks,
) -> f32 {
    effective_runtime_sea_level_m(registry, prefs, water_tweaks)
}

/// The island atlas river, if the generated island produced one. River
/// control points are already in world space.
fn atlas_river(pipeline: &TerrainPipelineState) -> Option<RiverSpline> {
    pipeline
        .density_source
        .as_ref()?
        .atlas()?
        .river_graph
        .clone()
}

/// River from atlas or legacy op-world river carve context.
fn pipeline_river(pipeline: &TerrainPipelineState) -> Option<RiverSpline> {
    if let Some(river) = atlas_river(pipeline) {
        return Some(river);
    }
    let source = pipeline.density_source.as_ref()?;
    let carve = source.river_carve()?;
    Some(carve.spline.clone())
}

fn world_coord_offset(registry: &ConfigRegistryResource, prefs: &UserSetupPrefs) -> [f32; 3] {
    let world_id = requested_world_id(prefs);
    registry
        .0
        .effective_world(Some(&world_id))
        .map(|world| world.coord_offset)
        .unwrap_or([0.0, 0.0, 0.0])
}

/// Build hydrology for an atlas island world: the global sea plus the
/// generated river (when present).
///
/// Note: the sea is an unbounded horizontal body, so carved volumes below
/// sea level (e.g. deep cave chambers on low flanks) register as flooded.
/// That is the intended global-ocean assumption for now.
pub fn build_hydrology(
    sea_level: f32,
    river: Option<RiverSpline>,
    lakes: &[CompiledHydrologyBody],
    coord_offset: [f32; 3],
) -> RiverHydrology {
    let mut bodies = BTreeMap::new();
    bodies.insert(
        WaterBodyId(1),
        WaterBody {
            id: WaterBodyId(1),
            stable_id: StableId::new("water.sea"),
            kind: WaterBodyKind::Sea,
            surface: WaterSurfaceDefinition::Horizontal {
                elevation: sea_level,
                footprint: None,
            },
            material_id: StableId::new("water.tropical_shallow"),
        },
    );
    let mut next_id = 2u32;
    for lake in lakes {
        let Some(center) = lake.center else {
            continue;
        };
        let Some(radius_m) = lake.radius_m else {
            continue;
        };
        if next_id == 3 {
            next_id = 4;
        }
        let kind = match lake.kind.as_str() {
            "pond" => WaterBodyKind::Pond,
            "spring" => WaterBodyKind::Spring,
            "cave_pool" => WaterBodyKind::CavePool,
            _ => WaterBodyKind::Lake,
        };
        let suffix = lake
            .id
            .as_str()
            .strip_prefix("hydrology.")
            .unwrap_or(lake.id.as_str());
        let world_center = [center[0] - coord_offset[0], center[1] - coord_offset[2]];
        bodies.insert(
            WaterBodyId(next_id),
            WaterBody {
                id: WaterBodyId(next_id),
                stable_id: StableId::new(&format!("water.{suffix}")),
                kind,
                surface: WaterSurfaceDefinition::Horizontal {
                    elevation: lake.elevation_m,
                    footprint: Some(HorizontalFootprint::Disc {
                        center_xz: world_center,
                        radius_m,
                    }),
                },
                material_id: StableId::new(&format!("waterbody.{suffix}")),
            },
        );
        next_id += 1;
    }
    if let Some(ref spline) = river {
        bodies.insert(
            WaterBodyId(3),
            WaterBody {
                id: WaterBodyId(3),
                stable_id: StableId::new("water.river.island"),
                kind: WaterBodyKind::River,
                surface: WaterSurfaceDefinition::SplineRibbon {
                    control_points: spline.points.clone(),
                },
                material_id: StableId::new("water.river"),
            },
        );
    }
    RiverHydrology {
        river,
        water: WaterBodyRegistry {
            bodies,
            sea_level_m: sea_level,
        },
    }
}

fn authored_lakes(
    registry: &ConfigRegistryResource,
    prefs: &UserSetupPrefs,
) -> Vec<CompiledHydrologyBody> {
    let world_id = requested_world_id(prefs);
    let Ok(world) = registry.0.effective_world(Some(&world_id)) else {
        return Vec::new();
    };
    world
        .hydrology_bodies
        .iter()
        .filter_map(|id| registry.0.hydrology.get(id).cloned())
        .collect()
}

fn apply_hydrology(features: &mut TerrainFeatureRegistry, hydrology: RiverHydrology) {
    if let Some(river) = hydrology.river.clone() {
        features.rivers.insert(1, river.clone());
        features.river_flow_bounds = Some(river_flow_bounds_from_spline(&river, 6.0));
    } else {
        features.rivers.remove(&1);
        features.river_flow_bounds = None;
    }
    features.water_bodies = hydrology.water.bodies.clone();
    features.hydrology = Some(hydrology);
    features.hydrology_epoch = features.hydrology_epoch.wrapping_add(1);
}

fn init_terrain_features(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    pipeline: Res<TerrainPipelineState>,
    water_tweaks: Res<WaterTweaks>,
    mut features: ResMut<TerrainFeatureRegistry>,
) {
    let sea = world_sea_level(&registry, &prefs, &water_tweaks);
    let offset = world_coord_offset(&registry, &prefs);
    let hydrology = build_hydrology(
        sea,
        pipeline_river(&pipeline),
        &authored_lakes(&registry, &prefs),
        offset,
    );
    apply_hydrology(&mut features, hydrology);
}

fn sync_hydrology_from_tweaks(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    terrain_tweaks: Res<TerrainTweaks>,
    water_tweaks: Res<WaterTweaks>,
    pipeline: Res<TerrainPipelineState>,
    terrain_revision: Res<TerrainRevision>,
    mut recipe_revision: ResMut<TerrainRecipeRevision>,
    mut features: ResMut<TerrainFeatureRegistry>,
    mut pending: ResMut<TerrainRegenPending>,
    mut last: Local<Option<HydrologyTweakKey>>,
) {
    let key = HydrologyTweakKey {
        ridge: terrain_tweaks.ridge_amplitude,
        valley: terrain_tweaks.valley_depth,
        terrain_overrides: terrain_tweaks.use_overrides,
        water_sea_level: water_tweaks.sea_level_m,
        water_overrides: water_tweaks.use_overrides,
        terrain_revision: terrain_revision.value,
        world_seed: prefs.seed,
        recipe_hash: recipe_revision.hash.clone(),
    };
    if last.as_ref() == Some(&key) {
        return;
    }
    let prev = last.clone();
    let first_run = last.is_none();
    *last = Some(key);
    if first_run {
        // init_terrain_features (OnEnter) already built hydrology this frame.
        // The previous version rebuilt here from the legacy demo config,
        // clobbering the atlas river one frame after init.
        return;
    }

    let sea = world_sea_level(&registry, &prefs, &water_tweaks);
    let offset = world_coord_offset(&registry, &prefs);
    let hydrology = build_hydrology(
        sea,
        pipeline_river(&pipeline),
        &authored_lakes(&registry, &prefs),
        offset,
    );
    apply_hydrology(&mut features, hydrology);

    let world_id = requested_world_id(&prefs);
    let island_world = registry
        .0
        .world_by_id(&world_id)
        .is_ok_and(|world| world.island_gen.is_some());
    let terrain_field_changed = prev.as_ref().is_some_and(|p| {
        terrain_tweaks.use_overrides
            && (p.ridge != terrain_tweaks.ridge_amplitude
                || p.valley != terrain_tweaks.valley_depth
                || p.terrain_overrides != terrain_tweaks.use_overrides)
    });

    if !island_world && terrain_field_changed {
        pending.pending = true;
        recipe_revision.hash.clear();
    }
}

fn update_camera_water_state(
    features: Res<TerrainFeatureRegistry>,
    cameras: Query<&Transform, With<crate::camera::MainGameCamera>>,
    mut state: ResMut<CameraWaterState>,
) {
    let Ok(tf) = cameras.single() else {
        return;
    };
    let Some(hydro) = features.hydrology.as_ref() else {
        return;
    };
    let point = [tf.translation.x, tf.translation.y, tf.translation.z];
    if let Some(sample) = hydro.water.water_at(point) {
        state.body = Some(sample.body);
        state.submerged_depth = sample.depth;
    } else {
        state.body = None;
        state.submerged_depth = 0.0;
    }
}
