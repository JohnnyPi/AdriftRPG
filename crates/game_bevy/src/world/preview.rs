// crates/game_bevy/src/world/preview.rs
//! Overhead map preview for the setup screen.

use bevy::prelude::*;
use game_data::ConfigRegistry;
use terrain_generation::{
    build_island_atlas, colorize_runtime_preview, land_surface_height, GenerationResolution,
    IslandAtlas, PREVIEW_PIXEL_SPACING_M,
};
use voxel_core::{fnv1a_update, quantize_density_mm, FNV_OFFSET};

use crate::data::UserSetupPrefs;
use crate::terrain::{build_density_source_from_prefs, compile_terrain_recipe, island_params_from_compiled};
use crate::ui::TerrainTweaks;

#[derive(Resource, Clone, Debug, Default)]
pub struct MapPreviewState {
    pub dirty: bool,
    pub building: bool,
    pub generation: u64,
    pub color_mode: String,
    pub atlas: Option<IslandAtlas>,
    pub validation_passed: bool,
    pub validation_messages: Vec<String>,
    pub spawn_validation_passed: bool,
    pub spawn_validation_messages: Vec<String>,
    pub error: Option<String>,
    pub pixels: Option<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub params_hash: u64,
}

pub fn hash_prefs(prefs: &UserSetupPrefs) -> u64 {
    let mut hash = FNV_OFFSET;
    hash = fnv1a_update(hash, prefs.world_id.as_str().as_bytes());
    hash = fnv1a_update(hash, prefs.seed.to_le_bytes());
    hash = fnv1a_update(hash, prefs.preview_color_mode.as_bytes());
    for (key, value) in &prefs.island_overrides {
        hash = fnv1a_update(hash, key.as_bytes());
        hash = fnv1a_update(hash, quantize_density_mm(*value).to_le_bytes());
    }
    hash
}

pub fn build_preview_atlas(registry: &ConfigRegistry, prefs: &UserSetupPrefs) -> IslandAtlas {
    let world_id = prefs.world_stable_id();
    let world = registry
        .world_by_id(&world_id)
        .or_else(|_| registry.active_world())
        .expect("world");
    if let Some(base) = registry.island_generation_for_world(world) {
        let merged = prefs.apply_overrides(base);
        let water = registry.water.get(&world.water).expect("water");
        let params = island_params_from_compiled(&merged, world, prefs.seed, water.sea_level_m);
        return build_island_atlas(&params);
    }
    fallback_recipe_preview(registry, world, prefs.seed)
}

fn fallback_recipe_preview(
    registry: &ConfigRegistry,
    world: &game_data::CompiledWorld,
    seed: u64,
) -> IslandAtlas {
    let water = registry.water.get(&world.water).expect("water");
    let recipe = compile_terrain_recipe(registry, world, water, Some(seed));
    let extent = world.ocean_extent_m.unwrap_or(256.0);
    let resolution = GenerationResolution::for_extent(extent);
    let spacing = resolution.local_m;
    let width = (extent / spacing).ceil() as u32 + 1;
    // World-centered origin, matching runtime island atlas via island_params_from_compiled.
    let origin = [-extent * 0.5, -extent * 0.5];
    let mut elevation_regional =
        terrain_generation::Field2D::<f32>::new(width, width, origin, spacing);
    let mut island_mask = terrain_generation::Field2D::<f32>::new(width, width, origin, spacing);
    for z in 0..width {
        for x in 0..width {
            let wx = origin[0] + x as f32 * spacing;
            let wz = origin[1] + z as f32 * spacing;
            let (rx, rz) = recipe_xz_from_world(wx, wz, world.coord_offset);
            let h = land_surface_height(&recipe, rx, rz);
            elevation_regional.set(x, z, h);
            island_mask.set(x, z, if h > recipe.sea_level { 1.0 } else { 0.0 });
        }
    }
    let elevation_local = terrain_generation::Field2D::new(width, width, origin, spacing);
    IslandAtlas {
        resolution,
        seed,
        sea_level_m: recipe.sea_level,
        voxel_amplitude_m: 0.0,
        origin,
        elevation_regional,
        elevation_local,
        bathymetry: terrain_generation::Field2D::new(width, width, origin, spacing),
        island_mask,
        slope: terrain_generation::Field2D::new(width, width, origin, spacing),
        coast_distance: terrain_generation::Field2D::new(width, width, origin, spacing),
        filled_elevation: terrain_generation::Field2D::new(width, width, origin, spacing),
        flow_direction: terrain_generation::Field2D::new(width, width, origin, spacing),
        flow_accumulation: terrain_generation::Field2D::new(width, width, origin, spacing),
        river_mask: terrain_generation::Field2D::new(width, width, origin, spacing),
        wetness: terrain_generation::Field2D::new(width, width, origin, spacing),
        sediment: terrain_generation::Field2D::new(width, width, origin, spacing),
        cliff_mask: terrain_generation::Field2D::new(width, width, origin, spacing),
        beach_mask: terrain_generation::Field2D::new(width, width, origin, spacing),
        soil_depth: terrain_generation::Field2D::new(width, width, origin, spacing),
        biome_weights: terrain_generation::Field2D::new(width, width, origin, spacing),
        river_graph: None,
        validation_passed: true,
        validation_messages: vec!["Legacy recipe preview".into()],
    }
}

