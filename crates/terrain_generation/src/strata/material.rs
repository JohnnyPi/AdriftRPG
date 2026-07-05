//! Strata / column material identifiers.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum StrataMaterialId {
    #[default]
    Basalt = 0,
    OrganicSoil = 1,
    Topsoil = 2,
    Subsoil = 3,
    WeatheredBasalt = 4,
    BeachSand = 5,
    RiverSediment = 6,
    CoralLimestone = 7,
    Peat = 8,
    VolcanicAsh = 9,
    Clay = 10,
    Tuff = 11,
    Limestone = 12,
}

impl StrataMaterialId {
    pub fn from_material_name(name: &str) -> Self {
        match name {
            "organic_soil" => Self::OrganicSoil,
            "topsoil" => Self::Topsoil,
            "subsoil" => Self::Subsoil,
            "weathered_basalt" | "weathered_rock" => Self::WeatheredBasalt,
            "basalt" => Self::Basalt,
            "beach_sand" => Self::BeachSand,
            "river_sediment" => Self::RiverSediment,
            "coral_limestone" => Self::CoralLimestone,
            "peat" => Self::Peat,
            "volcanic_ash" => Self::VolcanicAsh,
            "clay" => Self::Clay,
            "tuff" => Self::Tuff,
            "limestone" => Self::Limestone,
            _ => Self::Basalt,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Basalt,
            1 => Self::OrganicSoil,
            2 => Self::Topsoil,
            3 => Self::Subsoil,
            4 => Self::WeatheredBasalt,
            5 => Self::BeachSand,
            6 => Self::RiverSediment,
            7 => Self::CoralLimestone,
            8 => Self::Peat,
            9 => Self::VolcanicAsh,
            10 => Self::Clay,
            11 => Self::Tuff,
            12 => Self::Limestone,
            _ => Self::Basalt,
        }
    }
}

impl From<StrataMaterialId> for u8 {
    fn from(value: StrataMaterialId) -> Self {
        value as u8
    }
}
