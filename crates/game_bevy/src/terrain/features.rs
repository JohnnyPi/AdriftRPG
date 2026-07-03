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
use terrain_generation::water_body::{WaterBodyKind, WaterSurfaceDefinition};
use terrain_generation::{
    RiverHydrology, RiverSpline, WaterBody, WaterBodyId, WaterBodyRegistry, WaterQuery,
};

use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;
use crate::state::AppState;
use crate::terrain::{
    TerrainPipelineState, TerrainRegenPending, TerrainRecipeRevision, TerrainWorldInitSet,
};
use crate::ui::TerrainTweaks;
use crate::world::requested_world_id;

#[derive(Resource, Clone, Debug, Default)]
pub struct TerrainFeatureRegistry {
    pub rivers: BTreeMap<u32, RiverSpline>,
    pub water_bodies: BTreeMap<WaterBodyId, WaterBody>,
    pub hydrology: Option<RiverHydrology>,
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
            .add_systems(OnEnter(AppState::Running), init_terrain_features.after(TerrainWorldInitSet))
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
}

/// Sea level of the requested world's water definition.
fn world_sea_level(registry: &ConfigRegistryResource, prefs: &UserSetupPrefs) -> f32 {
    let world_id = requested_world_id(prefs);
    let world = registry.0.world_by_id(&world_id).expect("world profile");
    registry
        .0
        .water
        .get(&world.water)
        .map(|w| w.sea_level_m)
        .unwrap_or(0.0)
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

/// Build hydrology for an atlas island world: the global sea plus the
/// generated river (when present).
///
/// Note: the sea is an unbounded horizontal body, so carved volumes below
/// sea level (e.g. deep cave chambers on low flanks) register as flooded.
/// That is the intended global-ocean assumption for now.
pub fn build_hydrology(sea_level: f32, river: Option<RiverSpline>) -> RiverHydrology {
    let mut bodies = BTreeMap::new();
    bodies.insert(
        WaterBodyId(1),
        WaterBody {
            id: WaterBodyId(1),
            stable_id: StableId::new("water.sea"),
            kind: WaterBodyKind::Sea,
            surface: WaterSurfaceDefinition::Horizontal {
                elevation: sea_level,
            },
            material_id: StableId::new("water.tropical_shallow"),
        },
    );
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

fn apply_hydrology(features: &mut TerrainFeatureRegistry, hydrology: RiverHydrology) {
    if let Some(river) = hydrology.river.clone() {
        features.rivers.insert(1, river);
    } else {
        features.rivers.remove(&1);
    }
    features.water_bodies = hydrology.water.bodies.clone();
    features.hydrology = Some(hydrology);
}

fn init_terrain_features(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    pipeline: Res<TerrainPipelineState>,
    mut features: ResMut<TerrainFeatureRegistry>,
) {
    let sea = world_sea_level(&registry, &prefs);
    let hydrology = build_hydrology(sea, atlas_river(&pipeline));
    apply_hydrology(&mut features, hydrology);
}

fn sync_hydrology_from_tweaks(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    terrain_tweaks: Res<TerrainTweaks>,
    pipeline: Res<TerrainPipelineState>,
    mut features: ResMut<TerrainFeatureRegistry>,
    mut pending: ResMut<TerrainRegenPending>,
    mut recipe_revision: ResMut<TerrainRecipeRevision>,
    mut last: Local<Option<HydrologyTweakKey>>,
) {
    let key = HydrologyTweakKey {
        ridge: terrain_tweaks.ridge_amplitude,
        valley: terrain_tweaks.valley_depth,
        terrain_overrides: terrain_tweaks.use_overrides,
    };
    if last.as_ref() == Some(&key) {
        return;
    }
    let first_run = last.is_none();
    *last = Some(key);
    if first_run {
        // init_terrain_features (OnEnter) already built hydrology this frame.
        // The previous version rebuilt here from the legacy demo config,
        // clobbering the atlas river one frame after init.
        return;
    }

    let sea = world_sea_level(&registry, &prefs);
    let hydrology = build_hydrology(sea, atlas_river(&pipeline));
    apply_hydrology(&mut features, hydrology);

    // Terrain field parameters changed: the density source must regenerate.
    pending.pending = true;
    recipe_revision.hash.clear();
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