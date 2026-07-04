// crates/terrain_generation/src/island_gen/mod.rs
//! Island atlas builder — VS3 phases A–G orchestration.

mod bathymetry;
mod biome_field;
mod carving;
mod coast;
mod erosion;
mod footprint;
mod hydrology;
mod params;
mod soil_field;
mod util;
mod validate;
mod volcano;

pub use params::*;
pub use validate::{ValidationReport, min_peak_elevation_m, validate_atlas};

use crate::field2d::{Field2D, residual_from_absolute};
use crate::island_atlas::IslandAtlas;
use crate::noise::ValueNoise;
use bathymetry::{bathymetry_height, compute_coast_distance};
use biome_field::compute_biome_weights;
use carving::{carve_river_channels, compute_slope};
use coast::{apply_beach_profiles, classify_coast};
use erosion::{apply_stream_power_erosion, apply_thermal_erosion};
use footprint::build_island_mask;
use hydrology::{
    compute_flow, extract_river_mask, priority_flood, refresh_river_elevations_after_carve,
    trace_primary_river,
};
use soil_field::compute_soil_depth;
use volcano::{local_detail_at, regional_detail_at, volcanic_height};

/// Build the island atlas from `params`, exactly as given.
///
/// This function does NOT rescale: it used to call `fit_to_ocean_extent()` on
/// a clone internally, which silently and non-uniformly crushed any oversized
/// island into a distorted miniature (steepened slopes, proportionally
/// amplified noise and warp) on every runtime build. Fitting is now an
/// explicit caller decision, and runtime/diagnostic paths gate configs through
/// `world_setup::validate_island_world_budget` instead, which rejects any
/// island the fit would have altered.
pub fn build_island_atlas(params: &IslandGenParams) -> IslandAtlas {
    let resolution = params.resolution;
    let regional_spacing = resolution.regional_m;
    let local_spacing = resolution.local_m;
    let extent = params.ocean_extent_m;
    let origin = [
        params.center[0] - extent * 0.5,
        params.center[1] - extent * 0.5,
    ];

    // --- Regional tier: footprint, volcano, bathymetry, hydrology, erosion ---
    let mut island_mask = Field2D::<f32>::from_extent(extent, origin, regional_spacing);
    let mut elevation_regional = Field2D::<f32>::from_extent(extent, origin, regional_spacing);
    let mut bathymetry = Field2D::<f32>::from_extent(extent, origin, regional_spacing);

    let mask_noise = ValueNoise::new(params.seed);
    let regional_detail_noise = ValueNoise::new(params.seed.wrapping_add(17));
    let local_detail_noise = ValueNoise::new(params.seed.wrapping_add(23));
    let shelf_width_noise = ValueNoise::new(params.seed.wrapping_add(bathymetry::SALT_SHELF_WIDTH));
    let shelf_depth_noise = ValueNoise::new(params.seed.wrapping_add(bathymetry::SALT_SHELF_DEPTH));
    let deep_slope_noise = ValueNoise::new(params.seed.wrapping_add(bathymetry::SALT_DEEP_SLOPE));
    let berm_noise = ValueNoise::new(params.seed.wrapping_add(coast::SALT_BERM_HEIGHT));

    island_mask.for_each_world(|wx, wz, m| {
        *m = build_island_mask(params, wx, wz, &mask_noise);
    });

    elevation_regional.for_each_world(|wx, wz, h| {
        let gx = ((wx - origin[0]) / regional_spacing).round() as u32;
        let gz = ((wz - origin[1]) / regional_spacing).round() as u32;
        let mask = if gx < island_mask.width && gz < island_mask.height {
            island_mask.get(gx, gz)
        } else {
            0.0
        };
        *h = volcanic_height(params, wx, wz, mask)
            + regional_detail_at(params, wx, wz, &regional_detail_noise) * mask;
    });

    let coast_distance = compute_coast_distance(&island_mask, regional_spacing);

    bathymetry.for_each_world(|wx, wz, h| {
        let cd = coast_distance.sample_bilinear(wx, wz);
        *h = bathymetry_height(
            params,
            wx,
            wz,
            cd,
            &shelf_width_noise,
            &shelf_depth_noise,
            &deep_slope_noise,
        );
    });

    // Pre-erosion flow drives stream-power carving only.
    let filled_pre_erosion = priority_flood(&elevation_regional);
    let (_flow_direction_pre, flow_accumulation_pre) =
        compute_flow(&filled_pre_erosion, &island_mask, params);
    apply_stream_power_erosion(
        &mut elevation_regional,
        &flow_accumulation_pre,
        &island_mask,
        params,
    );
    apply_thermal_erosion(&mut elevation_regional, &island_mask, params);

    // Hydrology extraction runs on the post-erosion surface.
    let filled = priority_flood(&elevation_regional);
    let (flow_direction, flow_accumulation) = compute_flow(&filled, &island_mask, params);
    let river_mask = extract_river_mask(&flow_accumulation, &island_mask, params);
    let mut river_graph = trace_primary_river(
        &filled,
        &flow_accumulation,
        &flow_direction,
        &island_mask,
        params,
        params.island.sea_level_m,
    );

    let regional_width = elevation_regional.width;
    let regional_height = elevation_regional.height;
    let mut sediment = Field2D::<f32>::from_extent(extent, origin, regional_spacing);
    for z in 0..regional_height {
        for x in 0..regional_width {
            sediment.set(x, z, flow_accumulation.get(x, z) * 0.001);
        }
    }

    // Bilinear upsample softens regional peak curvature; local_detail_at restores micro-relief.
    let mut elevation_local_abs = elevation_regional.resample_to_spacing(local_spacing);
    elevation_local_abs.for_each_world(|wx, wz, h| {
        let mask = island_mask.sample_bilinear(wx, wz);
        *h += local_detail_at(params, wx, wz, &local_detail_noise) * mask;
    });

    let island_mask_local = island_mask.resample_to_spacing(local_spacing);
    let coast_distance_local = coast_distance.resample_to_spacing(local_spacing);
    let sediment_local = sediment.resample_to_spacing(local_spacing);

    if let Some(ref mut river) = river_graph {
        carve_river_channels(&mut elevation_local_abs, river, params);
        refresh_river_elevations_after_carve(
            river,
            &elevation_local_abs,
            params.island.sea_level_m,
            0.25,
        );
    }

    let mut slope = compute_slope(&elevation_local_abs);
    let (cliff_mask, beach_mask) = classify_coast(
        &elevation_local_abs,
        &slope,
        &coast_distance_local,
        &island_mask_local,
        &sediment_local,
        params,
    );
    apply_beach_profiles(
        &mut elevation_local_abs,
        &beach_mask,
        &coast_distance_local,
        params,
        &berm_noise,
    );
    slope = compute_slope(&elevation_local_abs);

    let wetness = flow_accumulation.resample_to_spacing(local_spacing);
    let soil_depth = compute_soil_depth(
        &elevation_local_abs,
        &slope,
        &sediment_local,
        &island_mask_local,
        params,
    );
    let biome_weights = compute_biome_weights(
        &elevation_local_abs,
        &slope,
        &wetness,
        &beach_mask,
        &island_mask_local,
        params,
    );

    let elevation_local = residual_from_absolute(&elevation_regional, &elevation_local_abs);

    let mut atlas = IslandAtlas {
        resolution,
        seed: params.seed,
        sea_level_m: params.island.sea_level_m,
        voxel_amplitude_m: params.surface_noise.voxel_amplitude_m,
        origin,
        elevation_regional,
        elevation_local,
        bathymetry,
        island_mask: island_mask_local,
        slope,
        coast_distance,
        filled_elevation: filled,
        flow_direction,
        flow_accumulation,
        river_mask,
        wetness,
        sediment,
        cliff_mask,
        beach_mask,
        soil_depth,
        biome_weights,
        river_graph,
        validation_passed: false,
        validation_messages: Vec::new(),
    };

    let report = validate_atlas(&atlas, params);
    atlas.validation_passed = report.passed;
    atlas.validation_messages = report.messages;
    atlas
}

