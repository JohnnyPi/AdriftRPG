// crates/terrain_generation/src/island_gen/validate.rs
//! Automated validation (VS3 §21).

use crate::island_atlas::IslandAtlas;
use crate::island_gen::params::IslandGenParams;

const RIVER_DESCENT_TOLERANCE_M: f32 = 0.5;
const RIVER_MOUTH_SEA_TOLERANCE_M: f32 = 0.5;
const MIN_PLAYABLE_LAND_FRACTION: f32 = 0.0025;

/// Fraction of the authored composed edifice (shield + summit) the sampled
/// peak must retain after the explicit erosion budget is subtracted. The
/// remaining slack absorbs regional-grid discretization of the summit cone
/// (the grid samples up to half a cell off-axis), the caldera fringe, and
/// surface noise troughs.
const MIN_PEAK_COMPOSED_FRACTION: f32 = 0.7;

/// Cells whose composed elevation sits within this band below the analytic
/// clamp (`sea + maximum_height_m`) are counted as pinned against the
/// ceiling. Sized so composed noise (±~2 m) cannot hide a real plateau while
/// a legitimate summit grazing the clamp stays under the fraction limit.
const PLATEAU_BAND_M: f32 = 1.0;

/// Maximum fraction of land cells allowed inside the plateau band. A healthy
/// island composes below its ceiling (0%); the hardcoded-ridge bug pinned
/// >5% of the land into a clipped cap.
const MAX_PLATEAU_LAND_FRACTION: f32 = 0.01;

#[derive(Clone, Debug, Default)]
pub struct ValidationReport {
    pub passed: bool,
    pub messages: Vec<String>,
}

fn min_land_area_m2(params: &IslandGenParams) -> f32 {
    let radius = params.island.playable_diameter_m * 0.05;
    (radius * radius * std::f32::consts::PI * MIN_PLAYABLE_LAND_FRACTION).max(64.0)
}

/// Minimum acceptable sampled peak.
///
/// Derived from the volcano params that actually produce the peak — not from
/// `island.maximum_height_m`, which is an independent clamp ceiling. Deriving
/// the floor from the ceiling required peaks the authored cones never
/// composed; that was only ever satisfied while a hardcoded ridge amplitude
/// inflated summits into the clamp.
///
/// Public so diagnostics (e.g. `tests/vs3_elevation_diag.rs`) assert against
/// the same floor validation enforces, instead of drifting hardcoded copies.
pub fn min_peak_elevation_m(params: &IslandGenParams) -> f32 {
    let composed = params.volcano.shield_height_m + params.volcano.summit_height_m;
    let erosion_budget =
        params.erosion.maximum_step_m * params.erosion.stream_power_iterations as f32;
    (params.island.sea_level_m + composed * MIN_PEAK_COMPOSED_FRACTION - erosion_budget)
        .max(params.island.sea_level_m)
}

fn validate_river_graph(
    atlas: &IslandAtlas,
    params: &IslandGenParams,
    messages: &mut Vec<String>,
) -> bool {
    let Some(river) = atlas.river_graph.as_ref() else {
        messages.push("Primary river missing".into());
        return false;
    };

    if river.points.len() < 2 {
        messages.push("Primary river has fewer than two control points".into());
        return false;
    }

    let mut ok = true;
    for window in river.points.windows(2) {
        let p0 = window[0].position_xz;
        let p1 = window[1].position_xz;
        let h0 = atlas.filled_elevation.sample_bilinear(p0[0], p0[1]);
        let h1 = atlas.filled_elevation.sample_bilinear(p1[0], p1[1]);
        if h1 > h0 + RIVER_DESCENT_TOLERANCE_M {
            ok = false;
            messages.push(format!(
                "River segment ascends on filled surface: {h0:.1} m -> {h1:.1} m"
            ));
            break;
        }
    }

    let mouth = river.points.last().expect("len >= 2");
    let mouth_limit = params.island.sea_level_m + RIVER_MOUTH_SEA_TOLERANCE_M;
    if mouth.bed_elevation > mouth_limit {
        ok = false;
        messages.push(format!(
            "River mouth bed {:.1} m exceeds sea level + {RIVER_MOUTH_SEA_TOLERANCE_M} m ({mouth_limit:.1} m)",
            mouth.bed_elevation
        ));
    }

    if ok {
        messages.push(format!(
            "Primary river descends to mouth ({:.1} m): OK",
            mouth.bed_elevation
        ));
    }
    ok
}

