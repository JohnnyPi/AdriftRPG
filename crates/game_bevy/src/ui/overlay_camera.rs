//! Persistent UI camera for egui across menu state transitions.
//!
//! bevy_egui attaches `PrimaryEguiContext` to the first camera it sees and never
//! re-attaches after that camera is despawned. Spawning a fresh `Camera2d` per
//! menu screen therefore leaves egui with no valid context and a blank UI.

use bevy::camera::Viewport;
use bevy::camera::ClearColorConfig;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContext, EguiPlugin, PrimaryEguiContext};

use crate::state::AppState;
use crate::ui::OptionsPanelState;

pub struct UiOverlayPlugin;

#[derive(Component)]
pub(crate) struct UiOverlayCamera;

const MENU_BG: Color = Color::srgb(0.08, 0.12, 0.2);

impl Plugin for UiOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .add_systems(Startup, spawn_ui_overlay_camera)
            .add_systems(
                OnEnter(AppState::MainMenu),
                (configure_ui_overlay_for_menu, sync_camera_viewports_to_window).chain(),
            )
            .add_systems(
                OnEnter(AppState::SetupOptions),
                (configure_ui_overlay_for_menu, sync_camera_viewports_to_window).chain(),
            )
            .add_systems(
                Update,
                (
                    sync_ui_overlay_active_for_options,
                    sync_camera_viewports_when_needed,
                )
                    .run_if(in_state(AppState::Running)),
            );
    }
}

/// Match every active camera viewport to the window's physical size.
///
/// Prevents wgpu validation errors when logical/physical sizes diverge (e.g. depth
/// 720 vs color 717) after menu → game transitions.
pub fn sync_camera_viewports_to_window(
    window: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<&mut Camera>,
) {
    let Ok(window) = window.single() else {
        return;
    };
    let size = window_physical_size(window);
    if size.x == 0 || size.y == 0 {
        return;
    }
    for mut camera in &mut cameras {
        if !camera.is_active {
            continue;
        }
        camera.viewport = Some(full_viewport(size));
    }
}

pub fn configure_ui_overlay_for_game(mut cameras: Query<&mut Camera, With<UiOverlayCamera>>) {
    if let Ok(mut camera) = cameras.single_mut() {
        camera.order = 100;
        camera.clear_color = ClearColorConfig::None;
        // Keep inactive during gameplay; options panel enables it when needed.
        camera.is_active = false;
        camera.viewport = None;
    }
}

fn spawn_ui_overlay_camera(mut commands: Commands, mut clear: ResMut<ClearColor>) {
    clear.0 = MENU_BG;
    commands.spawn((
        UiOverlayCamera,
        Camera2d,
        Camera {
            order: 0,
            clear_color: ClearColorConfig::Default,
            ..default()
        },
        EguiContext::default(),
        PrimaryEguiContext,
    ));
}

fn configure_ui_overlay_for_menu(
    mut clear: ResMut<ClearColor>,
    mut cameras: Query<&mut Camera, With<UiOverlayCamera>>,
) {
    clear.0 = MENU_BG;
    if let Ok(mut camera) = cameras.single_mut() {
        camera.order = 0;
        camera.clear_color = ClearColorConfig::Default;
        camera.is_active = true;
        camera.viewport = None;
    }
}

fn sync_ui_overlay_active_for_options(
    panel: Res<OptionsPanelState>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<&mut Camera, With<UiOverlayCamera>>,
) {
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };
    let should_render = panel.open;
    if camera.is_active != should_render {
        camera.is_active = should_render;
        if should_render {
            if let Ok(window) = window.single() {
                let size = window_physical_size(window);
                if size.x > 0 && size.y > 0 {
                    camera.viewport = Some(full_viewport(size));
                }
            }
        }
    }
}

fn sync_camera_viewports_when_needed(
    window: Query<&Window, With<PrimaryWindow>>,
    mut last: Local<Option<UVec2>>,
    mut cameras: Query<&mut Camera>,
) {
    let Ok(window) = window.single() else {
        *last = None;
        return;
    };
    let size = window_physical_size(window);
    if size.x == 0 || size.y == 0 {
        return;
    }
    if last.as_ref() == Some(&size) {
        return;
    }
    *last = Some(size);
    for mut camera in &mut cameras {
        if !camera.is_active {
            continue;
        }
        camera.viewport = Some(full_viewport(size));
    }
}

fn window_physical_size(window: &Window) -> UVec2 {
    UVec2::new(window.physical_width(), window.physical_height())
}

fn full_viewport(physical_size: UVec2) -> Viewport {
    Viewport {
        physical_position: UVec2::ZERO,
        physical_size,
        depth: 0.0..1.0,
    }
}
