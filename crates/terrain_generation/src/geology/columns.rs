//! Geological column sampling from compiled strata fields.

use game_data::CompiledStrataRecipe;

use crate::contract::coordinates::WorldXZ;
use crate::fields::scalar::ScalarField;
use crate::strata::material::StrataMaterialId;

pub struct ColumnSampler {
    pub regolith_depth: ScalarField,
    pub weathering_depth: ScalarField,
    pub soil_depth: ScalarField,
    pub deposit: ScalarField,
    pub recipe: CompiledStrataRecipe,
}

impl ColumnSampler {
    pub fn sample_material(
        &self,
        horizontal: WorldXZ,
        depth_below_surface_m: f32,
    ) -> StrataMaterialId {
        let (x, z) = self.grid_coords(horizontal);
        let deposit_id = self.deposit.get(x, z).round() as u8;
        if deposit_id > 0 && depth_below_surface_m < self.deposit_thickness(x, z) {
            return StrataMaterialId::from_u8(deposit_id);
        }

        let soil = self.soil_depth.get(x, z);
        let regolith = self.regolith_depth.get(x, z);
        let weathering = self.weathering_depth.get(x, z);

        if depth_below_surface_m <= soil * 0.15 {
            return StrataMaterialId::OrganicSoil;
        }
        if depth_below_surface_m <= soil {
            return StrataMaterialId::Topsoil;
        }
        if depth_below_surface_m <= soil + regolith {
            return StrataMaterialId::Subsoil;
        }
        if depth_below_surface_m <= soil + regolith + weathering {
            return StrataMaterialId::WeatheredBasalt;
        }
        StrataMaterialId::Basalt
    }

    fn deposit_thickness(&self, x: u32, z: u32) -> f32 {
        let deposit = StrataMaterialId::from_u8(self.deposit.get(x, z).round() as u8);
        self.recipe
            .deposits
            .iter()
            .find(|d| StrataMaterialId::from_material_name(&d.id) == deposit)
            .map(|d| (d.thickness_min_m + d.thickness_max_m) * 0.5)
            .unwrap_or(0.5)
    }

    fn grid_coords(&self, horizontal: WorldXZ) -> (u32, u32) {
        let d = &self.soil_depth.descriptor;
        let lx = (horizontal.x() - d.origin_x()) / d.cell_size_m;
        let lz = (horizontal.z() - d.origin_z()) / d.cell_size_m;
        let x = (lx.round() as i64).clamp(0, d.width.saturating_sub(1) as i64) as u32;
        let z = (lz.round() as i64).clamp(0, d.height.saturating_sub(1) as i64) as u32;
        (x, z)
    }
}

pub fn deposit_id_from_mask_name(name: &str) -> StrataMaterialId {
    match name {
        "beach_suitability" => StrataMaterialId::BeachSand,
        "river_corridor" => StrataMaterialId::RiverSediment,
        "reef_suitability" => StrataMaterialId::CoralLimestone,
        "wetland" => StrataMaterialId::Peat,
        _ => StrataMaterialId::Basalt,
    }
}
