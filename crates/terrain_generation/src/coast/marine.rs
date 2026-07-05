//! Marine feature suitability (reefs, lagoons, mangroves, tidal flats, sea caves).

use game_data::CompiledCoastRecipe;

use crate::fields::scalar::ScalarField;

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if edge0 >= edge1 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub struct MarineMasks {
    pub reef: ScalarField,
    pub lagoon: ScalarField,
    pub mangrove: ScalarField,
    pub tidal_flat: ScalarField,
    pub sea_cave: ScalarField,
    pub shelf: ScalarField,
}

pub fn compute_marine_masks(
    bathymetry: &ScalarField,
    coast_distance: &ScalarField,
    land_mask: &ScalarField,
    sediment: &ScalarField,
    temperature: &ScalarField,
    island_age: &ScalarField,
    wave_exposure: &ScalarField,
    river_mask: &ScalarField,
    cliff: &ScalarField,
    fracture: &ScalarField,
    slope: &ScalarField,
    humidity: &ScalarField,
    recipe: &CompiledCoastRecipe,
    sea_level_m: f32,
) -> MarineMasks {
    let desc = bathymetry.descriptor.clone();
    let mut reef = ScalarField::zeros(desc.clone());
    let mut lagoon = ScalarField::zeros(desc.clone());
    let mut mangrove = ScalarField::zeros(desc.clone());
    let mut tidal_flat = ScalarField::zeros(desc.clone());
    let mut sea_cave = ScalarField::zeros(desc.clone());
    let mut shelf = ScalarField::zeros(desc.clone());
    let w = desc.width;
    let h = desc.height;

    for z in 0..h {
        for x in 0..w {
            let land = land_mask.get(x, z);
            let coast = coast_distance.get(x, z);
            let depth = (sea_level_m - bathymetry.get(x, z)).max(0.0);
            let temp = temperature.get(x, z);
            let age = island_age.get(x, z);
            let sed = sediment.get(x, z);
            let exposure = wave_exposure.get(x, z);
            let river = river_mask.get(x, z);

            let depth_ok = smoothstep(recipe.reef_depth_min_m, recipe.reef_depth_max_m, depth)
                * (1.0
                    - smoothstep(
                        recipe.reef_depth_max_m,
                        recipe.reef_depth_max_m + 15.0,
                        depth,
                    ));
            let temp_ok = smoothstep(recipe.reef_min_temperature, 1.0, temp);
            let age_ok = smoothstep(recipe.reef_min_age_myr, recipe.reef_min_age_myr + 2.0, age);
            let sed_ok = 1.0
                - smoothstep(
                    recipe.reef_max_sediment * 0.5,
                    recipe.reef_max_sediment,
                    sed,
                );
            let river_plume = 1.0 - smoothstep(0.1, 0.6, river + sed * 0.5);
            let exposure_ok = smoothstep(0.15, 0.75, exposure);
            let offshore = if land > 0.5 {
                0.0
            } else {
                smoothstep(5.0, 80.0, coast.abs())
            };

            let reef_score =
                depth_ok * temp_ok * age_ok * sed_ok * river_plume * exposure_ok * offshore;
            reef.set(x, z, reef_score.clamp(0.0, 1.0));

            let shelf_score = if land > 0.5 {
                0.0
            } else {
                smoothstep(5.0, 60.0, depth) * (1.0 - smoothstep(80.0, 200.0, depth))
            };
            shelf.set(x, z, shelf_score);

            let tidal_score = if land > 0.5 {
                smoothstep(recipe.mangrove_salinity_max_m, 0.0, coast)
                    * (1.0 - smoothstep(3.0, 8.0, slope.get(x, z)))
                    * smoothstep(0.2, 0.8, sed)
            } else {
                smoothstep(0.0, 30.0, coast.abs()) * (1.0 - smoothstep(0.5, 3.0, depth))
            };
            tidal_flat.set(x, z, tidal_score.clamp(0.0, 1.0));

            let mangrove_score = if land > 0.5 {
                smoothstep(
                    recipe.mangrove_salinity_min_m,
                    recipe.mangrove_salinity_max_m,
                    coast,
                ) * (1.0
                    - smoothstep(
                        recipe.mangrove_max_slope_deg,
                        recipe.mangrove_max_slope_deg + 5.0,
                        slope.get(x, z),
                    ))
                    * humidity.get(x, z)
                    * (1.0 - cliff.get(x, z))
            } else {
                0.0
            };
            mangrove.set(x, z, mangrove_score.clamp(0.0, 1.0));

            sea_cave.set(
                x,
                z,
                (cliff.get(x, z) * exposure * fracture.get(x, z)).clamp(0.0, 1.0),
            );
        }
    }

    // Lagoon: shallow enclosed water with reef ring nearby
    for z in 0..h {
        for x in 0..w {
            if land_mask.get(x, z) > 0.5 {
                continue;
            }
            let depth = (sea_level_m - bathymetry.get(x, z)).max(0.0);
            if depth > recipe.lagoon_max_depth_m {
                continue;
            }
            let mut reef_ring = 0.0f32;
            for dz in -3i32..=3 {
                for dx in -3i32..=3 {
                    let nx = x as i32 + dx;
                    let nz = z as i32 + dz;
                    if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                        continue;
                    }
                    reef_ring = reef_ring.max(reef.get(nx as u32, nz as u32));
                }
            }
            let shallow = 1.0
                - smoothstep(
                    recipe.lagoon_max_depth_m * 0.5,
                    recipe.lagoon_max_depth_m,
                    depth,
                );
            let enclosed = smoothstep(recipe.lagoon_reef_enclosure_min, 1.0, reef_ring);
            lagoon.set(x, z, (shallow * enclosed).clamp(0.0, 1.0));
        }
    }

    MarineMasks {
        reef,
        lagoon,
        mangrove,
        tidal_flat,
        sea_cave,
        shelf,
    }
}

pub fn count_lagoon_components(lagoon: &ScalarField, threshold: f32) -> u32 {
    let w = lagoon.descriptor.width;
    let h = lagoon.descriptor.height;
    let mut visited = vec![false; (w * h) as usize];
    let mut count = 0u32;

    for z in 0..h {
        for x in 0..w {
            let idx = (z * w + x) as usize;
            if visited[idx] || lagoon.get(x, z) < threshold {
                continue;
            }
            count += 1;
            let mut stack = vec![(x, z)];
            visited[idx] = true;
            while let Some((cx, cz)) = stack.pop() {
                for (dx, dz) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                    let nx = cx as i32 + dx;
                    let nz = cz as i32 + dz;
                    if nx < 0 || nz < 0 || nx >= w as i32 || nz >= h as i32 {
                        continue;
                    }
                    let nidx = (nz as u32 * w + nx as u32) as usize;
                    if visited[nidx] || lagoon.get(nx as u32, nz as u32) < threshold {
                        continue;
                    }
                    visited[nidx] = true;
                    stack.push((nx as u32, nz as u32));
                }
            }
        }
    }
    count
}

pub fn reef_area_m2(reef: &ScalarField, threshold: f32, cell_m: f64) -> f64 {
    let cell_area = cell_m * cell_m;
    let mut cells = 0u64;
    for v in &reef.values {
        if *v >= threshold {
            cells += 1;
        }
    }
    cells as f64 * cell_area
}
