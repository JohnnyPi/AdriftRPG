# Phased Expansion Plan

## Vertical Slice → Full Data-Driven Tropical Island Generator

## 1. Target outcome

Expand the current vertical slice into a reusable tropical island world generator capable of producing:

* single islands;
* island chains;
* archipelagos;
* volcanic islands of different ages;
* beaches, cliffs, lagoons, reefs, and shelves;
* mountains, calderas, valleys, rivers, wetlands, and waterfalls;
* caves, lava tubes, sea caves, and overhangs;
* terrestrial and marine biomes;
* geological strata and material variation;
* vegetation and ecological zones;
* procedural structures and points of interest;
* deterministic generation from YAML recipes;
* streamed voxel realization using the existing `16 × 16 × 16` signed-density chunks.

The full system should be treated as a **world compiler**, not as one giant noise function:

```text
YAML world recipe
→ validated typed configuration
→ world-scale fields
→ regional geography
→ climate and hydrology
→ erosion and coastal processing
→ biome and ecology classification
→ features and structures
→ signed-density voxel materialization
→ chunk meshes, colliders, simulation data
```

This expands directly from the existing signed-density foundation rather than replacing it. The broad island shape should be generated efficiently with aligned 2D fields, while caves, overhangs, tunnels, and editable terrain remain fully volumetric. 

---

# 2. Architectural principles

## 2.1 Keep the voxel runtime unchanged

Retain:

```text
Voxel cell size: 1 meter
Chunk size: 16 × 16 × 16 cells
Density samples: 17 × 17 × 17
Surface extraction: Surface Nets initially
```

The island generator should not produce chunk meshes directly.

It should produce world data that the voxel runtime samples:

```text
elevation
geology
soil depth
biome
water level
cave fields
feature masks
structure edits
```

Then the voxel materializer converts those fields into density and material samples.

## 2.2 Separate generation resolutions

Use several resolutions rather than running every algorithm at voxel resolution.

| Resolution    |      Typical scale | Responsibility                                 |
| ------------- | -----------------: | ---------------------------------------------- |
| World control | 250–2,000 m/sample | Island chains, ocean basin, climate regions    |
| Regional      |    16–128 m/sample | Mountains, valleys, watersheds, coastline      |
| Local surface |       1–8 m/sample | Beaches, gullies, cliffs, river channels       |
| Voxel         |           1 m/cell | Caves, overhangs, materials, edits, structures |

## 2.3 Use aligned fields as the common language

The generator should maintain a `WorldAtlas` containing aligned maps and graphs.

```rust
pub struct WorldAtlas {
    pub metadata: WorldMetadata,

    pub elevation: ScalarField,
    pub bathymetry: ScalarField,
    pub slope: ScalarField,
    pub curvature: ScalarField,

    pub rainfall: ScalarField,
    pub temperature: ScalarField,
    pub moisture: ScalarField,
    pub evaporation: ScalarField,

    pub volcanic_age: ScalarField,
    pub rock_hardness: ScalarField,
    pub erodibility: ScalarField,

    pub land: MaskField,
    pub ocean: MaskField,
    pub shelf: MaskField,
    pub river: MaskField,
    pub lake: MaskField,
    pub wetland: MaskField,
    pub cliff: MaskField,
    pub reef: MaskField,

    pub geology: CategoricalField<GeologyId>,
    pub biome: CategoricalField<BiomeId>,

    pub wind: VectorField,
    pub hydrology: HydrologyGraph,
    pub features: FeatureRegistry,
}
```

The full terrain design already supports this field-based, pass-oriented architecture and should remain the organizing model. 

## 2.4 Use deterministic pass-local random streams

Every result should derive from:

```text
world seed
+ pass ID
+ region or tile coordinate
+ feature ID
+ generation version
```

Do not consume one global mutable RNG across the whole pipeline.

## 2.5 Generate meaning before voxel detail

Each pass should create interpretable world information.

Examples:

```text
This is an old eroded island.
This ridge is volcanic.
This valley carries a major river.
This coast is sheltered.
This shelf supports a reef.
This region is cloud forest.
This cave is a lava tube.
```

Voxel density should be the final realization of those meanings.

---

# 3. Proposed module layout

```text
crates/
├── voxel_core/
├── terrain_meshing/
├── terrain_runtime/
├── worldgen_core/
├── worldgen_fields/
├── worldgen_pipeline/
├── worldgen_geography/
├── worldgen_geology/
├── worldgen_climate/
├── worldgen_hydrology/
├── worldgen_erosion/
├── worldgen_coasts/
├── worldgen_biomes/
├── worldgen_features/
├── worldgen_ecology/
├── worldgen_structures/
├── worldgen_voxelization/
├── worldgen_storage/
├── game_data/
└── game_bevy/
```

This may begin as modules inside fewer crates, but the dependency direction should remain clear.

