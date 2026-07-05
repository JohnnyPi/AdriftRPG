// crates/game_bevy/src/environment/biomes.rs
use bevy::prelude::*;
use game_data::{BiomeRuleDefinition, ConfigRegistry};
use shared::StableId;
use terrain_generation::{CompilerBiomeId, RecipeDensitySource, WorldDensityProvider, WorldXZ};
use terrain_surface::{BiomeId, compiler_biome_to_presentation};

use super::biome_context::BiomeSampleContext;
use crate::state::AppState;

/// Presentation/runtime alias for the authoritative [`terrain_surface::BiomeId`].
pub type BiomeKind = BiomeId;

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

    pub fn material_id_for(&self, kind: BiomeId) -> u16 {
        let rule_id = kind.as_rule_id();
        self.rules
            .iter()
            .find(|r| r.id == rule_id)
            .map(|r| r.material_id)
            .unwrap_or_else(|| fallback_material_id(kind))
    }

    pub fn vegetation_profile_for(&self, kind: BiomeId) -> Option<StableId> {
        let rule_id = kind.as_rule_id();
        self.rules
            .iter()
            .find(|r| r.id == rule_id)
            .and_then(|r| r.vegetation_profile_id.clone())
    }
}

pub fn classify_biome(
    catalog: &BiomeCatalog,
    source: &RecipeDensitySource,
    x: f32,
    y: f32,
    z: f32,
    density: f32,
) -> BiomeId {
    let ctx = BiomeSampleContext::sample(source, x, y, z);
    classify_biome_with_context(catalog, source, &ctx, density)
}

pub fn classify_biome_from_provider(
    catalog: &BiomeCatalog,
    provider: &dyn WorldDensityProvider,
    x: f32,
    y: f32,
    z: f32,
    density: f32,
) -> BiomeId {
    let ctx = BiomeSampleContext::sample_from_provider(provider, x, y, z);
    classify_biome_with_provider_context(catalog, provider, &ctx, x, z, density)
}

pub fn classify_biome_with_provider_context(
    catalog: &BiomeCatalog,
    provider: &dyn WorldDensityProvider,
    ctx: &BiomeSampleContext,
    wx: f32,
    wz: f32,
    density: f32,
) -> BiomeId {
    let sea_level = provider.world_metadata().extent.sea_level_m;
    if density > 0.0 && ctx.world_y < sea_level + 1.5 {
        return BiomeId::ShallowWater;
    }
    if let Some(blend) = provider.sample_biome_blend(WorldXZ::new(wx as f64, wz as f64)) {
        return compiler_biome_to_presentation(blend.primary);
    }
    let column = provider.sample_column(WorldXZ::new(wx as f64, wz as f64));
    if column.primary_biome != 0 {
        return compiler_biome_to_presentation(CompilerBiomeId::from_u8(column.primary_biome));
    }
    classify_biome_from_context(catalog, ctx)
}

pub fn classify_biome_with_context(
    catalog: &BiomeCatalog,
    source: &RecipeDensitySource,
    ctx: &BiomeSampleContext,
    density: f32,
) -> BiomeId {
    if density > 0.0 && ctx.world_y < source.recipe().sea_level + 1.5 {
        return BiomeId::ShallowWater;
    }

    classify_biome_from_context(catalog, ctx)
}

fn classify_biome_from_context(catalog: &BiomeCatalog, ctx: &BiomeSampleContext) -> BiomeId {
    for rule in &catalog.rules {
        if !rule_matches(rule, ctx) {
            continue;
        }
        return BiomeId::from_rule_id(&rule.id);
    }

    if ctx.elevation < 3.0 {
        BiomeId::Beach
    } else if ctx.elevation > 28.0 {
        BiomeId::Alpine
    } else if ctx.elevation > 12.0 {
        BiomeId::RockyUpland
    } else if ctx.effective_moisture >= 0.55 {
        BiomeId::Forest
    } else if ctx.effective_moisture >= 0.38 {
        BiomeId::Scrub
    } else {
        BiomeId::Grassland
    }
}

