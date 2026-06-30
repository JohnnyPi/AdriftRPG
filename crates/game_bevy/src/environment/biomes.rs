use bevy::prelude::*;
use game_data::{BiomeRuleDefinition, ConfigRegistry};
use terrain_generation::RecipeDensitySource;

use super::biome_context::BiomeSampleContext;
use crate::state::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiomeKind {
    Beach,
    Grassland,
    RockyUpland,
    Cave,
    ShallowWater,
    Wetland,
    Riverbank,
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

    pub fn material_id_for(&self, kind: BiomeKind) -> u16 {
        let rule_id = biome_id_str(kind);
        self.rules
            .iter()
            .find(|r| r.id == rule_id)
            .map(|r| r.material_id)
            .unwrap_or_else(|| fallback_material_id(kind))
    }
}

pub fn classify_biome(
    catalog: &BiomeCatalog,
    source: &RecipeDensitySource,
    x: f32,
    y: f32,
    z: f32,
    density: f32,
) -> BiomeKind {
    let ctx = BiomeSampleContext::sample(source, x, y, z);
    classify_biome_with_context(catalog, source, &ctx, density)
}

pub fn classify_biome_with_context(
    catalog: &BiomeCatalog,
    source: &RecipeDensitySource,
    ctx: &BiomeSampleContext,
    density: f32,
) -> BiomeKind {
    if density > 0.0 && ctx.world_y < source.recipe().sea_level + 1.5 {
        return BiomeKind::ShallowWater;
    }

    classify_biome_from_context(catalog, ctx)
}

fn classify_biome_from_context(catalog: &BiomeCatalog, ctx: &BiomeSampleContext) -> BiomeKind {
    for rule in &catalog.rules {
        if !rule_matches(rule, ctx) {
            continue;
        }
        return biome_kind_from_id(&rule.id);
    }

    if ctx.elevation < 3.0 {
        BiomeKind::Beach
    } else if ctx.elevation > 10.0 {
        BiomeKind::RockyUpland
    } else {
        BiomeKind::Grassland
    }
}

fn rule_matches(rule: &BiomeRuleDefinition, ctx: &BiomeSampleContext) -> bool {
    if rule.id == "shallow_water" && !ctx.is_underwater() {
        return false;
    }
    if rule.id == "cave" && !ctx.is_cave() {
        return false;
    }

    if rule.elevation_min.is_some_and(|min| ctx.world_y < min) {
        return false;
    }
    if rule.elevation_max.is_some_and(|max| ctx.world_y > max) {
        return false;
    }
    if rule.slope_min.is_some_and(|min| ctx.slope_degrees < min) {
        return false;
    }
    if rule.slope_max.is_some_and(|max| ctx.slope_degrees > max) {
        return false;
    }
    if rule.water_distance_max.is_some_and(|max| ctx.distance_to_water > max) {
        return false;
    }
    if rule.cave_depth_min.is_some_and(|min| ctx.cave_depth < min) {
        return false;
    }
    if rule.moisture_min.is_some_and(|min| ctx.moisture < min) {
        return false;
    }
    true
}

pub fn biome_color(catalog: &BiomeCatalog, kind: BiomeKind) -> Color {
    let id = biome_id_str(kind);
    if let Some(rule) = catalog.rules.iter().find(|r| r.id == id) {
        return Color::srgb(rule.color[0], rule.color[1], rule.color[2]);
    }
    fallback_biome_color(kind)
}

pub fn biome_scalar_debug_value(ctx: &BiomeSampleContext) -> f32 {
    (ctx.moisture * 0.45 + ctx.temperature * 0.35 + ctx.transition_noise * 0.2).clamp(0.0, 1.0)
}

pub fn biome_discrete_debug_color(value: f32) -> Color {
    if value < 0.25 {
        Color::srgb(1.0, 0.0, 0.0)
    } else if value < 0.5 {
        Color::srgb(0.0, 1.0, 0.0)
    } else if value < 0.75 {
        Color::srgb(0.0, 0.0, 1.0)
    } else {
        Color::srgb(1.0, 1.0, 0.0)
    }
}

fn biome_id_str(kind: BiomeKind) -> &'static str {
    match kind {
        BiomeKind::Beach => "beach",
        BiomeKind::Grassland => "grassland",
        BiomeKind::RockyUpland => "rocky_upland",
        BiomeKind::Cave => "cave",
        BiomeKind::ShallowWater => "shallow_water",
        BiomeKind::Wetland => "wetland",
        BiomeKind::Riverbank => "riverbank",
    }
}

