// crates/terrain_generation/src/world_setup.rs
//! Shared world/recipe setup from compiled YAML (used by runtime and diagnostics).
//!
//! Also home to [`validate_island_world_budget`]: the loud, message-producing
//! check that an authored island actually fits the world that hosts it
//! (horizontal footprint vs. chunk extents, vertical relief vs. the chunk
//! ceiling/floor, sea-level agreement between the water and island defs, and
//! whether `fit_to_ocean_extent` would have silently rescaled). Callers that
//! construct atlas worlds should surface these messages instead of letting the
//! generator absorb a contradictory configuration.

use game_data::{
    CompiledIslandGeneration, CompiledWater, CompiledWorld, ConfigRegistry,
    GenerationResolutionDefinition, TerrainOperationDefinition,
};
use shared::StableId;

use crate::{
    build_island_atlas, default_vertical_slice_recipe, BeachParams, CaveParams, CoastParams,
    CoastModifierKind, CombineOp, ErosionParams, GenerationResolution, HydrologyParams,
    IslandGenParams, IslandShapeParams, RecipeDensitySource, RecipeOp, SurfaceNoiseParams,
    TerrainRecipe, VolcanoParams, WorldVolumeBounds,
};

/// Convert authored recipe XZ to world XZ (atlas grid space).
fn recipe_xz_to_world(recipe_x: f32, recipe_z: f32, world: &CompiledWorld) -> [f32; 2] {
    [
        recipe_x - world.coord_offset[0],
        recipe_z - world.coord_offset[2],
    ]
}

/// Padding kept between the island footprint and the atlas rim when deriving
/// extent from the chunk volume (see `CompiledWorld::DERIVED_OCEAN_PADDING_M`).
pub const DERIVED_OCEAN_PADDING_M: f32 = game_data::CompiledWorld::DERIVED_OCEAN_PADDING_M;

/// Atlas square extent that fully covers the world's horizontal chunk volume.
pub fn effective_ocean_extent_m(world: &CompiledWorld) -> f32 {
    world.effective_ocean_extent_m()
}

fn merge_resolution(
    yaml: &GenerationResolutionDefinition,
    extent_m: f32,
) -> GenerationResolution {
    let defaults = GenerationResolution::for_extent(extent_m);
    GenerationResolution {
        world_control_m: yaml.world_control_m.unwrap_or(defaults.world_control_m),
        regional_m: yaml.regional_m.unwrap_or(defaults.regional_m),
        local_m: yaml.local_m.unwrap_or(defaults.local_m),
        voxel_m: yaml.voxel_m.unwrap_or(defaults.voxel_m),
    }
}

fn resolve_generation_resolution(
    world: &CompiledWorld,
    compiled: &CompiledIslandGeneration,
) -> GenerationResolution {
    let extent_m = effective_ocean_extent_m(world);
    if let Some(ref island_res) = compiled.resolution {
        return merge_resolution(island_res, extent_m);
    }
    if let Some(ref world_res) = world.resolution {
        return merge_resolution(world_res, extent_m);
    }
    GenerationResolution::for_extent(extent_m)
}