fn rule_matches(rule: &BiomeRuleDefinition, ctx: &BiomeSampleContext) -> bool {
    if rule.id == "shallow_water" && !ctx.is_underwater() {
        return false;
    }
    // Seabed rules describe submerged terrain. Their YAML elevation bands
    // extend up to sea level (offshore_shelf reaches +2 m so the shelf can
    // shoal to shore), so without this gate they also match dry coastal land
    // whose surface sits inside that band — e.g. a resolved spawn on a berm
    // 1.6 m above sea classifying as OffshoreShelf.
    if (rule.id == "deep_water" || rule.id == "offshore_shelf") && ctx.elevation >= 0.0 {
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
    if rule
        .water_distance_max
        .is_some_and(|max| ctx.distance_to_water > max)
    {
        return false;
    }
    if rule.cave_depth_min.is_some_and(|min| ctx.cave_depth < min) {
        return false;
    }
    if rule
        .moisture_min
        .is_some_and(|min| ctx.effective_moisture < min)
    {
        return false;
    }
    if rule
        .moisture_max
        .is_some_and(|max| ctx.effective_moisture > max)
    {
        return false;
    }
    if rule
        .temperature_min
        .is_some_and(|min| ctx.temperature < min)
    {
        return false;
    }
    if rule
        .temperature_max
        .is_some_and(|max| ctx.temperature > max)
    {
        return false;
    }
    if rule
        .river_distance_max
        .is_some_and(|max| ctx.distance_to_river > max)
    {
        return false;
    }
    true
}

/// Multiplier applied to triplanar albedo so biome rules tint the terrain surface.
pub fn biome_surface_tint(catalog: &BiomeCatalog, kind: BiomeId) -> [f32; 3] {
    const STRENGTH: f32 = 1.65;
    let id = kind.as_rule_id();
    let (color, tint) = if let Some(rule) = catalog.rules.iter().find(|r| r.id == id) {
        (rule.color, rule.tint)
    } else {
        (fallback_biome_rgb(kind), [1.0, 1.0, 1.0])
    };
    tint_rgb(color, tint, STRENGTH)
}

/// Blend biome tints from soft weights for smoother transitions at biome boundaries.
pub fn biome_surface_tint_from_soft(
    catalog: &BiomeCatalog,
    soft: &terrain_surface::SoftBiomeWeights,
) -> [f32; 3] {
    let mut rgb = [0.0f32; 3];
    let mut weight_sum = 0.0f32;
    for (biome, w) in soft.weighted_biomes() {
        if w <= 0.001 {
            continue;
        }
        let tint = biome_surface_tint(catalog, biome);
        rgb[0] += tint[0] * w;
        rgb[1] += tint[1] * w;
        rgb[2] += tint[2] * w;
        weight_sum += w;
    }
    if weight_sum <= f32::EPSILON {
        return biome_surface_tint(catalog, BiomeId::Grassland);
    }
    [
        (rgb[0] / weight_sum).clamp(0.12, 2.8),
        (rgb[1] / weight_sum).clamp(0.12, 2.8),
        (rgb[2] / weight_sum).clamp(0.12, 2.8),
    ]
}

fn tint_rgb(color: [f32; 3], tint: [f32; 3], strength: f32) -> [f32; 3] {
    let target = [color[0] * tint[0], color[1] * tint[1], color[2] * tint[2]];
    [
        (1.0 + (target[0] - 1.0) * strength).clamp(0.12, 2.8),
        (1.0 + (target[1] - 1.0) * strength).clamp(0.12, 2.8),
        (1.0 + (target[2] - 1.0) * strength).clamp(0.12, 2.8),
    ]
}

pub fn biome_color(catalog: &BiomeCatalog, kind: BiomeId) -> Color {
    let id = kind.as_rule_id();
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

/// Debug color for baked compiler `PrimaryBiome` field values.
pub fn compiler_biome_debug_color(primary_biome: u8) -> Color {
    let kind = compiler_biome_to_presentation(CompilerBiomeId::from_u8(primary_biome));
    fallback_biome_color(kind)
}

fn fallback_material_id(kind: BiomeId) -> u16 {
    match kind {
        BiomeId::Beach | BiomeId::ShallowWater | BiomeId::DeepWater | BiomeId::OffshoreShelf => 1,
        BiomeId::Grassland | BiomeId::Wetland | BiomeId::Riverbank => 0,
        BiomeId::RockyUpland | BiomeId::Alpine => 2,
        BiomeId::Cave => 3,
        BiomeId::Forest => 5,
        BiomeId::Scrub | BiomeId::CoastalScrub => 6,
    }
}

fn fallback_biome_rgb(kind: BiomeId) -> [f32; 3] {
    match kind {
        BiomeId::Beach => [0.86, 0.78, 0.58],
        BiomeId::Grassland => [0.34, 0.52, 0.28],
        BiomeId::RockyUpland => [0.45, 0.44, 0.42],
        BiomeId::Cave => [0.28, 0.26, 0.30],
        BiomeId::ShallowWater => [0.18, 0.62, 0.58],
        BiomeId::Wetland => [0.28, 0.42, 0.22],
        BiomeId::Riverbank => [0.32, 0.48, 0.26],
        BiomeId::Forest => [0.22, 0.42, 0.20],
        BiomeId::Scrub => [0.48, 0.52, 0.28],
        BiomeId::Alpine => [0.58, 0.56, 0.54],
        BiomeId::CoastalScrub => [0.52, 0.54, 0.32],
        BiomeId::DeepWater => [0.08, 0.28, 0.42],
        BiomeId::OffshoreShelf => [0.12, 0.45, 0.52],
    }
}

fn fallback_biome_color(kind: BiomeId) -> Color {
    let rgb = fallback_biome_rgb(kind);
    Color::srgb(rgb[0], rgb[1], rgb[2])
}

pub struct BiomePlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct BiomeInitSet;

impl Plugin for BiomePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(OnEnter(AppState::Running), BiomeInitSet)
            .add_systems(
                OnEnter(AppState::Running),
                init_biome_catalog.in_set(BiomeInitSet),
            );
    }
}

