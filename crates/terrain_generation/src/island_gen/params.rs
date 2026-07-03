// crates/terrain_generation/src/island_gen/params.rs
//! Island generation parameters (runtime, independent of Bevy).
//!
//! Geometry budget helpers live here so that `fit_to_ocean_extent` and
//! `world_setup::validate_island_world_budget` can never disagree about what
//! "fits": both are built on [`IslandShapeParams::footprint_support_radius_m`]
//! and [`IslandGenParams::max_fit_diameter_m`].

use crate::resolution::GenerationResolution;

/// Ratio of the island's *support* radius (where the footprint mask finally
/// reaches zero) to `playable_diameter_m / 2`, excluding domain warp.
///
/// Derivation (must match `footprint.rs`): lobe centers are offset up to
/// `0.18 x radius` from the island center, lobe elliptical radii reach
/// `0.95 x radius`, and the mask falloff extends support to `1.05 x` the lobe
/// radius. Worst case: `0.18 + 0.95 * 1.05 = 1.1775`.
pub const FOOTPRINT_SUPPORT_FACTOR: f32 = 0.18 + 0.95 * 1.05;

/// Minimum ring of guaranteed open ocean between the island's support radius
/// and the atlas edge, before resolution-based padding takes over.
pub const MIN_OCEAN_PADDING_M: f32 = 16.0;

#[derive(Clone, Debug)]
pub struct IslandGenParams {
    pub seed: u64,
    pub center: [f32; 2],
    pub ocean_extent_m: f32,
    pub resolution: GenerationResolution,
    pub island: IslandShapeParams,
    pub volcano: VolcanoParams,
    pub surface_noise: SurfaceNoiseParams,
    pub hydrology: HydrologyParams,
    pub erosion: ErosionParams,
    pub coast: CoastParams,
    pub beaches: BeachParams,
    pub caves: CaveParams,
}

#[derive(Clone, Debug)]
pub struct IslandShapeParams {
    pub playable_diameter_m: f32,
    pub maximum_height_m: f32,
    pub sea_level_m: f32,
    pub lobe_count: u32,
    pub warp_frequency: f32,
    pub warp_amplitude: f32,
}

impl IslandShapeParams {
    /// Outermost radius (meters from island center) at which the footprint can
    /// still contribute support: lobe offsets + lobe radii + mask falloff
    /// ([`FOOTPRINT_SUPPORT_FACTOR`]) plus the worst-case outward domain warp.
    ///
    /// Everything beyond this radius is guaranteed open ocean. Validation
    /// compares this against the chunk volume's horizontal half-extent; the
    /// atlas fit compares it against `ocean_extent_m / 2` minus padding.
    pub fn footprint_support_radius_m(&self) -> f32 {
        self.playable_diameter_m * 0.5 * FOOTPRINT_SUPPORT_FACTOR + self.warp_amplitude.max(0.0)
    }
}

#[derive(Clone, Debug)]
pub struct VolcanoParams {
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
    pub radial_ridge_count: u32,
    pub collapse_direction_deg: f32,
    pub collapse_depth_m: f32,
}

#[derive(Clone, Debug)]
pub struct SurfaceNoiseParams {
    pub regional_amplitude_m: f32,
    pub local_amplitude_m: f32,
    pub voxel_amplitude_m: f32,
}

#[derive(Clone, Debug)]
pub struct HydrologyParams {
    pub rainfall_base: f32,
    pub stream_threshold: f32,
    pub permanent_river_threshold: f32,
    pub minimum_stream_length_m: f32,
}

#[derive(Clone, Debug)]
pub struct ErosionParams {
    pub stream_power_iterations: u32,
    pub m: f32,
    pub n: f32,
    pub maximum_step_m: f32,
    pub stream_power_erodibility: f32,
    pub thermal_iterations: u32,
    pub thermal_transfer_rate: f32,
    pub thermal_talus_deg: f32,
    pub river_bank_width_m: f32,
    pub river_carve_strength: f32,
}

#[derive(Clone, Debug)]
pub struct CoastParams {
    pub shelf_width_min_m: f32,
    pub shelf_width_max_m: f32,
    pub shelf_depth_min_m: f32,
    pub shelf_depth_max_m: f32,
    pub deep_slope_min: f32,
    pub deep_slope_max: f32,
}

