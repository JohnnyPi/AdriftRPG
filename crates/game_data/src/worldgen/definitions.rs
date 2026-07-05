//! YAML source definitions for Milestone A world compiler.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorldRecipeSource {
    pub schema_version: u32,
    pub id: String,
    pub seed: u64,
    pub extent: ExtentSource,
    pub resolutions: ResolutionSource,
    pub boundary: String,
    pub islands: Vec<String>,
    pub geology: String,
    pub refinement: String,
    pub climate: String,
    pub hydrology: String,
    pub erosion: String,
    pub coast: String,
    pub biomes: String,
    pub strata: String,
    #[serde(default = "default_caves_recipe")]
    pub caves: String,
    #[serde(default)]
    pub validation: Option<String>,
}

fn default_caves_recipe() -> String {
    "caves.tropical_volcanic".into()
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExtentSource {
    pub width_m: f64,
    pub depth_m: f64,
    #[serde(default = "default_vertical_min")]
    pub vertical_min_m: f64,
    #[serde(default = "default_vertical_max")]
    pub vertical_max_m: f64,
    #[serde(default)]
    pub sea_level_m: f32,
}

fn default_vertical_min() -> f64 {
    -2048.0
}
fn default_vertical_max() -> f64 {
    4096.0
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolutionSource {
    pub control_cell_m: f64,
    pub regional_cell_m: f64,
    pub local_cell_m: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoundaryRecipeSource {
    pub schema_version: u32,
    pub id: String,
    pub shape: BoundaryShapeSource,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BoundaryShapeSource {
    BoundedOcean(BoundedOceanSource),
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoundedOceanSource {
    pub ocean_edge_start_fraction: f32,
    pub maximum_depth_m: f32,
    #[serde(default = "default_safety_margin_fraction")]
    pub safety_margin_fraction: f32,
    #[serde(default)]
    pub variation_amplitude_m: f32,
    #[serde(default)]
    pub variation_frequency: f64,
}

fn default_safety_margin_fraction() -> f32 {
    0.08
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IslandRecipeSource {
    pub schema_version: u32,
    pub id: String,
    pub placement: IslandPlacementSource,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IslandPlacementSource {
    SingleCentered(SingleIslandSource),
    Explicit(ExplicitIslandListSource),
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SingleIslandSource {
    pub age_myr: f32,
    pub uplift: f32,
    pub volcanic_activity: f32,
    pub footprint: FootprintSource,
    pub volcano: VolcanoSource,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExplicitIslandListSource {
    pub islands: Vec<ExplicitIslandEntrySource>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExplicitIslandEntrySource {
    pub center_x_m: f64,
    pub center_z_m: f64,
    pub age_myr: f32,
    pub footprint: FootprintSource,
    pub volcano: VolcanoSource,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FootprintSource {
    Ellipse(EllipseFootprintSource),
    WarpedEllipse(WarpedEllipseFootprintSource),
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EllipseFootprintSource {
    pub major_radius_m: f32,
    pub minor_radius_m: f32,
    pub rotation_deg: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WarpedEllipseFootprintSource {
    pub major_radius_m: f32,
    pub minor_radius_m: f32,
    pub rotation_deg: f32,
    pub warp_amplitude_m: f32,
    pub warp_wavelength_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VolcanoSource {
    pub peak_height_m: f32,
    pub shield_radius_m: f32,
    pub caldera_radius_m: f32,
    pub caldera_depth_m: f32,
    #[serde(default)]
    pub secondary_vents: u32,
    #[serde(default)]
    pub ridge_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GeologyRecipeSource {
    pub schema_version: u32,
    pub id: String,
    pub weathering_age_threshold_myr: f32,
    pub tuff_age_threshold_myr: f32,
    pub coastal_weathering_band_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RefinementRecipeSource {
    pub schema_version: u32,
    pub id: String,
    pub window_interior_samples: [u32; 2],
    pub window_stride_samples: [u32; 2],
    pub window_halo_samples: u32,
    pub regional_amplitude_m: f32,
    pub coast_preserve_start_m: f32,
    pub coast_preserve_end_m: f32,
    pub seam_max_elevation_diff_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidationRecipeSource {
    pub schema_version: u32,
    pub id: String,
    pub land_fraction_min: f32,
    pub land_fraction_max: f32,
    pub min_peak_elevation_m: f32,
    pub max_peak_elevation_m: f32,
    #[serde(default = "default_river_ocean_ratio")]
    pub river_ocean_connection_ratio_min: f32,
    #[serde(default = "default_max_disconnected_river")]
    pub max_disconnected_river_fraction: f32,
    #[serde(default)]
    pub min_permanent_river_length_m: f32,
    #[serde(default)]
    pub reef_area_min_m2: f32,
    #[serde(default = "default_lagoon_count_min")]
    pub lagoon_count_min: u32,
    #[serde(default)]
    pub biome_entropy_min: u32,
    #[serde(default)]
    pub min_cave_systems: u32,
    #[serde(default = "default_min_traversable_caves")]
    pub min_traversable_cave_systems: u32,
    #[serde(default)]
    pub max_cave_mouth_breaches: u32,
}

fn default_min_traversable_caves() -> u32 {
    1
}

fn default_lagoon_count_min() -> u32 {
    0
}

fn default_river_ocean_ratio() -> f32 {
    0.85
}

fn default_max_disconnected_river() -> f32 {
    0.05
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClimateRecipeSource {
    pub schema_version: u32,
    pub id: String,
    pub base_temperature_c: f32,
    pub lapse_rate_c_per_km: f32,
    pub prevailing_wind: PrevailingWindSource,
    pub rainfall: RainfallClimateSource,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PrevailingWindSource {
    pub direction_deg: f32,
    pub strength: f32,
    pub moisture: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RainfallClimateSource {
    pub ocean_recharge: f32,
    pub orographic_factor: f32,
    pub rain_shadow_factor: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HydrologyRecipeSource {
    pub schema_version: u32,
    pub id: String,
    #[serde(default = "default_routing")]
    pub routing: String,
    pub rainfall_weight: f32,
    pub stream_threshold: f32,
    pub permanent_river_threshold: f32,
    pub minimum_stream_length_m: f32,
    #[serde(default = "default_lake_min")]
    pub lake_min_area_cells: u32,
    #[serde(default = "default_wetland_moisture")]
    pub wetland_moisture_threshold: f32,
    #[serde(default = "default_waterfall_drop")]
    pub waterfall_min_drop_m: f32,
    #[serde(default = "default_waterfall_discharge")]
    pub waterfall_min_discharge: f32,
}

fn default_routing() -> String {
    "d8".into()
}
fn default_lake_min() -> u32 {
    6
}
fn default_wetland_moisture() -> f32 {
    0.55
}
fn default_waterfall_drop() -> f32 {
    3.0
}
fn default_waterfall_discharge() -> f32 {
    20.0
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ErosionRecipeSource {
    pub schema_version: u32,
    pub id: String,
    pub iterations: u32,
    pub stream_power: StreamPowerErosionSource,
    pub thermal: ThermalErosionSource,
    pub sediment: SedimentErosionSource,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StreamPowerErosionSource {
    pub m: f32,
    pub n: f32,
    pub maximum_step_m: f32,
    pub iterations_per_cycle: u32,
    #[serde(default = "default_sp_erodibility")]
    pub erodibility: f32,
}

fn default_sp_erodibility() -> f32 {
    0.02
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ThermalErosionSource {
    pub talus_deg: f32,
    pub transfer_rate: f32,
    pub iterations_per_cycle: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SedimentErosionSource {
    pub pickup_rate: f32,
    pub transport_rate: f32,
    pub deposition_rate: f32,
    pub capacity_factor: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoastRecipeSource {
    pub schema_version: u32,
    pub id: String,
    pub beaches: CoastBeachSource,
    pub cliffs: CoastCliffSource,
    pub reefs: CoastReefSource,
    pub lagoons: CoastLagoonSource,
    pub mangroves: CoastMangroveSource,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoastBeachSource {
    pub width_max_m: f32,
    pub maximum_slope_deg: f32,
    pub berm_height_m: [f32; 2],
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoastCliffSource {
    pub minimum_slope_deg: f32,
    pub minimum_exposure: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoastReefSource {
    pub min_age_myr: f32,
    pub depth_m: [f32; 2],
    pub max_sediment: f32,
    pub min_temperature: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoastLagoonSource {
    pub max_depth_m: f32,
    pub reef_enclosure_min: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CoastMangroveSource {
    pub max_slope_deg: f32,
    pub salinity_band_m: [f32; 2],
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BiomeRecipeSource {
    pub schema_version: u32,
    pub id: String,
    pub land: BiomeLandTuningSource,
    pub marine: BiomeMarineTuningSource,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BiomeLandTuningSource {
    pub cloud_forest_elevation_m: [f32; 2],
    pub dry_forest_rainfall_max: f32,
    pub montane_shrub_elevation_m: f32,
    pub volcanic_barren_slope_deg: f32,
    pub wetland_moisture_min: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BiomeMarineTuningSource {
    pub reef_depth_m: [f32; 2],
    pub shelf_depth_m: f32,
    pub deep_coastal_depth_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrataRecipeSource {
    pub schema_version: u32,
    pub id: String,
    pub layers: Vec<StrataLayerSource>,
    pub deposits: Vec<StrataDepositSource>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrataLayerSource {
    pub material: String,
    pub thickness_m: StrataThicknessSource,
    #[serde(default)]
    pub requires: Option<StrataRequiresSource>,
    #[serde(default)]
    pub driven_by: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum StrataThicknessSource {
    Range([f32; 2]),
    Remaining(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrataRequiresSource {
    #[serde(default)]
    pub biome_tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrataDepositSource {
    pub id: String,
    pub mask: String,
    pub thickness_m: [f32; 2],
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CavesRecipeSource {
    pub schema_version: u32,
    pub id: String,
    pub lava_max_age_myr: f32,
    pub limestone_min_permeability: f32,
    pub sea_tidal_band_m: [f32; 2],
    pub lava_tube: CaveFamilyProfileSource,
    pub limestone: CaveFamilyProfileSource,
    pub sea_cave: CaveFamilyProfileSource,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaveFamilyProfileSource {
    pub systems_max: u32,
    pub chamber_count_min: u32,
    pub chamber_count_max: u32,
    pub passage_radius_min_m: f32,
    pub passage_radius_max_m: f32,
    pub minimum_cover_m: f32,
    pub maximum_depth_m: f32,
    pub entrance_threshold: f32,
    #[serde(default)]
    pub overhang_enabled: bool,
}

/// Loaded bundle of source definitions before resolution.
#[derive(Clone, Debug, Default)]
pub struct WorldgenSourceBundle {
    pub worlds: std::collections::BTreeMap<String, WorldRecipeSource>,
    pub boundaries: std::collections::BTreeMap<String, BoundaryRecipeSource>,
    pub islands: std::collections::BTreeMap<String, IslandRecipeSource>,
    pub geology: std::collections::BTreeMap<String, GeologyRecipeSource>,
    pub refinement: std::collections::BTreeMap<String, RefinementRecipeSource>,
    pub climate: std::collections::BTreeMap<String, ClimateRecipeSource>,
    pub hydrology: std::collections::BTreeMap<String, HydrologyRecipeSource>,
    pub erosion: std::collections::BTreeMap<String, ErosionRecipeSource>,
    pub coasts: std::collections::BTreeMap<String, CoastRecipeSource>,
    pub biomes: std::collections::BTreeMap<String, BiomeRecipeSource>,
    pub strata: std::collections::BTreeMap<String, StrataRecipeSource>,
    pub caves: std::collections::BTreeMap<String, CavesRecipeSource>,
    pub validation: std::collections::BTreeMap<String, ValidationRecipeSource>,
}
