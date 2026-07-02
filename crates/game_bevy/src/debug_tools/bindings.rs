// crates/game_bevy/src/debug_tools/bindings.rs
use bevy::prelude::*;
use game_data::{CompiledDebug, ConfigRegistry, DebugBindingsDefinition};

#[derive(Resource, Clone, Debug)]
pub struct DebugKeyBindings {
    pub panel: KeyCode,
    pub chunk_bounds: KeyCode,
    pub wireframe: KeyCode,
    pub biome: KeyCode,
    pub material: KeyCode,
    pub collider: KeyCode,
    pub density: KeyCode,
    pub normals: KeyCode,
    pub regen: KeyCode,
    pub next_seed: KeyCode,
    pub freeze_pipeline: KeyCode,
    pub subtract: KeyCode,
    pub add: KeyCode,
    pub paint: KeyCode,
}

impl Default for DebugKeyBindings {
    fn default() -> Self {
        Self::from_yaml_bindings(&default_bindings())
    }
}

impl DebugKeyBindings {
    pub fn from_registry(registry: &ConfigRegistry) -> Self {
        registry
            .debug
            .get(&shared::StableId::new("debug.default"))
            .map(Self::from_compiled)
            .unwrap_or_default()
    }

    pub fn from_compiled(debug: &CompiledDebug) -> Self {
        Self::from_yaml_bindings(&debug.bindings)
    }

    fn from_yaml_bindings(bindings: &DebugBindingsDefinition) -> Self {
        Self {
            panel: parse_key(&bindings.panel).unwrap_or(KeyCode::F1),
            chunk_bounds: parse_key(&bindings.chunk_bounds).unwrap_or(KeyCode::F2),
            wireframe: parse_key(&bindings.wireframe).unwrap_or(KeyCode::F3),
            biome: parse_key(&bindings.biome).unwrap_or(KeyCode::F4),
            material: parse_key(&bindings.material).unwrap_or(KeyCode::F5),
            collider: parse_key(&bindings.collider).unwrap_or(KeyCode::F6),
            density: parse_key(&bindings.density).unwrap_or(KeyCode::F7),
            normals: parse_key(&bindings.normals).unwrap_or(KeyCode::KeyN),
            regen: parse_key(&bindings.regen).unwrap_or(KeyCode::F8),
            next_seed: parse_key(&bindings.next_seed).unwrap_or(KeyCode::F9),
            freeze_pipeline: parse_key(&bindings.freeze_pipeline).unwrap_or(KeyCode::F10),
            subtract: parse_key(&bindings.subtract).unwrap_or(KeyCode::Digit1),
            add: parse_key(&bindings.add).unwrap_or(KeyCode::Digit2),
            paint: parse_key(&bindings.paint).unwrap_or(KeyCode::Digit3),
        }
    }
}

fn default_bindings() -> DebugBindingsDefinition {
    DebugBindingsDefinition {
        panel: "F1".to_string(),
        chunk_bounds: "F2".to_string(),
        wireframe: "F3".to_string(),
        biome: "F4".to_string(),
        material: "F5".to_string(),
        collider: "F6".to_string(),
        density: "F7".to_string(),
        normals: "KeyN".to_string(),
        regen: "F8".to_string(),
        next_seed: "F9".to_string(),
        freeze_pipeline: "F10".to_string(),
        subtract: "Digit1".to_string(),
        add: "Digit2".to_string(),
        paint: "Digit3".to_string(),
    }
}

fn parse_key(name: &str) -> Option<KeyCode> {
    match name {
        "F1" => Some(KeyCode::F1),
        "F2" => Some(KeyCode::F2),
        "F3" => Some(KeyCode::F3),
        "F4" => Some(KeyCode::F4),
        "F5" => Some(KeyCode::F5),
        "F6" => Some(KeyCode::F6),
        "F7" => Some(KeyCode::F7),
        "F8" => Some(KeyCode::F8),
        "F9" => Some(KeyCode::F9),
        "F10" => Some(KeyCode::F10),
        "KeyN" => Some(KeyCode::KeyN),
        "Digit1" => Some(KeyCode::Digit1),
        "Digit2" => Some(KeyCode::Digit2),
        "Digit3" => Some(KeyCode::Digit3),
        _ => None,
    }
}

pub fn init_debug_bindings(
    registry: Res<crate::data::ConfigRegistryResource>,
    mut commands: Commands,
) {
    commands.insert_resource(DebugKeyBindings::from_registry(&registry.0));
}
