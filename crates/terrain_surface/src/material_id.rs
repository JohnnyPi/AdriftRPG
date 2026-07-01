use procedural_textures::TerrainMaterialIdName;

#[repr(u16)]
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum TerrainMaterialId {
    FreshBasalt = 0,
    WeatheredBasalt = 1,
    CaveBasalt = 2,
    TropicalRedSoil = 3,
    JungleLoam = 4,
    JungleMoss = 5,
    LeafLitter = 6,
    CoralSand = 7,
    BlackSand = 8,
    CoralRubble = 9,
    RiverGravel = 10,
    RiverSilt = 11,
    Mud = 12,
    Limestone = 13,
    Flowstone = 14,
    VolcanicAsh = 15,
}

impl TerrainMaterialId {
    pub fn from_name(name: &TerrainMaterialIdName) -> Self {
        match name {
            TerrainMaterialIdName::FreshBasalt => Self::FreshBasalt,
            TerrainMaterialIdName::WeatheredBasalt => Self::WeatheredBasalt,
            TerrainMaterialIdName::CaveBasalt => Self::CaveBasalt,
            TerrainMaterialIdName::TropicalRedSoil => Self::TropicalRedSoil,
            TerrainMaterialIdName::JungleLoam => Self::JungleLoam,
            TerrainMaterialIdName::JungleMoss => Self::JungleMoss,
            TerrainMaterialIdName::LeafLitter => Self::LeafLitter,
            TerrainMaterialIdName::CoralSand => Self::CoralSand,
            TerrainMaterialIdName::BlackSand => Self::BlackSand,
            TerrainMaterialIdName::CoralRubble => Self::CoralRubble,
            TerrainMaterialIdName::RiverGravel => Self::RiverGravel,
            TerrainMaterialIdName::RiverSilt => Self::RiverSilt,
            TerrainMaterialIdName::Mud => Self::Mud,
            TerrainMaterialIdName::Limestone => Self::Limestone,
            TerrainMaterialIdName::Flowstone => Self::Flowstone,
            TerrainMaterialIdName::VolcanicAsh => Self::VolcanicAsh,
        }
    }
}

pub const CORE_TERRAIN_MATERIALS: &[TerrainMaterialId] = &[
    TerrainMaterialId::FreshBasalt,
    TerrainMaterialId::WeatheredBasalt,
    TerrainMaterialId::TropicalRedSoil,
    TerrainMaterialId::JungleLoam,
    TerrainMaterialId::JungleMoss,
    TerrainMaterialId::CoralSand,
    TerrainMaterialId::RiverGravel,
    TerrainMaterialId::RiverSilt,
];

pub const INITIAL_ISLAND_LAYERS: &[TerrainMaterialId] = CORE_TERRAIN_MATERIALS;
