// crates/game_data/src/compile.rs
use serde::Serialize;
use shared::{DataError, DataResult, StableId};

use crate::definitions::*;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledApp {
    pub id: StableId,
    pub world: StableId,
    pub player: StableId,
    pub camera: StableId,
    pub performance: StableId,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledPerformance {
    pub id: StableId,
    pub target_fps: u32,
    pub target_resolution: [u32; 2],
    pub maximum_density_jobs: u32,
    pub maximum_mesh_jobs: u32,
    pub mesh_uploads_per_frame: u32,
    pub collider_builds_per_frame: u32,
    pub shadows_enabled: bool,
    pub shadow_quality: String,
    pub vegetation_density_multiplier: f32,
    pub vegetation_maximum_distance_m: f32,
    pub water_quality: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledPlayer {
    pub id: StableId,
    pub capsule_radius_m: f32,
    pub capsule_half_height_m: f32,
    pub walk_speed_mps: f32,
    pub run_speed_mps: f32,
    pub acceleration_mps2: f32,
    pub deceleration_mps2: f32,
    pub rotation_speed_deg_per_s: f32,
    pub maximum_walkable_slope_deg: f32,
    pub step_height_m: f32,
    pub ground_snap_m: f32,
    pub jump_height_m: f32,
    pub jump_buffer_s: f32,
    pub coyote_time_s: f32,
    pub gravity_mps2: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledCamera {
    pub id: StableId,
    pub distance_default_m: f32,
    pub distance_minimum_m: f32,
    pub distance_maximum_m: f32,
    /// Elevation above horizontal in radians (converted from depression degrees in YAML).
    pub pitch_default_rad: f32,
    pub pitch_minimum_rad: f32,
    pub pitch_maximum_rad: f32,
    pub mouse_sensitivity_x: f32,
    pub mouse_sensitivity_y: f32,
    pub invert_y: bool,
    pub zoom_speed: f32,
    pub focus_height: f32,
    pub focus_offset_x: f32,
    pub focus_offset_z: f32,
    pub shoulder_offset: f32,
    pub follow_sharpness: f32,
    pub rotation_sharpness: f32,
    pub zoom_sharpness: f32,
    pub collision_radius: f32,
    pub collision_margin: f32,
    pub collision_inward_sharpness: f32,
    pub collision_outward_sharpness: f32,
    pub both_buttons_move_forward: bool,
    pub recenter_key: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledLighting {
    pub id: StableId,
    pub sun_direction: [f32; 3],
    pub sun_illuminance_lux: f32,
    pub sun_color: [f32; 3],
    pub sun_shadows_enabled: bool,
    pub ambient_brightness: f32,
    pub ambient_color: [f32; 3],
    pub fog_enabled: bool,
    pub fog_color: [f32; 3],
    pub fog_start_m: f32,
    pub fog_end_m: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledWater {
    pub id: StableId,
    pub sea_level_m: f32,
    pub shallow_color: [f32; 3],
    pub deep_color: [f32; 3],
    pub transparency: f32,
    pub wave_speed: f32,
    pub wave_amplitude: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledWorld {
    pub id: StableId,
    pub seed: u64,
    pub cell_size_m: f32,
    pub chunk_cells: [u32; 3],
    pub world_extent_chunks: [u32; 3],
    pub terrain: StableId,
    pub biomes: StableId,
    pub materials: StableId,
    pub surface: StableId,
    pub water: StableId,
    pub lighting: StableId,
    pub sky: Option<StableId>,
    pub landmarks: Option<StableId>,
    pub structures: Vec<StableId>,
    pub ocean_extent_m: Option<f32>,
    pub coord_offset: [f32; 3],
    pub island_gen: Option<StableId>,
    pub resolution: Option<GenerationResolutionDefinition>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledTerrain {
    pub id: StableId,
    pub spawn: Option<[f32; 3]>,
    pub includes: Vec<StableId>,
    pub operations: Vec<TerrainOperationDefinition>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledCave {
    pub id: StableId,
    pub operations: Vec<TerrainOperationDefinition>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledBiomes {
    pub id: StableId,
    pub rules: Vec<BiomeRuleDefinition>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledTerrainMaterials {
    pub id: StableId,
    pub materials: Vec<TerrainMaterialEntryDefinition>,
    pub layer_order: Vec<StableId>,
    pub key_to_layer: std::collections::BTreeMap<StableId, u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledSurfaceRules {
    pub id: StableId,
    pub gates: Vec<SurfaceGateDefinition>,
    pub classifiers: Vec<SurfaceClassifierDefinition>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledVegetation {
    pub id: StableId,
    pub rules: Vec<VegetationRuleDefinition>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledDebug {
    pub id: StableId,
    pub bindings: DebugBindingsDefinition,
}

impl From<&AppDefinition> for CompiledApp {
    fn from(def: &AppDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            world: def.world.clone(),
            player: def.player.clone(),
            camera: def.camera.clone(),
            performance: def.performance.clone(),
        }
    }
}

impl From<&PerformanceDefinition> for CompiledPerformance {
    fn from(def: &PerformanceDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            target_fps: def.target_fps,
            target_resolution: def.target_resolution,
            maximum_density_jobs: def.terrain.maximum_density_jobs,
            maximum_mesh_jobs: def.terrain.maximum_mesh_jobs,
            mesh_uploads_per_frame: def.terrain.mesh_uploads_per_frame,
            collider_builds_per_frame: def.terrain.collider_builds_per_frame,
            shadows_enabled: def.shadows.enabled,
            shadow_quality: def.shadows.quality.clone(),
            vegetation_density_multiplier: def.vegetation.density_multiplier,
            vegetation_maximum_distance_m: def.vegetation.maximum_distance_m,
            water_quality: def.water.quality.clone(),
        }
    }
}

impl From<&PlayerDefinition> for CompiledPlayer {
    fn from(def: &PlayerDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            capsule_radius_m: def.capsule.radius_m,
            capsule_half_height_m: def.capsule.half_height_m,
            walk_speed_mps: def.movement.walk_speed_mps,
            run_speed_mps: def.movement.run_speed_mps,
            acceleration_mps2: def.movement.acceleration_mps2,
            deceleration_mps2: def.movement.deceleration_mps2,
            rotation_speed_deg_per_s: def.movement.rotation_speed_deg_per_s,
            maximum_walkable_slope_deg: def.movement.maximum_walkable_slope_deg,
            step_height_m: def.movement.step_height_m,
            ground_snap_m: def.movement.ground_snap_m,
            jump_height_m: def.movement.jump_height_m,
            jump_buffer_s: def.movement.jump_buffer_s,
            coyote_time_s: def.movement.coyote_time_s,
            gravity_mps2: def.gravity_mps2,
        }
    }
}

impl From<&CameraDefinition> for CompiledCamera {
    fn from(def: &CameraDefinition) -> Self {
        // YAML stores depression angles (negative). Convert to positive elevation radians.
        let depression_to_elevation = |degrees: f32| (-degrees).to_radians();
        Self {
            id: def.header.id.clone(),
            distance_default_m: def.orbit.default_distance,
            distance_minimum_m: def.orbit.minimum_distance,
            distance_maximum_m: def.orbit.maximum_distance,
            pitch_default_rad: depression_to_elevation(def.orbit.default_pitch_degrees),
            pitch_minimum_rad: depression_to_elevation(def.orbit.maximum_pitch_degrees),
            pitch_maximum_rad: depression_to_elevation(def.orbit.minimum_pitch_degrees),
            mouse_sensitivity_x: def.orbit.mouse_sensitivity_x,
            mouse_sensitivity_y: def.orbit.mouse_sensitivity_y,
            invert_y: def.orbit.invert_y,
            zoom_speed: def.orbit.zoom_speed,
            focus_height: def.follow.focus_height,
            focus_offset_x: def.follow.focus_offset_x,
            focus_offset_z: def.follow.focus_offset_z,
            shoulder_offset: def.follow.shoulder_offset,
            follow_sharpness: def.follow.follow_sharpness,
            rotation_sharpness: def.follow.rotation_sharpness,
            zoom_sharpness: def.follow.zoom_sharpness,
            collision_radius: def.collision.radius,
            collision_margin: def.collision.margin,
            collision_inward_sharpness: def.collision.inward_sharpness,
            collision_outward_sharpness: def.collision.outward_sharpness,
            both_buttons_move_forward: def.controls.both_buttons_move_forward,
            recenter_key: def.controls.recenter_key.clone(),
        }
    }
}

impl From<&LightingDefinition> for CompiledLighting {
    fn from(def: &LightingDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            sun_direction: def.sun.direction,
            sun_illuminance_lux: def.sun.illuminance_lux,
            sun_color: def.sun.color,
            sun_shadows_enabled: def.sun.shadows_enabled,
            ambient_brightness: def.ambient.brightness,
            ambient_color: def.ambient.color,
            fog_enabled: def.fog.enabled,
            fog_color: def.fog.color,
            fog_start_m: def.fog.start_m,
            fog_end_m: def.fog.end_m,
        }
    }
}

impl From<&WaterDefinition> for CompiledWater {
    fn from(def: &WaterDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            sea_level_m: def.sea_level_m,
            shallow_color: def.shallow_color,
            deep_color: def.deep_color,
            transparency: def.transparency,
            wave_speed: def.wave_speed,
            wave_amplitude: def.wave_amplitude,
        }
    }
}

impl From<&WorldDefinition> for CompiledWorld {
    fn from(def: &WorldDefinition) -> Self {
        Self::try_from_definition(def)
            .expect("world definition must be validated before compile")
    }
}

impl CompiledWorld {
    pub fn try_from_definition(def: &WorldDefinition) -> DataResult<Self> {
        if (def.voxel.cell_size_m - 1.0).abs() > f32::EPSILON {
            return Err(DataError::InvalidValue {
                context: format!("world `{}`", def.header.id),
                message: "voxel.cell_size_m must be 1.0 until sub-meter voxel indexing is supported"
                    .to_string(),
            });
        }
        Ok(Self {
            id: def.header.id.clone(),
            seed: def.seed,
            cell_size_m: def.voxel.cell_size_m,
            chunk_cells: def.chunks.cells,
            world_extent_chunks: def.chunks.world_extent,
            terrain: def.terrain.clone(),
            biomes: def.biomes.clone(),
            materials: def.materials.clone(),
            surface: def
                .surface
                .clone()
                .unwrap_or_else(|| default_surface_for_materials(&def.materials)),
            water: def.water.clone(),
            lighting: def.lighting.clone(),
            sky: def.sky.clone(),
            landmarks: def.landmarks.clone(),
            structures: def.structures.clone(),
            ocean_extent_m: def.ocean_extent_m,
            coord_offset: def.coord_offset.unwrap_or([0.0, 0.0, 0.0]),
            island_gen: def.island_gen.clone(),
            resolution: def.resolution.clone(),
        })
    }

    pub fn recipe_to_world(&self, position: [f32; 3]) -> [f32; 3] {
        [
            position[0] - self.coord_offset[0],
            position[1] - self.coord_offset[1],
            position[2] - self.coord_offset[2],
        ]
    }

    /// Horizontal span of the chunk volume on X/Z in meters (square side length).
    pub fn horizontal_extent_m(&self) -> f32 {
        let cell_span = self.chunk_cells[0] as f32 * self.cell_size_m;
        let x_extent = self.world_extent_chunks[0] as f32 * cell_span;
        let z_extent = self.world_extent_chunks[2] as f32 * cell_span;
        x_extent.max(z_extent)
    }

    /// World-space axis bounds `[min, max)` in meters, matching `chunk_axis_range`.
    pub fn axis_bounds_m(&self) -> ([f32; 3], [f32; 3]) {
        let axis = |cells: u32, extent_chunks: u32| -> (f32, f32) {
            let start_chunk = -((extent_chunks / 2) as i32);
            let end_chunk = start_chunk + extent_chunks as i32;
            (
                start_chunk as f32 * cells as f32 * self.cell_size_m,
                end_chunk as f32 * cells as f32 * self.cell_size_m,
            )
        };
        let (x_min, x_max) = axis(self.chunk_cells[0], self.world_extent_chunks[0]);
        let (y_min, y_max) = axis(self.chunk_cells[1], self.world_extent_chunks[1]);
        let (z_min, z_max) = axis(self.chunk_cells[2], self.world_extent_chunks[2]);
        ([x_min, y_min, z_min], [x_max, y_max, z_max])
    }

    /// Padding between the island footprint and the atlas rim when deriving extent
    /// from the chunk volume.
    pub const DERIVED_OCEAN_PADDING_M: f32 = 32.0;

    /// Square atlas extent that fully covers the world's horizontal chunk volume.
    pub fn effective_ocean_extent_m(&self) -> f32 {
        let horizontal = self.horizontal_extent_m();
        let authored = self
            .ocean_extent_m
            .unwrap_or(horizontal + Self::DERIVED_OCEAN_PADDING_M);
        authored.max(horizontal)
    }
}

fn default_surface_for_materials(materials: &StableId) -> StableId {
    let suffix = materials
        .as_str()
        .strip_prefix("materials.")
        .unwrap_or(materials.as_str());
    StableId::new(&format!("surface.{suffix}"))
}

impl From<&TerrainGenerationDefinition> for CompiledTerrain {
    fn from(def: &TerrainGenerationDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            spawn: def.spawn,
            includes: def.includes.clone(),
            operations: def.operations.clone(),
        }
    }
}

impl From<&CaveDefinition> for CompiledCave {
    fn from(def: &CaveDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            operations: def.operations.clone(),
        }
    }
}

impl From<&BiomesDefinition> for CompiledBiomes {
    fn from(def: &BiomesDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            rules: def.rules.clone(),
        }
    }
}

impl From<&TerrainMaterialsDefinition> for CompiledTerrainMaterials {
    fn from(def: &TerrainMaterialsDefinition) -> Self {
        let mut key_to_layer = std::collections::BTreeMap::new();
        let layer_order = if def.layers.is_empty() {
            let mut ordered: Vec<_> = def.materials.iter().collect();
            ordered.sort_by_key(|m| m.resolved_legacy_id());
            ordered
                .into_iter()
                .map(|m| m.resolved_key())
                .collect()
        } else {
            def.layers.clone()
        };
        for (index, key) in layer_order.iter().enumerate() {
            key_to_layer.insert(key.clone(), index as u32);
        }
        Self {
            id: def.header.id.clone(),
            materials: def.materials.clone(),
            layer_order,
            key_to_layer,
        }
    }
}

impl CompiledTerrainMaterials {
    pub fn layer_count(&self) -> u32 {
        self.layer_order.len() as u32
    }

    pub fn layer_for_key(&self, key: &StableId) -> Option<u32> {
        self.key_to_layer.get(key).copied()
    }

    pub fn entry_for_key(&self, key: &StableId) -> Option<&TerrainMaterialEntryDefinition> {
        self.materials
            .iter()
            .find(|entry| &entry.resolved_key() == key)
    }
}

impl From<&SurfaceRulesDefinition> for CompiledSurfaceRules {
    fn from(def: &SurfaceRulesDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            gates: def.gates.clone(),
            classifiers: def.classifiers.clone(),
        }
    }
}

impl From<&VegetationDefinition> for CompiledVegetation {
    fn from(def: &VegetationDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            rules: def.rules.clone(),
        }
    }
}

impl From<&DebugDefinition> for CompiledDebug {
    fn from(def: &DebugDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            bindings: def.bindings.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledOptions {
    pub id: StableId,
    pub toggle_key: String,
    pub default_tab: String,
    pub stubs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledSetupParameter {
    pub id: String,
    pub label: String,
    pub bind: String,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub default: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledSetupGroup {
    pub id: String,
    pub label: String,
    pub parameters: Vec<CompiledSetupParameter>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledSetupPreviewMode {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledSetupSchema {
    pub id: StableId,
    pub groups: Vec<CompiledSetupGroup>,
    pub preview_modes: Vec<CompiledSetupPreviewMode>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledIslandGeneration {
    pub id: StableId,
    pub seed: u64,
    pub island: IslandShapeDefinition,
    pub volcano: VolcanoDefinition,
    pub surface_noise: SurfaceNoiseDefinition,
    pub hydrology: IslandHydrologyDefinition,
    pub erosion: IslandErosionDefinition,
    pub coast: IslandCoastDefinition,
    pub beaches: BeachDefinition,
    pub caves: IslandCaveDefinition,
    pub resolution: Option<GenerationResolutionDefinition>,
}

impl CompiledIslandGeneration {
    pub fn set_param(&mut self, bind: &str, value: f32) {
        match bind {
            "island.playable_diameter_m" => self.island.playable_diameter_m = value,
            "island.maximum_height_m" => self.island.maximum_height_m = value,
            "island.sea_level_m" => self.island.sea_level_m = value,
            "island.lobe_count" => self.island.lobe_count = value.round().max(1.0) as u32,
            "island.warp_amplitude" => self.island.warp_amplitude = value,
            "volcano.shield_radius_m" => self.volcano.shield_radius_m = value,
            "volcano.shield_height_m" => self.volcano.shield_height_m = value,
            "volcano.summit_height_m" => self.volcano.summit_height_m = value,
            "volcano.caldera_depth_m" => self.volcano.caldera_depth_m = value,
            "volcano.collapse_depth_m" => self.volcano.collapse_depth_m = value,
            "surface_noise.regional_amplitude_m" => self.surface_noise.regional_amplitude_m = value,
            "hydrology.stream_threshold" => self.hydrology.stream_threshold = value,
            "hydrology.permanent_river_threshold" => self.hydrology.permanent_river_threshold = value,
            "erosion.stream_power_iterations" => {
                self.erosion.stream_power_iterations = value.round().max(0.0) as u32
            }
            "erosion.maximum_step_m" => self.erosion.maximum_step_m = value,
            "coast.shelf_width_max_m" => self.coast.shelf_width_max_m = value,
            "beaches.maximum_slope_deg" => self.beaches.maximum_slope_deg = value,
            _ => {}
        }
    }

    pub fn param_value(&self, bind: &str) -> Option<f32> {
        Some(match bind {
            "island.playable_diameter_m" => self.island.playable_diameter_m,
            "island.maximum_height_m" => self.island.maximum_height_m,
            "island.sea_level_m" => self.island.sea_level_m,
            "island.lobe_count" => self.island.lobe_count as f32,
            "island.warp_amplitude" => self.island.warp_amplitude,
            "volcano.shield_radius_m" => self.volcano.shield_radius_m,
            "volcano.shield_height_m" => self.volcano.shield_height_m,
            "volcano.summit_height_m" => self.volcano.summit_height_m,
            "volcano.caldera_depth_m" => self.volcano.caldera_depth_m,
            "volcano.collapse_depth_m" => self.volcano.collapse_depth_m,
            "surface_noise.regional_amplitude_m" => self.surface_noise.regional_amplitude_m,
            "hydrology.stream_threshold" => self.hydrology.stream_threshold,
            "hydrology.permanent_river_threshold" => self.hydrology.permanent_river_threshold,
            "erosion.stream_power_iterations" => self.erosion.stream_power_iterations as f32,
            "erosion.maximum_step_m" => self.erosion.maximum_step_m,
            "coast.shelf_width_max_m" => self.coast.shelf_width_max_m,
            "beaches.maximum_slope_deg" => self.beaches.maximum_slope_deg,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledPhysics {
    pub id: StableId,
    pub gravity_mps2: f32,
    pub fixed_timestep_hz: u32,
    pub maximum_substeps: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledRiver {
    pub id: StableId,
    pub source_region_center: [f32; 2],
    pub source_region_radius_m: f32,
    pub minimum_elevation_m: f32,
    pub grid_spacing_m: f32,
    pub direction_inertia: f32,
    pub maximum_turn_deg: f32,
    pub depression_repair_radius_cells: u32,
    pub maximum_breach_depth_m: f32,
    pub source_width_m: f32,
    pub mouth_width_m: f32,
    pub source_depth_m: f32,
    pub mouth_depth_m: f32,
    pub bank_width_m: f32,
    pub minimum_depth_m: f32,
    pub maximum_segment_slope: f32,
    pub waterfall_threshold_m: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledHydrology {
    pub id: StableId,
    pub kind: String,
    pub elevation_m: f32,
    pub depth_m: Option<f32>,
    pub center: Option<[f32; 2]>,
    pub radius_m: Option<f32>,
}

impl From<&OptionsDefinition> for CompiledOptions {
    fn from(def: &OptionsDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            toggle_key: def.panel.toggle_key.clone(),
            default_tab: def.panel.default_tab.clone(),
            stubs: def.stubs.clone(),
        }
    }
}

impl From<&PhysicsDefinition> for CompiledPhysics {
    fn from(def: &PhysicsDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            gravity_mps2: def.gravity_mps2,
            fixed_timestep_hz: def.fixed_timestep_hz,
            maximum_substeps: def.maximum_substeps,
        }
    }
}

impl From<&RiverDefinition> for CompiledRiver {
    fn from(def: &RiverDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            source_region_center: def.source.region_center,
            source_region_radius_m: def.source.region_radius_m,
            minimum_elevation_m: def.source.minimum_elevation_m,
            grid_spacing_m: def.routing.grid_spacing_m,
            direction_inertia: def.routing.direction_inertia,
            maximum_turn_deg: def.routing.maximum_turn_deg,
            depression_repair_radius_cells: def.routing.depression_repair_radius_cells,
            maximum_breach_depth_m: def.routing.maximum_breach_depth_m,
            source_width_m: def.channel.source_width_m,
            mouth_width_m: def.channel.mouth_width_m,
            source_depth_m: def.channel.source_depth_m,
            mouth_depth_m: def.channel.mouth_depth_m,
            bank_width_m: def.channel.bank_width_m,
            minimum_depth_m: def.water.minimum_depth_m,
            maximum_segment_slope: def.water.maximum_segment_slope,
            waterfall_threshold_m: def.water.waterfall_threshold_m,
        }
    }
}

impl From<&HydrologyDefinition> for CompiledHydrology {
    fn from(def: &HydrologyDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            kind: def.kind.clone(),
            elevation_m: def.elevation_m,
            depth_m: def.depth_m,
            center: def.center,
            radius_m: def.radius_m,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledAtmosphere {
    pub id: StableId,
    pub sun_azimuth_deg: f32,
    pub sun_elevation_deg: f32,
    pub sun_illuminance_lux: f32,
    pub sun_color: [f32; 3],
    pub moon_enabled: bool,
    pub moon_azimuth_deg: f32,
    pub moon_elevation_deg: f32,
    pub moon_illuminance: f32,
    pub moon_phase: f32,
    pub moon_angular_radius: f32,
    pub ambient_color: [f32; 3],
    pub ambient_brightness: f32,
    pub exposure_target: f32,
    pub exposure_adaptation_speed: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledFog {
    pub id: StableId,
    pub distance_color: [f32; 3],
    pub distance_start_m: f32,
    pub distance_end_m: f32,
    pub height_base_m: f32,
    pub height_density: f32,
    pub height_color: [f32; 3],
    pub underwater_density: f32,
    pub underwater_color: [f32; 3],
    pub cave_density: f32,
    pub cave_color: [f32; 3],
    pub local_volumes: Vec<CompiledFogLocalVolume>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledFogLocalVolume {
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
    pub density: f32,
    pub color: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledLandmarks {
    pub id: StableId,
    pub facts: Vec<CompiledLandmarkFact>,
    pub route_signs: Vec<CompiledLandmarkSign>,
    pub fog_volumes: Vec<CompiledFogLocalVolume>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledLandmarkFact {
    pub tag: String,
    pub position: [f32; 3],
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledLandmarkSign {
    pub position: [f32; 3],
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledRoutes {
    pub id: StableId,
    pub routes: Vec<CompiledRoute>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledRoute {
    pub id: String,
    pub waypoints: Vec<[f32; 2]>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledStructure {
    pub id: StableId,
    pub anchor: [f32; 3],
    pub yaw_deg: f32,
    pub flatten_radius_m: f32,
    pub parts: Vec<CompiledStructurePart>,
    pub collision: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledStructurePart {
    pub kind: String,
    pub size: Option<[f32; 3]>,
    pub radius: Option<f32>,
    pub height: Option<f32>,
    pub offset: [f32; 3],
    pub material: Option<String>,
    pub tag: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledSky {
    pub id: StableId,
    pub zenith_color: [f32; 3],
    pub horizon_color: [f32; 3],
    pub mie_strength: f32,
    pub sun_disc_radius: f32,
    pub stars_enabled: bool,
    pub stars_density: f32,
    pub clouds_enabled: bool,
    pub clouds_opacity: f32,
    pub clouds_speed: f32,
    pub clouds_direction_deg: f32,
    pub clouds_altitude: f32,
    pub night_zenith_color: [f32; 3],
    pub night_horizon_color: [f32; 3],
    pub shader: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompiledWaterBodyMaterial {
    pub id: StableId,
    pub shallow_color: [f32; 3],
    pub deep_color: [f32; 3],
    pub transparency: f32,
    pub wave_amplitude: f32,
    pub wave_speed: f32,
    pub flow_tint: Option<[f32; 3]>,
}

impl From<&WaterBodyMaterialDefinition> for CompiledWaterBodyMaterial {
    fn from(def: &WaterBodyMaterialDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            shallow_color: def.material.shallow_color,
            deep_color: def.material.deep_color,
            transparency: def.material.transparency,
            wave_amplitude: def.material.wave_amplitude,
            wave_speed: def.material.wave_speed,
            flow_tint: def.material.flow_tint,
        }
    }
}

impl From<&AtmosphereDefinition> for CompiledAtmosphere {
    fn from(def: &AtmosphereDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            sun_azimuth_deg: def.sun.azimuth_deg,
            sun_elevation_deg: def.sun.elevation_deg,
            sun_illuminance_lux: def.sun.illuminance_lux,
            sun_color: def.sun.color,
            moon_enabled: def.moon.enabled,
            moon_azimuth_deg: def.moon.azimuth_deg,
            moon_elevation_deg: def.moon.elevation_deg,
            moon_illuminance: def.moon.illuminance,
            moon_phase: def.moon.phase,
            moon_angular_radius: def.moon.angular_radius,
            ambient_color: def.ambient.color,
            ambient_brightness: def.ambient.brightness,
            exposure_target: def.exposure.target,
            exposure_adaptation_speed: def.exposure.adaptation_speed,
        }
    }
}

impl From<&FogDefinition> for CompiledFog {
    fn from(def: &FogDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            distance_color: def.distance.color,
            distance_start_m: def.distance.start_m,
            distance_end_m: def.distance.end_m,
            height_base_m: def.height.base_height_m,
            height_density: def.height.density,
            height_color: def.height.color,
            underwater_density: def.underwater.density,
            underwater_color: def.underwater.color,
            cave_density: def.cave.density,
            cave_color: def.cave.color,
            local_volumes: def
                .local_volumes
                .iter()
                .map(|v| CompiledFogLocalVolume {
                    center: v.center,
                    half_extents: v.half_extents,
                    density: v.density,
                    color: v.color,
                })
                .collect(),
        }
    }
}

impl From<&SkyDefinition> for CompiledSky {
    fn from(def: &SkyDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            zenith_color: def.zenith_color,
            horizon_color: def.horizon_color,
            mie_strength: def.mie_strength,
            sun_disc_radius: def.sun_disc_radius,
            stars_enabled: def.stars_enabled,
            stars_density: def.stars_density,
            clouds_enabled: def.clouds_enabled,
            clouds_opacity: def.clouds_opacity,
            clouds_speed: def.clouds_speed,
            clouds_direction_deg: def.clouds_direction_deg,
            clouds_altitude: def.clouds_altitude,
            night_zenith_color: def.night_zenith_color,
            night_horizon_color: def.night_horizon_color,
            shader: def.shader.clone(),
        }
    }
}

impl From<&LandmarksDefinition> for CompiledLandmarks {
    fn from(def: &LandmarksDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            facts: def
                .facts
                .iter()
                .map(|f| CompiledLandmarkFact {
                    tag: f.tag.clone(),
                    position: f.position,
                    label: f.label.clone(),
                })
                .collect(),
            route_signs: def
                .route_signs
                .iter()
                .map(|s| CompiledLandmarkSign {
                    position: s.position,
                    label: s.label.clone(),
                })
                .collect(),
            fog_volumes: def
                .fog_volumes
                .iter()
                .map(|v| CompiledFogLocalVolume {
                    center: v.center,
                    half_extents: v.half_extents,
                    density: v.density,
                    color: v.color,
                })
                .collect(),
        }
    }
}

impl From<&RoutesDefinition> for CompiledRoutes {
    fn from(def: &RoutesDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            routes: def
                .routes
                .iter()
                .map(|r| CompiledRoute {
                    id: r.id.clone(),
                    waypoints: r.waypoints.clone(),
                })
                .collect(),
        }
    }
}

impl From<&StructureDefinition> for CompiledStructure {
    fn from(def: &StructureDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            anchor: def.placement.anchor,
            yaw_deg: def.placement.yaw_deg,
            flatten_radius_m: def.placement.flatten_radius_m,
            parts: def
                .parts
                .iter()
                .map(|p| CompiledStructurePart {
                    kind: p.kind.clone(),
                    size: p.size,
                    radius: p.radius,
                    height: p.height,
                    offset: p.offset,
                    material: p.material.clone(),
                    tag: p.tag.clone(),
                })
                .collect(),
            collision: def.collision.clone(),
        }
    }
}

impl From<&IslandGenerationDefinition> for CompiledIslandGeneration {
    fn from(def: &IslandGenerationDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            seed: 0,
            island: def.island.clone(),
            volcano: def.volcano.clone(),
            surface_noise: def.surface_noise.clone(),
            hydrology: def.hydrology.clone(),
            erosion: def.erosion.clone(),
            coast: def.coast.clone(),
            beaches: def.beaches.clone(),
            caves: def.caves.clone(),
            resolution: def.resolution.clone(),
        }
    }
}

impl From<&SetupSchemaDefinition> for CompiledSetupSchema {
    fn from(def: &SetupSchemaDefinition) -> Self {
        Self {
            id: def.header.id.clone(),
            groups: def
                .groups
                .iter()
                .map(|g| CompiledSetupGroup {
                    id: g.id.clone(),
                    label: g.label.clone(),
                    parameters: g
                        .parameters
                        .iter()
                        .map(|p| CompiledSetupParameter {
                            id: p.id.clone(),
                            label: p.label.clone(),
                            bind: p.bind.clone(),
                            min: p.min,
                            max: p.max,
                            step: p.step,
                            default: p.default,
                        })
                        .collect(),
                })
                .collect(),
            preview_modes: def
                .preview_modes
                .iter()
                .map(|m| CompiledSetupPreviewMode {
                    id: m.id.clone(),
                    label: m.label.clone(),
                })
                .collect(),
        }
    }
}
