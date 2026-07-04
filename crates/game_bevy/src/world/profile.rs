// crates/game_bevy/src/world/profile.rs
//! World profile selection override from options panel.

use bevy::prelude::*;
use shared::StableId;

use crate::data::{ConfigRegistryResource, UserSetupPrefs};
use crate::environment::config_init::refresh_presentation_for_profile;
use crate::environment::{SkyEffectsRevision, SkyPresentationConfig};
use crate::state::AppState;
use crate::terrain::{
    TerrainEditStore, TerrainPipelineState, TerrainRecipeRevision, TerrainRegenPending,
    TerrainRevision, TerrainSpawnPoint, WorldSeedOverride, regen_terrain_with_seed,
};
use crate::ui::TerrainTweaks;

pub struct WorldProfilePlugin;

impl Plugin for WorldProfilePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            auto_regen_on_profile_switch.run_if(in_state(AppState::Running)),
        );
    }
}

pub fn requested_world_id(prefs: &UserSetupPrefs) -> StableId {
    prefs.world_stable_id()
}

pub fn effective_world_from_prefs<'a>(
    registry: &'a game_data::ConfigRegistry,
    prefs: &UserSetupPrefs,
) -> shared::DataResult<&'a game_data::CompiledWorld> {
    registry.effective_world(Some(&requested_world_id(prefs)))
}

fn auto_regen_on_profile_switch(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    terrain_tweaks: Res<TerrainTweaks>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut recipe_revision: ResMut<TerrainRecipeRevision>,
    mut revision: ResMut<TerrainRevision>,
    seed_override: ResMut<WorldSeedOverride>,
    mut spawn_point: ResMut<TerrainSpawnPoint>,
    mut pending: ResMut<TerrainRegenPending>,
    mut edit_store: ResMut<TerrainEditStore>,
    mut runtime: ResMut<crate::terrain::TerrainWorldRuntime>,
    mut sky_config: ResMut<SkyPresentationConfig>,
    mut atmosphere: ResMut<crate::ui::AtmosphereTweaks>,
    mut sky_effects_revision: ResMut<SkyEffectsRevision>,
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

    refresh_presentation_for_profile(
        &registry,
        &prefs,
        &mut sky_config,
        &mut atmosphere,
        &mut sky_effects_revision,
    );
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
