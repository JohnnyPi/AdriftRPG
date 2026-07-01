use shared::{DataError, DataResult, StableId};

use crate::definitions::*;

#[derive(Debug, Default)]
pub struct ValidationReport {
    pub errors: Vec<DataError>,
}

impl ValidationReport {
    pub fn push(&mut self, error: DataError) {
        self.errors.push(error);
    }

    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn into_result(self) -> DataResult<()> {
        if self.is_ok() {
            Ok(())
        } else {
            let count = self.errors.len();
            let details = self
                .errors
                .iter()
                .map(|error| format!("- {error}"))
                .collect::<Vec<_>>()
                .join("\n");
            Err(DataError::ValidationFailed { count, details })
        }
    }
}

pub fn validate_definitions(definitions: &[RawDefinition]) -> ValidationReport {
    let mut report = ValidationReport::default();
    let ids = collect_ids(definitions);

    for definition in definitions {
        if let Err(error) = definition.validate_header() {
            report.push(error);
        }

        match definition {
            RawDefinition::Performance(def) => validate_performance(def, &mut report),
            RawDefinition::Player(def) => validate_player(def, &mut report),
            RawDefinition::Camera(def) => validate_camera(def, &mut report),
            RawDefinition::Lighting(def) => validate_lighting(def, &mut report),
            RawDefinition::Water(def) => validate_water(def, &mut report),
            RawDefinition::World(def) => validate_world(def, &ids, &mut report),
            RawDefinition::IslandGeneration(def) => validate_island_generation(def, &mut report),
            RawDefinition::App(def) => validate_references(def, &ids, &mut report),
            _ => {}
        }
    }

    report
}

fn collect_ids(definitions: &[RawDefinition]) -> Vec<StableId> {
    definitions.iter().map(|def| def.id().clone()).collect()
}

fn validate_references(
    app: &AppDefinition,
    ids: &[StableId],
    report: &mut ValidationReport,
) {
    for reference in [&app.world, &app.player, &app.camera, &app.performance] {
        require_reference(reference, "app.yaml", ids, report);
    }
}

fn validate_world(world: &WorldDefinition, ids: &[StableId], report: &mut ValidationReport) {
    if world.voxel.cell_size_m <= 0.0 {
        report.push(DataError::InvalidValue {
            context: format!("world `{}`", world.header.id),
            message: "voxel.cell_size_m must be positive".to_string(),
        });
    } else if (world.voxel.cell_size_m - 1.0).abs() > f32::EPSILON {
        report.push(DataError::InvalidValue {
            context: format!("world `{}`", world.header.id),
            message: "voxel.cell_size_m must be 1.0 until sub-meter voxel indexing is supported"
                .to_string(),
        });
    }

    for (index, cells) in world.chunks.cells.iter().enumerate() {
        if *cells == 0 {
            report.push(DataError::InvalidValue {
                context: format!("world `{}`", world.header.id),
                message: format!("chunks.cells[{index}] must be positive"),
            });
        }
    }

    for (index, extent) in world.chunks.world_extent.iter().enumerate() {
        if *extent == 0 {
            report.push(DataError::InvalidValue {
                context: format!("world `{}`", world.header.id),
                message: format!("chunks.world_extent[{index}] must be positive"),
            });
        }
    }

    for reference in [
        &world.terrain,
        &world.biomes,
        &world.materials,
        &world.water,
        &world.lighting,
    ] {
        require_reference(reference, "world definition", ids, report);
    }

    if let Some(ref resolution) = world.resolution {
        validate_generation_resolution(
            resolution,
            world.ocean_extent_m.unwrap_or(256.0),
            &format!("world `{}`", world.header.id),
            report,
        );
    }
}

fn validate_performance(perf: &PerformanceDefinition, report: &mut ValidationReport) {
    if perf.target_fps == 0 {
        report.push(DataError::InvalidValue {
            context: format!("performance `{}`", perf.header.id),
            message: "target_fps must be positive".to_string(),
        });
    }

    if perf.target_resolution[0] == 0 || perf.target_resolution[1] == 0 {
        report.push(DataError::InvalidValue {
            context: format!("performance `{}`", perf.header.id),
            message: "target_resolution must be positive".to_string(),
        });
    }
}

fn validate_player(player: &PlayerDefinition, report: &mut ValidationReport) {
    if player.capsule.radius_m <= 0.0 || player.capsule.half_height_m <= 0.0 {
        report.push(DataError::InvalidValue {
            context: format!("player `{}`", player.header.id),
            message: "capsule dimensions must be positive".to_string(),
        });
    }

    if player.movement.walk_speed_mps <= 0.0 || player.movement.run_speed_mps <= 0.0 {
        report.push(DataError::InvalidValue {
            context: format!("player `{}`", player.header.id),
            message: "movement speeds must be positive".to_string(),
        });
    }
}

fn validate_camera(camera: &CameraDefinition, report: &mut ValidationReport) {
    let orbit = &camera.orbit;
    if orbit.minimum_distance <= 0.0 || orbit.maximum_distance <= orbit.minimum_distance {
        report.push(DataError::InvalidValue {
            context: format!("camera `{}`", camera.header.id),
            message: "orbit.minimum_distance and orbit.maximum_distance must define a positive range"
                .to_string(),
        });
    }

    if orbit.default_distance < orbit.minimum_distance
        || orbit.default_distance > orbit.maximum_distance
    {
        report.push(DataError::InvalidValue {
            context: format!("camera `{}`", camera.header.id),
            message: "orbit.default_distance must be within [minimum_distance, maximum_distance]"
                .to_string(),
        });
    }

    if orbit.minimum_pitch_degrees > orbit.maximum_pitch_degrees {
        report.push(DataError::InvalidValue {
            context: format!("camera `{}`", camera.header.id),
            message: "orbit.minimum_pitch_degrees must be <= orbit.maximum_pitch_degrees"
                .to_string(),
        });
    }

    if orbit.default_pitch_degrees < orbit.minimum_pitch_degrees
        || orbit.default_pitch_degrees > orbit.maximum_pitch_degrees
    {
        report.push(DataError::InvalidValue {
            context: format!("camera `{}`", camera.header.id),
            message: "orbit.default_pitch_degrees must be within [minimum_pitch_degrees, maximum_pitch_degrees]"
                .to_string(),
        });
    }

    if camera.collision.radius <= 0.0 {
        report.push(DataError::InvalidValue {
            context: format!("camera `{}`", camera.header.id),
            message: "collision.radius must be positive".to_string(),
        });
    }
}