```text
worldgen definitions and algorithms
        ↓
world atlas
        ↓
voxel materialization
        ↓
runtime chunks
        ↓
Bevy rendering and gameplay
```

---

# 4. Phase 0 — Formalize the world-generation contract

## Goal

Convert the current vertical-slice terrain generator into a stable interface that can be replaced by a larger world recipe without affecting the voxel runtime.

## Implement

Define:

```rust
pub trait WorldDensityProvider {
    fn density_at(&self, position: DVec3) -> f32;
    fn material_at(&self, position: DVec3) -> MaterialId;
    fn biome_at(&self, position: DVec3) -> BiomeId;
    fn water_at(&self, position: DVec3) -> Option<WaterSample>;
}
```

Add:

* world-space coordinate conventions;
* sea-level convention;
* chunk-to-world mapping;
* field sampling conventions;
* world bounds;
* generation version;
* recipe hash;
* deterministic seed derivation;
* pass dependency rules.

## Deliverable

The existing vertical-slice terrain is accessed through the same interface that the future island generator will use.

## Exit gate

* no rendering code depends on the vertical-slice generator implementation;
* no physics code depends on terrain-generation details;
* identical seed and recipe produce identical chunk-density hashes.

---

# 5. Phase 1 — Typed YAML world recipes

## Goal

Replace the small collection of slice-specific settings with a composable world-definition system.

## Directory structure

```text
assets/worldgen/
├── worlds/
│   ├── tropical_single_island.world.yaml
│   └── tropical_archipelago.world.yaml
├── pipelines/
│   └── tropical_islands.pipeline.yaml
├── geography/
│   ├── volcanic_hotspot_chain.yaml
│   └── isolated_volcanic_island.yaml
├── geology/
│   └── basaltic_tropical.yaml
├── climate/
│   └── tropical_trade_winds.yaml
├── hydrology/
│   └── tropical_hydrology.yaml
├── erosion/
│   └── tropical_volcanic_erosion.yaml
├── coasts/
│   └── tropical_coasts.yaml
├── biomes/
│   ├── tropical_land.biomes.yaml
│   └── tropical_marine.biomes.yaml
├── caves/
│   └── volcanic_caves.yaml
├── ecology/
│   └── tropical_ecology.yaml
└── voxelization/
    └── tropical_voxels.yaml
```

## World recipe

```yaml
schema_version: 1
id: world.tropical_archipelago

seed: 48129

extent:
  width_m: 131072
  depth_m: 131072
  vertical_min_m: -2048
  vertical_max_m: 4096
  boundary: bounded_ocean

resolution:
  world_control_m: 512
  regional_m: 32
  local_m: 4
  voxel_m: 1

pipeline: pipeline.tropical_islands
geography: geography.volcanic_hotspot_chain
geology: geology.basaltic_tropical
climate: climate.tropical_trade_winds
hydrology: hydrology.tropical
erosion: erosion.tropical_volcanic
coasts: coasts.tropical
terrestrial_biomes: biomes.tropical_land
marine_biomes: biomes.tropical_marine
caves: caves.volcanic
ecology: ecology.tropical
voxelization: voxelization.tropical
```

## Requirements

* strict typed deserialization;
* `deny_unknown_fields` where practical;
* stable IDs;
* reference resolution;
* schema versions;
* source-file diagnostics;
* cross-file semantic validation;
* content hash generation.

## Deliverable

A complete world recipe loads and validates without running generation.

## Exit gate

Reject:

* invalid resolutions;
* missing passes;
* unknown material references;
* pipeline cycles;
* biome references to missing climate fields;
* cave profiles referencing missing geology;
* mismatched world bounds.

---

# 6. Phase 2 — Field framework and world atlas

## Goal

Build the reusable data structures required by all later terrain passes.

## Implement field types

```text
ScalarField
MaskField
CategoricalField<T>
VectorField
DistanceField
SparseFeatureField
GraphField
```

Required capabilities:

* world-coordinate sampling;
* bilinear and bicubic interpolation;
* nearest sampling for categorical fields;
* tiled storage;
* border halos;
* resampling;
* min/max scans;
* debug export;
* serialization;
* content hashing.

## Tiled field storage

Do not allocate every large-world field as one monolithic array.

Use tiled storage:

```rust
pub struct TiledScalarField {
    pub tile_size: UVec2,
    pub resolution_m: f64,
    pub tiles: BTreeMap<FieldTileCoord, ScalarTile>,
}
```

## Deliverable

Fields can be created, sampled, saved, loaded, visualized, and combined independently.

## Exit gate

* adjacent tile borders match;
* resampling is deterministic;
* fields can be exported as grayscale or false-color debug images;
* large fields can be generated tile-by-tile.

---

# 7. Phase 3 — Bounded ocean basin and world boundary

## Goal

Guarantee that the finite world is surrounded by ocean and has plausible deep-water bathymetry.

## Pipeline

