//! Island generation parameters (runtime, independent of Bevy).

use crate::resolution::GenerationResolution;

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
    pub thermal_iterations: u32,
    pub thermal_transfer_rate: f32,
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

    /// Shrink horizontal island features so the coastline fits inside the atlas with ocean padding.
    pub fn fit_to_ocean_extent(&mut self) {
        // The footprint is wider than `playable_diameter_m`: lobes are offset from center,
        // each lobe has an elliptical radius, the mask falloff extends past that radius,
        // and domain warp can push the coastline outward. Keep that full support inside
        // the atlas with a small ocean ring so terrain does not clip at chunk/world edges.
        let half_extent = self.ocean_extent_m * 0.5;
        let ocean_padding = (self.resolution.local_m * 4.0).max(16.0);
        let warp_padding = self.island.warp_amplitude.max(0.0);
        let available_radius = (half_extent - ocean_padding - warp_padding).max(16.0);
        let support_radius_factor = 0.18 + 0.95 * 1.05;
        let max_diameter = (available_radius / support_radius_factor) * 2.0;
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
    fn default() -> Self {
        Self {
            seed: 48_129,
            center: [0.0, 0.0],
            ocean_extent_m: 288.0,
            resolution: GenerationResolution::default(),
            island: IslandShapeParams {
                playable_diameter_m: 2200.0,
                maximum_height_m: 360.0,
                sea_level_m: 0.0,
                lobe_count: 3,
                warp_frequency: 0.004,
                warp_amplitude: 18.0,
            },
            volcano: VolcanoParams {
                center: [0.0, 0.0],
                shield_radius_m: 950.0,
                shield_exponent: 1.3,
                shield_height_m: 230.0,
                summit_radius_m: 360.0,
                summit_exponent: 2.4,
                summit_height_m: 135.0,
                caldera_radius_m: 90.0,
                caldera_depth_m: 38.0,
                caldera_rim_height_m: 12.0,
                radial_ridge_count: 7,
                collapse_direction_deg: 215.0,
                collapse_depth_m: 45.0,
            },
            surface_noise: SurfaceNoiseParams {
                regional_amplitude_m: 14.0,
                local_amplitude_m: 3.5,
                voxel_amplitude_m: 0.35,
            },
            hydrology: HydrologyParams {
                rainfall_base: 1.0,
                stream_threshold: 220.0,
                permanent_river_threshold: 900.0,
                minimum_stream_length_m: 60.0,
            },
            erosion: ErosionParams {
                stream_power_iterations: 22,
                m: 0.48,
                n: 1.0,
                maximum_step_m: 0.2,
                thermal_iterations: 12,
                thermal_transfer_rate: 0.16,
            },
            coast: CoastParams {
                shelf_width_min_m: 60.0,
                shelf_width_max_m: 300.0,
                shelf_depth_min_m: 8.0,
                shelf_depth_max_m: 28.0,
                deep_slope_min: 0.18,
                deep_slope_max: 0.65,
            },
            beaches: BeachParams {
                maximum_slope_deg: 13.0,
                width_min_m: 8.0,
                width_max_m: 35.0,
                berm_height_min_m: 0.6,
                berm_height_max_m: 1.8,
            },
            caves: CaveParams {
                chamber_count_min: 4,
                chamber_count_max: 7,
                passage_radius_min_m: 1.4,
                passage_radius_max_m: 3.5,
                minimum_cover_m: 4.0,
                maximum_depth_m: 45.0,
                overhang_enabled: true,
            },
        }
    }
}
