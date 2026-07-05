//! Tropical trade-wind climate model.

use game_data::CompiledClimateRecipe;

use crate::fields::scalar::ScalarField;

pub struct ClimateFields {
    pub temperature: ScalarField,
    pub rainfall: ScalarField,
    pub humidity: ScalarField,
    pub evaporation: ScalarField,
    pub wind_exposure: ScalarField,
}

pub fn compute_climate_fields(
    elevation: &ScalarField,
    coast_distance: &ScalarField,
    land_mask: &ScalarField,
    slope: &ScalarField,
    recipe: &CompiledClimateRecipe,
) -> ClimateFields {
    let desc = elevation.descriptor.clone();
    let mut temperature = ScalarField::zeros(desc.clone());
    let mut rainfall = ScalarField::zeros(desc.clone());
    let mut humidity = ScalarField::zeros(desc.clone());
    let mut evaporation = ScalarField::zeros(desc.clone());
    let mut wind_exposure = ScalarField::zeros(desc.clone());

    let wind_dir_rad = recipe.prevailing_wind_direction_deg.to_radians();
    let wind_vec = glam::Vec2::new(wind_dir_rad.cos(), wind_dir_rad.sin());
    let wind_strength = recipe.prevailing_wind_strength;
    let wind_moisture = recipe.prevailing_wind_moisture;

    for z in 0..desc.height {
        for x in 0..desc.width {
            let elev = elevation.get(x, z);
            let land = land_mask.get(x, z);
            let coast = coast_distance.get(x, z).max(0.0);
            let slope_deg = slope.get(x, z);

            let elev_km = (elev.max(0.0)) / 1000.0;
            let temp_c = recipe.base_temperature_c - recipe.lapse_rate_c_per_km * elev_km;
            let temp_norm = ((temp_c - 10.0) / 25.0).clamp(0.0, 1.0);
            temperature.set(x, z, temp_norm);

            let ocean_recharge = if land > 0.3 {
                (1.0 - (coast / 3000.0).clamp(0.0, 1.0)) * recipe.ocean_recharge
                    + wind_moisture * 0.3
            } else {
                recipe.ocean_recharge * 2.0
            };

            let slope_rad = slope_deg.to_radians();
            let upslope = (slope_rad.sin() * wind_vec.x + slope_rad.cos() * wind_vec.y).max(0.0);
            wind_exposure.set(x, z, (upslope * wind_strength).clamp(0.0, 1.0));

            let orographic = 1.0 + wind_exposure.get(x, z) * recipe.orographic_factor;
            let rain_shadow =
                1.0 - (1.0 - wind_exposure.get(x, z)) * recipe.rain_shadow_factor * 0.5;
            let rain = ocean_recharge * orographic * rain_shadow * land.max(0.1);
            rainfall.set(x, z, rain.clamp(0.0, 1.0));

            let evap = temp_norm * 0.04 * (1.0 - rain * 0.3);
            evaporation.set(x, z, evap);

            let hum = (rain * 0.55 + ocean_recharge * 0.35 - evap).clamp(0.0, 1.0);
            humidity.set(x, z, hum);
        }
    }

    ClimateFields {
        temperature,
        rainfall,
        humidity,
        evaporation,
        wind_exposure,
    }
}