```text
World boundary
→ edge-distance field
→ ocean-depth falloff
→ interior shelf-capable basin
→ boundary validation
```

## YAML

```yaml
boundary:
  shape: rectangle

  ocean_edge:
    start_fraction: 0.78
    maximum_depth_m: 5000
    curve: smoothstep

  variation:
    frequency: 0.00002
    amplitude_m: 250
```

## Requirements

* no land touches world edges;
* ocean becomes progressively deeper outward;
* future islands remain inside a safe margin;
* shelves can be created around land independently.

## Deliverable

A finite ocean world exists before any island generation.

## Exit gate

All boundary cells are ocean below a configured minimum depth.

---

# 8. Phase 4 — Island seeds and archipelago skeleton

## Goal

Generate the large-scale arrangement of islands independently from surface detail.

## Supported geography modes

```text
Single volcanic island
Hotspot island chain
Curved archipelago
Volcanic arc
Clustered islands
Mixed large and small islands
Authored island centers
```

## Island descriptors

```rust
pub struct IslandSeed {
    pub id: IslandId,
    pub center: DVec2,
    pub major_radius_m: f64,
    pub minor_radius_m: f64,
    pub rotation: f32,
    pub age_myr: f32,
    pub uplift: f32,
    pub volcanic_activity: f32,
    pub erosion_stage: f32,
    pub shelf_width_m: f32,
}
```

## Hotspot logic

Age should influence:

| Younger island   | Older island           |
| ---------------- | ---------------------- |
| taller           | lower                  |
| steeper          | more eroded            |
| active cones     | extinct volcanic forms |
| narrower shelves | wider shelves          |
| fewer reefs      | broader reefs          |
| more lava tubes  | more weathered caves   |
| sharper valleys  | wider valleys          |

## Deliverable

The atlas contains:

* island centers;
* influence fields;
* island IDs;
* volcanic age;
* broad island footprints.

## Exit gate

Different YAML recipes produce recognizably different island-chain layouts without changing code.

---

# 9. Phase 5 — Macro elevation and bathymetry

## Goal

Generate broad island shape, mountains, ocean floors, and continental shelves.

## Macro composition

```text
Island influence
+ volcanic uplift
+ elliptical deformation
+ domain warping
+ large-scale noise
+ ridge fields
+ caldera depressions
```

## Important rule

Do not introduce local terrain noise yet.

At this phase, the world should contain only:

* island masses;
* primary mountain systems;
* broad valleys;
* coastal plains;
* shelves;
* deep ocean.

## Bathymetry

Generate:

* shallow shelf;
* shelf break;
* island slope;
* abyssal basin;
* channels between islands;
* volcanic seamounts where appropriate.

## Deliverable

The world has coherent large-scale topography and underwater shape.

## Exit gate

* land fraction falls within recipe limits;
* island count is valid;
* no island touches the boundary;
* shelf width correlates with island age;
* macro landforms remain readable with local noise disabled.

---

# 10. Phase 6 — Structural geology

## Goal

Give each island a geological identity that influences later terrain.

## Generate

* bedrock type;
* volcanic flows;
* ash deposits;
* fault zones;
* caldera materials;
* limestone or coral-derived areas;
* hardness;
* erodibility;
* permeability;
* soil-parent material;
* talus angle.

## Example material data

```yaml
materials:
  basalt:
    hardness: 0.84
    erodibility: 0.24
    permeability: 0.48
    talus_angle_deg: 39

  volcanic_ash:
    hardness: 0.14
    erodibility: 0.91
    permeability: 0.33
    talus_angle_deg: 27

  reef_limestone:
    hardness: 0.58
    erodibility: 0.41
    permeability: 0.77
    talus_angle_deg: 34
```

## Geological constraints

Generate maps for:

* protected volcanic peaks;
* ridge lines;
* fault-aligned valleys;
* lava-flow lobes;
* erosion resistance;
* cave probability;
* groundwater behavior.

## Deliverable

The atlas contains geological categories and physical-property fields.

## Exit gate

Erosion, cave generation, and biome classification can query geology without inspecting voxel materials.

---

# 11. Phase 7 — Regional refinement

## Goal

Add mid-scale terrain features while maintaining seamless generation over a large world.

## Use overlapping regional windows

```text
Window size
Stride smaller than window size
Padding around each window
Center-weighted blending
```

Example:

```yaml
regional_refinement:
  window_size: 512
  stride: 320
  overlap_blend: raised_cosine
  padding: ocean_context
```

## Regional features

* secondary ridges;
* valley systems;
* volcanic shoulders;
* terraces;
* eroded plateaus;
* saddles;
* escarpments;
* broad ravines.

## Low/high-frequency separation

Store:

```text
elevation =
    macro elevation
  + regional residual
  + local detail
```

This makes it possible to alter local terrain without destroying the major drainage structure.

## Deliverable

Regional terrain can be generated tile-by-tile without visible seams.

## Exit gate

