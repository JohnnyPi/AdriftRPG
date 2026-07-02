// crates/terrain_generation/tests/coordinates.rs
use terrain_generation::{default_vertical_slice_recipe, RecipeDensitySource};
use voxel_core::{ChunkCoord, WorldCell};

#[test]
fn density_at_world_matches_density_at_recipe_after_offset() {
    let mut recipe = default_vertical_slice_recipe(42, 10.0);
    recipe.coord_offset = [128.0, 0.0, 128.0];
    let source = RecipeDensitySource::new(recipe);

    let world = (12.0, 25.0, -4.0);
    let recipe_x = world.0 + source.recipe().coord_offset[0];
    let recipe_y = world.1 + source.recipe().coord_offset[1];
    let recipe_z = world.2 + source.recipe().coord_offset[2];

    let from_world = source.density_at(world.0, world.1, world.2);
    let from_recipe = source.density_at_recipe(recipe_x, recipe_y, recipe_z);
    assert!((from_world - from_recipe).abs() < 1e-4);
}

#[test]
fn spawn_world_position_maps_to_expected_chunk() {
    let mut recipe = default_vertical_slice_recipe(7, 10.0);
    recipe.spawn_x = 128.0 + 16.0;
    recipe.spawn_z = 128.0 - 32.0;
    recipe.coord_offset = [128.0, 0.0, 128.0];
    let source = RecipeDensitySource::new(recipe);

    let (x, _y, z) = source.spawn_position();
    let cell = WorldCell::new(x.floor() as i32, 0, z.floor() as i32);
    assert_eq!(cell.chunk_coord(), ChunkCoord::new(1, 0, -2));
}

#[test]
fn surface_height_world_matches_recipe_conversion() {
    let mut recipe = default_vertical_slice_recipe(99, 10.0);
    recipe.coord_offset = [128.0, 0.0, 128.0];
    let source = RecipeDensitySource::new(recipe);

    let wx = 0.0;
    let wz = 0.0;
    let from_world = source.surface_height_at(wx, wz);
    let from_recipe = source.surface_height_at_recipe(
        wx + source.recipe().coord_offset[0],
        wz + source.recipe().coord_offset[2],
    );
    assert!((from_world - from_recipe).abs() < 1e-3);
}
