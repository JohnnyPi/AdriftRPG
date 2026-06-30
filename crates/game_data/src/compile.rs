use shared::StableId;

use crate::definitions::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledApp {
    pub id: StableId,
    pub world: StableId,
    pub player: StableId,
    pub camera: StableId,
    pub performance: StableId,
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
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
    pub gravity_mps2: f32,
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledWater {
    pub id: StableId,
    pub sea_level_m: f32,
    pub shallow_color: [f32; 3],
    pub deep_color: [f32; 3],
    pub transparency: f32,
    pub wave_speed: f32,
    pub wave_amplitude: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledWorld {
    pub id: StableId,
    pub seed: u64,
    pub cell_size_m: f32,
    pub chunk_cells: [u32; 3],
    pub world_extent_chunks: [u32; 3],
    pub terrain: StableId,
    pub biomes: StableId,
    pub materials: StableId,
    pub water: StableId,
    pub lighting: StableId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledTerrain {
    pub id: StableId,
    pub spawn: Option<[f32; 3]>,
    pub includes: Vec<StableId>,
    pub operations: Vec<TerrainOperationDefinition>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledCave {
    pub id: StableId,
    pub operations: Vec<TerrainOperationDefinition>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledBiomes {
    pub id: StableId,
    pub rules: Vec<BiomeRuleDefinition>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledTerrainMaterials {
    pub id: StableId,
    pub materials: Vec<TerrainMaterialEntryDefinition>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledVegetation {
    pub id: StableId,
    pub rules: Vec<VegetationRuleDefinition>,
}

#[derive(Clone, Debug, PartialEq)]
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
        Self {
            id: def.header.id.clone(),
            seed: def.seed,
            cell_size_m: def.voxel.cell_size_m,
            chunk_cells: def.chunks.cells,
            world_extent_chunks: def.chunks.world_extent,
            terrain: def.terrain.clone(),
            biomes: def.biomes.clone(),
            materials: def.materials.clone(),
            water: def.water.clone(),
            lighting: def.lighting.clone(),
        }
    }
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
        Self {
            id: def.header.id.clone(),
            materials: def.materials.clone(),
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
