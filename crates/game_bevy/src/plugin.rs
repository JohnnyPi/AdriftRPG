// crates/game_bevy/src/plugin.rs
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use tracing::info;

use crate::camera::ThirdPersonCameraPlugin;
use crate::data::{DataAssetPlugin, assets_root};
use crate::debug_tools::DebugToolsPlugin;
use crate::environment::volumetric_scatter::VolumetricScatterPlugin;
use crate::environment::{
    AtmosphereScenePlugin, BiomePlugin, CelestialPlugin, CloudPlugin, EnvironmentAudioStubPlugin,
    EnvironmentConfigPlugin, EnvironmentLightingPlugin, FogPlugin, LightingPlugin,
    SimulationTimePlugin, StarfieldPlugin, WeatherPlugin,
};
use crate::performance::PerformanceValidationPlugin;
use crate::physics::GamePhysicsPlugin;
use crate::player::{CharacterMotorPlugin, PlayerPlugin};
use crate::scene::BootstrapScenePlugin;
use crate::state::AppState;
use crate::terrain::{
    ChunkResidencyPlugin, TerrainEditingPlugin, TerrainFeaturePlugin, TerrainMaterialPlugin,
    TerrainPlugin,
};
use crate::ui::{
    HudPlugin, MainMenuPlugin, OptionsPanelPlugin, SetupOptionsPlugin, UiOverlayPlugin,
    configure_ui_overlay_for_game, sync_camera_viewports_to_window,
};
use crate::vegetation::VegetationPlugin;
use crate::water::WaterPlugin;
use crate::world::{WorldProfilePlugin, WorldSemanticPlugin};

pub struct VerticalSlicePlugin;

impl Plugin for VerticalSlicePlugin {
    fn build(&self, app: &mut App) {
        configure_vertical_slice_app(app, "RPG Adrift — Vertical Slice");
    }
}

pub fn configure_vertical_slice_app(app: &mut App, window_title: &str) {
    let assets = assets_root();

    // LogPlugin must be installed before any logging happens, so DefaultPlugins
    // goes in first and the assets_root log moves after it.
    app.add_plugins(
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
    );

    info!(assets_root = %assets.display(), "using asset directory");

    app.add_plugins((
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
        crate::lod::LodPolicyPlugin,
        crate::staging::StagingPlugin,
        crate::interest::ChunkInterestPlugin,
    ))
    .add_plugins((
        LightingPlugin,
        SimulationTimePlugin,
        EnvironmentConfigPlugin,
        EnvironmentLightingPlugin,
        CelestialPlugin,
        AtmosphereScenePlugin,
        VolumetricScatterPlugin,
        CloudPlugin,
        FogPlugin,
        StarfieldPlugin,
        WeatherPlugin,
    ))
    .add_plugins((
        EnvironmentAudioStubPlugin,
        WaterPlugin,
        VegetationPlugin,
        DebugToolsPlugin,
        HudPlugin,
    ))
    .add_plugins((
        UiOverlayPlugin,
        OptionsPanelPlugin,
        SetupOptionsPlugin,
        MainMenuPlugin,
        WorldSemanticPlugin,
    ))
    .add_plugins((WorldProfilePlugin, PerformanceValidationPlugin))
    .init_state::<AppState>()
    .add_systems(
        OnEnter(AppState::Running),
        (
            apply_window_resolution,
            sync_camera_viewports_to_window,
            configure_ui_overlay_for_game,
        )
            .chain(),
    );
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
