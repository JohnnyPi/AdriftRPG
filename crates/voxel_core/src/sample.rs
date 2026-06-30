#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialId(pub u16);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TerrainSample {
    pub density: f32,
    pub material: MaterialId,
}
