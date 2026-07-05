//! Compile source definitions into normalized compiled recipes.

use serde::{Deserialize, Serialize};

use super::definitions::*;
use super::hash::{RecipeHash, recipe_content_hash};

/// Resolved and validated world recipe ready for compilation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledWorldRecipe {
    pub id: String,
    pub seed: u64,
    pub extent: CompiledExtent,
    pub resolutions: CompiledResolutions,
    pub boundary: CompiledBoundaryRecipe,
    pub islands: Vec<CompiledIslandRecipe>,
    pub geology: CompiledGeologyRecipe,
    pub refinement: CompiledRefinementRecipe,
    pub climate: CompiledClimateRecipe,
    pub hydrology: CompiledHydrologyRecipe,
    pub erosion: CompiledErosionRecipe,
    pub coast: CompiledCoastRecipe,
    pub biomes: CompiledBiomeRecipe,
    pub strata: CompiledStrataRecipe,
    pub caves: CompiledCavesRecipe,
    pub validation: Option<CompiledValidationRecipe>,
    pub recipe_hash: RecipeHash,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledExtent {
    pub width_m: f64,
    pub depth_m: f64,
    pub vertical_min_m: f64,
    pub vertical_max_m: f64,
    pub sea_level_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledResolutions {
    pub control_cell_m: f64,
    pub regional_cell_m: f64,
    pub local_cell_m: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledBoundaryRecipe {
    pub id: String,
    pub ocean_edge_start_fraction: f32,
    pub maximum_depth_m: f32,
    pub safety_margin_fraction: f32,
    pub variation_amplitude_m: f32,
    pub variation_frequency: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledIslandRecipe {
    pub id: String,
    pub island_index: u32,
    pub center_x_m: f64,
    pub center_z_m: f64,
    pub age_myr: f32,
    pub uplift: f32,
    pub volcanic_activity: f32,
    pub footprint: CompiledFootprint,
    pub volcano: CompiledVolcano,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledFootprint {
    pub major_radius_m: f32,
    pub minor_radius_m: f32,
    pub rotation_rad: f32,
    pub warp_amplitude_m: f32,
    pub warp_wavelength_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledVolcano {
    pub peak_height_m: f32,
    pub shield_radius_m: f32,
    pub caldera_radius_m: f32,
    pub caldera_depth_m: f32,
    pub secondary_vents: u32,
    pub ridge_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledGeologyRecipe {
    pub id: String,
    pub weathering_age_threshold_myr: f32,
    pub tuff_age_threshold_myr: f32,
    pub coastal_weathering_band_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledRefinementRecipe {
    pub id: String,
    pub window_interior_samples: [u32; 2],
    pub window_stride_samples: [u32; 2],
    pub window_halo_samples: u32,
    pub regional_amplitude_m: f32,
    pub coast_preserve_start_m: f32,
    pub coast_preserve_end_m: f32,
    pub seam_max_elevation_diff_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledValidationRecipe {
    pub id: String,
    pub land_fraction_min: f32,
    pub land_fraction_max: f32,
    pub min_peak_elevation_m: f32,
    pub max_peak_elevation_m: f32,
    pub river_ocean_connection_ratio_min: f32,
    pub max_disconnected_river_fraction: f32,
    pub min_permanent_river_length_m: f32,
    pub reef_area_min_m2: f32,
    pub lagoon_count_min: u32,
    pub biome_entropy_min: u32,
    pub min_cave_systems: u32,
    pub min_traversable_cave_systems: u32,
    pub max_cave_mouth_breaches: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledCaveFamilyProfile {
    pub systems_max: u32,
    pub chamber_count_min: u32,
    pub chamber_count_max: u32,
    pub passage_radius_min_m: f32,
    pub passage_radius_max_m: f32,
    pub minimum_cover_m: f32,
    pub maximum_depth_m: f32,
    pub entrance_threshold: f32,
    pub overhang_enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledCavesRecipe {
    pub id: String,
    pub lava_max_age_myr: f32,
    pub limestone_min_permeability: f32,
    pub sea_tidal_band_m: [f32; 2],
    pub lava_tube: CompiledCaveFamilyProfile,
    pub limestone: CompiledCaveFamilyProfile,
    pub sea_cave: CompiledCaveFamilyProfile,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledClimateRecipe {
    pub id: String,
    pub base_temperature_c: f32,
    pub lapse_rate_c_per_km: f32,
    pub prevailing_wind_direction_deg: f32,
    pub prevailing_wind_strength: f32,
    pub prevailing_wind_moisture: f32,
    pub ocean_recharge: f32,
    pub orographic_factor: f32,
    pub rain_shadow_factor: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledHydrologyRecipe {
    pub id: String,
    pub routing: String,
    pub rainfall_weight: f32,
    pub stream_threshold: f32,
    pub permanent_river_threshold: f32,
    pub minimum_stream_length_m: f32,
    pub lake_min_area_cells: u32,
    pub wetland_moisture_threshold: f32,
    pub waterfall_min_drop_m: f32,
    pub waterfall_min_discharge: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledStreamPowerErosion {
    pub m: f32,
    pub n: f32,
    pub maximum_step_m: f32,
    pub iterations_per_cycle: u32,
    pub erodibility: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledThermalErosion {
    pub talus_deg: f32,
    pub transfer_rate: f32,
    pub iterations_per_cycle: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledSedimentErosion {
    pub pickup_rate: f32,
    pub transport_rate: f32,
    pub deposition_rate: f32,
    pub capacity_factor: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledErosionRecipe {
    pub id: String,
    pub iterations: u32,
    pub stream_power: CompiledStreamPowerErosion,
    pub thermal: CompiledThermalErosion,
    pub sediment: CompiledSedimentErosion,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledCoastRecipe {
    pub id: String,
    pub beach_width_max_m: f32,
    pub beach_max_slope_deg: f32,
    pub berm_height_min_m: f32,
    pub berm_height_max_m: f32,
    pub cliff_min_slope_deg: f32,
    pub cliff_min_exposure: f32,
    pub reef_min_age_myr: f32,
    pub reef_depth_min_m: f32,
    pub reef_depth_max_m: f32,
    pub reef_max_sediment: f32,
    pub reef_min_temperature: f32,
    pub lagoon_max_depth_m: f32,
    pub lagoon_reef_enclosure_min: f32,
    pub mangrove_max_slope_deg: f32,
    pub mangrove_salinity_min_m: f32,
    pub mangrove_salinity_max_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledBiomeRecipe {
    pub id: String,
    pub cloud_forest_elevation_min_m: f32,
    pub cloud_forest_elevation_max_m: f32,
    pub dry_forest_rainfall_max: f32,
    pub montane_shrub_elevation_m: f32,
    pub volcanic_barren_slope_deg: f32,
    pub wetland_moisture_min: f32,
    pub reef_depth_min_m: f32,
    pub reef_depth_max_m: f32,
    pub shelf_depth_m: f32,
    pub deep_coastal_depth_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledStrataLayer {
    pub material: String,
    pub thickness_min_m: f32,
    pub thickness_max_m: f32,
    pub remaining: bool,
    pub requires_vegetated: bool,
    pub driven_by_rainfall: bool,
    pub driven_by_slope: bool,
    pub driven_by_biome: bool,
    pub driven_by_age: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledStrataDeposit {
    pub id: String,
    pub mask: String,
    pub thickness_min_m: f32,
    pub thickness_max_m: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledStrataRecipe {
    pub id: String,
    pub layers: Vec<CompiledStrataLayer>,
    pub deposits: Vec<CompiledStrataDeposit>,
}

/// Fully resolved source bundle with compiled recipe.
#[derive(Clone, Debug)]
pub struct ResolvedWorldBundle {
    pub recipe: CompiledWorldRecipe,
}

pub fn resolve_world_bundle(
    world_id: &str,
    bundle: &WorldgenSourceBundle,
) -> Result<ResolvedWorldBundle, super::validate::WorldgenValidationError> {
    let world = bundle.worlds.get(world_id).ok_or_else(|| {
        super::validate::WorldgenValidationError::MissingReference {
            id: world_id.to_string(),
            kind: "world",
        }
    })?;

    super::validate::validate_world_source(world, bundle)?;

    let boundary = bundle.boundaries.get(&world.boundary).ok_or_else(|| {
        super::validate::WorldgenValidationError::MissingReference {
            id: world.boundary.clone(),
            kind: "boundary",
        }
    })?;

    let geology = bundle.geology.get(&world.geology).ok_or_else(|| {
        super::validate::WorldgenValidationError::MissingReference {
            id: world.geology.clone(),
            kind: "geology",
        }
    })?;

    let refinement = bundle.refinement.get(&world.refinement).ok_or_else(|| {
        super::validate::WorldgenValidationError::MissingReference {
            id: world.refinement.clone(),
            kind: "refinement",
        }
    })?;

    let climate = bundle.climate.get(&world.climate).ok_or_else(|| {
        super::validate::WorldgenValidationError::MissingReference {
            id: world.climate.clone(),
            kind: "climate",
        }
    })?;

    let hydrology = bundle.hydrology.get(&world.hydrology).ok_or_else(|| {
        super::validate::WorldgenValidationError::MissingReference {
            id: world.hydrology.clone(),
            kind: "hydrology",
        }
    })?;

    let erosion = bundle.erosion.get(&world.erosion).ok_or_else(|| {
        super::validate::WorldgenValidationError::MissingReference {
            id: world.erosion.clone(),
            kind: "erosion",
        }
    })?;

    let coast = bundle.coasts.get(&world.coast).ok_or_else(|| {
        super::validate::WorldgenValidationError::MissingReference {
            id: world.coast.clone(),
            kind: "coast",
        }
    })?;

    let biomes = bundle.biomes.get(&world.biomes).ok_or_else(|| {
        super::validate::WorldgenValidationError::MissingReference {
            id: world.biomes.clone(),
            kind: "biomes",
        }
    })?;

    let strata = bundle.strata.get(&world.strata).ok_or_else(|| {
        super::validate::WorldgenValidationError::MissingReference {
            id: world.strata.clone(),
            kind: "strata",
        }
    })?;

    let caves = bundle.caves.get(&world.caves).ok_or_else(|| {
        super::validate::WorldgenValidationError::MissingReference {
            id: world.caves.clone(),
            kind: "caves",
        }
    })?;

    let validation = world
        .validation
        .as_ref()
        .map(|id| {
            bundle.validation.get(id).ok_or_else(|| {
                super::validate::WorldgenValidationError::MissingReference {
                    id: id.clone(),
                    kind: "validation",
                }
            })
        })
        .transpose()?;

    if world.islands.len() != 1 {
        return Err(super::validate::WorldgenValidationError::Semantic {
            message: format!(
                "Milestone A requires exactly one island reference, got {}",
                world.islands.len()
            ),
        });
    }

    let island_src = bundle.islands.get(&world.islands[0]).ok_or_else(|| {
        super::validate::WorldgenValidationError::MissingReference {
            id: world.islands[0].clone(),
            kind: "island",
        }
    })?;

    let compiled_island = compile_island(island_src, &world.islands[0], 0)?;

    let mut recipe = CompiledWorldRecipe {
        id: world.id.clone(),
        seed: world.seed,
        extent: CompiledExtent {
            width_m: world.extent.width_m,
            depth_m: world.extent.depth_m,
            vertical_min_m: world.extent.vertical_min_m,
            vertical_max_m: world.extent.vertical_max_m,
            sea_level_m: world.extent.sea_level_m,
        },
        resolutions: CompiledResolutions {
            control_cell_m: world.resolutions.control_cell_m,
            regional_cell_m: world.resolutions.regional_cell_m,
            local_cell_m: world.resolutions.local_cell_m,
        },
        boundary: compile_boundary(boundary),
        islands: vec![compiled_island],
        geology: CompiledGeologyRecipe {
            id: geology.id.clone(),
            weathering_age_threshold_myr: geology.weathering_age_threshold_myr,
            tuff_age_threshold_myr: geology.tuff_age_threshold_myr,
            coastal_weathering_band_m: geology.coastal_weathering_band_m,
        },
        refinement: CompiledRefinementRecipe {
            id: refinement.id.clone(),
            window_interior_samples: refinement.window_interior_samples,
            window_stride_samples: refinement.window_stride_samples,
            window_halo_samples: refinement.window_halo_samples,
            regional_amplitude_m: refinement.regional_amplitude_m,
            coast_preserve_start_m: refinement.coast_preserve_start_m,
            coast_preserve_end_m: refinement.coast_preserve_end_m,
            seam_max_elevation_diff_m: refinement.seam_max_elevation_diff_m,
        },
        climate: compile_climate(climate),
        hydrology: compile_hydrology(hydrology),
        erosion: compile_erosion(erosion),
        coast: compile_coast(coast),
        biomes: compile_biomes(biomes),
        strata: compile_strata(strata),
        caves: compile_caves(caves),
        validation: validation.map(|v| CompiledValidationRecipe {
            id: v.id.clone(),
            land_fraction_min: v.land_fraction_min,
            land_fraction_max: v.land_fraction_max,
            min_peak_elevation_m: v.min_peak_elevation_m,
            max_peak_elevation_m: v.max_peak_elevation_m,
            river_ocean_connection_ratio_min: v.river_ocean_connection_ratio_min,
            max_disconnected_river_fraction: v.max_disconnected_river_fraction,
            min_permanent_river_length_m: v.min_permanent_river_length_m,
            reef_area_min_m2: v.reef_area_min_m2,
            lagoon_count_min: v.lagoon_count_min,
            biome_entropy_min: v.biome_entropy_min,
            min_cave_systems: v.min_cave_systems,
            min_traversable_cave_systems: v.min_traversable_cave_systems,
            max_cave_mouth_breaches: v.max_cave_mouth_breaches,
        }),
        recipe_hash: RecipeHash::from_bytes([0u8; 32]),
    };

    recipe.recipe_hash = recipe_content_hash(&recipe);
    Ok(ResolvedWorldBundle { recipe })
}

fn compile_boundary(src: &BoundaryRecipeSource) -> CompiledBoundaryRecipe {
    let BoundedOceanSource {
        ocean_edge_start_fraction,
        maximum_depth_m,
        safety_margin_fraction,
        variation_amplitude_m,
        variation_frequency,
    } = match &src.shape {
        BoundaryShapeSource::BoundedOcean(b) => b.clone(),
    };
    CompiledBoundaryRecipe {
        id: src.id.clone(),
        ocean_edge_start_fraction,
        maximum_depth_m,
        safety_margin_fraction,
        variation_amplitude_m,
        variation_frequency,
    }
}

fn compile_island(
    src: &IslandRecipeSource,
    id: &str,
    index: u32,
) -> Result<CompiledIslandRecipe, super::validate::WorldgenValidationError> {
    match &src.placement {
        IslandPlacementSource::SingleCentered(s) => Ok(CompiledIslandRecipe {
            id: id.to_string(),
            island_index: index,
            center_x_m: 0.0,
            center_z_m: 0.0,
            age_myr: s.age_myr,
            uplift: s.uplift,
            volcanic_activity: s.volcanic_activity,
            footprint: compile_footprint(&s.footprint),
            volcano: compile_volcano(&s.volcano),
        }),
        IslandPlacementSource::Explicit(e) => {
            if e.islands.len() != 1 {
                return Err(super::validate::WorldgenValidationError::Semantic {
                    message: "explicit island list must contain exactly one entry".into(),
                });
            }
            let entry = &e.islands[0];
            Ok(CompiledIslandRecipe {
                id: id.to_string(),
                island_index: index,
                center_x_m: entry.center_x_m,
                center_z_m: entry.center_z_m,
                age_myr: entry.age_myr,
                uplift: 1.0,
                volcanic_activity: 1.0,
                footprint: compile_footprint(&entry.footprint),
                volcano: compile_volcano(&entry.volcano),
            })
        }
    }
}

fn compile_footprint(src: &FootprintSource) -> CompiledFootprint {
    match src {
        FootprintSource::Ellipse(e) => CompiledFootprint {
            major_radius_m: e.major_radius_m,
            minor_radius_m: e.minor_radius_m,
            rotation_rad: e.rotation_deg.to_radians(),
            warp_amplitude_m: 0.0,
            warp_wavelength_m: 1.0,
        },
        FootprintSource::WarpedEllipse(w) => CompiledFootprint {
            major_radius_m: w.major_radius_m,
            minor_radius_m: w.minor_radius_m,
            rotation_rad: w.rotation_deg.to_radians(),
            warp_amplitude_m: w.warp_amplitude_m,
            warp_wavelength_m: w.warp_wavelength_m,
        },
    }
}

fn compile_volcano(src: &VolcanoSource) -> CompiledVolcano {
    CompiledVolcano {
        peak_height_m: src.peak_height_m,
        shield_radius_m: src.shield_radius_m,
        caldera_radius_m: src.caldera_radius_m,
        caldera_depth_m: src.caldera_depth_m,
        secondary_vents: src.secondary_vents,
        ridge_count: src.ridge_count,
    }
}

fn compile_climate(src: &ClimateRecipeSource) -> CompiledClimateRecipe {
    CompiledClimateRecipe {
        id: src.id.clone(),
        base_temperature_c: src.base_temperature_c,
        lapse_rate_c_per_km: src.lapse_rate_c_per_km,
        prevailing_wind_direction_deg: src.prevailing_wind.direction_deg,
        prevailing_wind_strength: src.prevailing_wind.strength,
        prevailing_wind_moisture: src.prevailing_wind.moisture,
        ocean_recharge: src.rainfall.ocean_recharge,
        orographic_factor: src.rainfall.orographic_factor,
        rain_shadow_factor: src.rainfall.rain_shadow_factor,
    }
}

fn compile_hydrology(src: &HydrologyRecipeSource) -> CompiledHydrologyRecipe {
    CompiledHydrologyRecipe {
        id: src.id.clone(),
        routing: src.routing.clone(),
        rainfall_weight: src.rainfall_weight,
        stream_threshold: src.stream_threshold,
        permanent_river_threshold: src.permanent_river_threshold,
        minimum_stream_length_m: src.minimum_stream_length_m,
        lake_min_area_cells: src.lake_min_area_cells,
        wetland_moisture_threshold: src.wetland_moisture_threshold,
        waterfall_min_drop_m: src.waterfall_min_drop_m,
        waterfall_min_discharge: src.waterfall_min_discharge,
    }
}

fn compile_erosion(src: &ErosionRecipeSource) -> CompiledErosionRecipe {
    CompiledErosionRecipe {
        id: src.id.clone(),
        iterations: src.iterations,
        stream_power: CompiledStreamPowerErosion {
            m: src.stream_power.m,
            n: src.stream_power.n,
            maximum_step_m: src.stream_power.maximum_step_m,
            iterations_per_cycle: src.stream_power.iterations_per_cycle,
            erodibility: src.stream_power.erodibility,
        },
        thermal: CompiledThermalErosion {
            talus_deg: src.thermal.talus_deg,
            transfer_rate: src.thermal.transfer_rate,
            iterations_per_cycle: src.thermal.iterations_per_cycle,
        },
        sediment: CompiledSedimentErosion {
            pickup_rate: src.sediment.pickup_rate,
            transport_rate: src.sediment.transport_rate,
            deposition_rate: src.sediment.deposition_rate,
            capacity_factor: src.sediment.capacity_factor,
        },
    }
}

fn compile_coast(src: &CoastRecipeSource) -> CompiledCoastRecipe {
    CompiledCoastRecipe {
        id: src.id.clone(),
        beach_width_max_m: src.beaches.width_max_m,
        beach_max_slope_deg: src.beaches.maximum_slope_deg,
        berm_height_min_m: src.beaches.berm_height_m[0],
        berm_height_max_m: src.beaches.berm_height_m[1],
        cliff_min_slope_deg: src.cliffs.minimum_slope_deg,
        cliff_min_exposure: src.cliffs.minimum_exposure,
        reef_min_age_myr: src.reefs.min_age_myr,
        reef_depth_min_m: src.reefs.depth_m[0],
        reef_depth_max_m: src.reefs.depth_m[1],
        reef_max_sediment: src.reefs.max_sediment,
        reef_min_temperature: src.reefs.min_temperature,
        lagoon_max_depth_m: src.lagoons.max_depth_m,
        lagoon_reef_enclosure_min: src.lagoons.reef_enclosure_min,
        mangrove_max_slope_deg: src.mangroves.max_slope_deg,
        mangrove_salinity_min_m: src.mangroves.salinity_band_m[0],
        mangrove_salinity_max_m: src.mangroves.salinity_band_m[1],
    }
}

fn compile_biomes(src: &BiomeRecipeSource) -> CompiledBiomeRecipe {
    CompiledBiomeRecipe {
        id: src.id.clone(),
        cloud_forest_elevation_min_m: src.land.cloud_forest_elevation_m[0],
        cloud_forest_elevation_max_m: src.land.cloud_forest_elevation_m[1],
        dry_forest_rainfall_max: src.land.dry_forest_rainfall_max,
        montane_shrub_elevation_m: src.land.montane_shrub_elevation_m,
        volcanic_barren_slope_deg: src.land.volcanic_barren_slope_deg,
        wetland_moisture_min: src.land.wetland_moisture_min,
        reef_depth_min_m: src.marine.reef_depth_m[0],
        reef_depth_max_m: src.marine.reef_depth_m[1],
        shelf_depth_m: src.marine.shelf_depth_m,
        deep_coastal_depth_m: src.marine.deep_coastal_depth_m,
    }
}

fn compile_strata(src: &StrataRecipeSource) -> CompiledStrataRecipe {
    let layers = src
        .layers
        .iter()
        .map(|layer| {
            let (thickness_min_m, thickness_max_m, remaining) = match &layer.thickness_m {
                StrataThicknessSource::Range(r) => (r[0], r[1], false),
                StrataThicknessSource::Remaining(s) => {
                    (0.0, 0.0, s.eq_ignore_ascii_case("remaining"))
                }
            };
            let requires_vegetated = layer
                .requires
                .as_ref()
                .map(|r| r.biome_tags.iter().any(|t| t == "vegetated"))
                .unwrap_or(false);
            let driven = |name: &str| layer.driven_by.iter().any(|d| d == name);
            CompiledStrataLayer {
                material: layer.material.clone(),
                thickness_min_m,
                thickness_max_m,
                remaining,
                requires_vegetated,
                driven_by_rainfall: driven("rainfall"),
                driven_by_slope: driven("slope"),
                driven_by_biome: driven("biome"),
                driven_by_age: driven("geological_age"),
            }
        })
        .collect();
    let deposits = src
        .deposits
        .iter()
        .map(|d| CompiledStrataDeposit {
            id: d.id.clone(),
            mask: d.mask.clone(),
            thickness_min_m: d.thickness_m[0],
            thickness_max_m: d.thickness_m[1],
        })
        .collect();
    CompiledStrataRecipe {
        id: src.id.clone(),
        layers,
        deposits,
    }
}

fn compile_cave_family(src: &CaveFamilyProfileSource) -> CompiledCaveFamilyProfile {
    CompiledCaveFamilyProfile {
        systems_max: src.systems_max,
        chamber_count_min: src.chamber_count_min,
        chamber_count_max: src.chamber_count_max,
        passage_radius_min_m: src.passage_radius_min_m,
        passage_radius_max_m: src.passage_radius_max_m,
        minimum_cover_m: src.minimum_cover_m,
        maximum_depth_m: src.maximum_depth_m,
        entrance_threshold: src.entrance_threshold,
        overhang_enabled: src.overhang_enabled,
    }
}

fn compile_caves(src: &CavesRecipeSource) -> CompiledCavesRecipe {
    CompiledCavesRecipe {
        id: src.id.clone(),
        lava_max_age_myr: src.lava_max_age_myr,
        limestone_min_permeability: src.limestone_min_permeability,
        sea_tidal_band_m: src.sea_tidal_band_m,
        lava_tube: compile_cave_family(&src.lava_tube),
        limestone: compile_cave_family(&src.limestone),
        sea_cave: compile_cave_family(&src.sea_cave),
    }
}