pub fn island_params_from_compiled(
    compiled: &CompiledIslandGeneration,
    world: &CompiledWorld,
    seed: u64,
    sea_level_m: f32,
) -> IslandGenParams {
    let center = [0.0, 0.0];
    let volcano_center =
        recipe_xz_to_world(compiled.volcano.center[0], compiled.volcano.center[1], world);
    let resolution = resolve_generation_resolution(world, compiled);
    IslandGenParams {
        seed,
        center,
        ocean_extent_m: effective_ocean_extent_m(world),
        resolution,
        island: IslandShapeParams {
            playable_diameter_m: compiled.island.playable_diameter_m,
            maximum_height_m: compiled.island.maximum_height_m,
            sea_level_m,
            lobe_count: compiled.island.lobe_count,
            warp_frequency: compiled.island.warp_frequency,
            warp_amplitude: compiled.island.warp_amplitude,
        },
        volcano: VolcanoParams {
            center: volcano_center,
            shield_radius_m: compiled.volcano.shield_radius_m,
            shield_exponent: compiled.volcano.shield_exponent,
            shield_height_m: compiled.volcano.shield_height_m,
            summit_radius_m: compiled.volcano.summit_radius_m,
            summit_exponent: compiled.volcano.summit_exponent,
            summit_height_m: compiled.volcano.summit_height_m,
            caldera_radius_m: compiled.volcano.caldera_radius_m,
            caldera_depth_m: compiled.volcano.caldera_depth_m,
            caldera_rim_height_m: compiled.volcano.caldera_rim_height_m,
            radial_ridge_count: compiled.volcano.radial_ridge_count,
            collapse_direction_deg: compiled.volcano.collapse_direction_deg,
            collapse_depth_m: compiled.volcano.collapse_depth_m,
        },
        surface_noise: SurfaceNoiseParams {
            regional_amplitude_m: compiled.surface_noise.regional_amplitude_m,
            local_amplitude_m: compiled.surface_noise.local_amplitude_m,
            voxel_amplitude_m: compiled.surface_noise.voxel_amplitude_m,
        },
        hydrology: HydrologyParams {
            rainfall_base: compiled.hydrology.rainfall_base,
            stream_threshold: compiled.hydrology.stream_threshold,
            permanent_river_threshold: compiled.hydrology.permanent_river_threshold,
            minimum_stream_length_m: compiled.hydrology.minimum_stream_length_m,
        },
        erosion: ErosionParams {
            stream_power_iterations: compiled.erosion.stream_power_iterations,
            m: compiled.erosion.m,
            n: compiled.erosion.n,
            maximum_step_m: compiled.erosion.maximum_step_m,
            stream_power_erodibility: 0.00002,
            thermal_iterations: compiled.erosion.thermal_iterations,
            thermal_transfer_rate: compiled.erosion.thermal_transfer_rate,
            thermal_talus_deg: 38.0,
            river_bank_width_m: 3.5,
            river_carve_strength: 1.2,
        },
        coast: CoastParams {
            shelf_width_min_m: compiled.coast.shelf_width_min_m,
            shelf_width_max_m: compiled.coast.shelf_width_max_m,
            shelf_depth_min_m: compiled.coast.shelf_depth_min_m,
            shelf_depth_max_m: compiled.coast.shelf_depth_max_m,
            deep_slope_min: compiled.coast.deep_slope_min,
            deep_slope_max: compiled.coast.deep_slope_max,
        },
        beaches: BeachParams {
            maximum_slope_deg: compiled.beaches.maximum_slope_deg,
            width_min_m: compiled.beaches.width_min_m,
            width_max_m: compiled.beaches.width_max_m,
            berm_height_min_m: compiled.beaches.berm_height_min_m,
            berm_height_max_m: compiled.beaches.berm_height_max_m,
        },
        caves: CaveParams {
            chamber_count_min: compiled.caves.chamber_count_min,
            chamber_count_max: compiled.caves.chamber_count_max,
            passage_radius_min_m: compiled.caves.passage_radius_min_m,
            passage_radius_max_m: compiled.caves.passage_radius_max_m,
            minimum_cover_m: compiled.caves.minimum_cover_m,
            maximum_depth_m: compiled.caves.maximum_depth_m,
            overhang_enabled: compiled.caves.overhang_enabled,
        },
    }
}


