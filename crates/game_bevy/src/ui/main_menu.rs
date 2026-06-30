//! Title / startup screen shown before entering the playable world.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::data::ConfigRegistryResource;
use crate::state::AppState;
use crate::ui::{OptionsKeyBindings, OptionsPanelState, WorldTweaks};
use crate::world::requested_world_id;

pub struct MainMenuPlugin;

#[derive(Component)]
struct MenuUiCamera;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::MainMenu), setup_main_menu)
            .add_systems(OnExit(AppState::MainMenu), teardown_main_menu)
            .add_systems(
                Update,
                (
                    main_menu_keyboard,
                    release_cursor_on_main_menu,
                )
                    .run_if(in_state(AppState::MainMenu)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                draw_main_menu.run_if(in_state(AppState::MainMenu)),
            );
    }
}

fn setup_main_menu(mut commands: Commands, mut clear: ResMut<ClearColor>) {
    clear.0 = Color::srgb(0.08, 0.12, 0.2);
    // bevy_egui attaches its primary context to the first camera; without one the UI is invisible.
    commands.spawn((MenuUiCamera, Camera2d));
}

fn teardown_main_menu(mut commands: Commands, cameras: Query<Entity, With<MenuUiCamera>>) {
    for entity in &cameras {
        commands.entity(entity).despawn();
    }
}

fn release_cursor_on_main_menu(mut windows: Query<&mut bevy::window::CursorOptions, With<PrimaryWindow>>) {
    let Ok(mut cursor) = windows.single_mut() else {
        return;
    };
    cursor.visible = true;
    cursor.grab_mode = bevy::window::CursorGrabMode::None;
}

fn main_menu_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    keys: Option<Res<OptionsKeyBindings>>,
    mut panel: ResMut<OptionsPanelState>,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keyboard.just_pressed(KeyCode::Enter) && !panel.open {
        next_state.set(AppState::Running);
        return;
    }

    let toggle = keys.map(|k| k.toggle).unwrap_or(KeyCode::Escape);
    if keyboard.just_pressed(toggle) || keyboard.just_pressed(KeyCode::F11) {
        panel.open = !panel.open;
    }

    if keyboard.just_pressed(KeyCode::F10) {
        exit.write(AppExit::Success);
    }
}

fn draw_main_menu(
    mut contexts: EguiContexts,
    registry: Res<ConfigRegistryResource>,
    world_tweaks: Res<WorldTweaks>,
    mut next_state: ResMut<NextState<AppState>>,
    mut panel: ResMut<OptionsPanelState>,
    mut exit: MessageWriter<AppExit>,
) {
    if panel.open {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let world_id = requested_world_id(&registry, &world_tweaks);
    let world_name = world_id.as_str();

    egui::Window::new("##main_menu")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .fixed_size(egui::vec2(460.0, 420.0))
        .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(20, 31, 51)))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(12.0);
                ui.heading(egui::RichText::new("RPG Adrift").size(42.0).strong());
                ui.label("Expanded Vertical Slice Showcase");
                ui.add_space(24.0);

                if ui.button(egui::RichText::new("Start Game").size(18.0)).clicked() {
                    next_state.set(AppState::Running);
                }
                if ui.button(egui::RichText::new("Options").size(18.0)).clicked() {
                    panel.open = true;
                }
                if ui.button(egui::RichText::new("Quit").size(18.0)).clicked() {
                    exit.write(AppExit::Success);
                }

                ui.add_space(32.0);
                ui.separator();
                ui.label(format!("Active world profile: {world_name}"));
                ui.label("Enter — start   ·   Esc / F11 — options   ·   WASD + mouse in-game");
                ui.label("Expanded island: coast, peak, trench, fort, clouds, and ocean");
            });
        });
}
