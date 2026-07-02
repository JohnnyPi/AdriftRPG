// crates/game_data/src/definitions.rs
use serde::{Deserialize, Serialize};
use shared::{DefinitionHeader, StableId};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub world: StableId,
    pub player: StableId,
    pub camera: StableId,
    pub performance: StableId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub target_fps: u32,
    pub target_resolution: [u32; 2],
    pub terrain: PerformanceTerrainDefinition,
    pub shadows: PerformanceShadowsDefinition,
    pub vegetation: PerformanceVegetationDefinition,
    pub water: PerformanceWaterDefinition,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceTerrainDefinition {
    pub maximum_density_jobs: u32,
    pub maximum_mesh_jobs: u32,
    pub mesh_uploads_per_frame: u32,
    pub collider_builds_per_frame: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceShadowsDefinition {
    pub enabled: bool,
    pub quality: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceVegetationDefinition {
    pub density_multiplier: f32,
    pub maximum_distance_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceWaterDefinition {
    pub quality: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub capsule: PlayerCapsuleDefinition,
    pub movement: PlayerMovementDefinition,
    pub gravity_mps2: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerCapsuleDefinition {
    pub radius_m: f32,
    pub half_height_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerMovementDefinition {
    pub walk_speed_mps: f32,
    pub run_speed_mps: f32,
    pub acceleration_mps2: f32,
    pub deceleration_mps2: f32,
    pub rotation_speed_deg_per_s: f32,
    pub maximum_walkable_slope_deg: f32,
    pub step_height_m: f32,
    pub ground_snap_m: f32,
    pub jump_height_m: f32,
    #[serde(default = "default_jump_buffer_s")]
    pub jump_buffer_s: f32,
    #[serde(default = "default_coyote_time_s")]
    pub coyote_time_s: f32,
}

fn default_jump_buffer_s() -> f32 {
    0.12
}

fn default_coyote_time_s() -> f32 {
    0.1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub orbit: CameraOrbitDefinition,
    pub follow: CameraFollowDefinition,
    pub collision: CameraCollisionDefinition,
    pub controls: CameraControlsDefinition,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraOrbitDefinition {
    pub default_distance: f32,
    pub minimum_distance: f32,
    pub maximum_distance: f32,
    pub default_pitch_degrees: f32,
    pub minimum_pitch_degrees: f32,
    pub maximum_pitch_degrees: f32,
    pub mouse_sensitivity_x: f32,
    pub mouse_sensitivity_y: f32,
    pub invert_y: bool,
    pub zoom_speed: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraFollowDefinition {
    pub focus_height: f32,
    pub focus_offset_x: f32,
    pub focus_offset_z: f32,
    pub shoulder_offset: f32,
    pub follow_sharpness: f32,
    pub rotation_sharpness: f32,
    pub zoom_sharpness: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraCollisionDefinition {
    pub radius: f32,
    pub margin: f32,
    pub inward_sharpness: f32,
    pub outward_sharpness: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraControlsDefinition {
    pub both_buttons_move_forward: bool,
    pub recenter_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightingDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub sun: LightingSunDefinition,
    pub ambient: LightingAmbientDefinition,
    pub fog: LightingFogDefinition,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightingSunDefinition {
    pub direction: [f32; 3],
    pub illuminance_lux: f32,
    pub color: [f32; 3],
    pub shadows_enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightingAmbientDefinition {
    pub brightness: f32,
    pub color: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LightingFogDefinition {
    pub enabled: bool,
    pub color: [f32; 3],
    pub start_m: f32,
    pub end_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaterDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub sea_level_m: f32,
    pub shallow_color: [f32; 3],
    pub deep_color: [f32; 3],
    pub transparency: f32,
    pub wave_speed: f32,
    pub wave_amplitude: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub seed: u64,
    pub voxel: WorldVoxelDefinition,
    pub chunks: WorldChunksDefinition,
    pub terrain: StableId,
    pub biomes: StableId,
    pub materials: StableId,
    #[serde(default)]
    pub surface: Option<StableId>,
    pub water: StableId,
    pub lighting: StableId,
    #[serde(default)]
    pub sky: Option<StableId>,
    #[serde(default)]
    pub landmarks: Option<StableId>,
    #[serde(default)]
    pub structures: Vec<StableId>,
    #[serde(default)]
    pub ocean_extent_m: Option<f32>,
    /// Recipe-space XZ origin mapped to world (0,0). For 256 m centered worlds use 128.
    #[serde(default)]
    pub coord_offset: Option<[f32; 3]>,
    #[serde(default)]
    pub island_gen: Option<StableId>,
    #[serde(default)]
    pub resolution: Option<GenerationResolutionDefinition>,
}

/// Multi-tier generation spacing (PhasedExpansionPlan §2.2).
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GenerationResolutionDefinition {
    #[serde(default)]
    pub world_control_m: Option<f32>,
    #[serde(default)]
    pub regional_m: Option<f32>,
    #[serde(default)]
    pub local_m: Option<f32>,
    #[serde(default)]
    pub voxel_m: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldVoxelDefinition {
    pub cell_size_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldChunksDefinition {
    pub cells: [u32; 3],
    pub world_extent: [u32; 3],
}

/// Terrain generation recipe (YAML-driven procedural shapes).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainGenerationDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub spawn: Option<[f32; 3]>,
    #[serde(default)]
    pub includes: Vec<StableId>,
    #[serde(default)]
    pub operations: Vec<TerrainOperationDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerrainOperationDefinition {
    CoastalSurface {
        origin: [f32; 2],
        scale: [f32; 2],
        base_height: f32,
        height_range: f32,
        ridge_origin: [f32; 2],
        ridge_scale: [f32; 2],
        ridge_amplitude: f32,
        detail_frequency: f32,
        detail_amplitude: f32,
        detail_octaves: u32,
        #[serde(default)]
        regional_frequency: f32,
        #[serde(default)]
        regional_amplitude: f32,
        #[serde(default)]
        local_frequency: f32,
        #[serde(default)]
        local_amplitude: f32,
        #[serde(default)]
        ridged_amplitude: f32,
        #[serde(default)]
        domain_warp: f32,
    },
    ValleyBasin {
        origin: [f32; 2],
        scale: [f32; 2],
        depth_m: f32,
    },
    CoastModifier {
        #[serde(default = "default_coast_modifier_kind")]
        kind: String,
        center: [f32; 2],
        radius_m: f32,
        depth_m: f32,
        #[serde(default = "default_min_land_factor")]
        min_land_factor: f32,
        #[serde(default = "default_max_land_factor")]
        max_land_factor: f32,
    },
    Ellipsoid {
        center: [f32; 3],
        radii: [f32; 3],
        #[serde(default)]
        peak_noise: Option<[f32; 2]>,
        #[serde(default = "default_combine_union")]
        combine: String,
    },
    Capsule {
        start: [f32; 3],
        end: [f32; 3],
        radius: f32,
        #[serde(default = "default_combine_union")]
        combine: String,
    },
    NoisePerturb {
        scale: f32,
        amplitude: f32,
        density_min: f32,
        density_max: f32,
    },
    IslandMask {
        center: [f32; 2],
        radius_m: f32,
        falloff_m: f32,
        ocean_floor_y: f32,
        #[serde(default)]
        domain_warp: f32,
    },
    OceanFloor {
        origin: [f32; 2],
        scale: [f32; 2],
        base_depth_m: f32,
        variation_m: f32,
        #[serde(default = "default_ocean_detail_frequency")]
        detail_frequency: f32,
        #[serde(default = "default_ocean_detail_octaves")]
        detail_octaves: u32,
    },
    MountainPeak {
        center: [f32; 2],
        base_elevation_m: f32,
        base_radius_m: f32,
        peak_height_m: f32,
        #[serde(default = "default_peak_steepness")]
        steepness: f32,
        #[serde(default)]
        peak_noise: Option<[f32; 2]>,
    },
    UnderwaterTrench {
        points: Vec<[f32; 3]>,
        width_m: f32,
    },
}

fn default_ocean_detail_frequency() -> f32 {
    0.02
}

fn default_ocean_detail_octaves() -> u32 {
    3
}

fn default_peak_steepness() -> f32 {
    1.6
}

fn default_combine_union() -> String {
    "union".to_string()
}

fn default_coast_modifier_kind() -> String {
    "cove".to_string()
}

fn default_min_land_factor() -> f32 {
    0.3
}

fn default_max_land_factor() -> f32 {
    0.95
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BiomesDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub rules: Vec<BiomeRuleDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BiomeRuleDefinition {
    pub id: String,
    pub material_id: u16,
    pub color: [f32; 3],
    #[serde(default)]
    pub elevation_min: Option<f32>,
    #[serde(default)]
    pub elevation_max: Option<f32>,
    #[serde(default)]
    pub slope_min: Option<f32>,
    #[serde(default)]
    pub slope_max: Option<f32>,
    #[serde(default)]
    pub water_distance_max: Option<f32>,
    #[serde(default)]
    pub cave_depth_min: Option<f32>,
    #[serde(default)]
    pub moisture_min: Option<f32>,
    #[serde(default)]
    pub moisture_max: Option<f32>,
    #[serde(default)]
    pub temperature_min: Option<f32>,
    #[serde(default)]
    pub temperature_max: Option<f32>,
    #[serde(default)]
    pub river_distance_max: Option<f32>,
    #[serde(default)]
    pub vegetation_profile_id: Option<StableId>,
    #[serde(default)]
    pub ambient_audio_profile_id: Option<StableId>,
    #[serde(default)]
    pub weather_profile_id: Option<StableId>,
    #[serde(default)]
    pub spawn_profile_id: Option<StableId>,
    #[serde(default)]
    pub gameplay_tags: Vec<String>,
    #[serde(default = "default_tint")]
    pub tint: [f32; 3],
    #[serde(default)]
    pub roughness_modifier: f32,
    #[serde(default)]
    pub wetness_modifier: f32,
}

fn default_tint() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

impl BiomeRuleDefinition {
    pub fn new(id: &str, material_id: u16, color: [f32; 3]) -> Self {
        Self {
            id: id.into(),
            material_id,
            color,
            elevation_min: None,
            elevation_max: None,
            slope_min: None,
            slope_max: None,
            water_distance_max: None,
            cave_depth_min: None,
            moisture_min: None,
            moisture_max: None,
            temperature_min: None,
            temperature_max: None,
            river_distance_max: None,
            vegetation_profile_id: None,
            ambient_audio_profile_id: None,
            weather_profile_id: None,
            spawn_profile_id: None,
            gameplay_tags: Vec::new(),
            tint: default_tint(),
            roughness_modifier: 0.0,
            wetness_modifier: 0.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerrainMaterialsDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub materials: Vec<TerrainMaterialEntryDefinition>,
    /// Explicit texture-array layer order (required for schema_version >= 2).
    #[serde(default)]
    pub layers: Vec<StableId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TerrainMaterialEntryDefinition {
    /// Stable material key used by surface rules and layer ordering.
    #[serde(default)]
    pub key: Option<StableId>,
    /// Legacy numeric id (biome rules).
    #[serde(default)]
    pub id: Option<u16>,
    pub name: String,
    pub albedo: [f32; 3],
    #[serde(default = "default_one")]
    pub triplanar_scale: f32,
    #[serde(default = "default_roughness")]
    pub roughness: f32,
    /// Optional procedural generator block (Rock/Ground/Sand/Cobblestone).
    #[serde(default)]
    pub generator: Option<serde_yaml::Value>,
}

impl TerrainMaterialEntryDefinition {
    pub fn resolved_key(&self) -> StableId {
        if let Some(ref key) = self.key {
            return key.clone();
        }
        if let Some(id) = self.id {
            return StableId::new(&format!("material_{id}"));
        }
        StableId::new(&self.name)
    }

    pub fn resolved_legacy_id(&self) -> u16 {
        self.id.unwrap_or_else(|| {
            self.name
                .chars()
                .fold(0u16, |acc, c| acc.wrapping_mul(31).wrapping_add(c as u16))
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SurfaceRulesDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub gates: Vec<SurfaceGateDefinition>,
    /// Named blend presets referenced by weighted gate entries.
    #[serde(default)]
    pub classifiers: Vec<SurfaceClassifierDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceGateDefinition {
    pub id: String,
    /// When all conditions match, this gate contributes (or fully applies if exclusive).
    #[serde(default)]
    pub when: SurfaceConditionsDefinition,
    /// Product of smooth ramps applied to this gate's contribution weight.
    #[serde(default)]
    pub gate_weight: SurfaceGateWeightDefinition,
    /// If true, first matching gate wins and stops evaluation.
    #[serde(default)]
    pub exclusive: bool,
    /// Inline blend entries (mutually exclusive with `classifier`).
    #[serde(default)]
    pub blend: Vec<SurfaceBlendEntryDefinition>,
    /// Reference to a named classifier preset.
    #[serde(default)]
    pub classifier: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields, default)]
pub struct SurfaceConditionsDefinition {
    #[serde(default)]
    pub cave_exposure_min: Option<f32>,
    #[serde(default)]
    pub water_depth_min: Option<f32>,
    #[serde(default)]
    pub coast_distance_max: Option<f32>,
    #[serde(default)]
    pub river_distance_max: Option<f32>,
    #[serde(default)]
    pub slope_min: Option<f32>,
    #[serde(default)]
    pub slope_max: Option<f32>,
    #[serde(default)]
    pub elevation_min: Option<f32>,
    #[serde(default)]
    pub elevation_max: Option<f32>,
    #[serde(default)]
    pub elevation_above_sea_min: Option<f32>,
    #[serde(default)]
    pub elevation_above_sea_max: Option<f32>,
    #[serde(default)]
    pub moisture_min: Option<f32>,
    #[serde(default)]
    pub moisture_max: Option<f32>,
    #[serde(default)]
    pub geology: Option<String>,
    #[serde(default)]
    pub biome: Option<String>,
    #[serde(default)]
    pub soft_grassland_min: Option<f32>,
    #[serde(default)]
    pub soft_forest_min: Option<f32>,
    #[serde(default)]
    pub soft_wetland_min: Option<f32>,
    #[serde(default)]
    pub soft_alpine_min: Option<f32>,
    #[serde(default)]
    pub fallback: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields, default)]
pub struct SurfaceGateWeightDefinition {
    #[serde(default)]
    pub coast_distance: Option<SurfaceRampDefinition>,
    #[serde(default)]
    pub river_distance: Option<SurfaceRampDefinition>,
    #[serde(default)]
    pub slope: Option<SurfaceRampDefinition>,
    #[serde(default)]
    pub elevation_above_sea: Option<SurfaceRampDefinition>,
    #[serde(default)]
    pub moisture: Option<SurfaceRampDefinition>,
    #[serde(default)]
    pub cave_exposure: Option<SurfaceRampDefinition>,
    #[serde(default)]
    pub wave_exposure: Option<SurfaceRampDefinition>,
    #[serde(default)]
    pub soft_alpine: Option<SurfaceRampDefinition>,
    #[serde(default)]
    pub soft_wetland: Option<SurfaceRampDefinition>,
    #[serde(default)]
    pub constant: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceRampDefinition {
    pub from: f32,
    pub to: f32,
    #[serde(default)]
    pub invert: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceBlendEntryDefinition {
    pub material: StableId,
    pub weight: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceClassifierDefinition {
    pub id: String,
    #[serde(default)]
    pub blend: Vec<SurfaceBlendEntryDefinition>,
    /// Weighted mix of other classifier ids.
    #[serde(default)]
    pub weighted_mix: Vec<SurfaceWeightedMixEntryDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceWeightedMixEntryDefinition {
    pub classifier: String,
    pub weight: f32,
}

fn default_one() -> f32 {
    1.0
}

fn default_roughness() -> f32 {
    0.85
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VegetationDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub rules: Vec<VegetationRuleDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VegetationRuleDefinition {
    pub category: String,
    pub mesh: String,
    #[serde(default)]
    pub biomes: Vec<String>,
    #[serde(default = "default_density")]
    pub density: f32,
    #[serde(default)]
    pub slope_max_deg: f32,
    #[serde(default)]
    pub spacing_m: f32,
}

fn default_density() -> f32 {
    0.35
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaveDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub operations: Vec<TerrainOperationDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DebugDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub bindings: DebugBindingsDefinition,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DebugBindingsDefinition {
    pub panel: String,
    pub chunk_bounds: String,
    pub wireframe: String,
    pub biome: String,
    pub material: String,
    pub collider: String,
    pub density: String,
    #[serde(default = "default_normals_key")]
    pub normals: String,
    pub regen: String,
    pub next_seed: String,
    pub freeze_pipeline: String,
    #[serde(default = "default_digit1")]
    pub subtract: String,
    #[serde(default = "default_digit2")]
    pub add: String,
    #[serde(default = "default_digit3")]
    pub paint: String,
}

fn default_digit1() -> String {
    "Digit1".to_string()
}

fn default_digit2() -> String {
    "Digit2".to_string()
}

fn default_digit3() -> String {
    "Digit3".to_string()
}

fn default_normals_key() -> String {
    "KeyN".to_string()
}

/// Options panel config (not compiled into gameplay registry).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IslandGenerationDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    #[serde(default)]
    pub description: String,
    pub island: IslandShapeDefinition,
    pub volcano: VolcanoDefinition,
    #[serde(default)]
    pub surface_noise: SurfaceNoiseDefinition,
    pub hydrology: IslandHydrologyDefinition,
    pub erosion: IslandErosionDefinition,
    pub coast: IslandCoastDefinition,
    #[serde(default)]
    pub beaches: BeachDefinition,
    #[serde(default)]
    pub caves: IslandCaveDefinition,
    #[serde(default)]
    pub resolution: Option<GenerationResolutionDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IslandShapeDefinition {
    pub playable_diameter_m: f32,
    pub maximum_height_m: f32,
    pub sea_level_m: f32,
    #[serde(default = "default_lobe_count")]
    pub lobe_count: u32,
    #[serde(default = "default_warp_frequency")]
    pub warp_frequency: f32,
    #[serde(default = "default_warp_amplitude")]
    pub warp_amplitude: f32,
}

fn default_lobe_count() -> u32 {
    3
}
fn default_warp_frequency() -> f32 {
    0.004
}
fn default_warp_amplitude() -> f32 {
    18.0
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VolcanoDefinition {
    pub center: [f32; 2],
    pub shield_radius_m: f32,
    pub shield_exponent: f32,
    pub shield_height_m: f32,
    pub summit_radius_m: f32,
    pub summit_exponent: f32,
    pub summit_height_m: f32,
    pub caldera_radius_m: f32,
    pub caldera_depth_m: f32,
    pub caldera_rim_height_m: f32,
    #[serde(default = "default_ridge_count")]
    pub radial_ridge_count: u32,
    #[serde(default)]
    pub collapse_direction_deg: f32,
    #[serde(default = "default_collapse_depth")]
    pub collapse_depth_m: f32,
}

fn default_ridge_count() -> u32 {
    7
}
fn default_collapse_depth() -> f32 {
    45.0
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurfaceNoiseDefinition {
    #[serde(default = "default_regional_amplitude")]
    pub regional_amplitude_m: f32,
    #[serde(default = "default_local_amplitude")]
    pub local_amplitude_m: f32,
    #[serde(default = "default_voxel_amplitude")]
    pub voxel_amplitude_m: f32,
}

fn default_regional_amplitude() -> f32 {
    14.0
}
fn default_local_amplitude() -> f32 {
    3.5
}
fn default_voxel_amplitude() -> f32 {
    0.35
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IslandHydrologyDefinition {
    #[serde(default = "default_routing")]
    pub routing: String,
    pub rainfall_base: f32,
    pub stream_threshold: f32,
    pub permanent_river_threshold: f32,
    pub minimum_stream_length_m: f32,
}

fn default_routing() -> String {
    "d8".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IslandErosionDefinition {
    pub stream_power_iterations: u32,
    pub m: f32,
    pub n: f32,
    pub maximum_step_m: f32,
    pub thermal_iterations: u32,
    pub thermal_transfer_rate: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IslandCoastDefinition {
    pub shelf_width_min_m: f32,
    pub shelf_width_max_m: f32,
    pub shelf_depth_min_m: f32,
    pub shelf_depth_max_m: f32,
    pub deep_slope_min: f32,
    pub deep_slope_max: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BeachDefinition {
    pub maximum_slope_deg: f32,
    pub width_min_m: f32,
    pub width_max_m: f32,
    pub berm_height_min_m: f32,
    pub berm_height_max_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct IslandCaveDefinition {
    pub chamber_count_min: u32,
    pub chamber_count_max: u32,
    pub passage_radius_min_m: f32,
    pub passage_radius_max_m: f32,
    pub minimum_cover_m: f32,
    pub maximum_depth_m: f32,
    #[serde(default = "default_overhang_enabled")]
    pub overhang_enabled: bool,
}

fn default_overhang_enabled() -> bool {
    true
}

/// Schema driving the setup/options UI sliders.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupSchemaDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub groups: Vec<SetupGroupDefinition>,
    #[serde(default)]
    pub preview_modes: Vec<SetupPreviewModeDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupGroupDefinition {
    pub id: String,
    pub label: String,
    pub parameters: Vec<SetupParameterDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupParameterDefinition {
    pub id: String,
    pub label: String,
    pub bind: String,
    pub min: f32,
    pub max: f32,
    #[serde(default = "default_param_step")]
    pub step: f32,
    #[serde(default)]
    pub default: f32,
}

fn default_param_step() -> f32 {
    0.1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupPreviewModeDefinition {
    pub id: String,
    pub label: String,
}

/// Options panel config (not compiled into gameplay registry).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionsDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub panel: OptionsPanelDefinition,
    #[serde(default)]
    pub stubs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionsPanelDefinition {
    pub toggle_key: String,
    pub default_tab: String,
}

/// Physics world settings stub for future compilation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicsDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub gravity_mps2: f32,
    pub fixed_timestep_hz: u32,
    pub maximum_substeps: u32,
}

/// River recipe metadata (generation runs in terrain_generation).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiverDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub source: RiverSourceDefinition,
    pub destination: RiverDestinationDefinition,
    pub routing: RiverRoutingDefinition,
    pub channel: RiverChannelDefinition,
    pub water: RiverWaterDefinition,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiverSourceDefinition {
    pub region_center: [f32; 2],
    pub region_radius_m: f32,
    pub minimum_elevation_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiverDestinationDefinition {
    #[serde(rename = "type")]
    pub destination_type: String,
    pub required_kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiverRoutingDefinition {
    pub grid_spacing_m: f32,
    pub direction_inertia: f32,
    pub maximum_turn_deg: f32,
    pub depression_repair_radius_cells: u32,
    pub maximum_breach_depth_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiverChannelDefinition {
    pub source_width_m: f32,
    pub mouth_width_m: f32,
    pub source_depth_m: f32,
    pub mouth_depth_m: f32,
    pub bank_width_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiverWaterDefinition {
    pub minimum_depth_m: f32,
    pub maximum_segment_slope: f32,
    pub waterfall_threshold_m: f32,
}

/// Hydrology body metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HydrologyDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub kind: String,
    pub elevation_m: f32,
    #[serde(default)]
    pub depth_m: Option<f32>,
    #[serde(default)]
    pub center: Option<[f32; 2]>,
    #[serde(default)]
    pub radius_m: Option<f32>,
}

/// Atmosphere presentation (assets/config/atmosphere.yaml).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtmosphereDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub sun: AtmosphereSunDefinition,
    #[serde(default)]
    pub moon: AtmosphereMoonDefinition,
    pub ambient: AtmosphereAmbientDefinition,
    pub exposure: AtmosphereExposureDefinition,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct AtmosphereMoonDefinition {
    pub enabled: bool,
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
    pub illuminance: f32,
    pub phase: f32,
    pub angular_radius: f32,
}

impl Default for AtmosphereMoonDefinition {
    fn default() -> Self {
        Self {
            enabled: false,
            azimuth_deg: 315.0,
            elevation_deg: 35.0,
            illuminance: 0.15,
            phase: 1.0,
            angular_radius: 0.008,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtmosphereSunDefinition {
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
    pub illuminance_lux: f32,
    pub color: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtmosphereAmbientDefinition {
    pub color: [f32; 3],
    pub brightness: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtmosphereExposureDefinition {
    pub target: f32,
    pub adaptation_speed: f32,
}

/// Fog presentation (assets/config/fog.yaml).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FogDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub distance: FogDistanceDefinition,
    pub height: FogHeightDefinition,
    pub underwater: FogVolumeDefinition,
    pub cave: FogVolumeDefinition,
    #[serde(default)]
    pub local_volumes: Vec<FogLocalVolumeDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FogLocalVolumeDefinition {
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
    pub density: f32,
    pub color: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FogDistanceDefinition {
    pub color: [f32; 3],
    pub start_m: f32,
    pub end_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FogHeightDefinition {
    pub base_height_m: f32,
    pub density: f32,
    pub color: [f32; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FogVolumeDefinition {
    pub density: f32,
    pub color: [f32; 3],
}

/// Sky presentation (assets/config/sky.yaml).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkyDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub zenith_color: [f32; 3],
    pub horizon_color: [f32; 3],
    pub mie_strength: f32,
    pub sun_disc_radius: f32,
    pub stars_enabled: bool,
    #[serde(default = "default_stars_density")]
    pub stars_density: f32,
    pub clouds_enabled: bool,
    #[serde(default = "default_clouds_opacity")]
    pub clouds_opacity: f32,
    #[serde(default = "default_clouds_speed")]
    pub clouds_speed: f32,
    #[serde(default = "default_clouds_direction_deg")]
    pub clouds_direction_deg: f32,
    #[serde(default = "default_clouds_altitude")]
    pub clouds_altitude: f32,
    #[serde(default = "default_night_zenith")]
    pub night_zenith_color: [f32; 3],
    #[serde(default = "default_night_horizon")]
    pub night_horizon_color: [f32; 3],
    pub shader: String,
}

fn default_stars_density() -> f32 {
    0.55
}

fn default_clouds_opacity() -> f32 {
    0.35
}

fn default_clouds_speed() -> f32 {
    0.015
}

fn default_clouds_direction_deg() -> f32 {
    45.0
}

fn default_clouds_altitude() -> f32 {
    0.22
}

fn default_night_zenith() -> [f32; 3] {
    [0.02, 0.04, 0.14]
}

fn default_night_horizon() -> [f32; 3] {
    [0.06, 0.08, 0.16]
}

/// Per-body water surface material (assets/terrain/water/*.water.yaml).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaterBodyMaterialDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub material: WaterBodyMaterialProps,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaterBodyMaterialProps {
    pub shallow_color: [f32; 3],
    pub deep_color: [f32; 3],
    pub transparency: f32,
    pub wave_amplitude: f32,
    pub wave_speed: f32,
    #[serde(default)]
    pub flow_tint: Option<[f32; 3]>,
}

/// Semantic landmarks and fog volumes for a world profile.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LandmarksDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    #[serde(default)]
    pub facts: Vec<LandmarkFactDefinition>,
    #[serde(default)]
    pub route_signs: Vec<LandmarkSignDefinition>,
    #[serde(default)]
    pub fog_volumes: Vec<FogLocalVolumeDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LandmarkFactDefinition {
    pub tag: String,
    pub position: [f32; 3],
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LandmarkSignDefinition {
    pub position: [f32; 3],
    pub label: String,
}

/// Traversal route waypoints for automated tests.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutesDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub routes: Vec<RouteDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteDefinition {
    pub id: String,
    pub waypoints: Vec<[f32; 2]>,
}

/// Procedural structure blueprint (fort, shelter, etc.).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructureDefinition {
    #[serde(flatten)]
    pub header: DefinitionHeader,
    pub placement: StructurePlacementDefinition,
    pub parts: Vec<StructurePartDefinition>,
    #[serde(default = "default_structure_collision")]
    pub collision: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructurePlacementDefinition {
    pub anchor: [f32; 3],
    #[serde(default)]
    pub yaw_deg: f32,
    #[serde(default = "default_flatten_radius")]
    pub flatten_radius_m: f32,
}

fn default_flatten_radius() -> f32 {
    12.0
}

fn default_structure_collision() -> String {
    "static_terrain".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructurePartDefinition {
    pub kind: String,
    #[serde(default)]
    pub size: Option<[f32; 3]>,
    #[serde(default)]
    pub radius: Option<f32>,
    #[serde(default)]
    pub height: Option<f32>,
    pub offset: [f32; 3],
    #[serde(default)]
    pub material: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Clone, Debug)]
pub enum RawDefinition {
    App(AppDefinition),
    Performance(PerformanceDefinition),
    Player(PlayerDefinition),
    Camera(CameraDefinition),
    Lighting(LightingDefinition),
    Water(WaterDefinition),
    World(WorldDefinition),
    TerrainGeneration(TerrainGenerationDefinition),
    Biomes(BiomesDefinition),
    TerrainMaterials(TerrainMaterialsDefinition),
    SurfaceRules(SurfaceRulesDefinition),
    Vegetation(VegetationDefinition),
    Cave(CaveDefinition),
    Debug(DebugDefinition),
    Options(OptionsDefinition),
    Physics(PhysicsDefinition),
    River(RiverDefinition),
    Hydrology(HydrologyDefinition),
    WaterBodyMaterial(WaterBodyMaterialDefinition),
    Atmosphere(AtmosphereDefinition),
    Fog(FogDefinition),
    Sky(SkyDefinition),
    Landmarks(LandmarksDefinition),
    Routes(RoutesDefinition),
    Structure(StructureDefinition),
    IslandGeneration(IslandGenerationDefinition),
    SetupSchema(SetupSchemaDefinition),
}

impl RawDefinition {
    pub fn id(&self) -> &StableId {
        match self {
            Self::App(def) => &def.header.id,
            Self::Performance(def) => &def.header.id,
            Self::Player(def) => &def.header.id,
            Self::Camera(def) => &def.header.id,
            Self::Lighting(def) => &def.header.id,
            Self::Water(def) => &def.header.id,
            Self::World(def) => &def.header.id,
            Self::TerrainGeneration(def) => &def.header.id,
            Self::Biomes(def) => &def.header.id,
            Self::TerrainMaterials(def) => &def.header.id,
            Self::SurfaceRules(def) => &def.header.id,
            Self::Vegetation(def) => &def.header.id,
            Self::Cave(def) => &def.header.id,
            Self::Debug(def) => &def.header.id,
            Self::Options(def) => &def.header.id,
            Self::Physics(def) => &def.header.id,
            Self::River(def) => &def.header.id,
            Self::Hydrology(def) => &def.header.id,
            Self::WaterBodyMaterial(def) => &def.header.id,
            Self::Atmosphere(def) => &def.header.id,
            Self::Fog(def) => &def.header.id,
            Self::Sky(def) => &def.header.id,
            Self::Landmarks(def) => &def.header.id,
            Self::Routes(def) => &def.header.id,
            Self::Structure(def) => &def.header.id,
            Self::IslandGeneration(def) => &def.header.id,
            Self::SetupSchema(def) => &def.header.id,
        }
    }

    pub fn validate_header(&self) -> shared::DataResult<()> {
        match self {
            Self::App(def) => def.header.validate(),
            Self::Performance(def) => def.header.validate(),
            Self::Player(def) => def.header.validate(),
            Self::Camera(def) => def.header.validate(),
            Self::Lighting(def) => def.header.validate(),
            Self::Water(def) => def.header.validate(),
            Self::World(def) => def.header.validate(),
            Self::TerrainGeneration(def) => def.header.validate(),
            Self::Biomes(def) => def.header.validate(),
            Self::TerrainMaterials(def) => def.header.validate_schema(&[1, 2]),
            Self::SurfaceRules(def) => def.header.validate(),
            Self::Vegetation(def) => def.header.validate(),
            Self::Cave(def) => def.header.validate(),
            Self::Debug(def) => def.header.validate(),
            Self::Options(def) => def.header.validate(),
            Self::Physics(def) => def.header.validate(),
            Self::River(def) => def.header.validate(),
            Self::Hydrology(def) => def.header.validate(),
            Self::WaterBodyMaterial(def) => def.header.validate(),
            Self::Atmosphere(def) => def.header.validate(),
            Self::Fog(def) => def.header.validate(),
            Self::Sky(def) => def.header.validate(),
            Self::Landmarks(def) => def.header.validate(),
            Self::Routes(def) => def.header.validate(),
            Self::Structure(def) => def.header.validate(),
            Self::IslandGeneration(def) => def.header.validate(),
            Self::SetupSchema(def) => def.header.validate(),
        }
    }
}
