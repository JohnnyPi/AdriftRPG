//! World profile picker shared by Setup and in-game Options.

use bevy_egui::egui;
use game_data::ConfigRegistry;

/// Stable ordering for the world profile dropdown (small, medium, large).
pub const WORLD_PROFILE_ORDER: &[(&str, &str)] = &[
    ("world.small", "Small Island"),
    ("world.medium", "Medium Island"),
    ("world.large", "Large Island"),
];

pub fn world_profile_label(id: &str) -> &str {
    WORLD_PROFILE_ORDER
        .iter()
        .find(|(world_id, _)| *world_id == id)
        .map(|(_, label)| *label)
        .unwrap_or(id)
}

pub fn ordered_world_profile_ids(registry: &ConfigRegistry) -> Vec<String> {
    let available: std::collections::HashSet<_> = registry
        .world_profiles()
        .map(|w| w.id.as_str().to_string())
        .collect();
    WORLD_PROFILE_ORDER
        .iter()
        .filter_map(|(id, _)| available.contains(*id).then(|| id.to_string()))
        .chain(
            available
                .iter()
                .filter(|id| !WORLD_PROFILE_ORDER.iter().any(|(known, _)| known == *id))
                .cloned(),
        )
        .collect()
}

/// Combo box for selecting the active presentation world profile.
pub fn draw_world_profile_combo(
    ui: &mut egui::Ui,
    registry: &ConfigRegistry,
    selected_id: &mut String,
) -> bool {
    let mut changed = false;
    let ids = ordered_world_profile_ids(registry);
    let selected_label = world_profile_label(selected_id);
    egui::ComboBox::from_id_salt("world_profile")
        .selected_text(selected_label)
        .show_ui(ui, |ui| {
            for id in &ids {
                let label = world_profile_label(id);
                if ui.selectable_label(selected_id == id, label).clicked() {
                    *selected_id = id.clone();
                    changed = true;
                }
            }
        });
    changed
}