pub fn validate_atlas(atlas: &IslandAtlas, params: &IslandGenParams) -> ValidationReport {
    let mut messages = Vec::new();
    let mut passed = true;

    let mut land_cells = 0u32;
    let mut edge_underwater = true;
    for z in 0..atlas.elevation_regional.height {
        for x in 0..atlas.elevation_regional.width {
            if atlas.island_mask.get(x, z) > 0.5 {
                land_cells += 1;
            }
            let on_edge = x == 0
                || z == 0
                || x == atlas.elevation_regional.width - 1
                || z == atlas.elevation_regional.height - 1;
            if on_edge && atlas.elevation_regional.get(x, z) > params.island.sea_level_m + 1.0 {
                edge_underwater = false;
            }
        }
    }
    if !edge_underwater {
        passed = false;
        messages.push("Map edges are not fully underwater".into());
    } else {
        messages.push("Map edges underwater: OK".into());
    }

    let spacing = atlas.island_mask.spacing;
    let land_area_m2 = land_cells as f32 * spacing * spacing;
    let min_land_area_m2 = min_land_area_m2(params);
    if land_area_m2 < min_land_area_m2 {
        passed = false;
        messages.push(format!(
            "Insufficient land area: {land_area_m2:.0} m² (minimum {min_land_area_m2:.0} m²)"
        ));
    } else {
        messages.push(format!("Land area: {land_area_m2:.0} m²: OK"));
    }

    if !validate_river_graph(atlas, params, &mut messages) {
        passed = false;
    }

    let mut max_h = f32::MIN;
    let mut local_land_cells = 0u32;
    let mut plateau_cells = 0u32;
    let plateau_floor = params.island.sea_level_m + params.island.maximum_height_m - PLATEAU_BAND_M;
    for z in 0..atlas.elevation_local.height {
        for x in 0..atlas.elevation_local.width {
            let wx = atlas.origin[0] + x as f32 * atlas.elevation_local.spacing;
            let wz = atlas.origin[1] + z as f32 * atlas.elevation_local.spacing;
            if atlas.island_mask.sample_bilinear(wx, wz) > 0.5 {
                local_land_cells += 1;
                let h = atlas.composed_land_elevation_at(wx, wz);
                max_h = max_h.max(h);
                if h >= plateau_floor {
                    plateau_cells += 1;
                }
            }
        }
    }
    let min_peak = min_peak_elevation_m(params);
    if max_h < min_peak {
        passed = false;
        messages.push(format!(
            "Peak too low: {max_h:.1} m (minimum {min_peak:.1} m from volcano config)"
        ));
    } else {
        messages.push(format!("Peak elevation {max_h:.1} m: OK"));
    }

    // Clamp-plateau check: the analytic clamp caps heights at
    // sea + maximum_height_m, so an over-composed edifice cannot be caught by
    // the peak value — it shows up as a large land fraction pinned against
    // the ceiling (the visible symptom of the old hardcoded-ridge bug).
    let plateau_fraction = if local_land_cells > 0 {
        plateau_cells as f32 / local_land_cells as f32
    } else {
        0.0
    };
    if plateau_fraction > MAX_PLATEAU_LAND_FRACTION {
        passed = false;
        messages.push(format!(
            "Clamp plateau: {:.1}% of land pinned at the height ceiling (edifice over-composed for maximum_height_m)",
            plateau_fraction * 100.0
        ));
    } else {
        messages.push("Height ceiling headroom: OK".into());
    }

    let mut fill_violations = 0u32;
    for z in 0..atlas.filled_elevation.height {
        for x in 0..atlas.filled_elevation.width {
            let raw = atlas.elevation_regional.get(x, z);
            let filled = atlas.filled_elevation.get(x, z);
            if filled + 0.001 < raw {
                fill_violations += 1;
            }
        }
    }
    if fill_violations > 0 {
        passed = false;
        messages.push(format!(
            "Filled elevation below raw surface in {fill_violations} cells (stale hydrology epoch)"
        ));
    } else {
        messages.push("Fill invariant (filled ≥ regional): OK".into());
    }

    let mut soil_samples = Vec::new();
    let mut flat_soil = 0.0f32;
    let mut flat_count = 0u32;
    let mut steep_soil = 0.0f32;
    let mut steep_count = 0u32;
    for z in 0..atlas.soil_depth.height {
        for x in 0..atlas.soil_depth.width {
            if atlas.island_mask.get(x, z) < 0.4 {
                continue;
            }
            let soil = atlas.soil_depth.get(x, z);
            soil_samples.push(soil);
            let sl = atlas.slope.get(x, z);
            if sl < 10.0 {
                flat_soil += soil;
                flat_count += 1;
            } else if sl > 35.0 {
                steep_soil += soil;
                steep_count += 1;
            }
        }
    }
    let soil_variance = if soil_samples.is_empty() {
        0.0
    } else {
        let mean = soil_samples.iter().sum::<f32>() / soil_samples.len() as f32;
        soil_samples.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / soil_samples.len() as f32
    };
    if soil_variance < 1e-4 {
        passed = false;
        messages.push("soil_depth field has no variance (likely uncomputed)".into());
    } else {
        messages.push(format!("soil_depth variance {soil_variance:.4}: OK"));
    }
    if flat_count > 0 && steep_count > 0 {
        let flat_mean = flat_soil / flat_count as f32;
        let steep_mean = steep_soil / steep_count as f32;
        if flat_mean <= steep_mean {
            passed = false;
            messages.push(format!(
                "soil_depth flat mean {flat_mean:.2} should exceed steep mean {steep_mean:.2}"
            ));
        } else {
            messages.push(format!(
                "soil_depth flat {flat_mean:.2} vs steep {steep_mean:.2}: OK"
            ));
        }
    }

    ValidationReport { passed, messages }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::island_gen::{IslandGenParams, build_island_atlas};

    #[test]
    fn peak_floor_scales_with_composed_edifice() {
        // The floor must track the volcano params that produce the peak, and
        // must NOT move when only the clamp ceiling changes.
        let base = IslandGenParams::default();
        let base_floor = min_peak_elevation_m(&base);

        let mut taller = base.clone();
        taller.volcano.shield_height_m *= 2.0;
        taller.volcano.summit_height_m *= 2.0;
        assert!(
            min_peak_elevation_m(&taller) > base_floor,
            "floor should rise with a taller composed edifice"
        );

        let mut higher_ceiling = base.clone();
        higher_ceiling.island.maximum_height_m *= 2.0;
        assert!(
            (min_peak_elevation_m(&higher_ceiling) - base_floor).abs() < 1e-4,
            "raising only the clamp ceiling must not demand a taller peak"
        );

        // Sanity on the default relationship: the authored composed peak must
        // clear its own floor with room for discretization losses.
        let composed =
            base.island.sea_level_m + base.volcano.shield_height_m + base.volcano.summit_height_m;
        assert!(base_floor < composed * 0.85);
    }

    #[test]
    fn raising_height_ceiling_alone_does_not_fail_peak_check() {
        // Regression for the original coupling: with the floor derived from
        // maximum_height_m, raising the ceiling without touching the volcano
        // demanded peaks the authored cones never composed.
        let mut params = IslandGenParams::default();
        params.island.maximum_height_m = 80.0;
        params.island.sea_level_m = 0.0;
        params.fit_to_ocean_extent();
        let atlas = build_island_atlas(&params);
        let report = validate_atlas(&atlas, &params);
        assert!(
            report.messages.iter().any(|m| m.contains("Peak elevation")),
            "expected peak message, got {report:?}"
        );
    }

    #[test]
    fn clamp_plateau_fails_ceiling_check() {
        // Edifice composed far above the ceiling: the analytic clamp flattens
        // the summit into a plateau at sea + maximum_height_m, which the
        // ceiling check must reject (this was the visible symptom of the
        // hardcoded-ridge bug).
        let mut params = IslandGenParams::default();
        params.island.maximum_height_m = 20.0;
        params.fit_to_ocean_extent();
        let atlas = build_island_atlas(&params);
        let report = validate_atlas(&atlas, &params);
        assert!(
            report.messages.iter().any(|m| m.contains("Clamp plateau")),
            "expected plateau violation, got {report:?}"
        );
        assert!(!report.passed);
    }

    #[test]
    fn land_area_uses_square_meters_not_raw_cells() {
        let params = IslandGenParams::default();
        let atlas = build_island_atlas(&params);
        let report = validate_atlas(&atlas, &params);
        assert!(
            report
                .messages
                .iter()
                .any(|m| m.contains("Land area:") && m.contains("m²")),
            "land area should be reported in m²: {report:?}"
        );
    }
}
