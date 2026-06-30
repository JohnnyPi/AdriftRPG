use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use tracing::info;

use crate::camera::ThirdPersonCameraPlugin;
use crate::data::{assets_root, DataAssetPlugin};
use crate::debug_tools::DebugToolsPlugin;
use crate::environment::{
    BiomePlugin, EnvironmentAudioStubPlugin, EnvironmentConfigPlugin, EnvironmentLightingPlugin,
    FogPlugin, LightingPlugin, SkyPlugin,
};
use crate::interaction::InteractionPlugin;
use crate::physics::GamePhysicsPlugin;
use crate::performance::PerformanceValidationPlugin;
use crate::player::{CharacterMotorPlugin, PlayerPlugin};
use crate::scene::BootstrapScenePlugin;
use crate::state::AppState;
use crate::terrain::{
    ChunkResidencyPlugin, TerrainEditingPlugin, TerrainFeaturePlugin, TerrainMaterialPlugin,
    TerrainPlugin,
};
use crate::ui::{HudPlugin, MainMenuPlugin, OptionsPanelPlugin};
use crate::vegetation::VegetationPlugin;
use crate::water::WaterPlugin;
use crate::structures::StructurePlugin;
use crate::world::WorldSemanticPlugin;
use crate::world::WorldProfilePlugin;

pub struct VerticalSlicePlugin;

impl Plugin for VerticalSlicePlugin {
    fn build(&self, app: &mut App) {
        configure_vertical_slice_app(app, "RPG Adrift — Vertical Slice");
    }
}

pub fn configure_vertical_slice_app(app: &mut App, window_title: &str) {
    let assets = assets_root();
    info!(assets_root = %assets.display(), "using asset directory");
    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: window_title.to_string(),
                    resolution: (1280, 720).into(),
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: assets.to_string_lossy().into_owned(),
                ..default()
            }),
        DataAssetPlugin,
        crate::data::VisualConfigHotReloadPlugin,
        BiomePlugin,
        TerrainPlugin,
        TerrainMaterialPlugin,
        TerrainEditingPlugin,
        BootstrapScenePlugin,
        GamePhysicsPlugin,
        CharacterMotorPlugin,
        PlayerPlugin,
        ThirdPersonCameraPlugin,
        TerrainFeaturePlugin,
        ChunkResidencyPlugin,
    ))
    .add_plugins((
        LightingPlugin,
        EnvironmentConfigPlugin,
        EnvironmentLightingPlugin,
        SkyPlugin,
        FogPlugin,
        EnvironmentAudioStubPlugin,
        WaterPlugin,
        VegetationPlugin,
        DebugToolsPlugin,
        InteractionPlugin,
        HudPlugin,
        OptionsPanelPlugin,
        MainMenuPlugin,
        WorldSemanticPlugin,
        WorldProfilePlugin,
    ))
    .add_plugins((StructurePlugin, PerformanceValidationPlugin))
    .init_state::<AppState>()
    .add_systems(OnEnter(AppState::Running), apply_window_resolution);
}

pub fn apply_window_resolution(
    registry: Res<crate::data::ConfigRegistryResource>,
    mut windows: Query<&mut Window>,
) {
    let Ok(perf) = registry.0.active_performance() else {
        return;
    };
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    window.resolution.set(
        perf.target_resolution[0] as f32,
        perf.target_resolution[1] as f32,
    );
}
