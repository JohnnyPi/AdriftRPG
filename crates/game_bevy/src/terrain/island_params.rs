//! Bridge compiled YAML island generation into runtime params.

use game_data::{CompiledIslandGeneration, CompiledWorld, GenerationResolutionDefinition};
use terrain_generation::{
    BeachParams, CaveParams, CoastParams, ErosionParams, GenerationResolution, HydrologyParams,
    IslandGenParams, IslandShapeParams, SurfaceNoiseParams, VolcanoParams,
};

/// Convert authored recipe XZ to world XZ (atlas grid space).
fn recipe_xz_to_world(recipe_x: f32, recipe_z: f32, world: &CompiledWorld) -> [f32; 2] {
    [
        recipe_x - world.coord_offset[0],
        recipe_z - world.coord_offset[2],
    ]
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
    let extent_m = world.ocean_extent_m.unwrap_or(256.0);
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
    // Island atlas samples world coordinates; recipe terrain is centered at coord_offset.
    let center = [0.0, 0.0];
    let volcano_center = recipe_xz_to_world(compiled.volcano.center[0], compiled.volcano.center[1], world);
    let resolution = resolve_generation_resolution(world, compiled);
    IslandGenParams {
        seed,
        center,
        ocean_extent_m: world.ocean_extent_m.unwrap_or(256.0),
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
            thermal_iterations: compiled.erosion.thermal_iterations,
            thermal_transfer_rate: compiled.erosion.thermal_transfer_rate,
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
