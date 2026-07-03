// crates/game_bevy/src/world/preview.rs
//! Overhead map preview for the setup screen.

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use game_data::ConfigRegistry;
use terrain_generation::{
    clamp_preview_output_side, colorize_runtime_preview, effective_sea_level_m,
    land_surface_height, preview_grid_for_atlas, resolve_island_atlas, GenerationResolution,
    IslandAtlas,
};
use tracing::info;
use voxel_core::{fnv1a_update, quantize_density_mm, FNV_OFFSET};

use crate::data::{assets_root, UserSetupPrefs};
use crate::environment::BiomeCatalog;
use crate::environment::biomes::{biome_color, classify_biome};
use crate::terrain::{build_density_source_from_prefs, compile_terrain_recipe};
use crate::ui::TerrainTweaks;
use crate::world::effective_world_from_prefs;

#[derive(Resource, Debug, Default)]
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
    pending_task: Option<Task<MapPreviewBuildResult>>,
}

struct MapPreviewBuildResult {
    atlas: IslandAtlas,
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    color_mode: String,
    params_hash: u64,
    validation_passed: bool,
    validation_messages: Vec<String>,
    spawn_validation_passed: bool,
    spawn_validation_messages: Vec<String>,
    error: Option<String>,
}

pub fn hash_prefs(prefs: &UserSetupPrefs) -> u64 {
    let mut hash = FNV_OFFSET;
    hash = fnv1a_update(hash, prefs.world_id.as_str().as_bytes());
    hash = fnv1a_update(hash, prefs.seed.to_le_bytes());
    hash = fnv1a_update(hash, prefs.preview_color_mode.as_bytes());
    hash = fnv1a_update(hash, prefs.preview_resolution.to_le_bytes());
    for (key, value) in &prefs.island_overrides {
        hash = fnv1a_update(hash, key.as_bytes());
        hash = fnv1a_update(hash, quantize_density_mm(*value).to_le_bytes());
    }
    hash
}

pub fn build_preview_atlas(registry: &ConfigRegistry, prefs: &UserSetupPrefs) -> IslandAtlas {
    let world = effective_world_from_prefs(registry, prefs).expect("world");
    if let Some(base) = registry.island_generation_for_world(world) {
        let merged = prefs.apply_overrides(base);
        let water = registry.water.get(&world.water).expect("water");
        let sea_level = effective_sea_level_m(water, Some(&merged));
        return resolve_island_atlas(
            &merged,
            world,
            prefs.seed,
            sea_level,
            Some(assets_root().as_path()),
        );
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
    let extent = world.effective_ocean_extent_m();
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

fn build_preview_pixels(
    registry: &ConfigRegistry,
    prefs: &UserSetupPrefs,
    atlas: &IslandAtlas,
    output_side: u32,
) -> (Vec<u8>, u32, u32) {
    let terrain_tweaks = TerrainTweaks::default();
    let source = build_density_source_from_prefs(registry, prefs, terrain_tweaks.field_stack_params());
    let mode = prefs.preview_color_mode.as_str();
    if mode == "biomes" {
        let catalog = BiomeCatalog::from_registry(registry, Some(&prefs.world_stable_id()));
        let (width, height, spacing_x, spacing_z) = preview_grid_for_atlas(atlas, output_side);
        let w = width as usize;
        let h = height as usize;
        let mut pixels = vec![0u8; w * h * 4];
        let sea = atlas.sea_level_m;
        for z in 0..height {
            for x in 0..width {
                let wx = atlas.origin[0] + x as f32 * spacing_x;
                let wz = atlas.origin[1] + z as f32 * spacing_z;
                let surface_y = source.terrain_surface_height_at(wx, wz);
                let color = if surface_y <= sea + 0.25 {
                    Color::srgb(0.08, 0.28, 0.42)
                } else {
                    let biome = classify_biome(&catalog, &source, wx, surface_y, wz, -0.1);
                    biome_color(&catalog, biome)
                };
                let srgba = color.to_srgba();
                let i = ((z as usize * w + x as usize) * 4) as usize;
                pixels[i] = (srgba.red * 255.0) as u8;
                pixels[i + 1] = (srgba.green * 255.0) as u8;
                pixels[i + 2] = (srgba.blue * 255.0) as u8;
                pixels[i + 3] = 255;
            }
        }
        (pixels, width, height)
    } else {
        colorize_runtime_preview(atlas, mode, output_side, |wx, wz| {
            source.terrain_surface_height_at(wx, wz)
        })
    }
}

fn build_map_preview_data(registry: &ConfigRegistry, prefs: &UserSetupPrefs) -> MapPreviewBuildResult {
    let output_side = clamp_preview_output_side(prefs.preview_resolution);
    info!(
        world = %prefs.world_id,
        seed = prefs.seed,
        mode = %prefs.preview_color_mode,
        output_side,
        "building setup map preview"
    );

    let atlas = build_preview_atlas(registry, prefs);
    let validation_passed = atlas.validation_passed;
    let validation_messages = atlas.validation_messages.clone();
    let mode = prefs.preview_color_mode.clone();
    let (pixels, width, height) = build_preview_pixels(registry, prefs, &atlas, output_side);

    info!(
        world = %prefs.world_id,
        width,
        height,
        atlas_cells = atlas.width().max(atlas.height()),
        "setup map preview ready"
    );

    let (spawn_ok, spawn_msgs) = validate_spawn_for_preview(registry, prefs);

    MapPreviewBuildResult {
        atlas,
        pixels,
        width,
        height,
        color_mode: mode,
        params_hash: hash_prefs(prefs),
        validation_passed,
        validation_messages,
        spawn_validation_passed: spawn_ok,
        spawn_validation_messages: spawn_msgs,
        error: None,
    }
}

fn apply_map_preview_build(preview: &mut MapPreviewState, result: MapPreviewBuildResult) {
    preview.atlas = Some(result.atlas);
    preview.pixels = Some(result.pixels);
    preview.width = result.width;
    preview.height = result.height;
    preview.color_mode = result.color_mode;
    preview.params_hash = result.params_hash;
    preview.validation_passed = result.validation_passed;
    preview.validation_messages = result.validation_messages;
    preview.spawn_validation_passed = result.spawn_validation_passed;
    preview.spawn_validation_messages = result.spawn_validation_messages;
    preview.error = result.error;
    preview.generation = preview.generation.wrapping_add(1);
    preview.dirty = false;
    preview.building = false;
}

/// Drop an in-flight preview task (e.g. when leaving the setup screen).
pub fn cancel_map_preview_build(preview: &mut MapPreviewState) {
    preview.pending_task = None;
    preview.building = false;
}

/// Queue async preview generation so the UI thread stays responsive.
pub fn start_map_preview_build(
    registry: &ConfigRegistry,
    prefs: &UserSetupPrefs,
    preview: &mut MapPreviewState,
) {
    preview.pending_task = None;
    preview.building = true;
    preview.error = None;

    let registry = registry.clone();
    let prefs = prefs.clone();
    preview.pending_task = Some(AsyncComputeTaskPool::get().spawn(async move {
        build_map_preview_data(&registry, &prefs)
    }));
}

/// Poll an in-flight preview build (call from `Update`).
pub fn poll_map_preview_build(preview: &mut MapPreviewState) {
    let Some(mut task) = preview.pending_task.take() else {
        return;
    };
    if let Some(result) =
        bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(&mut task))
    {
        apply_map_preview_build(preview, result);
    } else {
        preview.pending_task = Some(task);
    }
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