* seam heatmap remains below threshold;
* major peaks remain stable;
* island silhouettes remain unchanged by local window boundaries.

---

# 12. Phase 8 — Climate simulation

## Goal

Generate the environmental fields needed by rivers, vegetation, weather, and biomes.

## Inputs

* latitude;
* elevation;
* ocean proximity;
* prevailing wind;
* island orientation;
* slope;
* volcanic activity;
* seasonal settings.

## Generate

* temperature;
* rainfall;
* wind exposure;
* rain shadows;
* cloud exposure;
* humidity;
* evaporation;
* storm exposure.

## Tropical trade-wind model

Simulate:

```text
Ocean moisture recharge
→ windward uplift
→ orographic rain
→ mountain moisture loss
→ leeward rain shadow
```

## YAML

```yaml
climate:
  base_temperature_c: 28
  lapse_rate_c_per_km: 6.2

  prevailing_wind:
    direction_deg: 72
    strength: 0.8
    moisture: 0.95

  rainfall:
    ocean_recharge: 0.018
    orographic_factor: 2.6
    rain_shadow_factor: 0.72
```

## Deliverable

Windward and leeward island faces develop different climate signatures.

## Exit gate

* higher terrain is cooler;
* windward slopes are wetter;
* leeward slopes are drier;
* moisture fields are continuous across tiles.

---

# 13. Phase 9 — Hydrology

## Goal

Produce deterministic rivers, lakes, basins, wetlands, springs, and waterfalls.

## Recommended sequence

```text
Final pre-erosion elevation
→ depression analysis
→ fill or breach selected basins
→ downstream routing
→ flow accumulation
→ stream ordering
→ permanent river selection
→ lake extraction
→ waterfall detection
→ wetland detection
```

## Algorithms

Begin with:

* priority-flood depression handling;
* D8 routing;
* deterministic tie-breaking;
* accumulation from rainfall and runoff;
* stream-order calculation.

Later add:

* D-infinity;
* groundwater;
* seasonal flow;
* braided channels;
* tidal rivers.

## Hydrology graph

```rust
pub struct HydroNode {
    pub downstream: Option<CellIndex>,
    pub drainage_area: f32,
    pub discharge: f32,
    pub stream_order: u8,
    pub sediment: f32,
}
```

## Deliverable

Rivers and lakes exist as graph structures before they alter terrain.

## Exit gate

* almost all permanent rivers end in ocean, lake, or explicit sink;
* no chunk-local river generation;
* water flow is stable across region boundaries;
* waterfalls occur only where discharge and drop are sufficient.

---

# 14. Phase 10 — Erosion and sediment

## Goal

Make terrain morphology follow water, slope, and geology.

## Separate passes

### Fluvial erosion

Use stream-power incision based on:

* drainage area;
* slope;
* discharge;
* geology;
* constraint fields.

### Sediment transport

Track:

* carrying capacity;
* pickup;
* transport;
* deposition.

### Thermal erosion

Relax slopes exceeding material-specific talus angles.

### Coastal erosion

Reserve for the coast phase.

## Iteration loop

```text
Rebuild drainage
→ erode channels
→ transport sediment
→ deposit sediment
→ thermal relaxation
→ reapply constraints
```

## Important rule

Run erosion on regional surface fields, not on the full 3D voxel volume.

## Deliverable

The island develops:

* drainage-aligned valleys;
* floodplains;
* alluvial fans;
* deltas;
* ravines;
* sediment-rich coastal zones.

## Exit gate

* rivers visibly follow valleys;
* geology affects erosion rate;
* protected peaks survive;
* excessive slopes are reduced;
* sediment mass remains within tolerance.

---

# 15. Phase 11 — Coastal and marine terrain

## Goal

Generate tropical coasts as a distinct geomorphological system.

## Generate

* beach suitability;
* rocky coasts;
* sea cliffs;
* bays;
* lagoons;
* tidal flats;
* reefs;
* reef passes;
* shelf channels;
* mangrove zones;
* sea-cave candidates.

## Inputs

* water depth;
* shelf width;
* wave exposure;
* prevailing storm direction;
* sediment supply;
* rock hardness;
* slope;
* island age;
* temperature;
* river discharge.

## Beaches

Prefer:

* gentle slopes;
* high sediment;
* moderate to low exposure;
* shallow shelves.

## Cliffs

Prefer:

* steep slopes;
* high exposure;
* resistant geology;
* low sediment.

## Reefs

Require:

* tropical water temperature;
* suitable depth;
* limited sediment;
* sufficient island age;
* appropriate wave exposure.

## Lagoons

Generate where reef enclosure and shelf geometry permit.

## Deliverable

Coasts differ meaningfully around each island rather than forming uniform rings.

## Exit gate

* beaches, reefs, cliffs, and lagoons respond to fields;
* reefs avoid river-sediment plumes;
* old islands tend toward broader reef systems;
* young islands tend toward narrower shelves and steeper coasts.