/// Validate that an authored island fits inside the world that hosts it.
///
/// Returns human-readable messages; an empty vector means the configuration is
/// coherent. This is the loud replacement for `fit_to_ocean_extent`'s silent
/// (and distorting) rescale: authoring errors should fail with an explanation,
/// not be absorbed into a warped miniature.
///
/// Checks performed:
/// 1. Horizontal: island footprint support (lobe offsets + falloff + warp) must
///    fit inside the chunk volume's XZ extents, and inside the atlas
///    (`fit_to_ocean_extent` must be a no-op).
/// 2. Vertical ceiling: `maximum_height_m` plus surface-noise amplitudes must
///    stay below the chunk volume's top, and the composed volcano relief must
///    not exceed the declared `maximum_height_m`.
/// 3. Vertical floor: the coastal shelf (plus deep-falloff slack) must stay
///    above the chunk volume's bottom.
/// 4. Sea level: the water def's sea level must sit inside the world's Y range
///    and agree with the island def's `sea_level_m` (cave placement in
///    `append_generated_island_caves` reads the island value).
pub fn validate_island_world_budget(
    compiled: &CompiledIslandGeneration,
    world: &CompiledWorld,
    water_sea_level_m: f32,
) -> Vec<String> {
    let mut messages = Vec::new();
    let params = island_params_from_compiled(compiled, world, world.seed, water_sea_level_m);

    let (mins, maxs) = world.axis_bounds_m();
    let (x_min, y_min, z_min) = (mins[0], mins[1], mins[2]);
    let (x_max, y_max, z_max) = (maxs[0], maxs[1], maxs[2]);

    if let Some(authored) = world.ocean_extent_m {
        let span = world.horizontal_extent_m();
        if authored + 0.01 < span {
            messages.push(format!(
                "ocean_extent_m {authored:.0} is smaller than the chunk volume horizontal \
                 span {span:.0}; runtime generation expands the atlas to the world size, but \
                 YAML should match to avoid confusion."
            ));
        }
    }

    // --- 1. Horizontal footprint vs. chunk volume and atlas -----------------
    let support_radius = params.island.footprint_support_radius_m();
    let horizontal_budget = x_min
        .abs()
        .min(x_max)
        .min(z_min.abs())
        .min(z_max);
    if support_radius > horizontal_budget {
        messages.push(format!(
            "island footprint support radius {support_radius:.1} m (diameter \
             {diameter:.0} m incl. lobe offsets, falloff, and {warp:.1} m warp) exceeds \
             the chunk volume's horizontal half-extent {horizontal_budget:.1} m; the \
             coastline will clip at the world edge. Reduce playable_diameter_m or \
             warp_amplitude, or enlarge chunks.world_extent.",
            diameter = params.island.playable_diameter_m,
            warp = params.island.warp_amplitude,
        ));
    }
    let max_fit = params.max_fit_diameter_m();
    if params.island.playable_diameter_m > max_fit {
        let scale = max_fit / params.island.playable_diameter_m;
        messages.push(format!(
            "playable_diameter_m {diameter:.0} does not fit ocean_extent_m {extent:.0} \
             (maximum {max_fit:.0} m); fit_to_ocean_extent would silently rescale by \
             {scale:.3} horizontally and {vscale:.3} vertically, distorting the authored \
             shape. Author the island at world scale instead.",
            diameter = params.island.playable_diameter_m,
            extent = params.ocean_extent_m,
            vscale = scale.sqrt().clamp(0.25, 1.0),
        ));
    }

    // --- 2. Vertical ceiling -------------------------------------------------
    const CEILING_MARGIN_M: f32 = 2.0;
    let noise_headroom =
        params.surface_noise.regional_amplitude_m + params.surface_noise.local_amplitude_m;
    let worst_case_peak = params.island.maximum_height_m + noise_headroom;
    if worst_case_peak > y_max - CEILING_MARGIN_M {
        messages.push(format!(
            "maximum_height_m {height:.1} + noise amplitudes {noise:.1} = \
             {peak:.1} m exceeds the chunk volume ceiling {ceiling:.1} m (minus \
             {margin:.0} m margin); the summit will clip flat at the world top.",
            height = params.island.maximum_height_m,
            noise = noise_headroom,
            peak = worst_case_peak,
            ceiling = y_max,
            margin = CEILING_MARGIN_M,
        ));
    }
    let composed_relief = params.volcano.shield_height_m
        + params.volcano.summit_height_m
        + params.volcano.caldera_rim_height_m;
    if composed_relief > params.island.maximum_height_m {
        messages.push(format!(
            "composed volcano relief (shield {shield:.1} + summit {summit:.1} + rim \
             {rim:.1} = {relief:.1} m) exceeds declared maximum_height_m {max:.1}; \
             raise maximum_height_m or lower the volcano heights so validation and \
             classification thresholds see the true peak.",
            shield = params.volcano.shield_height_m,
            summit = params.volcano.summit_height_m,
            rim = params.volcano.caldera_rim_height_m,
            relief = composed_relief,
            max = params.island.maximum_height_m,
        ));
    }

    // --- 3. Vertical floor ---------------------------------------------------
    // The bathymetry continues past the shelf on the deep slope; reserve slack
    // for that falloff rather than letting the ocean floor clamp at the chunk
    // bottom into a vertical wall.
    const DEEP_FALLOFF_SLACK_M: f32 = 8.0;
    let shelf_floor = water_sea_level_m - params.coast.shelf_depth_max_m - DEEP_FALLOFF_SLACK_M;
    if shelf_floor < y_min {
        messages.push(format!(
            "coastal shelf bottom (sea {sea:.1} - shelf_depth_max_m {depth:.1} - \
             {slack:.0} m deep-falloff slack = {floor:.1} m) is below the chunk volume \
             floor {bottom:.1} m; the ocean floor will clamp into a vertical wall.",
            sea = water_sea_level_m,
            depth = params.coast.shelf_depth_max_m,
            slack = DEEP_FALLOFF_SLACK_M,
            floor = shelf_floor,
            bottom = y_min,
        ));
    }

    // --- 4. Sea level --------------------------------------------------------
    if water_sea_level_m <= y_min || water_sea_level_m >= y_max {
        messages.push(format!(
            "water sea level {water_sea_level_m:.1} m is outside the chunk volume \
             Y range [{y_min:.1}, {y_max:.1})."
        ));
    }
    let island_sea = compiled.island.sea_level_m;
    if (island_sea - water_sea_level_m).abs() > 0.01 {
        messages.push(format!(
            "island_gen sea_level_m {island_sea:.2} disagrees with the world's water \
             def sea level {water_sea_level_m:.2}; the runtime uses the water value \
             for the atlas but generated cave placement reads the island value \
             (cave_center_height). Set them equal.",
        ));
    }

    messages
}

