# Small world test guide

**Presentation ID:** `world.small`  
**Worldgen ID:** `world.small`  
**Extent:** 8192×8192 m (1× baseline area)

## Purpose

Primary dev and play world at 8 km span. Validates complete tropical volcanic island
generation through Milestones A–C.

## Terrain / worldgen features

- Bounded ocean + single centered shield volcano (`island.volcanic_small`)
- Macro elevation, bathymetry, geology, regional refinement
- Trade-wind climate; windward ≥ leeward forest
- Fluvial hydrology: permanent rivers to ocean, lake/wetland products
- Erosion and sediment fields
- Coast: beaches, reef suitability, marine exposure
- Biomes, soil, strata/regolith depth
- Caves and water carve passes

## Engine / runtime features

- Worldgen compilation on world load (`WorldgenPlugin`)
- Terrain density from compiled atlas
- Material catalog + procedural textures (`catalogs.tropical_island`)
- Chunk LOD rings, ocean tile grid, vegetation vertical slice

## Acceptance checklist

- [ ] Flyover: single volcanic peak ~1200 m, coherent shield slopes
- [ ] ≥1 permanent river reaches ocean (`validation.small`)
- [ ] Reef/coast band visible on windward shore
- [ ] Biome variation windward vs leeward
- [ ] Walkable cave entrance on flank
- [ ] Compile and enter world in Bevy without atlas errors

## Implementation

- Worldgen: [`assets/worldgen/worlds/small.world.yaml`](../../assets/worldgen/worlds/small.world.yaml)
- Presentation: [`assets/terrain/worlds/small_world.yaml`](../../assets/terrain/worlds/small_world.yaml)
- Tests: `milestone_a` (golden variants), `milestone_b`, `milestone_c`

Default world in [`assets/config/app.yaml`](../../assets/config/app.yaml).