---

# 16. Phase 12 — Terrestrial and marine biomes

## Goal

Classify the world into ecologically meaningful regions.

## Terrestrial biome inputs

* rainfall;
* temperature;
* elevation;
* slope;
* moisture;
* soil depth;
* geology;
* coast distance;
* wind exposure;
* disturbance;
* cave cover.

## Initial tropical land biomes

```text
Beach
Coastal scrub
Mangrove
Wet lowland forest
Dry forest
Grassland
Swamp
Cloud forest
Montane shrub
Volcanic barren
Rocky cliff
River corridor
Freshwater wetland
Cave
```

## Marine biomes

```text
Intertidal
Lagoon
Coral reef
Reef slope
Seagrass bed
Continental shelf
Deep coastal water
Open ocean
Abyssal basin
Hydrothermal or volcanic zone
```

## Classification model

Use weighted suitability rather than only strict first-match rules.

```rust
pub struct BiomeSuitability {
    pub biome: BiomeId,
    pub score: f32,
}
```

Allow smooth transition bands and deterministic noise.

## Deliverable

Every land and water region receives a biome identity and blend weights.

## Exit gate

* windward and leeward ecology differs;
* elevation bands are plausible;
* reef and lagoon biomes align with coast generation;
* biome boundaries avoid obvious contour-strip appearance.

---

# 17. Phase 13 — Soil, strata, and voxel materials

## Goal

Translate geology and biome information into layered voxel materials.

## Material model

Each surface location should derive:

```text
Organic layer
Topsoil
Subsoil
Weathered rock
Bedrock strata
Deposits
```

## Strata rules

```yaml
strata:
  - material: organic_soil
    thickness_m: [0.05, 0.4]
    requires:
      biome_tags: [vegetated]

  - material: topsoil
    thickness_m: [0.2, 1.8]
    driven_by:
      - rainfall
      - slope
      - biome

  - material: weathered_basalt
    thickness_m: [0.5, 6.0]
    driven_by:
      - rainfall
      - geological_age

  - material: basalt
    thickness_m: remaining
```

## Special deposits

Add:

* beach sand;
* river sediment;
* volcanic ash;
* talus;
* coral limestone;
* clay;
* peat;
* mineral veins.

## Deliverable

Voxel material selection is derived from fields and depth instead of a single surface material.

## Exit gate

Cave carving exposes plausible underlying strata.

---

# 18. Phase 14 — Volumetric caves and overhangs

## Goal

Expand from one authored cave into geology-driven procedural cave systems.

## Cave families

```text
Lava tubes
Limestone dissolution caves
Sea caves
Fracture caves
Volcanic vents
Talus caves
Hybrid natural/built caves
```

## Generation approach

Use:

```text
Cave-region suitability
→ entrance candidate selection
→ chamber-and-tunnel graph
→ 3D embedding
→ spline/capsule tunnels
→ warped chambers
→ noise perturbation
→ traversal validation
```

Avoid using unrestricted thresholded 3D noise as the primary cave generator.

## Lava tubes

Bias:

* young volcanic islands;
* basalt;
* downhill paths from former vents;
* moderate branching;
* relatively consistent tunnel cross-sections.

## Limestone caves

Bias:

* old reef-limestone regions;
* high permeability;
* groundwater;
* branching and chamber formation;
* sinkholes and springs.

## Sea caves

Bias:

* exposed cliffs;
* wave attack;
* weak layers;
* tidal elevation.

## Cave validation

Test:

* connected entrance;
* player clearance;
* roof thickness;
* no accidental ocean flooding unless intended;
* no intersection with protected structures;
* valid exit or terminus.

The procedural terrain design already recommends explicit cave graphs plus volumetric fields and noise rather than relying on raw noise alone. 

## Deliverable

Each island can contain multiple cave types based on geology and age.

## Exit gate

Generated caves remain traversable and reproducible.

---

# 19. Phase 15 — Water realization

## Goal

Convert hydrology and marine fields into runtime water bodies.

## Water types

```text
Ocean
Lagoon
River
Lake
Swamp
Spring
Waterfall
Groundwater
Cave pool
```

## Initial realization

Use:

* static ocean level;
* lake surface polygons;
* river channels carved into terrain;
* separate water surface meshes;
* waterfall feature meshes;
* water-type shader profiles.

## Defer full fluid simulation

Do not initially simulate every water voxel.

The world atlas should define water geometry and flow semantics.

## Runtime data

```rust
pub struct WaterBody {
    pub id: WaterBodyId,
    pub kind: WaterKind,
    pub surface_level: f32,
    pub flow: Option<Vec2>,
    pub salinity: f32,
    pub temperature: f32,
    pub turbidity: f32,
}
```

## Deliverable

Ocean, rivers, lakes, lagoons, and waterfalls appear in voxelized terrain.

## Exit gate