fn biome_kind_from_id(id: &str) -> BiomeKind {
    match id {
        "beach" => BiomeKind::Beach,
        "rocky_upland" => BiomeKind::RockyUpland,
        "cave" => BiomeKind::Cave,
        "shallow_water" => BiomeKind::ShallowWater,
        "wetland" => BiomeKind::Wetland,
        "riverbank" => BiomeKind::Riverbank,
        _ => BiomeKind::Grassland,
    }
}

fn fallback_material_id(kind: BiomeKind) -> u16 {
    match kind {
        BiomeKind::Beach | BiomeKind::ShallowWater => 1,
        BiomeKind::Grassland | BiomeKind::Wetland | BiomeKind::Riverbank => 0,
        BiomeKind::RockyUpland => 2,
        BiomeKind::Cave => 3,
    }
}

fn fallback_biome_color(kind: BiomeKind) -> Color {
    match kind {
        BiomeKind::Beach => Color::srgb(0.86, 0.78, 0.58),
        BiomeKind::Grassland => Color::srgb(0.34, 0.52, 0.28),
        BiomeKind::RockyUpland => Color::srgb(0.45, 0.44, 0.42),
        BiomeKind::Cave => Color::srgb(0.28, 0.26, 0.30),
        BiomeKind::ShallowWater => Color::srgb(0.18, 0.62, 0.58),
        BiomeKind::Wetland => Color::srgb(0.28, 0.42, 0.22),
        BiomeKind::Riverbank => Color::srgb(0.32, 0.48, 0.26),
    }
}

pub struct BiomePlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct BiomeInitSet;

impl Plugin for BiomePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(OnEnter(AppState::Running), BiomeInitSet)
            .add_systems(OnEnter(AppState::Running), init_biome_catalog.in_set(BiomeInitSet));
    }
}

fn init_biome_catalog(
    registry: Res<crate::data::ConfigRegistryResource>,
    mut commands: Commands,
) {
    commands.insert_resource(BiomeCatalog::from_registry(&registry.0));
}

#[cfg(test)]
mod tests {
    use super::*;
    use terrain_generation::default_vertical_slice_recipe;

    fn test_source() -> RecipeDensitySource {
        RecipeDensitySource::new(default_vertical_slice_recipe(42, 2.0))
    }

    fn test_catalog() -> BiomeCatalog {
        let mut shallow = BiomeRuleDefinition::new("shallow_water", 1, [0.18, 0.62, 0.58]);
        shallow.elevation_max = Some(3.5);
        let mut beach = BiomeRuleDefinition::new("beach", 1, [0.86, 0.78, 0.58]);
        beach.elevation_min = Some(-1.0);
        beach.elevation_max = Some(8.0);
        beach.water_distance_max = Some(45.0);
        let mut cave = BiomeRuleDefinition::new("cave", 3, [0.28, 0.26, 0.30]);
        cave.elevation_max = Some(30.0);
        cave.cave_depth_min = Some(0.0);
        let mut rocky = BiomeRuleDefinition::new("rocky_upland", 2, [0.45, 0.44, 0.42]);
        rocky.elevation_min = Some(12.0);
        rocky.slope_min = Some(20.0);
        BiomeCatalog {
            rules: vec![
                shallow,
                beach,
                cave,
                rocky,
                BiomeRuleDefinition::new("grassland", 0, [0.34, 0.52, 0.28]),
            ],
        }
    }

    #[test]
    fn solid_and_air_share_surface_biome() {
        let source = test_source();
        let catalog = test_catalog();
        let x = 10.0;
        let z = 10.0;
        let y = source.surface_height_at(x, z);
        let solid = classify_biome(&catalog, &source, x, y, z, 1.0);
        let air = classify_biome(&catalog, &source, x, y, z, -0.5);
        assert_eq!(solid, air, "surface solid/air biomes must match at ({x}, {z})");
    }

    #[test]
    fn surface_materials_match_for_solid_and_air_corners() {
        let source = test_source();
        let catalog = test_catalog();
        for x in [-10, 0, 10, 20] {
            for z in [-10, 0, 10, 20] {
                let wx = x as f32;
                let wz = z as f32;
                let y = source.surface_height_at(wx, wz);
                let solid_mat = super::super::materials::material_for_world(
                    &catalog, &source, wx, y, wz, 1.0,
                );
                let air_mat = super::super::materials::material_for_world(
                    &catalog, &source, wx, y, wz, -0.5,
                );
                assert_eq!(
                    solid_mat, air_mat,
                    "materials must match at surface ({wx}, {wz})"
                );
            }
        }
    }

    #[test]
    fn classification_is_deterministic_for_seed() {
        let source = test_source();
        let catalog = test_catalog();
        let a = classify_biome(&catalog, &source, 5.0, 12.0, 8.0, -0.1);
        let b = classify_biome(&catalog, &source, 5.0, 12.0, 8.0, -0.1);
        assert_eq!(a, b);
    }
}
