//! World profile selection override from options panel.

use bevy::prelude::*;
use shared::StableId;

use crate::data::ConfigRegistryResource;
use crate::environment::config_init::refresh_presentation_for_profile;
use crate::environment::sky::SkyState;
use crate::state::AppState;
use crate::terrain::{
    regen_terrain_with_seed, TerrainEditStore, TerrainPipelineState, TerrainRecipeRevision,
    TerrainRegenPending, TerrainRevision, TerrainSpawnPoint, WorldSeedOverride,
};
use crate::ui::{TerrainTweaks, WorldTweaks};

pub struct WorldProfilePlugin;

impl Plugin for WorldProfilePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            auto_regen_on_profile_switch.run_if(in_state(AppState::Running)),
        );
    }
}

pub fn requested_world_id(registry: &ConfigRegistryResource, tweaks: &WorldTweaks) -> StableId {
    if tweaks.use_expanded_profile {
        StableId::new("world.expanded_slice")
    } else {
        registry
            .0
            .active_world()
            .map(|w| w.id.clone())
            .unwrap_or_else(|_| StableId::new("world.vertical_slice"))
    }
}

fn auto_regen_on_profile_switch(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    world_tweaks: Res<WorldTweaks>,
    terrain_tweaks: Res<TerrainTweaks>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut recipe_revision: ResMut<TerrainRecipeRevision>,
    mut revision: ResMut<TerrainRevision>,
    seed_override: ResMut<WorldSeedOverride>,
    mut spawn_point: ResMut<TerrainSpawnPoint>,
    mut pending: ResMut<TerrainRegenPending>,
    mut edit_store: ResMut<TerrainEditStore>,
    mut runtime: ResMut<crate::terrain::TerrainWorldRuntime>,
    mut sky_state: ResMut<SkyState>,
    mut atmosphere: ResMut<crate::ui::AtmosphereTweaks>,
    mut last: Local<Option<bool>>,
) {
    let expanded = world_tweaks.use_expanded_profile;
    let Some(previous) = *last else {
        *last = Some(expanded);
        return;
    };
    if previous == expanded {
        return;
    }
    *last = Some(expanded);

    refresh_presentation_for_profile(
        &registry,
        &world_tweaks,
        &mut sky_state,
        &mut atmosphere,
    );
    regen_terrain_with_seed(
        &mut commands,
        &registry,
        &world_tweaks,
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