fn init_biome_catalog(
    registry: Res<crate::data::ConfigRegistryResource>,
    prefs: Res<crate::data::UserSetupPrefs>,
    mut commands: Commands,
) {
    let world_id = crate::world::requested_world_id(&prefs);
    commands.insert_resource(BiomeCatalog::from_registry(&registry.0, Some(&world_id)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use terrain_generation::default_vertical_slice_recipe;

    /// Recipe-space scan window shared by the island variety tests: covers
    /// the testbed island coast-to-summit (recipe center at coord_offset, support
    /// radius ~112 m on the default testbed footprint).
    const SCAN_RECIPE_MIN: usize = 144;
    const SCAN_RECIPE_MAX: usize = 368;
    const SCAN_STEP: usize = 8;

    fn recipe_scan_world_xz(rx: usize, rz: usize, coord_offset: [f32; 3]) -> (f32, f32) {
        (rx as f32 - coord_offset[0], rz as f32 - coord_offset[2])
    }

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
    fn seabed_rules_never_match_dry_land() {
        use crate::environment::biome_context::BiomeSampleContext;

        // Catalog mirroring biomes.expanded_slice ordering: seabed rules
        // first, with offshore_shelf's band reaching above sea level
        // (elevation +2).
        let mut deep = BiomeRuleDefinition::new("deep_water", 1, [0.08, 0.28, 0.42]);
        deep.elevation_max = Some(-5.0);
        let mut shelf = BiomeRuleDefinition::new("offshore_shelf", 1, [0.12, 0.45, 0.52]);
        shelf.elevation_min = Some(-15.0);
        shelf.elevation_max = Some(2.0);
        let mut beach = BiomeRuleDefinition::new("beach", 1, [0.86, 0.78, 0.58]);
        beach.elevation_min = Some(-1.0);
        beach.elevation_max = Some(14.0);
        beach.water_distance_max = Some(35.0);
        let catalog = BiomeCatalog {
            rules: vec![deep, shelf, beach],
        };

        // Dry berm: surface 1.6 m above sea (world_y = sea 2.0 + 1.6), close
        // to the waterline. Inside offshore_shelf's elevation band, but the
        // surface is not submerged — it must classify as beach, not seabed.
        let dry_berm = BiomeSampleContext::for_test(3.6, 1.6, 4.0, 6.0);
        let kind = classify_biome_from_context(&catalog, &dry_berm);
        assert_eq!(
            kind,
            BiomeKind::Beach,
            "dry coastal land classified as a seabed biome"
        );

        // Genuinely submerged shelf (surface 4 m below sea) still matches.
        let submerged = BiomeSampleContext::for_test(-2.0, -4.0, 3.0, 0.0);
        let kind = classify_biome_from_context(&catalog, &submerged);
        assert_eq!(kind, BiomeKind::OffshoreShelf);
    }

    #[test]
    #[ignore = "requires worldgen compiled atlas in test harness; see docs/worlds/small.md"]
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
        prefs.world_id = "world.small".into();
        prefs.seed = 800_000;
        let source = build_density_source_from_prefs(
            &registry,
            &prefs,
            terrain_generation::FieldStackParams::default(),
        );
        let catalog =
            BiomeCatalog::from_registry(&registry, Some(&shared::StableId::new("world.small")));

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
        assert_eq!(
            solid, air,
            "surface solid/air biomes must match at ({x}, {z})"
        );
    }

    #[test]
    #[ignore = "requires worldgen compiled atlas in test harness; see docs/worlds/small.md"]
    fn testbed_island_biome_distribution_has_variety() {
        use game_data::load_registry_from_directory;
        use std::collections::BTreeSet;
        use std::path::PathBuf;

        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .canonicalize()
            .expect("assets");
        let registry = load_registry_from_directory(assets).expect("registry");
        let world = registry
            .world_by_id(&shared::StableId::new("world.small"))
            .expect("testbed world");
        let catalog =
            BiomeCatalog::from_registry(&registry, Some(&shared::StableId::new("world.small")));
        let source = crate::terrain::build_density_source(
            &registry,
            Some(&shared::StableId::new("world.small")),
            None,
            terrain_generation::FieldStackParams::default(),
        );

        // Land-only scan across the island (see SCAN_* docs). The old
        // expanded_slice version also asserted on hand-picked recipe points
        // tuned to the removed op-based terrain; the grid scan covers the
        // same ground without depending on authored heights.
        let mut kinds = BTreeSet::new();
        for rx in (SCAN_RECIPE_MIN..=SCAN_RECIPE_MAX).step_by(SCAN_STEP) {
            for rz in (SCAN_RECIPE_MIN..=SCAN_RECIPE_MAX).step_by(SCAN_STEP) {
                let (wx, wz) = recipe_scan_world_xz(rx, rz, world.coord_offset);
                let y = source.terrain_surface_height_at(wx, wz);
                if y <= source.recipe().sea_level + 0.5 {
                    continue;
                }
                kinds.insert(classify_biome(&catalog, &source, wx, y, wz, -0.1));
            }
        }

        assert!(
            kinds.len() >= 3,
            "expected at least 3 biome kinds on testbed island, got {:?}",
            kinds
        );
        assert!(kinds.contains(&BiomeKind::RockyUpland) || kinds.contains(&BiomeKind::Alpine));
        assert!(
            kinds
                .iter()
                .any(|k| !matches!(k, BiomeKind::Grassland | BiomeKind::RockyUpland)),
            "expected coastal or moisture-driven biomes, got {:?}",
            kinds
        );
    }

    #[test]
    #[ignore = "requires worldgen compiled atlas in test harness; see docs/worlds/small.md"]
    fn testbed_seed_800000_has_biome_variety_at_spawn_and_coast() {
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
        prefs.world_id = "world.small".into();
        prefs.seed = 800_000;
        let source = build_density_source_from_prefs(
            &registry,
            &prefs,
            terrain_generation::FieldStackParams::default(),
        );
        let catalog =
            BiomeCatalog::from_registry(&registry, Some(&shared::StableId::new("world.small")));
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
        let offset = source.recipe().coord_offset;
        for rx in (SCAN_RECIPE_MIN..=SCAN_RECIPE_MAX).step_by(SCAN_STEP) {
            for rz in (SCAN_RECIPE_MIN..=SCAN_RECIPE_MAX).step_by(SCAN_STEP) {
                let (wx, wz) = recipe_scan_world_xz(rx, rz, offset);
                let y = source.terrain_surface_height_at(wx, wz);
                if y <= source.recipe().sea_level + 0.5 {
                    continue;
                }
                kinds.insert(classify_biome(&catalog, &source, wx, y, wz, -0.1));
            }
        }
        assert!(
            kinds.len() >= 3,
            "expected varied biomes on testbed volcanic island, got {:?}",
            kinds
        );
        assert!(
            kinds.contains(&BiomeKind::Alpine) || kinds.contains(&BiomeKind::RockyUpland),
            "volcanic testbed should reach upland biomes, got {:?}",
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