* rivers occupy carved channels;
* lakes do not visibly float;
* waterfalls align with hydrology drops;
* marine and freshwater shaders differ.

---

# 20. Phase 16 — Vegetation and ecological succession

## Goal

Populate the island according to biome, climate, terrain, and disturbance.

## Ecological inputs

* biome;
* rainfall;
* temperature;
* slope;
* soil depth;
* sunlight exposure;
* wind;
* salinity;
* flood frequency;
* volcanic age;
* disturbance;
* distance to water.

## Vegetation layers

```text
Ground cover
Shrubs
Understory
Canopy
Emergent trees
Aquatic vegetation
Reef vegetation or coral
```

## Placement architecture

Use:

* deterministic candidate points;
* Poisson-style spacing;
* suitability scoring;
* density targets;
* species competition;
* biome-specific palettes.

## Succession stages

Allow:

```text
Fresh lava
Pioneer plants
Grass and shrubs
Young forest
Mature forest
Disturbed forest
Abandoned cultivation
```

## Deliverable

Vegetation reflects island age and local environment.

## Exit gate

* mangroves occur in appropriate tidal zones;
* cloud forest occurs at wet elevation;
* dry forest appears leeward;
* cliffs and exposed ridges remain sparse;
* vegetation does not block essential routes indiscriminately.

---

# 21. Phase 17 — Fauna and habitat metadata

## Goal

Create ecological data that later supports spawning and AI.

## Habitat outputs

For each region, calculate:

* food availability;
* cover;
* nesting suitability;
* water access;
* temperature;
* disturbance;
* predator risk;
* traversal category.

## Initial animal groups

Potential examples:

* shorebirds;
* seabirds;
* bats;
* crabs;
* reef fish;
* feral pigs;
* small reptiles;
* insects;
* freshwater species.

## Important boundary

The generator should produce habitat and population capacity, not simulate every animal during world generation.

## Deliverable

Biome and habitat data can support later AI population systems.

---

# 22. Phase 18 — Procedural points of interest

## Goal

Place natural landmarks and gameplay-relevant locations.

## Natural POIs

* waterfalls;
* caves;
* crater lakes;
* geothermal vents;
* unusual reefs;
* isolated beaches;
* natural arches;
* mountain passes;
* sinkholes;
* giant trees;
* exposed mineral seams.

## Placement process

```text
Candidate generation
→ utility scoring
→ spacing constraints
→ route accessibility
→ biome and geology validation
→ feature registration
```

## Feature interface

```rust
pub trait FeatureAgent {
    fn collect_candidates(
        &self,
        atlas: &WorldAtlas,
        output: &mut Vec<FeatureCandidate>,
    );

    fn score(
        &self,
        candidate: &FeatureCandidate,
        atlas: &WorldAtlas,
    ) -> f32;

    fn apply(
        &self,
        candidate: FeatureCandidate,
        atlas: &mut WorldAtlas,
        blueprint: &mut WorldBlueprint,
    ) -> Result<(), WorldgenError>;
}
```

## Deliverable

Worlds contain intentional landmarks rather than only continuous noise.

## Exit gate

POI distribution meets minimum variety and spacing goals.

---

# 23. Phase 19 — Procedural structures and settlements

## Goal

Integrate built environments into generated terrain.

## First structure types

```text
Small native village
Abandoned camp
Research outpost
Ancient ruin
Small bunker
Dock or fishing site
```

## Generation flow

```text
Site suitability
→ structure archetype selection
→ semantic graph
→ 3D spatial embedding
→ voxel and entity blueprint
→ terrain adaptation
→ validation
→ commit
```

Structures should not be prebuilt blocks pasted blindly into terrain.

They should be semantic spatial graphs realized into voxels and entities. This allows the same framework to support villages, ruins, caves, bunkers, and research stations. 

## Terrain integration

Structures may:

* flatten small pads;
* cut foundations;
* connect to paths;
* attach to caves;
* create docks;
* bridge streams;
* occupy terraces;
* use natural shelter.

## Deliverable

At least one small village and one ruin can be placed coherently.

## Exit gate

* entrances connect to navigable terrain;
* structures do not float;
* structures do not block major rivers;
* required rooms remain reachable;
* cave and structure intersections are validated.

---

# 24. Phase 20 — Roads, trails, and regional connectivity

## Goal

Connect settlements, POIs, water access, and passes.

## Network inputs

* slope;
* rivers;
* biome traversal cost;
* landslide risk;
* bridges;
* passes;
* coastlines;
* ownership;
* destination importance.

## Generate

* footpaths;
* village trails;
* roads;
* bridges;
* docks;
* mountain switchbacks;
* cave connections.

## Hierarchy

```text
Regional route
→ local trail
→ settlement paths
```

## Deliverable

Important sites form a connected regional graph.

## Exit gate

The main island can be traversed between important sites without impossible routes.

---

# 25. Phase 21 — Quest and simulation hooks

## Goal

