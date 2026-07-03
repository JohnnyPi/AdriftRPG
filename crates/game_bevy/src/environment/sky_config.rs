//! YAML-driven sky presentation config (clouds/stars flags, horizon reference colors).

use bevy::prelude::*;

/// Bumped when sky profile or world presentation changes; cloud/god-ray systems respawn.
#[derive(Resource, Default, Clone, Debug)]
pub struct SkyEffectsRevision(pub u32);

pub fn bump_sky_effects_revision(revision: &mut SkyEffectsRevision) {
    revision.0 = revision.0.wrapping_add(1);
}

/// Presentation data loaded from sky YAML profiles. The Bevy procedural atmosphere
/// renders the sky; this resource retains per-world tuning hooks and fog color references.
#[derive(Resource, Clone, Debug)]
pub struct SkyPresentationConfig {
    pub zenith_color: [f32; 3],
    pub horizon_color: [f32; 3],
    pub night_zenith_color: [f32; 3],
    pub night_horizon_color: [f32; 3],
    pub clouds_enabled: bool,
    pub clouds_opacity: f32,
    pub clouds_speed: f32,
    pub clouds_direction_deg: f32,
    pub clouds_altitude: f32,
    pub stars_enabled: bool,
    pub stars_density: f32,
    pub cloud_base_height_m: f32,
    pub cloud_shell_radius_m: f32,
}

impl Default for SkyPresentationConfig {
    fn default() -> Self {
        Self {
            zenith_color: [0.25, 0.45, 0.75],
            horizon_color: [0.62, 0.74, 0.86],
            night_zenith_color: [0.02, 0.04, 0.14],
            night_horizon_color: [0.06, 0.08, 0.16],
            clouds_enabled: false,
            clouds_opacity: 0.35,
            clouds_speed: 0.015,
            clouds_direction_deg: 45.0,
            clouds_altitude: 0.22,
            stars_enabled: false,
            stars_density: 0.55,
            cloud_base_height_m: 500.0,
            cloud_shell_radius_m: 2800.0,
        }
    }
}

/// Night gradient mix from sun elevation (VS2 §13 Stage 3).
pub fn night_mix_from_elevation(sun_elevation_deg: f32) -> f32 {
    (1.0 - (sun_elevation_deg + 8.0) / 18.0).clamp(0.0, 1.0)
}

pub fn apply_sky_profile(
    config: &mut SkyPresentationConfig,
    atmosphere: &mut crate::ui::AtmosphereTweaks,
    sky: &game_data::CompiledSky,
) {
    config.zenith_color = sky.zenith_color;
    config.horizon_color = sky.horizon_color;
    config.night_zenith_color = sky.night_zenith_color;
    config.night_horizon_color = sky.night_horizon_color;
    config.clouds_enabled = sky.clouds_enabled;
    config.clouds_opacity = sky.clouds_opacity;
    config.clouds_speed = sky.clouds_speed;
    config.clouds_direction_deg = sky.clouds_direction_deg;
    config.clouds_altitude = sky.clouds_altitude;
    config.stars_enabled = sky.stars_enabled;
    config.stars_density = sky.stars_density;
    config.cloud_base_height_m = sky.cloud_base_height_m;
    config.cloud_shell_radius_m = sky.cloud_shell_radius_m;

    atmosphere.zenith_color = sky.zenith_color;
    atmosphere.horizon_color = sky.horizon_color;
    atmosphere.mie_strength = sky.mie_strength;
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_data::CompiledSky;
    use shared::StableId;

    #[test]
    fn night_mix_increases_below_horizon() {
        assert!(night_mix_from_elevation(-10.0) > night_mix_from_elevation(30.0));
    }

    #[test]
    fn apply_sky_profile_copies_cloud_fields() {
        let sky = CompiledSky {
            id: StableId::new("sky.test"),
            zenith_color: [0.1, 0.2, 0.3],
            horizon_color: [0.4, 0.5, 0.6],
            mie_strength: 0.5,
            sun_disc_radius: 0.02,
            stars_enabled: true,
            stars_density: 0.5,
            clouds_enabled: true,
            clouds_opacity: 0.72,
            clouds_speed: 0.03,
            clouds_direction_deg: 55.0,
            clouds_altitude: 0.32,
            cloud_base_height_m: 500.0,
            cloud_shell_radius_m: 2800.0,
            night_zenith_color: [0.0, 0.0, 0.1],
            night_horizon_color: [0.1, 0.1, 0.2],
        };
        let mut config = SkyPresentationConfig::default();
        let mut atmosphere = crate::ui::AtmosphereTweaks::default();
        apply_sky_profile(&mut config, &mut atmosphere, &sky);
        assert!(config.clouds_enabled);
        assert_eq!(config.clouds_opacity, 0.72);
        assert_eq!(config.clouds_speed, 0.03);
        assert_eq!(config.clouds_direction_deg, 55.0);
        assert_eq!(config.clouds_altitude, 0.32);
    }
}