pub fn sample_atlas_surface(atlas: &IslandAtlas, wx: f32, wz: f32) -> f32 {
    atlas.surface_height_at(wx, wz)
}

pub fn colorize_preview(atlas: &IslandAtlas, mode: &str) -> Vec<u8> {
    let w = atlas.width() as usize;
    let h = atlas.height() as usize;
    let mut pixels = vec![0u8; w * h * 4];
    let sea = atlas.sea_level_m;
    let spacing = atlas.spacing_m();
    let mut max_land = sea;
    if mode == "elevation" || mode.is_empty() {
        for z in 0..atlas.height() {
            for x in 0..atlas.width() {
                let wx = atlas.origin[0] + x as f32 * spacing;
                let wz = atlas.origin[1] + z as f32 * spacing;
                let v = atlas.composed_land_elevation_at(wx, wz);
                if atlas.island_mask.sample_bilinear(wx, wz) > 0.5 {
                    max_land = max_land.max(v);
                }
            }
        }
        max_land = max_land.max(sea + 1.0);
    }
    let mut min_v = f32::MAX;
    let mut max_v = f32::MIN;
    if mode != "elevation" && !mode.is_empty() {
        for z in 0..atlas.height() {
            for x in 0..atlas.width() {
                let v = preview_value(atlas, x, z, mode);
                min_v = min_v.min(v);
                max_v = max_v.max(v);
            }
        }
    }
    let range = (max_v - min_v).max(0.001);
    for z in 0..atlas.height() {
        for x in 0..atlas.width() {
            let wx = atlas.origin[0] + x as f32 * spacing;
            let wz = atlas.origin[1] + z as f32 * spacing;
            let (r, g, b) = if mode == "elevation" || mode.is_empty() {
                let land = atlas.island_mask.sample_bilinear(wx, wz);
                let elev = if land > 0.5 {
                    atlas.composed_land_elevation_at(wx, wz)
                } else {
                    atlas.bathymetry.sample_bilinear(wx, wz)
                };
                elevation_color_absolute(elev, sea, max_land, land)
            } else {
                let v = preview_value(atlas, x, z, mode);
                let t = ((v - min_v) / range).clamp(0.0, 1.0);
                elevation_color(t, mode)
            };
            let i = ((z as usize * w + x as usize) * 4) as usize;
            pixels[i] = r;
            pixels[i + 1] = g;
            pixels[i + 2] = b;
            pixels[i + 3] = 255;
        }
    }
    pixels
}

