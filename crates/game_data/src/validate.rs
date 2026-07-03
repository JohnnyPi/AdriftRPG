// crates/game_data/src/validate.rs
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
            RawDefinition::TerrainGeneration(def) => validate_terrain_generation(def, &ids, &mut report),
            RawDefinition::TerrainMaterials(def) => validate_terrain_materials(def, &mut report),
            RawDefinition::SurfaceRules(def) => validate_surface_rules(def, &mut report),
            RawDefinition::Cave(def) => validate_cave(def, &mut report),
            RawDefinition::App(def) => validate_references(def, &ids, &mut report),
            _ => {}
        }
    }

    validate_cross_definition_links(definitions, &mut report);
    report
}

/// Cross-check surface rules reference valid palette materials.
fn validate_cross_definition_links(definitions: &[RawDefinition], report: &mut ValidationReport) {
    let palettes: std::collections::BTreeMap<StableId, std::collections::BTreeSet<StableId>> =
        definitions
            .iter()
            .filter_map(|def| match def {
                RawDefinition::TerrainMaterials(materials) => {
                    let keys = materials
                        .materials
                        .iter()
                        .map(|entry| entry.resolved_key())
                        .collect();
                    Some((materials.header.id.clone(), keys))
                }
                _ => None,
            })
            .collect();

    for definition in definitions {
        let RawDefinition::SurfaceRules(rules) = definition else {
            continue;
        };
        let context = format!("surface `{}`", rules.header.id);
        for gate in &rules.gates {
            for entry in &gate.blend {
                // validated when world links palette; keys checked below if palette known
                let _ = &entry.material;
            }
            if let Some(ref classifier_id) = gate.classifier {
                if !rules
                    .classifiers
                    .iter()
                    .any(|c| c.id == *classifier_id)
                {
                    report.push(DataError::InvalidValue {
                        context: context.clone(),
                        message: format!(
                            "gate `{}` references unknown classifier `{classifier_id}`",
                            gate.id
                        ),
                    });
                }
            }
        }
        for classifier in &rules.classifiers {
            for entry in &classifier.blend {
                let _ = &entry.material;
            }
            for mix in &classifier.weighted_mix {
                if !rules
                    .classifiers
                    .iter()
                    .any(|c| c.id == mix.classifier)
                {
                    report.push(DataError::InvalidValue {
                        context: context.clone(),
                        message: format!(
                            "classifier `{}` references unknown classifier `{}`",
                            classifier.id, mix.classifier
                        ),
                    });
                }
            }
        }
        // If a palette with matching suffix exists, validate material keys.
        let palette_suffix = rules
            .header
            .id
            .as_str()
            .strip_prefix("surface.")
            .unwrap_or(rules.header.id.as_str());
        let palette_id = StableId::new(&format!("materials.{palette_suffix}"));
        if let Some(keys) = palettes.get(&palette_id) {
            for gate in &rules.gates {
                for entry in &gate.blend {
                    if !keys.contains(&entry.material) {
                        report.push(DataError::InvalidValue {
                            context: context.clone(),
                            message: format!(
                                "gate `{}` references unknown material `{}`",
                                gate.id, entry.material
                            ),
                        });
                    }
                }
            }
            for classifier in &rules.classifiers {
                for entry in &classifier.blend {
                    if !keys.contains(&entry.material) {
                        report.push(DataError::InvalidValue {
                            context: context.clone(),
                            message: format!(
                                "classifier `{}` references unknown material `{}`",
                                classifier.id, entry.material
                            ),
                        });
                    }
                }
            }
        }
    }

    for definition in definitions {
        let RawDefinition::TerrainMaterials(materials) = definition else {
            continue;
        };
        let context = format!("materials `{}`", materials.header.id);
        let keys: std::collections::BTreeSet<_> = materials
            .materials
            .iter()
            .map(|entry| entry.resolved_key())
            .collect();
        if keys.is_empty() {
            report.push(DataError::InvalidValue {
                context: context.clone(),
                message: "materials must declare at least one entry".to_string(),
            });
        }
        if keys.len() != materials.materials.len() {
            report.push(DataError::InvalidValue {
                context: context.clone(),
                message: "duplicate material keys in palette".to_string(),
            });
        }
        let layer_order = if materials.layers.is_empty() {
            let mut ordered: Vec<_> = materials.materials.iter().collect();
            ordered.sort_by_key(|m| m.resolved_legacy_id());
            ordered
                .into_iter()
                .map(|m| m.resolved_key())
                .collect::<Vec<_>>()
        } else {
            materials.layers.clone()
        };
        if layer_order.is_empty() {
            report.push(DataError::InvalidValue {
                context: context.clone(),
                message: "layers must be non-empty".to_string(),
            });
        }
        if layer_order.len() > 256 {
            report.push(DataError::InvalidValue {
                context: context.clone(),
                message: "layer count must be <= 256".to_string(),
            });
        }
        let mut seen_layers = std::collections::BTreeSet::new();
        for key in &layer_order {
            if !keys.contains(key) {
                report.push(DataError::InvalidValue {
                    context: context.clone(),
                    message: format!("layer references unknown material key `{key}`"),
                });
            }
            if !seen_layers.insert(key.clone()) {
                report.push(DataError::InvalidValue {
                    context: context.clone(),
                    message: format!("duplicate layer key `{key}`"),
                });
            }
        }
    }

    let water_levels: std::collections::BTreeMap<StableId, f32> = definitions
        .iter()
        .filter_map(|def| match def {
            RawDefinition::Water(water) => Some((water.header.id.clone(), water.sea_level_m)),
            _ => None,
        })
        .collect();
    let island_levels: std::collections::BTreeMap<StableId, f32> = definitions
        .iter()
        .filter_map(|def| match def {
            RawDefinition::IslandGeneration(island) => {
                Some((island.header.id.clone(), island.island.sea_level_m))
            }
            _ => None,
        })
        .collect();
    let island_defs: std::collections::BTreeMap<StableId, &IslandGenerationDefinition> =
        definitions
            .iter()
            .filter_map(|def| match def {
                RawDefinition::IslandGeneration(island) => {
                    Some((island.header.id.clone(), island))
                }
                _ => None,
            })
            .collect();

    for definition in definitions {
        let RawDefinition::World(world) = definition else {
            continue;
        };
        let context = format!("world `{}`", world.header.id);
        let Some(ref island_id) = world.island_gen else {
            continue;
        };
        let Some(island_sea) = island_levels.get(island_id) else {
            continue;
        };
        let Some(water_sea) = water_levels.get(&world.water) else {
            continue;
        };
        if (island_sea - water_sea).abs() > 0.01 {
            report.push(DataError::InvalidValue {
                context: context.clone(),
                message: format!(
                    "island_gen `{island_id}` sea_level_m {island_sea:.2} must match water `{}` sea_level_m {water_sea:.2}",
                    world.water
                ),
            });
        }
        if let Some(island) = island_defs.get(island_id) {
            if let Some(ref resolution) = island.resolution {
                validate_generation_resolution(
                    resolution,
                    world.effective_ocean_extent_m(),
                    &format!("island generation `{island_id}` (world `{}`)", world.header.id),
                    report,
                );
            }
        }
    }

    let water_body_ids: std::collections::BTreeSet<StableId> = definitions
        .iter()
        .filter_map(|def| match def {
            RawDefinition::WaterBodyMaterial(body) => Some(body.header.id.clone()),
            _ => None,
        })
        .collect();

    for definition in definitions {
        let RawDefinition::World(world) = definition else {
            continue;
        };
        let context = format!("world `{}`", world.header.id);
        for hydrology_id in &world.hydrology_bodies {
            let suffix = hydrology_id
                .as_str()
                .strip_prefix("hydrology.")
                .unwrap_or(hydrology_id.as_str());
            let waterbody_id = StableId::new(&format!("waterbody.{suffix}"));
            if !water_body_ids.contains(&waterbody_id) {
                report.push(DataError::InvalidValue {
                    context: context.clone(),
                    message: format!(
                        "hydrology_bodies entry `{hydrology_id}` requires render material `{waterbody_id}`"
                    ),
                });
            }
        }
    }

    let mut player_gravity = None;
    let mut physics_gravity = None;
    for definition in definitions {
        match definition {
            RawDefinition::Player(player) => player_gravity = Some(player.gravity_mps2),
            RawDefinition::Physics(physics) => physics_gravity = Some(physics.gravity_mps2),
            _ => {}
        }
    }
    if let (Some(player_g), Some(physics_g)) = (player_gravity, physics_gravity) {
        if (player_g - physics_g).abs() > f32::EPSILON {
            report.push(DataError::InvalidValue {
                context: "physics vs player gravity".to_string(),
                message: format!(
                    "gravity_mps2 mismatch: player={player_g:.2}, physics={physics_g:.2}"
                ),
            });
        }
    }

    let mut biome_world_heights: std::collections::BTreeMap<StableId, Vec<(StableId, f32)>> =
        std::collections::BTreeMap::new();
    for definition in definitions {
        let RawDefinition::World(world) = definition else {
            continue;
        };
        let Some(island_id) = &world.island_gen else {
            continue;
        };
        let Some(island) = island_defs.get(island_id) else {
            continue;
        };
        biome_world_heights
            .entry(world.biomes.clone())
            .or_default()
            .push((world.header.id.clone(), island.island.maximum_height_m));
    }
    for (biomes_id, worlds) in biome_world_heights {
        if worlds.len() < 2 {
            continue;
        }
        let min_h = worlds
            .iter()
            .map(|(_, height)| *height)
            .fold(f32::INFINITY, f32::min);
        let max_h = worlds
            .iter()
            .map(|(_, height)| *height)
            .fold(f32::NEG_INFINITY, f32::max);
        if min_h > 0.0 && max_h / min_h > 2.0 {
            let world_ids: Vec<_> = worlds.iter().map(|(id, _)| id.as_str()).collect();
            report.push(DataError::InvalidValue {
                context: format!("biomes `{biomes_id}` shared across worlds"),
                message: format!(
                    "maximum_height_m spans {min_h:.0}–{max_h:.0} (>2×) across [{}]; use separate biome profiles per scale",
                    world_ids.join(", ")
                ),
            });
        }
    }
}