pub fn compile_terrain_recipe(
    registry: &ConfigRegistry,
    world: &CompiledWorld,
    water: &CompiledWater,
    seed_override: Option<u64>,
) -> TerrainRecipe {
    compile_terrain_recipe_with_island(registry, world, water, seed_override, None)
}

/// Like [`compile_terrain_recipe`], but generated island cave ops are built
/// from `island_gen_override` when provided instead of the registry's base
/// definition.
///
/// Callers that apply user overrides to the island def (e.g. setup-screen
/// prefs) must pass the merged def here; otherwise the cave geometry is
/// compiled from the base def while the atlas and the terrain-recipe hash use
/// the merged one, so cave-parameter overrides change the hash without
/// changing the terrain (or vice versa after a cache hit).
pub fn compile_terrain_recipe_with_island(
    registry: &ConfigRegistry,
    world: &CompiledWorld,
    water: &CompiledWater,
    seed_override: Option<u64>,
    island_gen_override: Option<&CompiledIslandGeneration>,
) -> TerrainRecipe {
    let terrain = registry
        .terrain
        .get(&world.terrain)
        .expect("terrain definition");

    let mut ops = Vec::new();
    for op_def in &terrain.operations {
        ops.push(convert_op(op_def));
    }
    for include in &terrain.includes {
        if let Some(cave) = registry.caves.get(include) {
            for op_def in &cave.operations {
                ops.push(convert_op(op_def));
            }
        }
    }
    let island_gen = island_gen_override.or_else(|| registry.island_generation_for_world(world));
    if let Some(island_gen) = island_gen {
        append_generated_island_caves(&mut ops, island_gen, seed_override.unwrap_or(world.seed));
    }

    // The hardcoded fallback slice exists so op-based worlds without any
    // authored operations still produce terrain. Atlas worlds get their terrain
    // from the island generator; injecting the fallback underneath the atlas
    // superimposes a phantom surface on the generated island (and previously
    // did exactly that whenever generated caves were disabled).
    if ops.is_empty() && island_gen.is_none() {
        return default_vertical_slice_recipe(
            seed_override.unwrap_or(world.seed),
            water.sea_level_m,
        );
    }

    let (spawn_x, spawn_z) = terrain
        .spawn
        .map(|s| (s[0], s[2]))
        .unwrap_or((-30.0, -25.0));

    TerrainRecipe {
        seed: seed_override.unwrap_or(world.seed),
        sea_level: water.sea_level_m,
        spawn_x,
        spawn_z,
        coord_offset: world.coord_offset,
        ops,
    }
}

/// Island-atlas density source matching the runtime compile path (for diagnostics/tests).
///
/// Panics with the full message list if [`validate_island_world_budget`] fails:
/// diagnostics and tests must not run against a world/island configuration the
/// generator would have to distort or clip to satisfy.
pub fn build_atlas_density_source(
    registry: &ConfigRegistry,
    world_id: &StableId,
    seed: u64,
) -> RecipeDensitySource {
    let world = registry.world_by_id(world_id).expect("world");
    let water = registry.water.get(&world.water).expect("water");
    let base = registry
        .island_generation_for_world(world)
        .expect("island gen");
    let mut merged = base.clone();
    merged.seed = seed;

    let budget_messages = validate_island_world_budget(&merged, world, water.sea_level_m);
    if !budget_messages.is_empty() {
        panic!(
            "island/world budget validation failed for {world_id:?}:\n  - {}",
            budget_messages.join("\n  - ")
        );
    }

    let params = island_params_from_compiled(&merged, world, seed, water.sea_level_m);
    let atlas = build_island_atlas(&params);
    let recipe =
        compile_terrain_recipe_with_island(registry, world, water, Some(seed), Some(&merged));
    let bounds = WorldVolumeBounds::from_compiled_world(world);
    RecipeDensitySource::new(recipe)
        .with_world_bounds(bounds)
        .with_atlas(atlas, 3.5)
}

