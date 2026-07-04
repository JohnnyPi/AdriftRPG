// crates/terrain_surface/src/overlay.rs
//! Dynamic surface overlay state computed at mesh time.

use crate::context::SurfaceContext;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SurfaceOverlayState {
    pub wetness: f32,
    pub moss: f32,
    pub snow: f32,
    pub ash: f32,
    pub scorch: f32,
    pub mud: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OverlayResponseParams {
    pub wetness_darkening: f32,
    pub wetness_roughness_reduction: f32,
    pub wetness_normal_flattening: f32,
    pub moss_affinity: f32,
    pub snow_retention: f32,
    pub scorch_visibility: f32,
    pub mud_affinity: f32,
}

impl OverlayResponseParams {
    pub fn wet_rock() -> Self {
        Self {
            wetness_darkening: 0.28,
            wetness_roughness_reduction: 0.32,
            wetness_normal_flattening: 0.06,
            moss_affinity: 0.44,
            ..Default::default()
        }
    }

    pub fn rock() -> Self {
        Self {
            wetness_darkening: 0.22,
            wetness_roughness_reduction: 0.25,
            moss_affinity: 0.35,
            snow_retention: 0.08,
            ..Default::default()
        }
    }

    pub fn soil() -> Self {
        Self {
            wetness_darkening: 0.18,
            wetness_roughness_reduction: 0.20,
            moss_affinity: 0.55,
            mud_affinity: 0.45,
            ..Default::default()
        }
    }
}

pub fn compute_overlay_state(
    context: &SurfaceContext,
    responses: &OverlayResponseParams,
    persistence_wetness: f32,
    persistence_scorch: f32,
) -> SurfaceOverlayState {
    let field_wetness = (context.moisture * 0.55
        + context.soft.wetland * 0.35
        + (1.0 - (context.coast_distance_m / 120.0).clamp(0.0, 1.0)) * 0.10)
        .clamp(0.0, 1.0);
    let wetness = (field_wetness + persistence_wetness).clamp(0.0, 1.0);

    let shade = (1.0 - context.cave_exposure).clamp(0.0, 1.0);
    let moss = (context.moisture
        * responses.moss_affinity
        * shade
        * (1.0 - context.slope_degrees / 55.0).clamp(0.0, 1.0))
    .clamp(0.0, 1.0);

    let upward = (context.world_normal[1] * 0.5 + 0.5).clamp(0.0, 1.0);
    let cold = (1.0 - context.moisture * 0.35 - context.elevation_m / 400.0).clamp(0.0, 1.0);
    let snow = (upward
        * cold
        * responses.snow_retention
        * (1.0 - context.slope_degrees / 50.0).clamp(0.0, 1.0))
    .clamp(0.0, 1.0);

    let river_mask = (1.0 - (context.river_distance_m / 40.0).clamp(0.0, 1.0)).clamp(0.0, 1.0);
    let mud = (context.soft.wetland * responses.mud_affinity + river_mask * 0.4 + wetness * 0.2)
        .clamp(0.0, 1.0);

    SurfaceOverlayState {
        wetness,
        moss,
        snow,
        ash: 0.0,
        scorch: persistence_scorch.clamp(0.0, 1.0),
        mud,
    }
}

pub fn overlay_response_for_material_name(name: &str) -> OverlayResponseParams {
    let lower = name.to_ascii_lowercase();
    if lower.contains("wet") {
        OverlayResponseParams::wet_rock()
    } else if lower.contains("rock") || lower.contains("stone") || lower.contains("basalt") {
        OverlayResponseParams::rock()
    } else {
        OverlayResponseParams::soil()
    }
}
