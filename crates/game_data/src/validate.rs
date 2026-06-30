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
