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
    pub water: StableId,
    pub lighting: StableId,
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
}

fn default_combine_union() -> String {
    "union".to_string()
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
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TerrainMaterialEntryDefinition {
    pub id: u16,
    pub name: String,
    pub albedo: [f32; 3],
    #[serde(default = "default_one")]
    pub triplanar_scale: f32,
    #[serde(default = "default_roughness")]
    pub roughness: f32,
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
}

fn default_normals_key() -> String {
    "KeyN".to_string()
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
    Vegetation(VegetationDefinition),
    Cave(CaveDefinition),
    Debug(DebugDefinition),
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
            Self::Vegetation(def) => &def.header.id,
            Self::Cave(def) => &def.header.id,
            Self::Debug(def) => &def.header.id,
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
            Self::TerrainMaterials(def) => def.header.validate(),
            Self::Vegetation(def) => def.header.validate(),
            Self::Cave(def) => def.header.validate(),
            Self::Debug(def) => def.header.validate(),
        }
    }
}
