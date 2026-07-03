// crates/game_bevy/src/ui/tweaks.rs
//! Runtime tweak resources mutated by the options panel.

use bevy::prelude::*;

/// Live atmosphere overrides applied on top of YAML lighting config.
#[derive(Resource, Clone, Debug)]
pub struct LightingTweaks {
    pub fog_color: [f32; 3],
    pub fog_start_m: f32,
    pub fog_end_m: f32,
    pub override_fog: bool,
}

impl Default for LightingTweaks {
    fn default() -> Self {
        Self {
            fog_color: [0.62, 0.74, 0.86],
            fog_start_m: 40.0,
            fog_end_m: 220.0,
            override_fog: true,
        }
    }
}

/// Movement tuning overrides (Phase 1+).
#[derive(Resource, Clone, Debug)]
pub struct MovementTweaks {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub acceleration: f32,
    pub deceleration: f32,
    pub jump_buffer_s: f32,
    pub coyote_time_s: f32,
    pub max_slope_deg: f32,
    pub use_overrides: bool,
}

impl Default for MovementTweaks {
    fn default() -> Self {
        Self {
            walk_speed: 4.8,
            run_speed: 7.5,
            acceleration: 26.0,
            deceleration: 32.0,
            jump_buffer_s: 0.12,
            coyote_time_s: 0.1,
            max_slope_deg: 47.0,
            use_overrides: false,
        }
    }
}

/// Physics tuning overrides (Phase 2+).
#[derive(Resource, Clone, Debug)]
pub struct PhysicsTweaks {
    pub gravity: f32,
    pub prop_friction: f32,
    pub platform_speed: f32,
    pub use_overrides: bool,
}

impl Default for PhysicsTweaks {
    fn default() -> Self {
        Self {
            gravity: 18.0,
            prop_friction: 0.6,
            platform_speed: 2.5,
            use_overrides: false,
        }
    }
}

/// World / residency overrides (Phase 3+).
///
/// The legacy `use_expanded_profile` flag (op-based expanded_slice worlds)
/// was removed with the two-world condensation; world selection lives in the
/// Setup screen via `UserSetupPrefs.world_id`.
#[derive(Resource, Clone, Debug)]
pub struct WorldTweaks {
    pub density_radius: i32,
    pub render_radius: i32,
    pub physics_radius: i32,
    pub decoration_radius: i32,
    pub high_detail_radius: i32,
    pub show_residency_rings: bool,
}

impl Default for WorldTweaks {
    fn default() -> Self {
        Self {
            density_radius: 10,
            render_radius: 7,
            physics_radius: 5,
            decoration_radius: 5,
            high_detail_radius: 4,
            show_residency_rings: false,
        }
    }
}

/// Terrain field stack overrides (Phase 4+).
#[derive(Resource, Clone, Debug)]
pub struct TerrainTweaks {
    pub ridge_amplitude: f32,
    pub valley_depth: f32,
    pub coast_blend: f32,
    pub show_masks: bool,
    pub use_overrides: bool,
}

impl TerrainTweaks {
    pub fn field_stack_params(&self) -> terrain_generation::FieldStackParams {
        if self.use_overrides {
            terrain_generation::FieldStackParams {
                ridge_amplitude: self.ridge_amplitude,
                valley_depth: self.valley_depth,
                coast_blend: self.coast_blend,
            }
        } else {
            terrain_generation::FieldStackParams::default()
        }
    }
}

impl Default for TerrainTweaks {
    fn default() -> Self {
        Self {
            ridge_amplitude: 1.0,
            valley_depth: 1.0,
            coast_blend: 1.0,
            show_masks: false,
            use_overrides: false,
        }
    }
}

/// Water body overrides (Phase 5+).
///
/// `pool_elevation_m` was removed with the legacy demo hydrology (the
/// unbounded upland pool at 31.5 m); atlas worlds have sea + generated river.
#[derive(Resource, Clone, Debug)]
pub struct WaterTweaks {
    pub sea_level_m: f32,
    pub shallow_color: [f32; 3],
    pub deep_color: [f32; 3],
    pub use_overrides: bool,
}

impl Default for WaterTweaks {
    fn default() -> Self {
        Self {
            sea_level_m: 0.0,
            shallow_color: [0.2, 0.55, 0.65],
            deep_color: [0.05, 0.2, 0.35],
            use_overrides: false,
        }
    }
}

/// River debug visualization (Phase 6+).
///
/// The river itself comes from the island atlas; the legacy
/// source-radius / mouth-width generation inputs were removed with
/// `demo_river.yaml`. Tune rivers via island_gen hydrology parameters.
#[derive(Resource, Clone, Debug)]
pub struct RiverTweaks {
    pub show_spline: bool,
    pub show_flow_arrows: bool,
}