pub fn rebuild_preview_pixels(
    preview: &mut MapPreviewState,
    atlas: &IslandAtlas,
    color_mode: &str,
    height_at: impl Fn(f32, f32) -> f32,
) {
    let (pixels, width, height) =
        colorize_runtime_preview(atlas, color_mode, PREVIEW_PIXEL_SPACING_M, height_at);
    preview.width = width;
    preview.height = height;
    preview.pixels = Some(pixels);
}

/// Build atlas, colorize pixels, and run spawn validation for the setup screen.
pub fn generate_map_preview(
    registry: &ConfigRegistry,
    prefs: &UserSetupPrefs,
    preview: &mut MapPreviewState,
) {
    let atlas = build_preview_atlas(registry, prefs);
    preview.validation_passed = atlas.validation_passed;
    preview.validation_messages = atlas.validation_messages.clone();
    let terrain_tweaks = TerrainTweaks::default();
    let source = build_density_source_from_prefs(registry, prefs, terrain_tweaks.field_stack_params());
    let mode = prefs.preview_color_mode.clone();
    rebuild_preview_pixels(preview, &atlas, &mode, |wx, wz| {
        source.terrain_surface_height_at(wx, wz)
    });
    preview.atlas = Some(atlas);
    preview.color_mode = mode;
    preview.params_hash = hash_prefs(prefs);
    preview.generation = preview.generation.wrapping_add(1);
    preview.dirty = false;
    preview.building = false;
    preview.error = None;
    let (spawn_ok, spawn_msgs) = validate_spawn_for_preview(registry, prefs);
    preview.spawn_validation_passed = spawn_ok;
    preview.spawn_validation_messages = spawn_msgs;
}

pub fn validate_spawn_for_preview(registry: &ConfigRegistry, prefs: &UserSetupPrefs) -> (bool, Vec<String>) {
    let terrain_tweaks = TerrainTweaks::default();
    let source = build_density_source_from_prefs(registry, prefs, terrain_tweaks.field_stack_params());
    let (_x, _y, _z, report) = source.resolve_player_spawn(
        terrain_generation::PLAYER_SPAWN_MIN_CLEARANCE_M,
        48.0,
    );
    (report.passed, report.messages)
}

fn recipe_xz_from_world(wx: f32, wz: f32, coord_offset: [f32; 3]) -> (f32, f32) {
    (wx + coord_offset[0], wz + coord_offset[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_xz_from_world_adds_coord_offset() {
        let (rx, rz) = recipe_xz_from_world(0.0, 0.0, [128.0, 0.0, 128.0]);
        assert!((rx - 128.0).abs() < f32::EPSILON);
        assert!((rz - 128.0).abs() < f32::EPSILON);
    }

    #[test]
    fn preview_does_not_double_apply_coord_offset() {
        let wx = 10.0;
        let wz = -5.0;
        let offset = [128.0, 0.0, 128.0];
        let (rx, rz) = recipe_xz_from_world(wx, wz, offset);
        assert!((rx - (wx + offset[0])).abs() < f32::EPSILON);
        assert!((rz - (wz + offset[2])).abs() < f32::EPSILON);
        // Legacy bug sampled at recipe coords plus offset again.
        let double_shifted_x = rx + offset[0];
        assert!((double_shifted_x - (wx + 2.0 * offset[0])).abs() < f32::EPSILON);
        assert!((double_shifted_x - rx).abs() > 1.0);
    }
}
