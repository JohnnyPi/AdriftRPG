//! World atlas — aligned field products from compiler passes.

use game_data::CompiledWorldRecipe;

use crate::contract::coordinates::WorldXZ;
use crate::contract::metadata::{RecipeHash, WorldExtent, WorldMetadata};
use crate::fields::descriptor::FieldDescriptor;
use crate::fields::key::FieldKey;
use crate::fields::registry::FieldRegistry;
use crate::islands::seed::IslandBlueprint;

use super::graphs::GraphRegistry;

#[derive(Clone)]
pub struct WorldAtlas {
    pub metadata: WorldMetadata,
    pub fields: FieldRegistry,
    pub graphs: GraphRegistry,
    pub islands: Vec<IslandBlueprint>,
    pub control_descriptor: FieldDescriptor,
}

impl WorldAtlas {
    pub fn new(recipe: &CompiledWorldRecipe) -> Self {
        let extent = WorldExtent {
            width_m: recipe.extent.width_m,
            depth_m: recipe.extent.depth_m,
            vertical_min_m: recipe.extent.vertical_min_m,
            vertical_max_m: recipe.extent.vertical_max_m,
            sea_level_m: recipe.extent.sea_level_m,
        };
        let origin = WorldXZ::new(-recipe.extent.width_m * 0.5, -recipe.extent.depth_m * 0.5);
        let cell = recipe.resolutions.regional_cell_m;
        let width = (recipe.extent.width_m / cell).ceil() as u32;
        let height = (recipe.extent.depth_m / cell).ceil() as u32;
        let control_descriptor = FieldDescriptor::new(
            FieldKey::BaseElevation,
            origin,
            cell,
            width.max(2),
            height.max(2),
        );

        Self {
            metadata: WorldMetadata::new(
                recipe.id.clone(),
                recipe.id.clone(),
                RecipeHash::from_bytes(recipe.recipe_hash.0),
                recipe.seed,
                extent,
            ),
            fields: FieldRegistry::new(),
            graphs: GraphRegistry::default(),
            islands: Vec::new(),
            control_descriptor,
        }
    }
}
