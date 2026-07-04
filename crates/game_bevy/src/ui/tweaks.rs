// crates/game_bevy/src/ui/tweaks.rs
//! Runtime tweak resources mutated by the options panel.
//!
//! # Authored defaults vs live overrides
//!
//! YAML compiles into `game_data::Compiled*` structs at load time. Each `*Tweaks`
//! resource here holds **live** values the options panel and debug tools mutate.
//! On world enter, `environment::config_init` seeds compiled profiles into
//! `EnvironmentLightingState` (sun/exposure) and `AtmosphereTweaks` (sky/fog colors).

use bevy::prelude::*;
use game_data::{CompiledChunkResidency, CompiledFog, CompiledWater};

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
            fog_color: [0.58, 0.68, 0.76],
            fog_start_m: 60.0,
            fog_end_m: 520.0,
            override_fog: false,
        }
    }
}

impl LightingTweaks {
    /// Seed distance-fog UI defaults from compiled `fog.yaml`.
    pub fn apply_authored_defaults(&mut self, fog: &CompiledFog) {
        self.fog_color = fog.distance_color;
        self.fog_start_m = fog.distance_start_m;
        self.fog_end_m = fog.distance_end_m;
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

impl MovementTweaks {
    /// Seed movement fields from compiled `player.yaml`.
    pub fn apply_authored_player(&mut self, player: &game_data::CompiledPlayer) {
        self.walk_speed = player.walk_speed_mps;
        self.run_speed = player.run_speed_mps;
        self.acceleration = player.acceleration_mps2;
        self.deceleration = player.deceleration_mps2;
        self.jump_buffer_s = player.jump_buffer_s;
        self.coyote_time_s = player.coyote_time_s;
        self.max_slope_deg = player.maximum_walkable_slope_deg;
    }
}

/// Physics tuning overrides (Phase 2+).
#[derive(Resource, Clone, Debug)]
pub struct PhysicsTweaks {
    pub gravity: f32,
    pub prop_friction: f32,
    pub use_overrides: bool,
}

impl PhysicsTweaks {
    /// Seed gravity from compiled `physics.yaml`.
    pub fn apply_authored_physics(&mut self, physics: &game_data::CompiledPhysics) {
        self.gravity = physics.gravity_mps2;
    }
}

impl Default for PhysicsTweaks {
    fn default() -> Self {
        Self {
            gravity: 18.0,
            prop_friction: 0.6,
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
    pub show_semantic_landmarks: bool,
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
            show_semantic_landmarks: false,
        }
    }
}

impl WorldTweaks {
    /// Seed chunk residency radii from the active world's compiled profile.
    pub fn apply_authored_residency(&mut self, residency: &CompiledChunkResidency) {
        self.density_radius = residency.density_radius;
        self.render_radius = residency.render_radius;
        self.physics_radius = residency.physics_radius;
        self.decoration_radius = residency.decoration_radius;
        self.high_detail_radius = residency.high_detail_radius;
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

/// Terrain material shader and overlay overrides.
#[derive(Resource, Clone, Debug)]
pub struct TerrainMaterialTweaks {
    pub global_wetness: f32,
    pub global_moss: f32,
    pub macro_variation_strength: f32,
    pub debug_mode: u32,
    pub use_overrides: bool,
}

impl Default for TerrainMaterialTweaks {
    fn default() -> Self {
        Self {
            global_wetness: 0.0,
            global_moss: 0.0,
            macro_variation_strength: 0.10,
            debug_mode: 0,
            use_overrides: false,
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
#[derive(Resource, Clone, Debug)]
pub struct WaterTweaks {
    pub sea_level_m: f32,
    pub shallow_color: [f32; 3],
    pub deep_color: [f32; 3],
    pub use_overrides: bool,
}

impl WaterTweaks {
    /// Seed sea level and colors from compiled `water.yaml`.
    pub fn apply_authored_water(&mut self, water: &CompiledWater) {
        self.sea_level_m = water.sea_level_m;
        self.shallow_color = water.shallow_color;
        self.deep_color = water.deep_color;
    }
}

impl Default for WaterTweaks {
    fn default() -> Self {
        Self {
            sea_level_m: 2.0,
            shallow_color: [0.18, 0.62, 0.58],
            deep_color: [0.03, 0.18, 0.32],
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
    use std::f32::consts::TAU;
    let hour = hours.rem_euclid(24.0);
    let phase = (hour - 6.0) / 24.0 * TAU;
    let elevation = phase.sin() * 62.0;
    let azimuth = 55.0 + hour * 15.0;
    (azimuth.rem_euclid(360.0), elevation)
}

/// Directional lux from sun elevation (matches perceived day/night).
pub fn sun_illuminance_for_elevation(elevation_deg: f32) -> f32 {
    if elevation_deg <= -6.0 {
        return 0.0;
    }
    let day = ((elevation_deg + 6.0) / 70.0).clamp(0.0, 1.0);
    day.powf(0.65) * 100_000.0
}

/// Peak sun scale applied on top of the elevation curve (keeps HDR headroom).
pub const SUN_PEAK_SCALE: f32 = 0.8;

/// Atmosphere environment-map intensity from sun elevation (night floor for readability).
pub fn environment_intensity_for_elevation(elevation_deg: f32, scale: f32) -> f32 {
    let day = ((elevation_deg + 6.0) / 66.0).clamp(0.0, 1.0);
    const NIGHT_FLOOR: f32 = 0.15;
    const DAY_PEAK: f32 = 0.9;
    (NIGHT_FLOOR + day.powf(0.7) * (DAY_PEAK - NIGHT_FLOOR)) * scale
}

/// Gameplay moon lux from celestial state (readable moonlit nights).
pub fn moon_gameplay_illuminance(
    sun_elevation_deg: f32,
    moon_elevation_deg: f32,
    moon_phase: f32,
    moon_lux_max: f32,
    cloud_cover: f32,
) -> f32 {
    if sun_elevation_deg >= 6.0 {
        return 0.0;
    }
    let night = sun_elevation_deg < -2.0;
    let twilight = !night;
    let elevation = (moon_elevation_deg / 45.0).clamp(0.0, 1.0);
    let phase = moon_phase.clamp(0.0, 1.0).powf(0.8);
    let clouds = 1.0 - cloud_cover * 0.85;
    let strength = elevation * phase * clouds * moon_lux_max;
    if night {
        strength
    } else if twilight {
        strength * 0.25
    } else {
        0.0
    }
}

/// Ambient fill that dims with the sun.
pub fn ambient_brightness_for_elevation(elevation_deg: f32, base: f32) -> f32 {
    let day = ((elevation_deg + 6.0) / 60.0).clamp(0.0, 1.0);
    base * (0.12 + day * 0.88)
}

/// Warm sun color near the horizon, neutral at midday.
pub fn sun_color_for_elevation(elevation_deg: f32) -> [f32; 3] {
    let t = (1.0 - (elevation_deg / 12.0).clamp(0.0, 1.0)).powf(1.4);
    [
        1.0 * (1.0 - t) + t * 1.0,
        0.97 * (1.0 - t) + t * 0.72,
        0.92 * (1.0 - t) + t * 0.45,
    ]
}

/// Camera EV100 from sun elevation; tuned for readable nights and clear midday.
pub fn exposure_ev_for_elevation(elevation_deg: f32, min_ev: f32, max_ev: f32, bias: f32) -> f32 {
    let day = ((elevation_deg + 6.0) / 66.0).clamp(0.0, 1.0);
    let ev = min_ev + day.powf(0.85) * (max_ev - min_ev);
    (ev + bias).clamp(min_ev, max_ev)
}

/// Sky color and fog-density presentation overrides (Phase 7–9).
#[derive(Resource, Clone, Debug)]
pub struct AtmosphereTweaks {
    pub zenith_color: [f32; 3],
    pub horizon_color: [f32; 3],
    pub mie_strength: f32,
    pub height_fog_density: f32,
    pub underwater_fog_density: f32,
}

impl Default for AtmosphereTweaks {
    fn default() -> Self {
        Self {
            zenith_color: [0.22, 0.42, 0.78],
            horizon_color: [0.58, 0.72, 0.88],
            mie_strength: 0.55,
            height_fog_density: 0.02,
            underwater_fog_density: 0.15,
        }
    }
}

impl AtmosphereTweaks {
    /// Seed height/underwater fog densities from compiled `fog.yaml`.
    pub fn apply_authored_fog_densities(&mut self, fog: &CompiledFog) {
        self.height_fog_density = fog.height_density;
        self.underwater_fog_density = fog.underwater_density;
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

impl CameraTweaks {
    /// Seed orbit and collision fields from compiled `camera.yaml`.
    pub fn apply_authored_camera(&mut self, camera: &game_data::CompiledCamera) {
        self.orbit_distance = camera.distance_default_m;
        self.collision_inward_sharpness = camera.collision_inward_sharpness;
        self.collision_outward_sharpness = camera.collision_outward_sharpness;
    }
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
    pub swim_up_speed_mps: f32,
    pub submerged_sink_cap_mps: f32,
    /// When true, buoyancy only applies in the shallow wading band near the surface.
    pub buoyancy_surface_only: bool,
}

impl WaterPhysicsTweaks {
    pub const DEFAULT_SHALLOW_DEPTH_M: f32 = 1.5;
}

impl Default for WaterPhysicsTweaks {
    fn default() -> Self {
        Self {
            buoyancy_strength: 0.35,
            flow_multiplier: 1.0,
            shallow_depth_m: Self::DEFAULT_SHALLOW_DEPTH_M,
            shallow_speed_scale: 0.7,
            swim_up_speed_mps: 3.2,
            submerged_sink_cap_mps: 2.5,
            buoyancy_surface_only: true,
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

#[cfg(test)]
mod lighting_curve_tests {
    use super::*;

    #[test]
    fn noon_exposure_exceeds_night_in_valid_ev_range() {
        let noon = exposure_ev_for_elevation(62.0, 9.0, 15.0, 0.0);
        let night = exposure_ev_for_elevation(-10.0, 9.0, 15.0, 0.0);
        assert!(noon > night);
        assert!((8.0..=16.0).contains(&noon));
        assert!((8.0..=16.0).contains(&night));
    }

    #[test]
    fn night_exposure_is_readable() {
        let night = exposure_ev_for_elevation(-10.0, 9.0, 15.0, 0.0);
        assert!(night >= 9.0);
    }

    #[test]
    fn exposure_not_stuck_at_legacy_multiplier() {
        let noon = exposure_ev_for_elevation(62.0, 9.0, 15.0, 0.0);
        let night = exposure_ev_for_elevation(-10.0, 9.0, 15.0, 0.0);
        assert!((noon - 1.6).abs() > 0.5);
        assert!((night - 1.6).abs() > 0.5);
    }

    #[test]
    fn environment_intensity_drops_at_night() {
        let day = environment_intensity_for_elevation(60.0, 1.0);
        let night = environment_intensity_for_elevation(-10.0, 1.0);
        assert!(day > night);
        assert!(night >= 0.12);
    }

    #[test]
    fn moon_visible_at_night() {
        let lux = moon_gameplay_illuminance(-10.0, 35.0, 1.0, 2.0, 0.0);
        assert!(lux > 0.5);
    }

    #[test]
    fn day_night_cycle_key_hours_have_sane_lighting() {
        let hours = [0.0, 6.0, 12.0, 18.0, 24.0];
        let mut noon_ev = 0.0f32;
        let mut midnight_ev = f32::MAX;
        for hour in hours {
            let (_, elevation) = sun_angles_from_time_of_day(hour);
            let ev = exposure_ev_for_elevation(elevation, 9.0, 15.0, 0.0);
            let env = environment_intensity_for_elevation(elevation, 1.0);
            assert!((8.0..=16.0).contains(&ev), "hour {hour} ev {ev}");
            assert!((0.1..=1.0).contains(&env), "hour {hour} env {env}");
            noon_ev = noon_ev.max(ev);
            midnight_ev = midnight_ev.min(ev);
        }
        assert!(noon_ev > midnight_ev + 3.0);
        let (_, midnight_elev) = sun_angles_from_time_of_day(0.0);
        let moon_lux = moon_gameplay_illuminance(midnight_elev, 35.0, 1.0, 2.0, 0.0);
        assert!(moon_lux > 0.3, "readable moonlit midnight");
    }
}