pub fn append_generated_island_caves(
    ops: &mut Vec<RecipeOp>,
    island_gen: &CompiledIslandGeneration,
    seed: u64,
) {
    let caves = &island_gen.caves;
    if caves.chamber_count_max == 0 || caves.passage_radius_max_m <= 0.0 {
        return;
    }

    let min = caves.chamber_count_min;
    let max = caves.chamber_count_max.max(min);
    if max == 0 {
        return;
    }
    let count = if min == max {
        min as usize
    } else {
        min as usize + (hash_unit(seed, 0) * (max - min) as f32).round() as usize
    };
    let base_angle = island_gen.volcano.collapse_direction_deg.to_radians();
    let base_radius = island_gen.volcano.shield_radius_m * 0.38;
    let radius_span = island_gen.volcano.shield_radius_m * 0.18;
    let min_passage = caves.passage_radius_min_m.max(0.6);
    let max_passage = caves.passage_radius_max_m.max(min_passage);
    let mut previous = None;

    for index in 0..count {
        let t = if count == 1 {
            0.5
        } else {
            index as f32 / (count - 1) as f32
        };
        let angle_jitter = hash_unit(seed, index as u32) - 0.5;
        let angle = base_angle + (t - 0.5) * 0.7 + angle_jitter * 0.2;
        let radial = base_radius + radius_span * t;
        let chamber_radius = lerp(
            min_passage,
            max_passage,
            0.35 + 0.5 * hash_unit(seed ^ 0xA5A5_A5A5, index as u32),
        );
        let center = [
            island_gen.volcano.center[0] + radial * angle.cos(),
            cave_center_height(island_gen, t),
            island_gen.volcano.center[1] + radial * angle.sin(),
        ];
        ops.push(RecipeOp::Ellipsoid {
            center,
            radii: [chamber_radius * 1.7, chamber_radius, chamber_radius * 1.5],
            peak_noise: None,
            combine: CombineOp::Subtract,
        });
        if let Some(previous_center) = previous {
            ops.push(RecipeOp::Capsule {
                start: previous_center,
                end: center,
                radius: chamber_radius.min(max_passage) * 0.72,
                combine: CombineOp::Subtract,
            });
        }
        previous = Some(center);
    }

    if caves.overhang_enabled {
        let mouth_radius = min_passage * 1.15;
        let mouth = [
            island_gen.volcano.center[0] + (base_radius + radius_span * 1.15) * base_angle.cos(),
            (island_gen.island.sea_level_m + caves.minimum_cover_m + mouth_radius)
                .min(cave_center_height(island_gen, 0.1)),
            island_gen.volcano.center[1] + (base_radius + radius_span * 1.15) * base_angle.sin(),
        ];
        if let Some(last_center) = previous {
            ops.push(RecipeOp::Capsule {
                start: last_center,
                end: mouth,
                radius: mouth_radius,
                combine: CombineOp::Subtract,
            });
        }
    }
}

fn cave_center_height(island_gen: &CompiledIslandGeneration, t: f32) -> f32 {
    let caves = &island_gen.caves;
    let base = island_gen.island.sea_level_m + caves.minimum_cover_m + 6.0;
    let depth_span = caves.maximum_depth_m * (0.18 + 0.18 * t);
    let floor = island_gen.island.sea_level_m + caves.minimum_cover_m + 1.0;
    let ceiling_limit = island_gen.island.maximum_height_m * 0.28;
    (base - depth_span).clamp(floor, ceiling_limit.max(floor + 2.0))
}

