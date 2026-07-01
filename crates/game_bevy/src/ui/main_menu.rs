//! Title / startup screen shown before entering the playable world.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::data::{save_user_prefs, UserSetupPrefs};
use crate::state::AppState;
use crate::world::requested_world_id;

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
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

fn release_cursor_on_main_menu(mut windows: Query<&mut bevy::window::CursorOptions, With<PrimaryWindow>>) {
    let Ok(mut cursor) = windows.single_mut() else {
        return;
    };
    cursor.visible = true;
    cursor.grab_mode = bevy::window::CursorGrabMode::None;
}

fn main_menu_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(AppState::Running);
        return;
    }

    if keyboard.just_pressed(KeyCode::F10) {
        exit.write(AppExit::Success);
    }
}

fn draw_main_menu(
    mut contexts: EguiContexts,
    prefs: Res<UserSetupPrefs>,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let world_name = requested_world_id(&prefs).as_str().to_string();

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
                ui.label("VS3 Island Generator Vertical Slice");
                ui.add_space(24.0);

                if ui.button(egui::RichText::new("Start Game").size(18.0)).clicked() {
                    let _ = save_user_prefs(&prefs);
                    next_state.set(AppState::Running);
                }
                if ui.button(egui::RichText::new("Options").size(18.0)).clicked() {
                    next_state.set(AppState::SetupOptions);
                }
                if ui.button(egui::RichText::new("Quit").size(18.0)).clicked() {
                    exit.write(AppExit::Success);
                }

                ui.add_space(32.0);
                ui.separator();
                ui.label(format!("Active world profile: {world_name}"));
                ui.label(format!("Seed: {}", prefs.seed));
                ui.label("Enter — start with saved setup   ·   Options — world generation");
            });
        });
}
