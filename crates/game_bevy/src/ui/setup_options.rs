// crates/game_bevy/src/ui/setup_options.rs
//! Full-screen setup / options with overhead map preview.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::data::{
    save_user_prefs, ConfigRegistryResource, UserSetupPrefs,
};
use crate::state::AppState;
use crate::world::{
    cancel_map_preview_build, hash_prefs, poll_map_preview_build, start_map_preview_build,
    MapPreviewState,
};

pub struct SetupOptionsPlugin;

#[derive(Resource, Default)]
struct SetupUiState {
    selected_group: usize,
    status_message: Option<String>,
    params_stale: bool,
    preview_texture: Option<egui::TextureHandle>,
    preview_texture_generation: u64,
}

impl Plugin for SetupOptionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapPreviewState>()
            .init_resource::<SetupUiState>()
            .add_systems(OnEnter(AppState::SetupOptions), on_enter_setup_options)
            .add_systems(
                Update,
                (
                    release_cursor_on_setup,
                    track_preview_param_changes,
                    poll_map_preview_build_system,
                )
                    .run_if(in_state(AppState::SetupOptions)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                draw_setup_options.run_if(in_state(AppState::SetupOptions)),
            );
    }
}

fn on_enter_setup_options(mut preview: ResMut<MapPreviewState>, mut ui_state: ResMut<SetupUiState>) {
    preview.dirty = false;
    cancel_map_preview_build(&mut preview);
    preview.error = None;
    ui_state.params_stale = true;
    ui_state.preview_texture = None;
    ui_state.preview_texture_generation = 0;
}

fn poll_map_preview_build_system(mut preview: ResMut<MapPreviewState>) {
    poll_map_preview_build(&mut preview);
}

fn release_cursor_on_setup(
    mut windows: Query<&mut bevy::window::CursorOptions, With<PrimaryWindow>>,
) {
    let Ok(mut cursor) = windows.single_mut() else {
        return;
    };
    cursor.visible = true;
    cursor.grab_mode = bevy::window::CursorGrabMode::None;
}

fn track_preview_param_changes(
    prefs: Res<UserSetupPrefs>,
    preview: Res<MapPreviewState>,
    mut ui_state: ResMut<SetupUiState>,
) {
    let hash = hash_prefs(&prefs);
    if preview.generation > 0 && hash != preview.params_hash {
        ui_state.params_stale = true;
    }
}

fn ensure_preview_texture(
    ctx: &egui::Context,
    preview: &MapPreviewState,
    ui_state: &mut SetupUiState,
) {
    if preview.generation == 0 || ui_state.preview_texture_generation == preview.generation {
        return;
    }
    let Some(pixels) = preview.pixels.as_ref() else {
        return;
    };
    if preview.width == 0 || preview.height == 0 {
        return;
    }

    let color_image = egui::ColorImage::from_rgba_unmultiplied(
        [preview.width as usize, preview.height as usize],
        pixels,
    );
    let texture = ctx.load_texture(
        "setup_map_preview",
        color_image,
        egui::TextureOptions::LINEAR,
    );
    ui_state.preview_texture = Some(texture);
    ui_state.preview_texture_generation = preview.generation;
}

