// crates/game_data/tests/coordinates.rs
use game_data::{
    CompiledChunkResidency, CompiledChunkStaging, CompiledWorld, CompiledWorldLod, RawDefinition,
    WorldChunksDefinition, WorldDefinition, WorldVoxelDefinition, validate_definitions,
};
use shared::{DataError, DefinitionHeader, StableId};

#[test]
fn recipe_to_world_subtracts_coord_offset() {
    let world = CompiledWorld {
        id: StableId::new("world.test"),
        seed: 1,
        cell_size_m: 1.0,
        chunk_cells: [16, 16, 16],
        world_extent_chunks: [6, 3, 6],
        terrain: StableId::new("terrain.test"),
        biomes: StableId::new("biomes.test"),
        materials: StableId::new("materials.test"),
        surface: StableId::new("surface.test"),
        water: StableId::new("water.test"),
        lighting: StableId::new("lighting.test"),
        sky: None,
        landmarks: None,
        structures: vec![],
        ocean_extent_m: Some(256.0),
        coord_offset: [128.0, 0.0, 128.0],
        island_gen: None,
        resolution: None,
        island_atlas_baked: None,
        hydrology_bodies: Vec::new(),
        material_catalog: None,
        vegetation: None,
        weather: None,
        residency: CompiledChunkResidency::default(),
        lod: CompiledWorldLod::default(),
        staging: CompiledChunkStaging::default(),
    };

    let recipe = [140.0, 50.0, 132.0];
    let world_pos = world.recipe_to_world(recipe);
    assert!((world_pos[0] - 12.0).abs() < f32::EPSILON);
    assert!((world_pos[1] - 50.0).abs() < f32::EPSILON);
    assert!((world_pos[2] - 4.0).abs() < f32::EPSILON);
}

#[test]
fn rejects_non_unit_cell_size() {
    let report = validate_definitions(&[RawDefinition::World(WorldDefinition {
        header: DefinitionHeader {
            schema_version: 1,
            id: StableId::new("world.bad_cell"),
        },
        seed: 1,
        voxel: WorldVoxelDefinition { cell_size_m: 0.5 },
        chunks: WorldChunksDefinition {
            cells: [16, 16, 16],
            world_extent: [6, 3, 6],
            residency: Default::default(),
            lod: Default::default(),
            staging: Default::default(),
        },
        terrain: StableId::new("terrain.test"),
        biomes: StableId::new("biomes.test"),
        materials: StableId::new("materials.test"),
        surface: None,
        water: StableId::new("water.test"),
        lighting: StableId::new("lighting.test"),
        sky: None,
        landmarks: None,
        structures: vec![],
        ocean_extent_m: None,
        coord_offset: None,
        island_gen: None,
        resolution: None,
        island_atlas_baked: None,
        hydrology_bodies: Vec::new(),
        material_catalog: None,
        vegetation: None,
        weather: None,
    })]);

    assert!(!report.is_ok());
    let result = report.into_result();
    assert!(matches!(result, Err(DataError::ValidationFailed { .. })));
    let err = result.unwrap_err().to_string();
    assert!(err.contains("cell_size_m must be 1.0"));
}
