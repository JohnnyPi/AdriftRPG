//! Mutable compilation context shared across passes.

use game_data::CompiledWorldRecipe;

use crate::islands::seed::{IslandBlueprint, IslandSeed};
use crate::islands::skeleton::IslandSkeleton;
use crate::world::atlas::WorldAtlas;

pub struct CompileContext<'a> {
    pub recipe: &'a CompiledWorldRecipe,
    pub atlas: WorldAtlas,
    pub island_seed: Option<IslandSeed>,
    pub skeleton: Option<IslandSkeleton>,
    pub blueprints: Vec<IslandBlueprint>,
}

impl<'a> CompileContext<'a> {
    pub fn new(recipe: &'a CompiledWorldRecipe) -> Self {
        Self {
            recipe,
            atlas: WorldAtlas::new(recipe),
            island_seed: None,
            skeleton: None,
            blueprints: Vec::new(),
        }
    }
}