#[derive(Clone, Debug)]
pub struct BeachParams {
    pub maximum_slope_deg: f32,
    pub width_min_m: f32,
    pub width_max_m: f32,
    pub berm_height_min_m: f32,
    pub berm_height_max_m: f32,
}

#[derive(Clone, Debug)]
pub struct CaveParams {
    pub chamber_count_min: u32,
    pub chamber_count_max: u32,
    pub passage_radius_min_m: f32,
    pub passage_radius_max_m: f32,
    pub minimum_cover_m: f32,
    pub maximum_depth_m: f32,
    pub overhang_enabled: bool,
}

impl IslandGenParams {
    /// Finest rasterized field spacing (deprecated alias for `resolution.local_m`).
    pub fn macro_spacing_m(&self) -> f32 {
        self.resolution.macro_spacing_m()
    }

    /// Ocean ring reserved between the island's support radius and the atlas
    /// edge: at least [`MIN_OCEAN_PADDING_M`], growing with local resolution.
    pub fn ocean_padding_m(&self) -> f32 {
        (self.resolution.local_m * 4.0).max(MIN_OCEAN_PADDING_M)
    }

    /// Largest `playable_diameter_m` whose full footprint support (including
    /// warp) fits inside `ocean_extent_m` with [`Self::ocean_padding_m`] of
    /// guaranteed ocean. Islands at or below this diameter make
    /// [`Self::fit_to_ocean_extent`] a no-op.
    pub fn max_fit_diameter_m(&self) -> f32 {
        let half_extent = self.ocean_extent_m * 0.5;
        let warp_padding = self.island.warp_amplitude.max(0.0);
        let available_radius = (half_extent - self.ocean_padding_m() - warp_padding).max(16.0);
        (available_radius / FOOTPRINT_SUPPORT_FACTOR) * 2.0
    }

    /// Shrink horizontal island features so the coastline fits inside the
    /// atlas with ocean padding.
    ///
    /// WARNING - lossy last resort, not an authoring tool. The rescale is
    /// non-uniform: horizontal features scale by `s`, vertical relief by
    /// `sqrt(s)` (clamped to `>= 0.25`), and surface-noise amplitudes, warp,
    /// berm heights, shelf *depths*, and cave cover/depth are not scaled at
    /// all - so a heavily over-sized island comes out steeper, lumpier, and
    /// more warped than authored (a 2200 m island crushed into a 288 m atlas
    /// gains a 3.4x slope exaggeration). Author islands at world scale and
    /// gate configs through `world_setup::validate_island_world_budget`, which
    /// reports exactly what this function would have distorted.
    pub fn fit_to_ocean_extent(&mut self) {
        // The footprint is wider than `playable_diameter_m`: lobes are offset
        // from center, each lobe has an elliptical radius, the mask falloff
        // extends past that radius, and domain warp can push the coastline
        // outward (see `footprint_support_radius_m`). Keep that full support
        // inside the atlas with a small ocean ring so terrain does not clip at
        // chunk/world edges.
        let max_diameter = self.max_fit_diameter_m();
        if self.island.playable_diameter_m <= max_diameter {
            return;
        }
        let scale = max_diameter / self.island.playable_diameter_m;
        self.island.playable_diameter_m = max_diameter;
        let vertical_scale = scale.sqrt().clamp(0.25, 1.0);
        self.island.maximum_height_m *= vertical_scale;
        self.volcano.shield_radius_m *= scale;
        self.volcano.summit_radius_m *= scale;
        self.volcano.caldera_radius_m *= scale;
        self.volcano.shield_height_m *= vertical_scale;
        self.volcano.summit_height_m *= vertical_scale;
        self.volcano.caldera_depth_m *= vertical_scale;
        self.volcano.caldera_rim_height_m *= vertical_scale;
        self.volcano.collapse_depth_m *= vertical_scale;
        self.coast.shelf_width_min_m *= scale;
        self.coast.shelf_width_max_m *= scale;
        self.beaches.width_min_m *= scale;
        self.beaches.width_max_m *= scale;
        self.hydrology.permanent_river_threshold *= scale * scale;
        self.hydrology.stream_threshold *= scale * scale;
        self.hydrology.minimum_stream_length_m *= scale;
        if scale < 1.0 {
            self.hydrology.permanent_river_threshold =
                self.hydrology.permanent_river_threshold.min(40.0);
            self.hydrology.stream_threshold = self.hydrology.stream_threshold.min(12.0);
            self.hydrology.minimum_stream_length_m =
                self.hydrology.minimum_stream_length_m.max(12.0);
        }
    }
}