Expose generated geography as meaningful simulation data.

Register:

* settlements;
* resources;
* hazards;
* routes;
* blocked routes;
* fresh-water sources;
* food capacity;
* cave entrances;
* shelters;
* fertile soil;
* fishing areas;
* dangerous cliffs;
* volcanic risk;
* flood risk;
* damaged structures.

These facts can later feed the unified quest-driven AI architecture, where quests express desired world-state transitions for settlements, factions, NPCs, ecosystems, and the player.

## Example

```text
Village water capacity below requirement
→ water-security pressure
→ repair spring or construct cistern quest
```

```text
Landslide blocks mountain trail
→ route-access pressure
→ clear trail or establish alternate route quest
```

## Deliverable

Generated worlds provide structured facts rather than only meshes.

---

# 26. Phase 22 — Runtime chunk materialization

## Goal

Stream voxel chunks from the finalized world atlas.

## Per-chunk pipeline

```text
Request chunk
→ load required field tiles
→ sample surface elevation
→ calculate signed density
→ apply strata
→ carve caves
→ carve rivers and water basins
→ apply structures
→ assign biome and material data
→ mesh
→ collider
→ runtime entities
```

## Density composition

```text
Surface solid
+ cliffs and arches
+ structure solids
- caves and tunnels
- river channels
- structure interiors
+ saved edits
```

## Important separation

Keep distinct:

* world-generation tile;
* voxel chunk;
* render chunk;
* physics collider;
* simulation region;
* AI region.

They may use different sizes and lifecycles.

## Deliverable

The player can move through a world much larger than the vertical slice while chunks stream around them.

## Exit gate

* no chunk seams;
* no river discontinuities;
* cave tunnels cross chunk boundaries;
* structure edits remain consistent;
* streaming does not alter deterministic generation.

---

# 27. Phase 23 — Caching and incremental regeneration

## Goal

Avoid regenerating the complete world whenever one YAML file changes.

## Cache products

```text
world_cache/
├── manifest.bin
├── world_metadata.bin
├── fields/
│   ├── elevation/
│   ├── geology/
│   ├── climate/
│   ├── hydrology/
│   ├── biomes/
│   └── ecology/
├── graphs/
│   ├── rivers.bin
│   ├── roads.bin
│   └── structures.bin
└── chunks/
    ├── density/
    └── mesh/
```

## Dependency invalidation examples

```text
Climate settings changed
→ regenerate climate
→ hydrology
→ erosion
→ biomes
→ ecology
→ affected chunks
```

```text
Terrain material shader changed
→ preserve world fields
→ rebuild or reload materials only
```

```text
Cave profile changed
→ preserve elevation and hydrology
→ regenerate cave features and affected chunks
```

## Deliverable

Generation becomes iterative enough for practical world authoring.

## Exit gate

The dependency graph correctly identifies downstream products.

---

# 28. Phase 24 — Validation and repair

## Goal

Quantitatively evaluate each generated world and repair local defects.

## World metrics

```yaml
validation:
  land_fraction: [0.08, 0.34]
  island_count: [3, 40]
  largest_island_fraction_max: 0.55

  river_ocean_connection_ratio_min: 0.92
  traversable_land_fraction_min: 0.45
  freshwater_access_fraction_min: 0.60

  beach_fraction: [0.02, 0.30]
  reef_fraction: [0.01, 0.25]

  maximum_extreme_slope_fraction: 0.08
  required_major_poi_count: 8
```

## Validation categories

### Geographic

* world bounded by ocean;
* island counts;
* land fraction;
* shelf continuity;
* no impossible isolated spikes.

### Hydrological

* river endpoints;
* lake overflow;
* basin validity;
* waterfall plausibility.

### Ecological

* biome coverage;
* fresh-water access;
* soil and vegetation consistency;
* reef suitability.

### Traversal

* major settlements connected;
* cave entrances reachable;
* routes avoid impossible slopes;
* sufficient player clearance.

### Structural

* structures grounded;
* doors reachable;
* no river obstruction without bridge or culvert.

## Repair passes

Examples:

* breach invalid basin;
* reconnect river;
* widen trail;
* lower impossible slope;
* move structure;
* carve emergency cave route;
* remove tiny terrain speck;
* add freshwater spring;
* preserve one route through damaged terrain.

## Deliverable

World generation produces an acceptance report rather than merely finishing.

## Exit gate

A world is marked accepted only after all hard constraints pass.

---

# 29. Phase 25 — World-generation editor and diagnostics

## Goal

Make the generator authorable and debuggable.

## Views

* elevation;
* bathymetry;
* island age;
* geology;
* rainfall;
* temperature;
* hydrology;
* erosion difference;
* sediment;
* beaches;
* reefs;
* biomes;
* cave suitability;
* cave graphs;
* vegetation density;
* POIs;
* roads;
* structures;
* traversal heatmap;
* validation failures.

## Controls

