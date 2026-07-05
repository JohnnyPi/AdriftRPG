//! Compiler biome identifiers (Milestone C).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum CompilerBiomeId {
    #[default]
    OpenOcean = 0,
    Grassland = 1,
    Forest = 2,
    Scrub = 3,
    CoastalScrub = 4,
    Wetland = 5,
    Beach = 6,
    Alpine = 7,
    RockyUpland = 8,
    Cave = 9,
    Riverbank = 10,
    ShallowWater = 11,
    DeepWater = 12,
    OffshoreShelf = 13,
    Mangrove = 14,
    CloudForest = 15,
    DryForest = 16,
    MontaneShrub = 17,
    VolcanicBarren = 18,
    Swamp = 19,
    FreshwaterWetland = 20,
    RockyCliff = 21,
    Intertidal = 22,
    Lagoon = 23,
    CoralReef = 24,
    ReefSlope = 25,
    SeagrassBed = 26,
    ContinentalShelf = 27,
    DeepCoastalWater = 28,
    AbyssalBasin = 29,
    HydrothermalZone = 30,
}

impl CompilerBiomeId {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::OpenOcean,
            1 => Self::Grassland,
            2 => Self::Forest,
            3 => Self::Scrub,
            4 => Self::CoastalScrub,
            5 => Self::Wetland,
            6 => Self::Beach,
            7 => Self::Alpine,
            8 => Self::RockyUpland,
            9 => Self::Cave,
            10 => Self::Riverbank,
            11 => Self::ShallowWater,
            12 => Self::DeepWater,
            13 => Self::OffshoreShelf,
            14 => Self::Mangrove,
            15 => Self::CloudForest,
            16 => Self::DryForest,
            17 => Self::MontaneShrub,
            18 => Self::VolcanicBarren,
            19 => Self::Swamp,
            20 => Self::FreshwaterWetland,
            21 => Self::RockyCliff,
            22 => Self::Intertidal,
            23 => Self::Lagoon,
            24 => Self::CoralReef,
            25 => Self::ReefSlope,
            26 => Self::SeagrassBed,
            27 => Self::ContinentalShelf,
            28 => Self::DeepCoastalWater,
            29 => Self::AbyssalBasin,
            30 => Self::HydrothermalZone,
            _ => Self::Grassland,
        }
    }

    pub fn is_vegetated(self) -> bool {
        matches!(
            self,
            Self::Grassland
                | Self::Forest
                | Self::Scrub
                | Self::CoastalScrub
                | Self::Wetland
                | Self::Mangrove
                | Self::CloudForest
                | Self::DryForest
                | Self::MontaneShrub
                | Self::Swamp
                | Self::FreshwaterWetland
        )
    }

    pub fn is_land(self) -> bool {
        !matches!(
            self,
            Self::OpenOcean
                | Self::ShallowWater
                | Self::DeepWater
                | Self::OffshoreShelf
                | Self::Intertidal
                | Self::Lagoon
                | Self::CoralReef
                | Self::ReefSlope
                | Self::SeagrassBed
                | Self::ContinentalShelf
                | Self::DeepCoastalWater
                | Self::AbyssalBasin
                | Self::HydrothermalZone
        )
    }
}

impl From<CompilerBiomeId> for u8 {
    fn from(value: CompilerBiomeId) -> Self {
        value as u8
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BiomeBlendCell {
    pub primary: CompilerBiomeId,
    pub primary_weight: f32,
    pub secondary: CompilerBiomeId,
    pub secondary_weight: f32,
}