/// Legacy full-res spacing used when no output-side cap is applied (tests / atlas-native previews).
pub const PREVIEW_PIXEL_SPACING_M: f32 = 2.0;

pub const PREVIEW_OUTPUT_MIN: u32 = 64;
pub const PREVIEW_OUTPUT_MAX: u32 = 512;

/// Clamp setup-screen preview side length (pixels per axis).
pub fn clamp_preview_output_side(side: u32) -> u32 {
    side.clamp(PREVIEW_OUTPUT_MIN, PREVIEW_OUTPUT_MAX).max(2)
}

/// World extent and per-axis sample spacing for a capped square preview grid.
pub fn preview_grid_for_atlas(atlas: &IslandAtlas, output_side: u32) -> (u32, u32, f32, f32) {
    let width = clamp_preview_output_side(output_side);
    let height = width;
    let extent_x = (atlas.width().saturating_sub(1)) as f32 * atlas.spacing_m();
    let extent_z = (atlas.height().saturating_sub(1)) as f32 * atlas.spacing_m();
    let spacing_x = if width > 1 {
        extent_x / (width - 1) as f32
    } else {
        extent_x
    };
    let spacing_z = if height > 1 {
        extent_z / (height - 1) as f32
    } else {
        extent_z
    };
    (width, height, spacing_x, spacing_z)
}

