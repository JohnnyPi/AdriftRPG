use bevy::prelude::*;
use game_data::{BiomeRuleDefinition, ConfigRegistry};

use crate::state::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiomeKind {
    Beach,
    Grassland,
    RockyUpland,
    Cave,
    ShallowWater,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct BiomeCatalog {
    pub rules: Vec<BiomeRuleDefinition>,
}

impl BiomeCatalog {
    pub fn from_registry(registry: &ConfigRegistry) -> Self {
        let world = registry.active_world().expect("world");
        let biomes = registry.biomes.get(&world.biomes).expect("biomes");
        Self {
            rules: biomes.rules.clone(),
        }
    }
}

pub fn classify_biome(
    catalog: &BiomeCatalog,
    sea_level: f32,
    x: f32,
    y: f32,
    z: f32,
    density: f32,
) -> BiomeKind {
    if density > 0.0 && y < sea_level + 1.5 {
        return BiomeKind::ShallowWater;
    }
    if density > 0.0 {
        return BiomeKind::Grassland;
    }

    let slope_deg = estimate_slope_deg(y, sea_level);
    let water_dist = ((x + 30.0).powi(2) + (z + 25.0).powi(2)).sqrt();
    let cave_depth = if y < sea_level { sea_level - y } else { 0.0 };

    for rule in &catalog.rules {
        if rule.elevation_min.is_some_and(|min| y < min) {
            continue;
        }
        if rule.elevation_max.is_some_and(|max| y > max) {
            continue;
        }
        if rule.slope_min.is_some_and(|min| slope_deg < min) {
            continue;
        }
        if rule.slope_max.is_some_and(|max| slope_deg > max) {
            continue;
        }
        if rule.water_distance_max.is_some_and(|max| water_dist > max) {
            continue;
        }
        if rule.cave_depth_min.is_some_and(|min| cave_depth < min) {
            continue;
        }
        return biome_kind_from_id(&rule.id);
    }

    if y < sea_level + 3.0 {
        BiomeKind::Beach
    } else if y > sea_level + 10.0 {
        BiomeKind::RockyUpland
    } else {
        BiomeKind::Grassland
    }
}

fn estimate_slope_deg(y: f32, sea_level: f32) -> f32 {
    ((y - sea_level).abs() / 8.0).atan().to_degrees() * 4.0
}

fn biome_kind_from_id(id: &str) -> BiomeKind {
    match id {
        "beach" => BiomeKind::Beach,
        "rocky_upland" => BiomeKind::RockyUpland,
        "cave" => BiomeKind::Cave,
        "shallow_water" => BiomeKind::ShallowWater,
        _ => BiomeKind::Grassland,
    }
}

pub fn biome_color(catalog: &BiomeCatalog, kind: BiomeKind) -> Color {
    let id = match kind {
        BiomeKind::Beach => "beach",
        BiomeKind::Grassland => "grassland",
        BiomeKind::RockyUpland => "rocky_upland",
        BiomeKind::Cave => "cave",
        BiomeKind::ShallowWater => "shallow_water",
    };
    if let Some(rule) = catalog.rules.iter().find(|r| r.id == id) {
        return Color::srgb(rule.color[0], rule.color[1], rule.color[2]);
    }
    fallback_biome_color(kind)
}

fn fallback_biome_color(kind: BiomeKind) -> Color {
    match kind {
        BiomeKind::Beach => Color::srgb(0.86, 0.78, 0.58),
        BiomeKind::Grassland => Color::srgb(0.34, 0.52, 0.28),
        BiomeKind::RockyUpland => Color::srgb(0.45, 0.44, 0.42),
        BiomeKind::Cave => Color::srgb(0.28, 0.26, 0.30),
        BiomeKind::ShallowWater => Color::srgb(0.18, 0.62, 0.58),
    }
}

pub struct BiomePlugin;

impl Plugin for BiomePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Running), init_biome_catalog);
    }
}

fn init_biome_catalog(
    registry: Res<crate::data::ConfigRegistryResource>,
    mut commands: Commands,
) {
    commands.insert_resource(BiomeCatalog::from_registry(&registry.0));
}
