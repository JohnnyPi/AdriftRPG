//! Graph products stored on the world atlas.

use crate::biomes::BiomeBlendCell;
use crate::caves::graph::CaveGraphRegistry;
use crate::caves::sdf::CaveSubtractOps;
use crate::hydrology::graph::HydrologyGraph;
use crate::hydrology::realize::CompiledHydrologyProducts;

#[derive(Clone, Debug, Default)]
pub struct BiomeGrid {
    pub width: u32,
    pub height: u32,
    pub cells: Vec<BiomeBlendCell>,
}

#[derive(Clone, Debug, Default)]
pub struct GraphRegistry {
    pub hydrology: Option<HydrologyGraph>,
    pub biome: Option<BiomeGrid>,
    pub cave_systems: Option<CaveGraphRegistry>,
    pub cave_subtract_ops: Option<CaveSubtractOps>,
    pub hydrology_products: Option<CompiledHydrologyProducts>,
}