fn preview_value_at_world(atlas: &IslandAtlas, wx: f32, wz: f32, mode: &str) -> f32 {
    match mode {
        "island_mask" => atlas.island_mask.sample_bilinear(wx, wz),
        "flow_accumulation" => atlas.flow_accumulation.sample_bilinear(wx, wz),
        "river_mask" => atlas.river_mask.sample_bilinear(wx, wz),
        "beach_suitability" => atlas.beach_mask.sample_bilinear(wx, wz),
        "cliff_suitability" => atlas.cliff_mask.sample_bilinear(wx, wz),
        "elevation_regional" => atlas.elevation_regional.sample_bilinear(wx, wz),
        "elevation_local" | "elevation_local_residual" => {
            atlas.elevation_local.sample_bilinear(wx, wz)
        }
        _ => atlas.composed_land_elevation_at(wx, wz),
    }
}

/// Colorize a capped preview using runtime terrain heights (world XZ).
///
/// `output_side` is the target pixel count per axis (clamped to
/// [`PREVIEW_OUTPUT_MIN`]..=[`PREVIEW_OUTPUT_MAX`]); sample spacing is derived
/// from the atlas world extent so large worlds stay responsive.
pub fn colorize_runtime_preview<F>(
    atlas: &IslandAtlas,
    mode: &str,
    output_side: u32,
    height_at: F,
) -> (Vec<u8>, u32, u32)
where
    F: Fn(f32, f32) -> f32,
{
    let (width, height, spacing_x, spacing_z) = preview_grid_for_atlas(atlas, output_side);
    let w = width as usize;
    let h = height as usize;
    let mut pixels = vec![0u8; w * h * 4];
    let sea = atlas.sea_level_m;
    let mut max_land = sea;
    if mode == "elevation" || mode.is_empty() {
        for z in 0..height {
            for x in 0..width {
                let wx = atlas.origin[0] + x as f32 * spacing_x;
                let wz = atlas.origin[1] + z as f32 * spacing_z;
                let elev = height_at(wx, wz);
                if elev > sea + 0.25 {
                    max_land = max_land.max(elev);
                }
            }
        }
        max_land = max_land.max(sea + 1.0);
    }
    let mut min_v = f32::MAX;
    let mut max_v = f32::MIN;
    if mode != "elevation" && !mode.is_empty() {
        for z in 0..height {
            for x in 0..width {
                let wx = atlas.origin[0] + x as f32 * spacing_x;
                let wz = atlas.origin[1] + z as f32 * spacing_z;
                let v = preview_value_at_world(atlas, wx, wz, mode);
                min_v = min_v.min(v);
                max_v = max_v.max(v);
            }
        }
    }
    let range = (max_v - min_v).max(0.001);
    for z in 0..height {
        for x in 0..width {
            let wx = atlas.origin[0] + x as f32 * spacing_x;
            let wz = atlas.origin[1] + z as f32 * spacing_z;
            let (r, g, b) = if mode == "elevation" || mode.is_empty() {
                let elev = height_at(wx, wz);
                let land = if elev > sea + 0.25 { 1.0 } else { 0.0 };
                elevation_color_absolute(elev, sea, max_land, land)
            } else {
                let v = preview_value_at_world(atlas, wx, wz, mode);
                let t = ((v - min_v) / range).clamp(0.0, 1.0);
                elevation_color(t, mode)
            };
            let i = ((z as usize * w + x as usize) * 4) as usize;
            pixels[i] = r;
            pixels[i + 1] = g;
            pixels[i + 2] = b;
            pixels[i + 3] = 255;
        }
    }
    (pixels, width, height)
}

/// Colorize like [`colorize_preview`] but sample elevation from runtime terrain height (world XZ).
pub fn colorize_preview_with_heights<F>(atlas: &IslandAtlas, mode: &str, height_at: F) -> Vec<u8>
where
    F: Fn(f32, f32) -> f32,
{
    let side = atlas.width().max(atlas.height());
    colorize_runtime_preview(atlas, mode, side, height_at).0
}

fn preview_value(atlas: &IslandAtlas, x: u32, z: u32, mode: &str) -> f32 {
    let wx = atlas.origin[0] + x as f32 * atlas.spacing_m();
    let wz = atlas.origin[1] + z as f32 * atlas.spacing_m();
    preview_value_at_world(atlas, wx, wz, mode)
}

