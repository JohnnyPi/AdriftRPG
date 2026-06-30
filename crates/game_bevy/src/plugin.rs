use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use tracing::info;

use crate::camera::ThirdPersonCameraPlugin;
use crate::data::{assets_root, DataAssetPlugin};
use crate::debug_tools::DebugToolsPlugin;
use crate::environment::{BiomePlugin, LightingPlugin};
use crate::interaction::InteractionPlugin;
use crate::physics::GamePhysicsPlugin;
use crate::player::PlayerPlugin;
use crate::scene::BootstrapScenePlugin;
use crate::state::AppState;
use crate::terrain::{TerrainEditingPlugin, TerrainMaterialPlugin, TerrainPlugin};
use crate::ui::HudPlugin;
use crate::vegetation::VegetationPlugin;
use crate::water::WaterPlugin;

pub struct VerticalSlicePlugin;

impl Plugin for VerticalSlicePlugin {
    fn build(&self, app: &mut App) {
        let assets = assets_root();
        info!(assets_root = %assets.display(), "using asset directory");
        app.add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "RPG Adrift — Vertical Slice".to_string(),
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
            TerrainPlugin,
            TerrainMaterialPlugin,
            TerrainEditingPlugin,
            BootstrapScenePlugin,
            GamePhysicsPlugin,
            PlayerPlugin,
            ThirdPersonCameraPlugin,
        ))
        .add_plugins((
            BiomePlugin,
            LightingPlugin,
            WaterPlugin,
            VegetationPlugin,
            DebugToolsPlugin,
            InteractionPlugin,
            HudPlugin,
        ))
        .init_state::<AppState>()
        .add_systems(OnEnter(AppState::Running), apply_window_resolution);
    }
}

fn apply_window_resolution(
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
