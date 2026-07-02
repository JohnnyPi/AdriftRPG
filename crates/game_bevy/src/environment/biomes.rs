// crates/game_bevy/src/environment/biomes.rs
use bevy::prelude::*;
use game_data::{BiomeRuleDefinition, ConfigRegistry};
use shared::StableId;
use terrain_generation::RecipeDensitySource;

use super::biome_context::BiomeSampleContext;
use crate::state::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BiomeKind {
    Beach,
    Grassland,
    RockyUpland,
    Cave,
    ShallowWater,
    Wetland,
    Riverbank,
    Forest,
    Scrub,
    Alpine,
    CoastalScrub,
    DeepWater,
    OffshoreShelf,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct BiomeCatalog {
    pub rules: Vec<BiomeRuleDefinition>,
}

impl BiomeCatalog {
    pub fn from_registry(registry: &ConfigRegistry, world_id: Option<&StableId>) -> Self {
        let world = registry.effective_world(world_id).expect("world");
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
    } else if ctx.elevation > 24.0 {
        BiomeKind::Alpine
    } else if ctx.elevation > 10.0 {
        BiomeKind::RockyUpland
    } else if ctx.effective_moisture >= 0.55 {
        BiomeKind::Forest
    } else if ctx.effective_moisture >= 0.38 {
        BiomeKind::Scrub
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

    if rule.elevation_min.is_some_and(|min| ctx.elevation < min) {
        return false;
    }
    if rule.elevation_max.is_some_and(|max| ctx.elevation > max) {
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
    if rule.moisture_min.is_some_and(|min| ctx.effective_moisture < min) {
        return false;
    }
    if rule.moisture_max.is_some_and(|max| ctx.effective_moisture > max) {
        return false;
    }
    if rule.temperature_min.is_some_and(|min| ctx.temperature < min) {
        return false;
    }
    if rule.temperature_max.is_some_and(|max| ctx.temperature > max) {
        return false;
    }
    if rule.river_distance_max.is_some_and(|max| ctx.distance_to_river > max) {
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
    (ctx.effective_moisture * 0.45 + ctx.temperature * 0.35 + ctx.transition_noise * 0.2)
        .clamp(0.0, 1.0)
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
        BiomeKind::Forest => "forest",
        BiomeKind::Scrub => "scrub",
        BiomeKind::Alpine => "mountain_alpine",
        BiomeKind::CoastalScrub => "coastal_scrub",
        BiomeKind::DeepWater => "deep_water",
        BiomeKind::OffshoreShelf => "offshore_shelf",
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
        "forest" => BiomeKind::Forest,
        "scrub" => BiomeKind::Scrub,
        "mountain_alpine" => BiomeKind::Alpine,
        "coastal_scrub" => BiomeKind::CoastalScrub,
        "deep_water" => BiomeKind::DeepWater,
        "offshore_shelf" => BiomeKind::OffshoreShelf,
        _ => BiomeKind::Grassland,
    }
}

fn fallback_material_id(kind: BiomeKind) -> u16 {
    match kind {
        BiomeKind::Beach | BiomeKind::ShallowWater | BiomeKind::DeepWater | BiomeKind::OffshoreShelf => 1,
        BiomeKind::Grassland | BiomeKind::Wetland | BiomeKind::Riverbank => 0,
        BiomeKind::RockyUpland | BiomeKind::Alpine => 2,
        BiomeKind::Cave => 3,
        BiomeKind::Forest => 5,
        BiomeKind::Scrub | BiomeKind::CoastalScrub => 6,
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
        BiomeKind::Forest => Color::srgb(0.22, 0.42, 0.20),
        BiomeKind::Scrub => Color::srgb(0.48, 0.52, 0.28),
        BiomeKind::Alpine => Color::srgb(0.58, 0.56, 0.54),
        BiomeKind::CoastalScrub => Color::srgb(0.52, 0.54, 0.32),
        BiomeKind::DeepWater => Color::srgb(0.08, 0.28, 0.42),
        BiomeKind::OffshoreShelf => Color::srgb(0.12, 0.45, 0.52),
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
    prefs: Res<crate::data::UserSetupPrefs>,
    mut commands: Commands,
) {
    let world_id = crate::world::requested_world_id(&prefs);
    commands.insert_resource(BiomeCatalog::from_registry(
        &registry.0,
        Some(&world_id),
    ));
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
    fn biome_elevation_follows_surface_not_sample_height() {
        use crate::data::UserSetupPrefs;
        use crate::terrain::build_density_source_from_prefs;
        use game_data::load_registry_from_directory;
        use std::path::PathBuf;

        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .canonicalize()
            .expect("assets");
        let registry = load_registry_from_directory(assets).expect("registry");
        let mut prefs = UserSetupPrefs::default();
        prefs.world_id = "world.expanded_slice_hd".into();
        prefs.seed = 800_000;
        let source = build_density_source_from_prefs(
            &registry,
            &prefs,
            terrain_generation::FieldStackParams::default(),
        );
        let catalog = BiomeCatalog::from_registry(
            &registry,
            Some(&shared::StableId::new("world.expanded_slice_hd")),
        );

        let mut wx = 0.0f32;
        let mut wz = 0.0f32;
        let mut surface_y = 0.0f32;
        let mut found = false;
        'scan: for x in -120..120 {
            for z in -120..120 {
                let candidate_x = x as f32;
                let candidate_z = z as f32;
                let y = source.terrain_surface_height_at(candidate_x, candidate_z);
                let elev = y - source.recipe().sea_level;
                if elev > 4.0 && elev < 15.0 {
                    wx = candidate_x;
                    wz = candidate_z;
                    surface_y = y;
                    found = true;
                    break 'scan;
                }
            }
        }
        assert!(found, "could not find lowland sample on seed 800000");

        let at_surface = classify_biome(&catalog, &source, wx, surface_y, wz, -0.1);
        let airborne = classify_biome(&catalog, &source, wx, surface_y + 80.0, wz, -0.1);
        assert_eq!(
            at_surface, airborne,
            "biome must follow surface elevation; at surface={at_surface:?}, airborne={airborne:?}"
        );
        assert!(
            !matches!(at_surface, BiomeKind::Alpine),
            "lowland should not classify as alpine, got {at_surface:?}"
        );
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
    fn expanded_island_biome_distribution_has_variety() {
        use game_data::load_registry_from_directory;
        use std::collections::BTreeSet;
        use std::path::PathBuf;

        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .canonicalize()
            .expect("assets");
        let registry = load_registry_from_directory(assets).expect("registry");
        let world = registry
            .world_by_id(&shared::StableId::new("world.expanded_slice"))
            .expect("expanded world");
        let catalog = BiomeCatalog::from_registry(
            &registry,
            Some(&shared::StableId::new("world.expanded_slice")),
        );
        let source = crate::terrain::build_density_source(
            &registry,
            Some(&shared::StableId::new("world.expanded_slice")),
            None,
            terrain_generation::FieldStackParams::default(),
        );

        let mut kinds = BTreeSet::new();
        let sample_points = [
            (70.0, 160.0),
            (55.0, 145.0),
            (82.0, 196.0),
            (120.0, 140.0),
            (188.0, 178.0),
            (56.0, 116.0),
        ];
        for (rx, rz) in sample_points {
            let wx = rx - world.coord_offset[0];
            let wz = rz - world.coord_offset[2];
            let y = source.surface_height_at(wx, wz);
            kinds.insert(classify_biome(&catalog, &source, wx, y, wz, -0.1));
        }
        for rx in (55..=190).step_by(10) {
            for rz in (90..=200).step_by(10) {
                let wx = rx as f32 - world.coord_offset[0];
                let wz = rz as f32 - world.coord_offset[2];
                let y = source.surface_height_at(wx, wz);
                kinds.insert(classify_biome(&catalog, &source, wx, y, wz, -0.1));
            }
        }

        assert!(
            kinds.len() >= 3,
            "expected at least 3 biome kinds on expanded island, got {:?}",
            kinds
        );
        assert!(kinds.contains(&BiomeKind::RockyUpland) || kinds.contains(&BiomeKind::Alpine));
        assert!(
            kinds.iter().any(|k| !matches!(k, BiomeKind::Grassland | BiomeKind::RockyUpland)),
            "expected coastal or moisture-driven biomes, got {:?}",
            kinds
        );
    }

    #[test]
    fn expanded_hd_seed_800000_has_biome_variety_at_spawn_and_coast() {
        use crate::data::UserSetupPrefs;
        use crate::terrain::build_density_source_from_prefs;
        use game_data::load_registry_from_directory;
        use std::collections::BTreeSet;
        use std::path::PathBuf;

        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .canonicalize()
            .expect("assets");
        let registry = load_registry_from_directory(assets).expect("registry");
        let mut prefs = UserSetupPrefs::default();
        prefs.world_id = "world.expanded_slice_hd".into();
        prefs.seed = 800_000;
        let source = build_density_source_from_prefs(
            &registry,
            &prefs,
            terrain_generation::FieldStackParams::default(),
        );
        let catalog = BiomeCatalog::from_registry(
            &registry,
            Some(&shared::StableId::new("world.expanded_slice_hd")),
        );
        let (sx, sy, sz, report) = source.resolve_player_spawn(2.0, 48.0);
        assert!(report.passed, "spawn failed: {:?}", report.messages);

        let spawn_biome = classify_biome(&catalog, &source, sx, sy, sz, -0.1);
        assert!(
            matches!(
                spawn_biome,
                BiomeKind::Wetland
                    | BiomeKind::Beach
                    | BiomeKind::CoastalScrub
                    | BiomeKind::Scrub
                    | BiomeKind::Grassland
                    | BiomeKind::Forest
                    | BiomeKind::Alpine
                    | BiomeKind::RockyUpland
            ),
            "spawn should resolve to a playable surface biome, got {spawn_biome:?}"
        );

        let mut kinds = BTreeSet::new();
        for rx in (40..=200).step_by(8) {
            for rz in (40..=220).step_by(8) {
                let wx = rx as f32 - 128.0;
                let wz = rz as f32 - 128.0;
                let y = source.terrain_surface_height_at(wx, wz);
                if y <= source.recipe().sea_level + 0.5 {
                    continue;
                }
                kinds.insert(classify_biome(&catalog, &source, wx, y, wz, -0.1));
            }
        }
        assert!(
            kinds.len() >= 5,
            "expected varied biomes on volcanic hd island, got {:?}",
            kinds
        );
        assert!(
            kinds.contains(&BiomeKind::Forest)
                || kinds.contains(&BiomeKind::Grassland)
                || kinds.contains(&BiomeKind::Beach)
                || kinds.contains(&BiomeKind::CoastalScrub),
            "expected lowland vegetation or beach biomes, got {:?}",
            kinds
        );
    }
}