fn elevation_color_absolute(elev: f32, sea: f32, max_land: f32, land_mask: f32) -> (u8, u8, u8) {
    let on_land = elev > sea + 0.25 && land_mask > 0.5;
    if !on_land {
        let depth = (sea - elev).max(0.0);
        let t = (depth / 35.0).clamp(0.0, 1.0);
        return (
            (18.0 + t * 20.0) as u8,
            (70.0 + t * 50.0) as u8,
            (160.0 + t * 60.0) as u8,
        );
    }
    let t = ((elev - sea) / (max_land - sea).max(1.0)).clamp(0.0, 1.0);
    elevation_color(t, "elevation")
}

fn elevation_color(t: f32, mode: &str) -> (u8, u8, u8) {
    if mode == "river_mask" || mode == "flow_accumulation" {
        return (
            (20.0 + t * 40.0) as u8,
            (80.0 + t * 120.0) as u8,
            (180.0 + t * 60.0) as u8,
        );
    }
    if t < 0.35 {
        let u = t / 0.35;
        (
            (20.0 + u * 30.0) as u8,
            (60.0 + u * 80.0) as u8,
            (120.0 + u * 80.0) as u8,
        )
    } else if t < 0.55 {
        let u = (t - 0.35) / 0.2;
        (
            (50.0 + u * 60.0) as u8,
            (140.0 + u * 50.0) as u8,
            (60.0 + u * 30.0) as u8,
        )
    } else {
        let u = (t - 0.55) / 0.45;
        (
            (110.0 + u * 80.0) as u8,
            (110.0 + u * 70.0) as u8,
            (90.0 + u * 60.0) as u8,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IslandGenParams;

    #[test]
    fn builds_island_atlas_with_fields() {
        let params = IslandGenParams::default();
        let atlas = build_island_atlas(&params);
        assert!(!atlas.elevation_regional.samples.is_empty());
        assert!(!atlas.elevation_local.samples.is_empty());
        assert!(!atlas.validation_messages.is_empty());
    }

    #[test]
    fn validation_report_runs() {
        let params = IslandGenParams::default();
        let atlas = build_island_atlas(&params);
        let report = validate_atlas(&atlas, &params);
        assert!(!report.messages.is_empty());
    }

    #[test]
    fn default_params_pass_validation() {
        // Default params are authored at world scale (mirroring
        // vs3_volcanic_island.yaml) and must build cleanly with no rescaling.
        let params = IslandGenParams::default();
        let atlas = build_island_atlas(&params);
        assert!(
            atlas.validation_passed,
            "validation failed: {:?}",
            atlas.validation_messages
        );
    }

    #[test]
    fn tier_composition_tracks_regional_when_local_detail_disabled() {
        let mut params = IslandGenParams::default();
        params.surface_noise.local_amplitude_m = 0.0;
        params.beaches.width_max_m = 0.0;
        params.beaches.width_min_m = 0.0;
        let atlas = build_island_atlas(&params);
        let wx = atlas.origin[0] + 10.0;
        let wz = atlas.origin[1] + 10.0;
        let composed = atlas.composed_land_elevation_at(wx, wz);
        let regional = atlas.elevation_regional.sample_bilinear(wx, wz);
        assert!(
            (composed - regional).abs() < 2.0,
            "with local detail disabled, composed should track regional"
        );
    }

    #[test]
    fn different_seeds_change_island_shape() {
        let mut low = IslandGenParams::default();
        low.seed = 42;
        let mut high = IslandGenParams::default();
        high.seed = 650_000;
        let a = build_island_atlas(&low);
        let b = build_island_atlas(&high);
        let mut differs = false;
        for (va, vb) in a
            .elevation_regional
            .samples
            .iter()
            .zip(b.elevation_regional.samples.iter())
        {
            if (*va - *vb).abs() > 0.05 {
                differs = true;
                break;
            }
        }
        assert!(
            differs,
            "different seeds should produce different elevation fields"
        );
    }

    #[test]
    fn atlas_spawn_respects_coord_offset_and_high_peaks() {
        use crate::recipe::{RecipeDensitySource, TerrainRecipe};

        let params = IslandGenParams::default();
        let atlas = build_island_atlas(&params);
        let recipe = TerrainRecipe {
            seed: params.seed,
            // Must match IslandGenParams::default().island.sea_level_m (and the
            // water def) -- see validate_island_world_budget's agreement check.
            sea_level: 2.0,
            spawn_x: 70.0,
            spawn_z: 160.0,
            coord_offset: [128.0, 0.0, 128.0],
            ops: Vec::new(),
        };
        let source = RecipeDensitySource::new(recipe).with_atlas(atlas, 3.5);
        let (sx, sy, sz, report) = source.resolve_player_spawn(2.0, 48.0);
        assert!(report.passed, "spawn should resolve: {:?}", report.messages);
        let authored_x = -58.0;
        let authored_z = 32.0;
        let dist = ((sx - authored_x).powi(2) + (sz - authored_z).powi(2)).sqrt();
        assert!(
            dist <= 48.0,
            "spawn ({sx}, {sz}) should stay near authored ({authored_x}, {authored_z})"
        );
        let floor = source.terrain_surface_height_at(sx, sz);
        assert!(
            (sy - floor - 0.05).abs() < 0.55,
            "spawn foot should sit on surface (floor={floor}, sy={sy})"
        );
        // World-scale defaults: relief is 48 m, but (0, 0) sits inside the
        // caldera (radius 10 m, depth 7 m), so the center reads ~41 m, not the
        // rim height. Alpine classification starts at 28 m elevation
        // (biomes.expanded_slice mountain_alpine); assert the summit region
        // clears it with margin rather than asserting the old fitted-island
        // peak of 50+.
        let peak_y = source.surface_height_at(0.0, 0.0);
        assert!(
            peak_y > 35.0,
            "volcano center should remain alpine-capable (peak_y={peak_y})"
        );
        let headroom = source.density_at(sx, sy + 2.0, sz);
        assert!(
            headroom > 0.0,
            "player should have air above spawn (density={headroom})"
        );
    }

    #[test]
    fn runtime_terrain_height_matches_atlas_without_recipe_ops() {
        use crate::recipe::{RecipeDensitySource, TerrainRecipe};

        let params = IslandGenParams::default();
        let atlas = build_island_atlas(&params);
        let recipe = TerrainRecipe {
            seed: params.seed,
            // Must match IslandGenParams::default().island.sea_level_m.
            sea_level: 2.0,
            spawn_x: 70.0,
            spawn_z: 160.0,
            coord_offset: [128.0, 0.0, 128.0],
            ops: Vec::new(),
        };
        let source = RecipeDensitySource::new(recipe).with_atlas(atlas.clone(), 3.5);
        let center_wx =
            atlas.origin[0] + (atlas.width() / 2) as f32 * atlas.elevation_local.spacing;
        let center_wz =
            atlas.origin[1] + (atlas.height() / 2) as f32 * atlas.elevation_local.spacing;
        let center_h = atlas.composed_land_elevation_at(center_wx, center_wz);
        let runtime_center = source.terrain_surface_height_at(0.0, 0.0);
        assert!(
            (center_h - runtime_center).abs() < 0.35,
            "volcano center runtime={runtime_center} atlas={center_h}"
        );
        let (wx, _, wz, report) = source.resolve_player_spawn(2.0, 48.0);
        assert!(report.passed, "spawn should resolve: {:?}", report.messages);
        let spawn_atlas = atlas.surface_height_at(wx, wz);
        let spawn_runtime = source.terrain_surface_height_at(wx, wz);
        assert!(
            (spawn_atlas - spawn_runtime).abs() < 1.0,
            "spawn runtime={spawn_runtime} atlas={spawn_atlas}"
        );
    }

    #[test]
    fn river_descends_post_erosion_filled_surface() {
        let atlas = build_island_atlas(&IslandGenParams::default());
        let river = atlas
            .river_graph
            .as_ref()
            .expect("default island should trace a primary river");
        for window in river.points.windows(2) {
            let p0 = window[0].position_xz;
            let p1 = window[1].position_xz;
            let h0 = atlas.filled_elevation.sample_bilinear(p0[0], p0[1]);
            let h1 = atlas.filled_elevation.sample_bilinear(p1[0], p1[1]);
            assert!(
                h1 <= h0 + 0.5,
                "river segment should descend on post-erosion filled surface: {h0:.2} -> {h1:.2}"
            );
        }
    }

    #[test]
    fn soil_depth_field_is_populated() {
        let atlas = build_island_atlas(&IslandGenParams::default());
        let mut flat_sum = 0.0f32;
        let mut flat_n = 0u32;
        let mut steep_sum = 0.0f32;
        let mut steep_n = 0u32;
        let mut variance_acc = 0.0f32;
        let mut samples = Vec::new();
        for z in 0..atlas.soil_depth.height {
            for x in 0..atlas.soil_depth.width {
                if atlas.island_mask.get(x, z) < 0.4 {
                    continue;
                }
                let soil = atlas.soil_depth.get(x, z);
                samples.push(soil);
                let sl = atlas.slope.get(x, z);
                if sl < 10.0 {
                    flat_sum += soil;
                    flat_n += 1;
                } else if sl > 35.0 {
                    steep_sum += soil;
                    steep_n += 1;
                }
            }
        }
        let mean = samples.iter().sum::<f32>() / samples.len().max(1) as f32;
        for s in &samples {
            variance_acc += (s - mean).powi(2);
        }
        let variance = variance_acc / samples.len().max(1) as f32;
        assert!(
            variance > 1e-4,
            "soil_depth should vary (variance={variance})"
        );
        assert!(
            flat_n > 0 && steep_n > 0,
            "need both flat and steep land cells"
        );
        assert!(
            flat_sum / flat_n as f32 > steep_sum / steep_n as f32,
            "flats should retain more soil than steep slopes"
        );
    }

    /// Diagnostic: `surface_height_at` land/sea switch should not cliff at the coastline.
    #[test]
    fn surface_height_coast_seam_is_continuous() {
        let params = IslandGenParams::default();
        let atlas = build_island_atlas(&params);
        let cx = params.center[0];
        let cz = params.center[1];
        let step = atlas.spacing_m() * 0.25;
        let mut max_step_near_coast = 0.0f32;
        for quadrant in 0..4 {
            let dir = quadrant as f32 * std::f32::consts::FRAC_PI_2;
            let (dx, dz) = (dir.cos(), dir.sin());
            for i in 2..400 {
                let r = i as f32 * step;
                let wx = cx + dx * r;
                let wz = cz + dz * r;
                let mask = atlas.island_mask.sample_bilinear(wx, wz);
                if !(0.15..=0.85).contains(&mask) {
                    continue;
                }
                let h_prev = atlas.surface_height_at(wx - dx * step, wz - dz * step);
                let h = atlas.surface_height_at(wx, wz);
                let h_next = atlas.surface_height_at(wx + dx * step, wz + dz * step);
                max_step_near_coast = max_step_near_coast
                    .max((h - h_prev).abs())
                    .max((h_next - h).abs());
            }
        }
        assert!(
            max_step_near_coast < atlas.spacing_m() * 3.0,
            "coast seam discontinuity {max_step_near_coast:.2} m (threshold {:.2} m)",
            atlas.spacing_m() * 3.0
        );
    }

    /// Diagnostic: beyond-atlas samples continue the offshore slope instead of
    /// snapping to a shallow false seabed.
    #[test]
    fn surface_height_atlas_rim_does_not_snap_to_zero() {
        let atlas = build_island_atlas(&IslandGenParams::default());
        let spacing = atlas.spacing_m();
        let max_x = atlas.origin[0] + (atlas.width() - 1) as f32 * spacing;
        let cz = atlas.origin[1] + (atlas.height() / 2) as f32 * spacing;
        let h_inner = atlas.surface_height_at(max_x - spacing * 2.0, cz);
        let h_edge = atlas.surface_height_at(max_x, cz);
        let h_beyond = atlas.surface_height_at(max_x + spacing * 4.0, cz);
        assert!(
            h_beyond < h_edge,
            "beyond-atlas should deepen past the rim: edge={h_edge} beyond={h_beyond}"
        );
        assert!(
            h_beyond < h_inner,
            "offshore extrapolation should stay below the inner shelf (y={h_beyond}, inner={h_inner})"
        );
    }
}
