//! Terrain feature registry — rivers, water bodies, caves (VS2 §19).

use bevy::prelude::*;
use game_data::ConfigRegistry;
use shared::StableId;
use std::collections::BTreeMap;
use terrain_generation::{
    generate_river_spline, RiverHydrology, RiverSpline, WaterBody, WaterBodyId, WaterBodyRegistry,
    WaterQuery,
};

use crate::data::ConfigRegistryResource;
use crate::state::AppState;
use crate::terrain::recipe::river_gen_config;
use crate::terrain::{TerrainRegenPending, TerrainRecipeRevision};
use crate::ui::{RiverTweaks, TerrainTweaks, WorldTweaks};
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
            .add_systems(OnEnter(AppState::Running), init_terrain_features)
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
    world_expanded: bool,
    river_source_radius: f32,
    river_mouth_width: f32,
    river_overrides: bool,
    ridge: f32,
    valley: f32,
    terrain_overrides: bool,
}

fn sync_hydrology_from_tweaks(
    registry: Res<ConfigRegistryResource>,
    world_tweaks: Res<WorldTweaks>,
    river_tweaks: Res<RiverTweaks>,
    terrain_tweaks: Res<TerrainTweaks>,
    mut features: ResMut<TerrainFeatureRegistry>,
    mut pending: ResMut<TerrainRegenPending>,
    mut recipe_revision: ResMut<TerrainRecipeRevision>,
    mut last: Local<Option<HydrologyTweakKey>>,
) {
    let key = HydrologyTweakKey {
        world_expanded: world_tweaks.use_expanded_profile,
        river_source_radius: river_tweaks.source_radius_m,
        river_mouth_width: river_tweaks.mouth_width_m,
        river_overrides: river_tweaks.use_overrides,
        ridge: terrain_tweaks.ridge_amplitude,
        valley: terrain_tweaks.valley_depth,
        terrain_overrides: terrain_tweaks.use_overrides,
    };
    if last.as_ref() == Some(&key) {
        return;
    }
    let changed = last.is_some();
    *last = Some(key.clone());

    let world_id = requested_world_id(&registry, &world_tweaks);
    let world = registry.0.world_by_id(&world_id).expect("world profile");
    let sea = registry
        .0
        .water
        .get(&world.water)
        .map(|w| w.sea_level_m)
        .unwrap_or(0.0);
    let pool_elevation = registry
        .0
        .upland_pool_hydrology()
        .map(|h| h.elevation_m)
        .unwrap_or(31.5);

    let hydrology = build_hydrology(
        &registry.0,
        world,
        sea,
        pool_elevation,
        &river_tweaks,
        terrain_tweaks.field_stack_params(),
    );
    if let Some(river) = hydrology.river.clone() {
        features.rivers.insert(1, river);
    } else {
        features.rivers.remove(&1);
    }
    features.water_bodies = hydrology.water.bodies.clone();
    features.hydrology = Some(hydrology);

    if changed && (key.terrain_overrides || key.river_overrides) {
        pending.pending = true;
        recipe_revision.hash.clear();
    }
}

fn init_terrain_features(
    registry: Res<ConfigRegistryResource>,
    world_tweaks: Res<WorldTweaks>,
    river_tweaks: Res<RiverTweaks>,
    terrain_tweaks: Res<TerrainTweaks>,
    mut features: ResMut<TerrainFeatureRegistry>,
) {
    let world_id = requested_world_id(&registry, &world_tweaks);
    let world = registry.0.world_by_id(&world_id).expect("world profile");
    let sea = registry
        .0
        .water
        .get(&world.water)
        .map(|w| w.sea_level_m)
        .unwrap_or(0.0);
    let pool_elevation = registry
        .0
        .upland_pool_hydrology()
        .map(|h| h.elevation_m)
        .unwrap_or(31.5);

    let hydrology = build_hydrology(
        &registry.0,
        world,
        sea,
        pool_elevation,
        &river_tweaks,
        terrain_tweaks.field_stack_params(),
    );
    if let Some(river) = hydrology.river.clone() {
        features.rivers.insert(1, river);
    }
    features.water_bodies = hydrology.water.bodies.clone();
    features.hydrology = Some(hydrology);
}

pub fn build_hydrology(
    registry: &ConfigRegistry,
    world: &game_data::CompiledWorld,
    sea_level: f32,
    pool_elevation: f32,
    river_tweaks: &RiverTweaks,
    field_stack: terrain_generation::FieldStackParams,
) -> RiverHydrology {
    let mut config = registry
        .demo_river()
        .map(|river| river_gen_config(river, world.seed, field_stack))
        .unwrap_or_default();
    if world.coord_offset != [0.0, 0.0, 0.0] {
        config.source_center = [
            config.source_center[0] - world.coord_offset[0],
            config.source_center[1] - world.coord_offset[2],
        ];
    }
    if river_tweaks.use_overrides {
        config.source_radius_m = river_tweaks.source_radius_m;
        config.mouth_width_m = river_tweaks.mouth_width_m;
    }
    let mut river = generate_river_spline(&config, sea_level);
    if let Some(ref mut spline) = river {
        if world.coord_offset != [0.0, 0.0, 0.0] {
            for pt in &mut spline.points {
                pt.position_xz[0] -= world.coord_offset[0];
                pt.position_xz[1] -= world.coord_offset[2];
            }
        }
    }
    let mut water = WaterBodyRegistry::demo_registry(sea_level, pool_elevation);
    if let Some(ref spline) = river {
        use terrain_generation::water_body::{
            WaterBody, WaterBodyId, WaterBodyKind, WaterSurfaceDefinition,
        };
        water.bodies.insert(
            WaterBodyId(3),
            WaterBody {
                id: WaterBodyId(3),
                stable_id: StableId::new("water.river.demo"),
                kind: WaterBodyKind::River,
                surface: WaterSurfaceDefinition::SplineRibbon {
                    control_points: spline.points.clone(),
                },
                material_id: StableId::new("water.river"),
            },
        );
    }
    RiverHydrology { river, water }
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