fn draw_setup_options(
    mut contexts: EguiContexts,
    registry: Res<ConfigRegistryResource>,
    mut prefs: ResMut<UserSetupPrefs>,
    mut preview: ResMut<MapPreviewState>,
    mut ui_state: ResMut<SetupUiState>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let schema = registry.0.active_setup_schema().ok();
    let island_base = registry
        .0
        .world_by_id(&prefs.world_stable_id())
        .ok()
        .and_then(|w| registry.0.island_generation_for_world(w));

    let mut viewport_ui = egui::Ui::new(
        ctx.clone(),
        "setup_viewport".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    egui::Panel::top("setup_top").show_inside(&mut viewport_ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("RPG Adrift — World Setup");
            ui.separator();
            if preview.building {
                ui.label("Generating preview…");
            } else if preview.generation == 0 {
                ui.label("Preview not generated");
            } else if preview.validation_passed {
                ui.colored_label(egui::Color32::LIGHT_GREEN, "Validation: PASS");
            } else {
                ui.colored_label(egui::Color32::LIGHT_RED, "Validation: FAIL");
            }
            ui.label(format!("Seed: {}", prefs.seed));
        });
    });

    egui::Panel::bottom("setup_bottom").show_inside(&mut viewport_ui, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Back to Main Menu").clicked() {
                if let Err(error) = save_user_prefs(&prefs) {
                    ui_state.status_message = Some(error);
                }
                next_state.set(AppState::MainMenu);
            }
            if ui.button("Apply & Save").clicked() {
                match save_user_prefs(&prefs) {
                    Ok(()) => ui_state.status_message = Some("Settings saved.".into()),
                    Err(error) => ui_state.status_message = Some(error),
                }
            }
            if let Some(msg) = &ui_state.status_message {
                ui.label(msg);
            }
        });
    });

    egui::Panel::left("setup_controls")
        .default_size(360.0)
        .resizable(true)
        .show_inside(&mut viewport_ui, |ui| {
            ui.heading("World profile");
            let worlds: Vec<_> = registry.0.world_profiles().map(|w| w.id.as_str().to_string()).collect();
            egui::ComboBox::from_id_salt("world_profile")
                .selected_text(&prefs.world_id)
                .show_ui(ui, |ui| {
                    for id in &worlds {
                        if ui.selectable_label(&prefs.world_id == id, id).clicked() {
                            prefs.world_id = id.clone();
                            ui_state.params_stale = true;
                        }
                    }
                });

            if ui
                .add(egui::Slider::new(&mut prefs.seed, 1..=999_999).text("Seed"))
                .changed()
            {
                ui_state.params_stale = true;
            }
            ui.separator();

            if let Some(schema) = schema {
                if !schema.groups.is_empty() {
                    ui.horizontal(|ui| {
                        for (i, group) in schema.groups.iter().enumerate() {
                            if ui
                                .selectable_label(ui_state.selected_group == i, &group.label)
                                .clicked()
                            {
                                ui_state.selected_group = i;
                            }
                        }
                    });
                    ui.separator();
                    let group = &schema.groups[ui_state.selected_group.min(schema.groups.len() - 1)];
                    for param in &group.parameters {
                        let mut value = prefs
                            .island_overrides
                            .get(&param.bind)
                            .copied()
                            .or_else(|| island_base.and_then(|b| b.param_value(&param.bind)))
                            .unwrap_or(param.default);
                        if ui
                            .add(egui::Slider::new(&mut value, param.min..=param.max).text(&param.label))
                            .changed()
                        {
                            prefs.island_overrides.insert(param.bind.clone(), value);
                            ui_state.params_stale = true;
                        }
                    }
                }
            }

            ui.separator();
            ui.heading("Preview mode");
            if let Some(schema) = schema {
                for mode in &schema.preview_modes {
                    if ui
                        .selectable_label(prefs.preview_color_mode == mode.id, &mode.label)
                        .clicked()
                    {
                        prefs.preview_color_mode = mode.id.clone();
                        ui_state.params_stale = true;
                    }
                }
            }

            ui.separator();
            ui.heading("Island preview");
            if island_base.is_none() {
                ui.label("This world uses legacy recipe terrain; island generation sliders are inactive.");
            }
            if ui_state.params_stale && !preview.building {
                ui.label("Parameters changed — generate to refresh.");
            }
            let generate = ui
                .add_enabled(!preview.building, egui::Button::new("Generate preview"))
                .clicked();
            if generate {
                preview.error = None;
                start_map_preview_build(&registry.0, &prefs, &mut preview);
                ui_state.params_stale = false;
                ui_state.preview_texture_generation = 0;
                ui_state.preview_texture = None;
            }

            if preview.generation > 0 {
                ui.label(format!(
                    "Map: {}×{} cells",
                    preview.width, preview.height
                ));
            }
            if let Some(error) = &preview.error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }

            ui.separator();
            ui.collapsing("Validation", |ui| {
                if preview.generation == 0 {
                    ui.label("Generate a preview to run validation.");
                    return;
                }
                ui.label("Island generation");
                for msg in &preview.validation_messages {
                    ui.label(msg);
                }
                ui.separator();
                ui.label("Player spawn (outdoor terrain)");
                if preview.spawn_validation_passed {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, "Spawn: PASS");
                } else {
                    ui.colored_label(egui::Color32::LIGHT_RED, "Spawn: FAIL");
                }
                for msg in &preview.spawn_validation_messages {
                    ui.label(msg);
                }
            });
        });

    egui::CentralPanel::default().show_inside(&mut viewport_ui, |ui| {
        ensure_preview_texture(ui.ctx(), &preview, &mut ui_state);
        ui.vertical_centered(|ui| {
            ui.heading("Overhead map preview");
            ui.label("Top-down view of procedurally generated island fields");
            if preview.building {
                ui.spinner();
                ui.label("Building atlas…");
            } else if preview.generation == 0 {
                ui.label("Click \"Generate preview\" in the left panel to build the map.");
            } else if let Some(texture) = &ui_state.preview_texture {
                let size = egui::vec2(512.0, 512.0);
                ui.image((texture.id(), size));
            } else if preview.pixels.is_some() {
                ui.label("Uploading preview texture…");
            } else {
                ui.label("Preview image unavailable.");
            }
        });
    });
}