* enable/disable passes;
* regenerate selected pass and downstream dependencies;
* change seed;
* compare two seeds;
* inspect a cell;
* inspect an island;
* show pass timings;
* show field histograms;
* show seam heatmaps;
* export maps;
* lock authored landmarks;
* preview without voxelization.

## Deliverable

Designers can understand why a terrain feature exists.

---

# 30. Phase 26 — Quality and performance scaling

## Goal

Scale the generator from development maps to large production archipelagos.

## Optimizations

* tile-based parallel passes;
* job scheduling;
* field compression;
* memory-mapped storage;
* regional lazy loading;
* cached distance fields;
* batched mesh uploads;
* chunk LOD;
* distant heightfield rendering;
* horizon silhouette rendering;
* structure and vegetation LOD;
* statistical ecology outside active regions.

## GPU use

The RTX 3070 target allows later experimentation with:

* compute-based erosion;
* field generation;
* vegetation placement;
* water simulation;
* GPU cave previews.

Do not make the canonical generator GPU-only unless deterministic cross-driver results are verified.

## Performance goals

Separate:

```text
Offline world compilation time
Runtime chunk generation time
Frame-time cost
Memory footprint
Disk-cache size
```

Do not optimize all four with one metric.

---

# 31. Recommended delivery milestones

## Milestone A — Single-island compiler

Includes Phases 0–7.

Result:

```text
YAML
→ bounded ocean
→ one volcanic island
→ macro elevation
→ geology
→ regional detail
```

## Milestone B — Hydrologically coherent island

Includes Phases 8–10.

Result:

```text
Climate
→ rivers
→ lakes
→ erosion
→ sediment
```

## Milestone C — Complete tropical coast and biomes

Includes Phases 11–13.

Result:

```text
Beaches
→ reefs
→ lagoons
→ land and marine biomes
→ strata
```

## Milestone D — Fully volumetric island

Includes Phases 14–15.

Result:

```text
Caves
→ lava tubes
→ overhangs
→ rivers and water realization
```

## Milestone E — Living island

Includes Phases 16–18.

Result:

```text
Vegetation
→ habitat
→ landmarks
```

## Milestone F — Inhabited island

Includes Phases 19–21.

Result:

```text
Structures
→ villages
→ routes
→ simulation and quest hooks
```

## Milestone G — Streamed archipelago

Includes Phases 22–26.

Result:

```text
Large cached world
→ streamed voxel chunks
→ validation
→ editor
→ production performance
```

---

# 32. Recommended first full-island target

Before building an entire archipelago, create one production-quality island with:

```text
World area: 16 × 16 km
One primary volcanic peak
One secondary ridge
Two major watersheds
Three to six permanent rivers
One crater or caldera
Two waterfalls
One lagoon
One fringing reef
One swamp or wetland
One cloud-forest region
One dry leeward region
Three cave systems
One village
One ruin
One research or survivor camp
A connected route network
```

This island should use the complete pipeline before multiplying the problem across many islands.

Once successful, expand to:

```text
Archipelago area: 64 × 64 km or larger
5–12 significant islands
Numerous islets and reefs
Island-age progression
Inter-island marine biomes
Regional weather and travel routes
```

---

# 33. Final success criteria

The island generator is ready for full production when:

## Data-driven architecture

* worlds are defined through composable YAML assets;
* algorithms remain implemented in Rust;
* passes form a validated dependency graph;
* generation is deterministic and versioned;
* downstream products can regenerate independently.

## Geography

* islands have intentional macro shapes;
* age affects topography, shelves, caves, and reefs;
* terrain is not visibly produced by one repeated noise formula;
* coasts vary according to exposure, geology, and sediment.

## Hydrology

* rivers follow drainage;
* lakes and waterfalls are topologically valid;
* erosion affects terrain morphology;
* wetlands and deltas occur in plausible locations.

## Volumetric terrain

* caves and overhangs use signed-density geometry;
* caves cross chunk boundaries;
* geological strata are exposed correctly;
* terrain remains editable.

## Ecology

* climate creates windward and leeward differences;
* biomes respond to fields;
* vegetation follows biome and terrain;
* marine biomes align with bathymetry and reefs.

## Gameplay integration

* settlements and POIs are reachable;
* structures fit terrain;
* the world exposes resources, hazards, routes, and needs;
* generated state can feed AI and quest systems;
* runtime chunks stream without changing world identity.

## Tooling

* every major field is viewable;
* generation passes are timed and inspectable;
* validation failures identify causes;
* worlds can be compared by seed;
* designers can lock authored landmarks.

The central design remains:

> **Generate the tropical island as a hierarchy of meaningful fields and graphs, then realize those results into the existing signed-density voxel world.**

That preserves the strongest aspects of the vertical slice—smooth volumetric terrain, `16³` chunks, caves, overhangs, YAML data, and deterministic generation—while scaling them into a complete tropical archipelago system.