fn validate_lighting(lighting: &LightingDefinition, report: &mut ValidationReport) {
    if lighting.fog.enabled && lighting.fog.end_m <= lighting.fog.start_m {
        report.push(DataError::InvalidValue {
            context: format!("lighting `{}`", lighting.header.id),
            message: "fog.end_m must be greater than fog.start_m".to_string(),
        });
    }
}

fn validate_water(water: &WaterDefinition, report: &mut ValidationReport) {
    if !(0.0..=1.0).contains(&water.transparency) {
        report.push(DataError::InvalidValue {
            context: format!("water `{}`", water.header.id),
            message: "transparency must be in [0, 1]".to_string(),
        });
    }
}

fn validate_island_generation(island: &IslandGenerationDefinition, report: &mut ValidationReport) {
    if let Some(ref resolution) = island.resolution {
        validate_generation_resolution(
            resolution,
            288.0,
            &format!("island generation `{}`", island.header.id),
            report,
        );
    }
}

fn validate_generation_resolution(
    resolution: &GenerationResolutionDefinition,
    extent_m: f32,
    context: &str,
    report: &mut ValidationReport,
) {
    let defaults = default_resolution_for_extent(extent_m);
    let voxel_m = resolution.voxel_m.unwrap_or(defaults.voxel_m);
    let local_m = resolution.local_m.unwrap_or(defaults.local_m);
    let regional_m = resolution.regional_m.unwrap_or(defaults.regional_m);
    let world_control_m = resolution
        .world_control_m
        .unwrap_or(defaults.world_control_m);

    if (voxel_m - 1.0).abs() > f32::EPSILON {
        report.push(DataError::InvalidValue {
            context: context.to_string(),
            message: "resolution.voxel_m must equal 1.0".to_string(),
        });
    }

    if !(8.0..=128.0).contains(&regional_m) {
        report.push(DataError::InvalidValue {
            context: context.to_string(),
            message: "resolution.regional_m must be between 8 and 128".to_string(),
        });
    }

    if !(1.0..=8.0).contains(&local_m) {
        report.push(DataError::InvalidValue {
            context: context.to_string(),
            message: "resolution.local_m must be between 1 and 8".to_string(),
        });
    }

    if world_control_m > 0.0 && world_control_m < 250.0 {
        report.push(DataError::InvalidValue {
            context: context.to_string(),
            message: "resolution.world_control_m must be at least 250 when enabled".to_string(),
        });
    }

    let wc = if world_control_m > 0.0 {
        world_control_m
    } else {
        f32::MAX
    };
    if wc + f32::EPSILON < regional_m
        || regional_m + f32::EPSILON < local_m
        || local_m + f32::EPSILON < voxel_m
    {
        report.push(DataError::InvalidValue {
            context: context.to_string(),
            message: "resolution tiers must be coarse to fine: world >= regional >= local >= voxel"
                .to_string(),
        });
    }

    if world_control_m > 0.0 && !integer_ratio(world_control_m, regional_m) {
        report.push(DataError::InvalidValue {
            context: context.to_string(),
            message: "world_control_m must divide evenly into regional_m".to_string(),
        });
    }
    if !integer_ratio(regional_m, local_m) {
        report.push(DataError::InvalidValue {
            context: context.to_string(),
            message: "regional_m must divide evenly into local_m".to_string(),
        });
    }
    if !integer_ratio(local_m, voxel_m) {
        report.push(DataError::InvalidValue {
            context: context.to_string(),
            message: "local_m must divide evenly into voxel_m".to_string(),
        });
    }
}

fn default_resolution_for_extent(extent_m: f32) -> ResolvedGenerationResolution {
    if extent_m <= 512.0 {
        ResolvedGenerationResolution {
            world_control_m: 0.0,
            regional_m: 8.0,
            local_m: 4.0,
            voxel_m: 1.0,
        }
    } else if extent_m <= 8_000.0 {
        ResolvedGenerationResolution {
            world_control_m: 512.0,
            regional_m: 32.0,
            local_m: 4.0,
            voxel_m: 1.0,
        }
    } else {
        ResolvedGenerationResolution {
            world_control_m: 1024.0,
            regional_m: 64.0,
            local_m: 8.0,
            voxel_m: 1.0,
        }
    }
}

struct ResolvedGenerationResolution {
    world_control_m: f32,
    regional_m: f32,
    local_m: f32,
    voxel_m: f32,
}

fn integer_ratio(coarse: f32, fine: f32) -> bool {
    if fine <= f32::EPSILON {
        return false;
    }
    let ratio = coarse / fine;
    (ratio - ratio.round()).abs() < 0.001 && ratio >= 1.0
}

fn require_reference(
    reference: &StableId,
    context: &str,
    ids: &[StableId],
    report: &mut ValidationReport,
) {
    if !ids.iter().any(|id| id == reference) {
        report.push(DataError::UnknownReference {
            reference: reference.clone(),
            context: context.to_string(),
        });
    }
}