fn hash_unit(seed: u64, index: u32) -> f32 {
    let mut value = seed
        ^ (index as u64)
            .wrapping_add(1)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    ((value >> 40) as u32) as f32 / u32::MAX as f32
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn convert_op(def: &TerrainOperationDefinition) -> RecipeOp {
    match def {
        TerrainOperationDefinition::CoastalSurface {
            origin,
            scale,
            base_height,
            height_range,
            ridge_origin,
            ridge_scale,
            ridge_amplitude,
            detail_frequency,
            detail_amplitude,
            detail_octaves,
            regional_frequency,
            regional_amplitude,
            local_frequency,
            local_amplitude,
            ridged_amplitude,
            domain_warp,
        } => RecipeOp::CoastalSurface {
            origin: *origin,
            scale: *scale,
            base_height: *base_height,
            height_range: *height_range,
            ridge_origin: *ridge_origin,
            ridge_scale: *ridge_scale,
            ridge_amplitude: *ridge_amplitude,
            detail_frequency: *detail_frequency,
            detail_amplitude: *detail_amplitude,
            detail_octaves: *detail_octaves,
            regional_frequency: *regional_frequency,
            regional_amplitude: *regional_amplitude,
            local_frequency: *local_frequency,
            local_amplitude: *local_amplitude,
            ridged_amplitude: *ridged_amplitude,
            domain_warp: *domain_warp,
        },
        TerrainOperationDefinition::ValleyBasin {
            origin,
            scale,
            depth_m,
        } => RecipeOp::ValleyBasin {
            origin: *origin,
            scale: *scale,
            depth_m: *depth_m,
        },
        TerrainOperationDefinition::CoastModifier {
            kind,
            center,
            radius_m,
            depth_m,
            min_land_factor,
            max_land_factor,
        } => RecipeOp::CoastModifier {
            kind: parse_coast_modifier_kind(kind),
            center: *center,
            radius_m: *radius_m,
            depth_m: *depth_m,
            min_land_factor: *min_land_factor,
            max_land_factor: *max_land_factor,
        },
        TerrainOperationDefinition::Ellipsoid {
            center,
            radii,
            peak_noise,
            combine,
        } => RecipeOp::Ellipsoid {
            center: *center,
            radii: *radii,
            peak_noise: peak_noise.map(|p| (p[0], p[1])),
            combine: parse_combine(combine),
        },
        TerrainOperationDefinition::Capsule {
            start,
            end,
            radius,
            combine,
        } => RecipeOp::Capsule {
            start: *start,
            end: *end,
            radius: *radius,
            combine: parse_combine(combine),
        },
        TerrainOperationDefinition::NoisePerturb {
            scale,
            amplitude,
            density_min,
            density_max,
        } => RecipeOp::NoisePerturb {
            scale: *scale,
            amplitude: *amplitude,
            density_min: *density_min,
            density_max: *density_max,
        },
        TerrainOperationDefinition::IslandMask {
            center,
            radius_m,
            falloff_m,
            ocean_floor_y,
            domain_warp,
        } => RecipeOp::IslandMask {
            center: *center,
            radius_m: *radius_m,
            falloff_m: *falloff_m,
            ocean_floor_y: *ocean_floor_y,
            domain_warp: *domain_warp,
        },
        TerrainOperationDefinition::OceanFloor {
            origin,
            scale,
            base_depth_m,
            variation_m,
            detail_frequency,
            detail_octaves,
        } => RecipeOp::OceanFloor {
            origin: *origin,
            scale: *scale,
            base_depth_m: *base_depth_m,
            variation_m: *variation_m,
            detail_frequency: *detail_frequency,
            detail_octaves: *detail_octaves,
        },
        TerrainOperationDefinition::MountainPeak {
            center,
            base_elevation_m,
            base_radius_m,
            peak_height_m,
            steepness,
            peak_noise,
        } => RecipeOp::MountainPeak {
            center: *center,
            base_elevation_m: *base_elevation_m,
            base_radius_m: *base_radius_m,
            peak_height_m: *peak_height_m,
            steepness: *steepness,
            peak_noise: peak_noise.map(|p| (p[0], p[1])),
        },
        TerrainOperationDefinition::UnderwaterTrench { points, width_m } => {
            RecipeOp::UnderwaterTrench {
                points: points.clone(),
                width_m: *width_m,
            }
        }
    }
}

fn parse_combine(value: &str) -> CombineOp {
    match value.to_ascii_lowercase().as_str() {
        "union" => CombineOp::Union,
        "subtract" => CombineOp::Subtract,
        other => panic!("invalid combine op '{other}' (should have been rejected at validation)"),
    }
}

fn parse_coast_modifier_kind(value: &str) -> CoastModifierKind {
    match value.to_ascii_lowercase().as_str() {
        "harbor" => CoastModifierKind::Harbor,
        "cove" => CoastModifierKind::Cove,
        other => panic!("invalid coast modifier '{other}' (should have been rejected at validation)"),
    }
}