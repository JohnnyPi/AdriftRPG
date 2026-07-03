// crates/game_bevy/src/environment/sky.rs
//! Atmospheric sky rendering (VS2 §13 Stages 2–3).

use bevy::prelude::*;
use bevy::pbr::Material;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

use super::lighting_state::{sun_direction_from_angles, EnvironmentLightingState};
use crate::data::ConfigRegistryResource;
use crate::state::AppState;
use crate::ui::AtmosphereTweaks;

/// VS2 §13.3 celestial body parameters for sun/moon rendering.
#[derive(Clone, Copy, Debug)]
pub struct CelestialBodyState {
    pub direction: Vec3,
    pub angular_radius: f32,
    pub brightness: f32,
    pub phase: f32,
}

#[derive(ShaderType, Clone, Copy, Debug)]
pub struct SkyParams {
    pub zenith: Vec4,
    pub horizon: Vec4,
    pub sun_dir: Vec4,
    /// `.x` = mie strength, `.y` = sun elevation (deg), `.z` = star opacity, `.w` = sun disc radius
    pub mie: Vec4,
    /// moon direction xyz, angular radius in `.w`
    pub moon: Vec4,
    /// `.x` = moon brightness, `.y` = moon phase, `.z` = cloud opacity, `.w` = night mix
    pub celestial: Vec4,
    /// `.x/.y` = cloud scroll, `.z` = cloud speed factor, `.w` = cloud altitude
    pub clouds: Vec4,
    pub night_zenith: Vec4,
    pub night_horizon: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SkyMaterial {
    #[uniform(0)]
    pub params: SkyParams,
}

impl Material for SkyMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/sky.wgsl".into()
    }

    fn vertex_shader() -> ShaderRef {
        "shaders/sky.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }
}

#[derive(Component)]
pub struct AtmosphericSky;

#[derive(Resource, Clone, Debug)]
pub struct SkyState {
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
    pub moon: CelestialBodyState,
}

impl Default for SkyState {
    fn default() -> Self {
        Self {
            zenith_color: [0.25, 0.45, 0.75],
            horizon_color: [0.62, 0.74, 0.86],
            mie_strength: 0.5,
            sun_disc_radius: 0.02,
            stars_enabled: false,
            stars_density: 0.55,
            clouds_enabled: false,
            clouds_opacity: 0.35,
            clouds_speed: 0.015,
            clouds_direction_deg: 45.0,
            clouds_altitude: 0.22,
            night_zenith_color: [0.02, 0.04, 0.14],
            night_horizon_color: [0.06, 0.08, 0.16],
            moon: CelestialBodyState {
                direction: Vec3::new(0.5, 0.6, 0.2).normalize_or_zero(),
                angular_radius: 0.008,
                brightness: 0.0,
                phase: 1.0,
            },
        }
    }
}

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SkyMaterial>::default())
            .init_resource::<SkyState>()
            .add_systems(OnEnter(AppState::Running), spawn_atmospheric_sky)
            .add_systems(Update, update_atmospheric_sky.run_if(in_state(AppState::Running)));
    }
}

fn spawn_atmospheric_sky(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    sky_state: Res<SkyState>,
    lighting: Res<EnvironmentLightingState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SkyMaterial>>,
) {
    let sun_dir = sun_direction_from_angles(lighting.sun_azimuth_deg, lighting.sun_elevation_deg);
    let mat = materials.add(SkyMaterial {
        params: sky_params_from_state(&sky_state, lighting.sun_elevation_deg, sun_dir, 0.0),
    });
    commands.spawn((
        AtmosphericSky,
        Mesh3d(meshes.add(Sphere::new(1.0))),
        MeshMaterial3d(mat),
        // Inverted sphere: camera sits inside; render the inner surface behind geometry.
        Transform::from_scale(Vec3::splat(-450.0)),
    ));
    let _ = registry;
}

pub(crate) fn sky_params_from_state(
    state: &SkyState,
    sun_elevation_deg: f32,
    sun_dir: Vec3,
    elapsed_secs: f32,
) -> SkyParams {
    let night_mix = night_mix_from_elevation(sun_elevation_deg);
    let star_opacity = if state.stars_enabled {
        state.stars_density
    } else {
        0.0
    };
    let cloud_opacity = if state.clouds_enabled {
        state.clouds_opacity
    } else {
        0.0
    };
    let dir_rad = state.clouds_direction_deg.to_radians();
    let scroll = Vec2::new(dir_rad.cos(), dir_rad.sin()) * state.clouds_speed * elapsed_secs;

    SkyParams {
        zenith: vec4_rgb(state.zenith_color),
        horizon: vec4_rgb(state.horizon_color),
        sun_dir: Vec4::new(sun_dir.x, sun_dir.y, sun_dir.z, 0.0),
        mie: Vec4::new(
            state.mie_strength,
            sun_elevation_deg,
            star_opacity,
            state.sun_disc_radius,
        ),
        moon: Vec4::new(
            state.moon.direction.x,
            state.moon.direction.y,
            state.moon.direction.z,
            state.moon.angular_radius,
        ),
        celestial: Vec4::new(
            state.moon.brightness,
            state.moon.phase,
            cloud_opacity,
            night_mix,
        ),
        clouds: Vec4::new(
            scroll.x,
            scroll.y,
            state.clouds_speed,
            state.clouds_altitude,
        ),
        night_zenith: vec4_rgb(state.night_zenith_color),
        night_horizon: vec4_rgb(state.night_horizon_color),
    }
}

fn vec4_rgb(color: [f32; 3]) -> Vec4 {
    Vec4::new(color[0], color[1], color[2], 1.0)
}

/// Night gradient mix from sun elevation (VS2 §13 Stage 3).
pub fn night_mix_from_elevation(sun_elevation_deg: f32) -> f32 {
    (1.0 - (sun_elevation_deg + 8.0) / 18.0).clamp(0.0, 1.0)
}

fn update_atmospheric_sky(
    time: Res<Time>,
    lighting: Res<EnvironmentLightingState>,
    tweaks: Res<AtmosphereTweaks>,
    mut sky_state: ResMut<SkyState>,
    mut skies: Query<&MeshMaterial3d<SkyMaterial>, With<AtmosphericSky>>,
    mut materials: ResMut<Assets<SkyMaterial>>,
) {
    if tweaks.use_overrides {
        sky_state.zenith_color = tweaks.zenith_color;
        sky_state.horizon_color = tweaks.horizon_color;
        sky_state.mie_strength = tweaks.mie_strength;
    }

    let sun_dir = sun_direction_from_angles(lighting.sun_azimuth_deg, lighting.sun_elevation_deg);
    let params = sky_params_from_state(
        &sky_state,
        lighting.sun_elevation_deg,
        sun_dir,
        time.elapsed_secs(),
    );

    for mat_handle in &mut skies {
        if let Some(mut mat) = materials.get_mut(&mat_handle.0) {
            mat.params = params;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn night_mix_increases_below_horizon() {
        assert!(night_mix_from_elevation(-10.0) > night_mix_from_elevation(30.0));
    }

    #[test]
    fn moon_body_tracks_shader_uniform() {
        let state = SkyState {
            moon: CelestialBodyState {
                direction: Vec3::Y,
                angular_radius: 0.01,
                brightness: 0.2,
                phase: 0.75,
            },
            ..Default::default()
        };
        let params = sky_params_from_state(&state, 40.0, Vec3::Y, 0.0);
        assert!((params.moon.y - 1.0).abs() < 1e-5);
        assert!((params.celestial.x - 0.2).abs() < 1e-5);
    }
}
