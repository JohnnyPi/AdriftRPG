//! Automated validation (VS3 §21).

use crate::island_atlas::IslandAtlas;
use crate::island_gen::params::IslandGenParams;

#[derive(Clone, Debug, Default)]
pub struct ValidationReport {
    pub passed: bool,
    pub messages: Vec<String>,
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

    let min_land = 50u32;
    if land_cells < min_land {
        passed = false;
        messages.push(format!("Insufficient land area: {land_cells} cells"));
    } else {
        messages.push(format!("Land area: {land_cells} cells: OK"));
    }

    if atlas.river_graph.is_some() {
        messages.push("Primary river traced: OK".into());
    } else {
        passed = false;
        messages.push("Primary river missing".into());
    }

    let mut max_h = f32::MIN;
    for z in 0..atlas.elevation_local.height {
        for x in 0..atlas.elevation_local.width {
            let wx = atlas.origin[0] + x as f32 * atlas.elevation_local.spacing;
            let wz = atlas.origin[1] + z as f32 * atlas.elevation_local.spacing;
            if atlas.island_mask.sample_bilinear(wx, wz) > 0.5 {
                max_h = max_h.max(atlas.composed_land_elevation_at(wx, wz));
            }
        }
    }
    if max_h < params.island.sea_level_m + 40.0 {
        passed = false;
        messages.push(format!("Peak too low: {max_h:.1} m"));
    } else {
        messages.push(format!("Peak elevation {max_h:.1} m: OK"));
    }

    ValidationReport { passed, messages }
}
