//! Island topology validation.

use crate::fields::scalar::ScalarField;
use crate::islands::seed::IslandSeed;

pub struct IslandValidationReport {
    pub connected_components: u32,
    pub land_cell_count: u32,
    pub total_cells: u32,
    pub passed: bool,
    pub messages: Vec<String>,
}

pub fn validate_single_island(
    influence: &ScalarField,
    _seed: &IslandSeed,
) -> IslandValidationReport {
    let mut land_cells = 0u32;
    let mut messages = Vec::new();
    let threshold = 0.05f32;

    for z in 0..influence.descriptor.height {
        for x in 0..influence.descriptor.width {
            if influence.get(x, z) > threshold {
                land_cells += 1;
            }
        }
    }

    let components = count_land_components(influence, threshold);
    if components != 1 {
        messages.push(format!(
            "expected exactly one land component, found {components}"
        ));
    }
    if land_cells == 0 {
        messages.push("no land cells found".into());
    }

    let passed = components == 1 && land_cells > 0;
    IslandValidationReport {
        connected_components: components,
        land_cell_count: land_cells,
        total_cells: influence.descriptor.width * influence.descriptor.height,
        passed,
        messages,
    }
}

fn count_land_components(field: &ScalarField, threshold: f32) -> u32 {
    let w = field.descriptor.width;
    let h = field.descriptor.height;
    let mut visited = vec![false; (w * h) as usize];
    let mut components = 0u32;

    for z in 0..h {
        for x in 0..w {
            let idx = (z * w + x) as usize;
            if visited[idx] || field.get(x, z) <= threshold {
                continue;
            }
            components += 1;
            flood_fill(field, x, z, threshold, &mut visited);
        }
    }
    components
}

fn flood_fill(
    field: &ScalarField,
    start_x: u32,
    start_z: u32,
    threshold: f32,
    visited: &mut [bool],
) {
    let w = field.descriptor.width;
    let h = field.descriptor.height;
    let mut stack = vec![(start_x, start_z)];
    while let Some((x, z)) = stack.pop() {
        let idx = (z * w + x) as usize;
        if visited[idx] || field.get(x, z) <= threshold {
            continue;
        }
        visited[idx] = true;
        if x > 0 {
            stack.push((x - 1, z));
        }
        if x + 1 < w {
            stack.push((x + 1, z));
        }
        if z > 0 {
            stack.push((x, z - 1));
        }
        if z + 1 < h {
            stack.push((x, z + 1));
        }
    }
}
