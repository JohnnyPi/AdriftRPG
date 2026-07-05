//! Helpers for compiled cave recipe profiles.

use game_data::CompiledCavesRecipe;

use super::graph::CaveFamily;

pub fn profile_for<'a>(
    recipe: &'a CompiledCavesRecipe,
    family: CaveFamily,
) -> &'a game_data::CompiledCaveFamilyProfile {
    match family {
        CaveFamily::LavaTube => &recipe.lava_tube,
        CaveFamily::Limestone => &recipe.limestone,
        CaveFamily::SeaCave => &recipe.sea_cave,
        CaveFamily::Fracture | CaveFamily::Talus => &recipe.lava_tube,
    }
}