fn default_surface_for_materials(materials: &StableId) -> StableId {
    let suffix = materials
        .as_str()
        .strip_prefix("materials.")
        .unwrap_or(materials.as_str());
    StableId::new(&format!("surface.{suffix}"))
}

fn validate_terrain_materials(materials: &TerrainMaterialsDefinition, report: &mut ValidationReport) {
    let context = format!("materials `{}`", materials.header.id);
    let material_keys: std::collections::BTreeSet<StableId> = materials
        .materials
        .iter()
        .map(|entry| entry.resolved_key())
        .collect();

    if materials.header.schema_version >= 2 && materials.layers.is_empty() {
        report.push(DataError::InvalidValue {
            context: context.clone(),
            message: "schema_version 2 requires non-empty `layers` texture-array order".to_string(),
        });
    }

    if !materials.layers.is_empty() {
        if materials.layers.len() > 256 {
            report.push(DataError::InvalidValue {
                context: context.clone(),
                message: "layers must contain at most 256 entries".to_string(),
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for key in &materials.layers {
            if !material_keys.contains(key) {
                report.push(DataError::InvalidValue {
                    context: context.clone(),
                    message: format!("layers references unknown material key `{key}`"),
                });
            }
            if !seen.insert(key.clone()) {
                report.push(DataError::InvalidValue {
                    context: context.clone(),
                    message: format!("duplicate layer key `{key}`"),
                });
            }
        }
    }
}

fn validate_surface_rules(rules: &SurfaceRulesDefinition, report: &mut ValidationReport) {
    if rules.gates.is_empty() && rules.classifiers.is_empty() {
        report.push(DataError::InvalidValue {
            context: format!("surface `{}`", rules.header.id),
            message: "surface rules must declare gates or classifiers".to_string(),
        });
    }
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

    if world.chunks.cells != [16, 16, 16] {
        report.push(DataError::InvalidValue {
            context: format!("world `{}`", world.header.id),
            message: "chunks.cells must be [16, 16, 16] until alternate chunk sizes are supported"
                .to_string(),
        });
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

    if let Some(ref island_gen) = world.island_gen {
        require_reference(island_gen, "world island_gen", ids, report);
    }

    let surface_id = world
        .surface
        .clone()
        .unwrap_or_else(|| default_surface_for_materials(&world.materials));
    require_reference(&surface_id, "world surface rules", ids, report);

    if let Some(ref resolution) = world.resolution {
        validate_generation_resolution(
            resolution,
            world.effective_ocean_extent_m(),
            &format!("world `{}`", world.header.id),
            report,
        );
    }

    for hydrology_id in &world.hydrology_bodies {
        require_reference(hydrology_id, "world hydrology_bodies", ids, report);
    }

    if let Some(ref catalog_id) = world.material_catalog {
        require_reference(catalog_id, "world material_catalog", ids, report);
    }

    if let Some(ref vegetation_id) = world.vegetation {
        require_reference(vegetation_id, "world vegetation", ids, report);
    }

    if let Some(ref weather_id) = world.weather {
        require_reference(weather_id, "world weather", ids, report);
    }

    require_reference(
        &world.chunks.lod.materials.render_profile,
        "world render_profile",
        ids,
        report,
    );
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
    let context = format!("island generation `{}`", island.header.id);
    if island.caves.chamber_count_min > island.caves.chamber_count_max {
        report.push(DataError::InvalidValue {
            context: context.clone(),
            message: "caves.chamber_count_min must be <= caves.chamber_count_max".to_string(),
        });
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

fn validate_combine(value: &str, context: &str, report: &mut ValidationReport) {
    match value.to_ascii_lowercase().as_str() {
        "union" | "subtract" => {}
        other => report.push(DataError::InvalidValue {
            context: context.to_string(),
            message: format!(
                "combine must be 'union' or 'subtract', got '{other}'"
            ),
        }),
    }
}

fn validate_coast_modifier_kind(kind: &str, context: &str, report: &mut ValidationReport) {
    match kind.to_ascii_lowercase().as_str() {
        "cove" | "harbor" => {}
        "cliff_shelf" | "cliff" => report.push(DataError::InvalidValue {
            context: context.to_string(),
            message: "coast modifier kind 'cliff_shelf' is not implemented".to_string(),
        }),
        other => report.push(DataError::InvalidValue {
            context: context.to_string(),
            message: format!(
                "unknown coast modifier kind '{other}' (expected cove or harbor)"
            ),
        }),
    }
}

fn validate_terrain_operation(op: &TerrainOperationDefinition, context: &str, report: &mut ValidationReport) {
    match op {
        TerrainOperationDefinition::CoastModifier { kind, .. } => {
            validate_coast_modifier_kind(kind, context, report);
        }
        TerrainOperationDefinition::Ellipsoid { combine, .. }
        | TerrainOperationDefinition::Capsule { combine, .. } => {
            validate_combine(combine, context, report);
        }
        _ => {}
    }
}

fn validate_terrain_generation(
    terrain: &TerrainGenerationDefinition,
    ids: &[StableId],
    report: &mut ValidationReport,
) {
    let context = format!("terrain `{}`", terrain.header.id);
    let coastal_count = terrain
        .operations
        .iter()
        .filter(|op| matches!(op, TerrainOperationDefinition::CoastalSurface { .. }))
        .count();
    if coastal_count > 1 {
        report.push(DataError::InvalidValue {
            context: context.clone(),
            message: "terrain may declare at most one coastal_surface operation".to_string(),
        });
    }
    for op in &terrain.operations {
        validate_terrain_operation(op, &context, report);
    }
    for include in &terrain.includes {
        require_reference(include, &context, ids, report);
    }
}

fn validate_cave(cave: &CaveDefinition, report: &mut ValidationReport) {
    let context = format!("cave `{}`", cave.header.id);
    for op in &cave.operations {
        validate_terrain_operation(op, &context, report);
    }
}