impl Default for RiverTweaks {
    fn default() -> Self {
        Self {
            show_spline: false,
            show_flow_arrows: false,
        }
    }
}

/// Map clock hours (0–24) to sun azimuth/elevation for lighting tests.
pub fn sun_angles_from_time_of_day(hours: f32) -> (f32, f32) {
    use std::f32::consts::PI;
    let hour = hours.rem_euclid(24.0);
    // Solar noon at 12:00, sunrise/sunset near 6:00 / 18:00.
    let day_phase = ((hour - 6.0) / 12.0).clamp(0.0, 1.0) * PI;
    let elevation = day_phase.sin() * 62.0;
    let azimuth = 55.0 + hour * 15.0;
    (azimuth.rem_euclid(360.0), elevation)
}

/// Directional lux from sun elevation (matches perceived day/night).
pub fn sun_illuminance_for_elevation(elevation_deg: f32) -> f32 {
    let day = ((elevation_deg + 8.0) / 70.0).clamp(0.0, 1.0);
    400.0 + day.powf(1.35) * 99_600.0
}

/// Ambient fill that dims with the sun.
pub fn ambient_brightness_for_elevation(elevation_deg: f32, base: f32) -> f32 {
    let day = ((elevation_deg + 6.0) / 60.0).clamp(0.0, 1.0);
    base * (0.12 + day * 0.88)
}

/// Atmosphere / sky overrides (Phase 7–9).
#[derive(Resource, Clone, Debug)]
pub struct AtmosphereTweaks {
    /// When true, sun angles are driven from [`Self::time_of_day_hours`].
    pub drive_sun_from_time_of_day: bool,
    pub time_of_day_hours: f32,
    pub sun_azimuth_deg: f32,
    pub sun_elevation_deg: f32,
    pub exposure_min: f32,
    pub exposure_max: f32,
    pub zenith_color: [f32; 3],
    pub horizon_color: [f32; 3],
    pub mie_strength: f32,
    pub height_fog_density: f32,
    pub underwater_fog_density: f32,
    pub use_overrides: bool,
}

impl Default for AtmosphereTweaks {
    fn default() -> Self {
        let (azimuth, elevation) = sun_angles_from_time_of_day(10.0);
        Self {
            drive_sun_from_time_of_day: false,
            time_of_day_hours: 10.0,
            sun_azimuth_deg: azimuth,
            sun_elevation_deg: elevation,
            exposure_min: 0.4,
            exposure_max: 1.6,
            zenith_color: [0.25, 0.45, 0.75],
            horizon_color: [0.62, 0.74, 0.86],
            mie_strength: 0.5,
            height_fog_density: 0.02,
            underwater_fog_density: 0.15,
            use_overrides: false,
        }
    }
}

/// Camera overrides (Phase 10+).
#[derive(Resource, Clone, Debug)]
pub struct CameraTweaks {
    /// Collision-less free camera for world inspection.
    pub fly_cam: bool,
    pub fly_cam_speed_mps: f32,
    pub orbit_distance: f32,
    pub collision_inward_sharpness: f32,
    pub collision_outward_sharpness: f32,
    pub interior_distance_scale: f32,
    pub use_overrides: bool,
}

impl Default for CameraTweaks {
    fn default() -> Self {
        Self {
            fly_cam: false,
            fly_cam_speed_mps: 28.0,
            orbit_distance: 8.0,
            collision_inward_sharpness: 18.0,
            collision_outward_sharpness: 6.0,
            interior_distance_scale: 0.75,
            use_overrides: false,
        }
    }
}

/// Water physics overrides (Phase 11+).
#[derive(Resource, Clone, Debug)]
pub struct WaterPhysicsTweaks {
    pub buoyancy_strength: f32,
    pub flow_multiplier: f32,
    pub shallow_depth_m: f32,
    pub shallow_speed_scale: f32,
}

impl Default for WaterPhysicsTweaks {
    fn default() -> Self {
        Self {
            buoyancy_strength: 1.0,
            flow_multiplier: 1.0,
            shallow_depth_m: 1.5,
            shallow_speed_scale: 0.7,
        }
    }
}

/// Vegetation / biome overrides (Phase 12).
#[derive(Resource, Clone, Debug)]
pub struct EcologyTweaks {
    pub vegetation_density: f32,
    pub show_wetness_heatmap: bool,
    pub biome_debug_mode: u32,
}

impl Default for EcologyTweaks {
    fn default() -> Self {
        Self {
            vegetation_density: 1.0,
            show_wetness_heatmap: false,
            biome_debug_mode: 0,
        }
    }
}