impl Default for IslandGenParams {
    /// Self-consistent defaults at the default `ocean_extent_m` (288 m atlas,
    /// 256 m chunk world, sea level 2.0). Mirrors
    /// `assets/terrain/generation/island_testbed.yaml`; keep the two in sync
    /// when retuning.
    ///
    /// The previous defaults described a 2200 m / 360 m island inside the same
    /// 288 m atlas - a configuration `validate_island_world_budget` rejects -
    /// so every consumer of `Default` that skipped `fit_to_ocean_extent` was
    /// silently exercising a clipped, coastless island.
    fn default() -> Self {
        Self {
            seed: 48_129,
            center: [0.0, 0.0],
            ocean_extent_m: 288.0,
            resolution: GenerationResolution::default(),
            island: IslandShapeParams {
                // Support radius: 90 * 1.1775 + 6 warp = 112 m; fits the
                // 128 m chunk half-extent and the 144 m atlas half-extent
                // with padding (max_fit_diameter ~= 207 m).
                playable_diameter_m: 180.0,
                // Composed relief is 48 m (shield 30 + summit 16 + rim 2);
                // +5.2 m of noise stays under a +80 m chunk ceiling.
                maximum_height_m: 50.0,
                sea_level_m: 2.0,
                lobe_count: 3,
                // 50 m warp period -> 3-4 coastline undulations across the island.
                warp_frequency: 0.02,
                warp_amplitude: 6.0,
            },
            volcano: VolcanoParams {
                center: [0.0, 0.0],
                // 30 m over 78 m, exponent 1.3: gentle near center, ~27 deg at rim.
                shield_radius_m: 78.0,
                shield_exponent: 1.3,
                shield_height_m: 30.0,
                summit_radius_m: 30.0,
                summit_exponent: 2.2,
                summit_height_m: 16.0,
                caldera_radius_m: 10.0,
                caldera_depth_m: 7.0,
                caldera_rim_height_m: 2.0,
                radial_ridge_count: 7,
                collapse_direction_deg: 215.0,
                collapse_depth_m: 6.0,
            },
            surface_noise: SurfaceNoiseParams {
                // ~8% of island relief; keep regional + local within the
                // ceiling margin left by maximum_height_m.
                regional_amplitude_m: 4.0,
                local_amplitude_m: 1.2,
                voxel_amplitude_m: 0.3,
            },
            hydrology: HydrologyParams {
                rainfall_base: 1.0,
                // Units: accumulation cells on the regional grid
                // (288 / 8 = 36x36, ~400 land cells).
                stream_threshold: 12.0,
                permanent_river_threshold: 40.0,
                minimum_stream_length_m: 20.0,
            },
            erosion: ErosionParams {
                stream_power_iterations: 22,
                m: 0.48,
                n: 1.0,
                // 22 iterations x 0.15 m cap = <= 3.3 m carving, ~7% of relief.
                maximum_step_m: 0.15,
                stream_power_erodibility: 0.00002,
                thermal_iterations: 12,
                thermal_transfer_rate: 0.16,
                thermal_talus_deg: 38.0,
                river_bank_width_m: 3.5,
                river_carve_strength: 1.2,
            },
            coast: CoastParams {
                // Coastline sits ~78-95 m out; chunk edge at 128 m leaves a
                // 30-50 m ring for shelf + deep falloff.
                shelf_width_min_m: 15.0,
                shelf_width_max_m: 45.0,
                shelf_depth_min_m: 6.0,
                shelf_depth_max_m: 16.0,
                deep_slope_min: 0.18,
                deep_slope_max: 0.5,
            },
            beaches: BeachParams {
                maximum_slope_deg: 13.0,
                width_min_m: 5.0,
                width_max_m: 14.0,
                berm_height_min_m: 0.4,
                berm_height_max_m: 1.0,
            },
            caves: CaveParams {
                chamber_count_min: 4,
                chamber_count_max: 5,
                passage_radius_min_m: 2.2,
                passage_radius_max_m: 3.6,
                minimum_cover_m: 3.0,
                // Relief is ~48 m authored; baked peak ~37 m after erosion.
                maximum_depth_m: 20.0,
                overhang_enabled: true,
            },
        }
    }
}