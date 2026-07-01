# Rust + Bevy Voxel RPG Vertical Slice

## Commit-Ready Implementation Plan

## 1. Objective

Build a small but technically complete third-person voxel RPG slice that proves the engine’s foundational systems before expanding into a large simulation world.

The slice will contain:

* a smooth volumetric voxel environment;
* slopes, cliffs, overhangs, and a traversable cave;
* a third-person placeholder character;
* MMO-style orbital camera controls;
* camera-relative WASD movement;
* terrain collision and slope handling;
* sunlight, shadows, fog, and cave contrast;
* a simple animated water shader;
* biome-aware terrain materials;
* sparse vegetation and environmental props;
* YAML-driven configuration and content;
* deterministic terrain generation;
* asynchronous chunk generation;
* developer visualization and diagnostics.

The slice is not intended to be a complete procedural world. It is intended to prove that the chosen terrain representation, chunk model, controller, rendering, and data architecture can support the larger engine without replacement.

---

# 2. Locked target platform and performance

## Platform

```text
Operating system: Windows 10/11
Graphics API:     Bevy/wgpu default Windows backend
Target GPU:       NVIDIA RTX 3070
Target frame rate: 60 FPS
Target frame time: 16.67 ms
Initial resolution target: 2560 × 1440
Minimum test resolution:   1920 × 1080
```

The project will target Windows only during this phase. Platform abstractions should not be deliberately broken, but Linux, macOS, WebAssembly, mobile, and console support are outside the slice.

## Engine version

Pin:

```toml
bevy = "=0.19.0"
```

Bevy 0.19 was released on June 19, 2026. Because major Bevy releases regularly include breaking API changes, the slice should pin the exact version rather than tracking a floating dependency. ([Bevy][1])

Do not upgrade Bevy during the vertical slice unless a blocking defect requires it.

---

# 3. Final scale decisions

## 3.1 Voxel scale

```text
One voxel cell = 1 × 1 × 1 meter
```

This provides enough resolution for:

* a roughly 1.8-meter humanoid;
* two-meter standing clearance;
* human-scaled cave entrances;
* one-meter terrain edits;
* recognizable stairs, ledges, and shorelines;
* future structures that do not feel excessively coarse.

Although the rendered terrain will be smooth, the one-meter cell remains the simulation and terrain sampling unit.

## 3.2 Canonical chunk size

```text
Chunk cells:     16 × 16 × 16
Density samples: 17 × 17 × 17
Physical span:   16 × 16 × 16 meters
```

Each chunk therefore contains:

```text
4,096 voxel cells
4,913 signed-density samples
```

An `8³` chunk remains useful for unit tests, but it will not be the production terrain chunk.

A `16³` chunk offers a practical balance:

* fewer mesh and collider entities than `8³`;
* less costly regeneration than `32³`;
* uniform cubic indexing;
* sufficient underground and vertical volume;
* manageable terrain-edit invalidation;
* straightforward chunk-neighbor relationships.

## 3.3 Vertical-slice world size

Use:

```text
6 × 3 × 6 chunks
```

Total logical volume:

```text
96 × 48 × 96 voxel cells
96 × 48 × 96 meters
108 total chunk positions
```

Not every chunk will contain visible terrain.

This is large enough for:

* a coastline;
* beach and shallow water;
* grassy lowlands;
* a rocky hill or ridge;
* an overhang;
* a cave entrance and internal chamber;
* a looped traversal route;
* meaningful camera distances;
* several biome transitions.

The world should be centered around logical world origin so that negative chunk coordinates are tested immediately.

Example extent:

```text
Chunk X: -3 through 2
Chunk Y: -1 through 1
Chunk Z: -3 through 2
```

---

# 4. Core terrain representation

See [docs/coordinate-system.md](docs/coordinate-system.md) for the canonical world/recipe/chunk coordinate conventions.

## 4.1 Signed-density field

The authoritative terrain representation is a scalar density field:

```text
density < 0: solid
density > 0: air
density = 0: terrain surface
```

```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct TerrainSample {
    pub density: f32,
    pub material: MaterialId,
}
```

This representation is required because the game needs:

* slopes;
* curved surfaces;
* caves;
* tunnels;
* overhangs;
* arches;
* terrain subtraction;
* later digging and construction.

A simple occupied/unoccupied block representation would make smooth terrain difficult and would not provide the same surface interpolation.

The terrain design already establishes a hybrid approach: broad landform generation can use height-style fields, while the final authoritative terrain is volumetric and caves or overhangs are composed in full 3D. 

## 4.2 Cell and sample distinction

A chunk contains `16³` cells but needs `17³` corner samples.

```rust
pub const CHUNK_CELLS: usize = 16;
pub const CHUNK_SAMPLES: usize = CHUNK_CELLS + 1;

pub const CELL_COUNT: usize =
    CHUNK_CELLS * CHUNK_CELLS * CHUNK_CELLS;

pub const SAMPLE_COUNT: usize =
    CHUNK_SAMPLES * CHUNK_SAMPLES * CHUNK_SAMPLES;
```

Adjacent chunks share world-space sample positions:

```text
Chunk (0,0,0), local sample X = 16
Chunk (1,0,0), local sample X = 0

Both reference world sample X = 16
```

Each chunk will store its own copy of its `17³` sample grid.

This duplicates boundary samples but simplifies:

* asynchronous chunk jobs;
* meshing;
* serialization;
* unloading;
* collider construction;
* isolated tests.

Shared samples must be generated from absolute world coordinates and verified to be identical.

## 4.3 Density storage

For the slice:

```text
Generation density: f32
Runtime density:    f32
```

Do not quantize to `i16` yet.

Later, the engine can use:

* `f32` while generating;
* quantized `i16` for stored terrain;
* delta compression for edited chunks.

The first slice should prioritize correctness over density-memory optimization.

---

# 5. Meshing strategy

## 5.1 Initial mesher

Use **Surface Nets**.

It is suitable because it:

* handles signed-density terrain;
* produces smooth slopes;
* supports caves and overhangs;
* creates fewer vertices than many naive Marching Cubes implementations;
* is substantially easier to implement correctly than robust Dual Contouring;
* provides a future upgrade path.

```rust
pub trait TerrainMesher: Send + Sync {
    fn build_mesh(
        &self,
        input: &ChunkMeshingInput,
    ) -> Result<TerrainMeshData, MeshingError>;
}
```

Initial implementation:

```text
SurfaceNetsMesher
```

Future implementations:

```text
DualContouringMesher
BlockStructureMesher
DebugCellMesher
```

## 5.2 Padded meshing input

Meshing should operate on a padded sample region so normals and topology are correct at chunk edges.

Conceptually:

```text
Visible chunk samples: 17 × 17 × 17
Meshing work area:      sample halo beyond relevant borders
```

The halo may be obtained by evaluating the deterministic density source at neighboring world positions.

The slice does not need chunks to wait for all neighbors before meshing.

## 5.3 Normals

Compute terrain normals from the density gradient:

```rust
fn estimate_normal(
    p: Vec3,
    epsilon: f32,
    density: impl Fn(Vec3) -> f32,
) -> Vec3 {
    let dx = density(p + Vec3::X * epsilon)
        - density(p - Vec3::X * epsilon);

    let dy = density(p + Vec3::Y * epsilon)
        - density(p - Vec3::Y * epsilon);

    let dz = density(p + Vec3::Z * epsilon)
        - density(p - Vec3::Z * epsilon);

    Vec3::new(dx, dy, dz).normalize_or_zero()
}
```

This is particularly important for:

* cave ceilings;
* overhang undersides;
* rounded tunnel walls;
* smooth lighting across chunk seams.

---

# 6. Terrain generation model

## 6.1 Hybrid authored/procedural approach

The vertical slice terrain will be generated from:

```text
Procedural broad terrain
+ YAML-authored feature anchors
+ deterministic noise variation
+ volumetric cave subtraction
+ explicit overhang geometry
```

Do not attempt to generate the entire slice from unrestricted noise.

The slice needs guaranteed gameplay features, including:

* a valid spawn point;
* a walkable route;
* a beach;
* a gentle slope;
* an unwalkable steep slope;
* an overhang;
* a cave entrance;
* a traversable tunnel;
* a cave chamber;
* a water body.

The terrain system should generate broad shape procedurally but construct those required landmarks from explicit descriptors.

## 6.2 Density composition

Under the selected negative-is-solid convention:

```rust
#[inline]
fn solid_union(a: f32, b: f32) -> f32 {
    a.min(b)
}

#[inline]
fn solid_subtract(solid: f32, cavity: f32) -> f32 {
    solid.max(-cavity)
}
```

Final density:

```rust
fn final_density(
    p: Vec3,
    context: &TerrainGenerationContext,
) -> f32 {
    let surface_height = context.surface.height_at(p.x, p.z);
    let mut density = p.y - surface_height;

    for addition in &context.solid_additions {
        density = solid_union(density, addition.sample(p));
    }

    for cavity in &context.cavities {
        density = solid_subtract(density, cavity.sample(p));
    }

    density
}
```

## 6.3 Surface generation

The initial surface combines:

```text
Island or headland footprint
+ broad mound or ridge
+ low-frequency fBm
+ localized rocky ridge
+ small detail noise
```

Do not run hydraulic erosion during the vertical slice.

The larger terrain design can later introduce macro fields, hydrology, erosion, geology, coasts, and voxel materialization without replacing the signed-density foundation. 

## 6.4 Cave generation

The cave will use an explicit graph:

```text
Entrance chamber
→ descending tunnel
→ main chamber
→ short side branch
```

Major tunnels are generated as capsule signed-distance fields.

```rust
fn capsule_sdf(
    p: Vec3,
    a: Vec3,
    b: Vec3,
    radius: f32,
) -> f32;
```

Chambers use warped ellipsoids.

Noise may perturb cave walls but may not determine connectivity.

This guarantees:

* traversability;
* sufficient headroom;
* valid entrance placement;
* no accidental roof perforation;
* reproducible structure.

## 6.5 Overhang generation

The overhang is created using constructive field composition:

```text
Add cliff mass
Subtract cavity beneath cliff
Add small noise perturbation
Validate remaining support thickness
```

The overhang must be large enough for the player and camera to pass beneath.

---

# 7. World layout

The initial environment should be a **tropical coastal headland or small island section**.

## Primary traversal route

```text
Player spawn
→ sandy shoreline
→ grassy lowland
→ inclined path
→ rocky upland
→ shaded overhang
→ cave entrance
→ descending tunnel
→ cave chamber
→ optional second exit or return route
```

## Biome set

Use four primary terrain biomes:

```text
Beach
Grassland
Rocky upland
Cave
```

Optional fifth classification:

```text
Shallow coastal water
```

## Landmark distribution

Suggested placement:

```text
Southwest:
    player spawn
    beach
    shallow water

Center:
    grassland
    sloped path
    vegetation

Northeast:
    rocky ridge
    overhang
    cave entrance

Underground:
    tunnel
    chamber
    interactive object
```

---

# 8. Character controller

## 8.1 Physics backend

Use **Avian3D** for the slice, wrapped behind project-owned interfaces.

The engine should not make gameplay code depend directly on Avian-specific component shapes everywhere.

Project-owned abstractions:

```rust
pub trait CharacterCollisionQuery {
    fn cast_character(
        &self,
        origin: Vec3,
        displacement: Vec3,
        shape: CharacterShape,
    ) -> CharacterCastResult;

    fn probe_ground(
        &self,
        origin: Vec3,
        distance: f32,
    ) -> GroundProbeResult;
}
```

Physics integration remains replaceable later.

## 8.2 Player dimensions

```text
Height: approximately 1.8 m
Capsule radius: 0.35–0.40 m
Capsule half-height: approximately 0.70–0.75 m
```

## 8.3 Movement set

Include:

* walking;
* running;
* jumping;
* falling;
* gravity;
* grounded detection;
* step-up handling;
* slope classification;
* sliding on steep terrain.

Defer:

* crouching;
* climbing;
* mantling;
* swimming;
* falling damage;
* prone movement;
* combat movement.

## 8.4 Movement feel

Target responsive MMO-style movement:

```text
Walk speed:  4.5–5.0 m/s
Run speed:   7.0–8.0 m/s
Jump height: approximately 1.0–1.3 m
Acceleration: short smoothing, not instant teleportation
Deceleration: fast and responsive
Rotation: rapid interpolation toward intended movement
```

Maximum walkable slope:

```text
Approximately 45–48 degrees
```

Step height:

```text
Approximately 0.45 m
```

## 8.5 Movement basis

Movement is camera-relative but ignores camera pitch.

```rust
let forward = camera_forward
    .reject_from(Vec3::Y)
    .normalize_or_zero();

let right = forward
    .cross(Vec3::Y)
    .normalize_or_zero();

let desired_direction =
    forward * input.forward
    + right * input.right;
```

---

# 9. MMO-style orbital camera

## 9.1 Camera controls

Use:

```text
Right mouse drag:
    rotate camera and character facing

Left mouse drag:
    orbit camera independently around character

Both mouse buttons:
    move character forward

Mouse wheel:
    zoom

WASD:
    camera-relative movement

Home or configurable key:
    recenter behind character
```

The camera should not enter first-person in this slice.

Minimum zoom remains close third-person.

## 9.2 Camera hierarchy

```text
Player
CameraFollowTarget
CameraRig
CameraPivot
Camera3d
```

The follow target should be around shoulder or upper-torso height.

## 9.3 Camera smoothing

Use exponential damping or half-life-based smoothing for:

* target following;
* zoom;
* obstruction recovery;
* yaw recentering.

Avoid frame-rate-dependent interpolation constants.

## 9.4 Camera collision

Sphere-cast from the player focus point to the desired camera position.

When obstructed:

* move inward;
* retain requested zoom;
* smoothly return when space becomes available.

Do not implement terrain transparency or cutaway rendering yet.

The collision system must work:

* against cliffs;
* beneath overhangs;
* inside tunnels;
* at the cave entrance;
* close to the player.

---

# 10. Terrain collision

Each visible terrain chunk receives:

* one render mesh;
* one triangle-mesh collider;
* one chunk lifecycle component.

Collider generation occurs after mesh generation.

```text
Generate density
→ generate mesh buffers
→ upload Bevy render mesh
→ construct terrain collider
→ mark chunk ready
```

Do not rebuild colliders unless chunk geometry changes.

## Required collision tests

* walking across chunk seams;
* jumping near seams;
* grounding on slopes;
* steep-slope rejection;
* no ceiling-ground confusion inside caves;
* no tunnel-floor penetration;
* no snagging at cave entrances;
* stable step-up behavior;
* camera collision independent of player collision.

---

# 11. Terrain materials

## 11.1 Triplanar mapping

Use a custom terrain material with triplanar mapping.

Traditional planar UVs will stretch on:

* cliffs;
* cave walls;
* cave ceilings;
* overhang undersides.

Triplanar blending should use:

* world-space position;
* world-space normal;
* material scale;
* material IDs and weights;
* biome modifiers.

## 11.2 Material model

Separate:

```text
Geological material
Biome classification
Rendered surface appearance
```

Example:

```text
Geology: basalt
Biome: grassland
Surface: grass with exposed basalt on steep slopes
```

Each terrain vertex should support up to four material weights:

```rust
pub struct TerrainVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub material_ids: [u16; 4],
    pub material_weights: [f32; 4],
}
```

The slice may normally use only two or three simultaneous materials, but the format should allow four.

## 11.3 Initial material palette

```text
Sand
Grass/soil
Rock/basalt
Cave rock
Wet rock
```

Wet rock may be a modifier rather than a separate texture set.

---

# 12. Biome system

## 12.1 Biome classification inputs

The first biome system uses:

* elevation;
* slope;
* distance to water;
* cave depth;
* moisture;
* deterministic transition noise.

```rust
pub struct BiomeSampleContext {
    pub elevation: f32,
    pub slope_degrees: f32,
    pub distance_to_water: f32,
    pub cave_cover: f32,
    pub moisture: f32,
    pub transition_noise: f32,
}
```

## 12.2 Biome outputs

Each biome supplies:

* terrain material preferences;
* tint;
* roughness modifier;
* wetness modifier;
* vegetation profile ID;
* ambient audio profile stub;
* weather profile stub;
* spawn profile stub;
* gameplay tags.

Only material appearance and sparse vegetation need to be implemented now.

The remaining references should load and validate but may not yet affect simulation.

---

# 13. Vegetation and props

Add sparse environment decoration after terrain and collision are stable.

## Initial vegetation

* grass clumps;
* small tropical plants;
* one or two shrubs;
* a small number of trees;
* cave moss or glowing fungus.

## Placement

Use deterministic biome-driven sampling:

```text
Biome suitability
× slope allowance
× soil/material allowance
× noise distribution
× spacing rejection
```

Do not place vegetation as individual voxels.

Use instancing or repeated mesh handles.

## Props

Add a few simple props:

* rocks;
* driftwood;
* cave stones;
* a glowing or emissive interaction object.

---

# 14. Lighting and atmosphere

## 14.1 Fixed lighting scenario

Use a fixed late-morning or early-afternoon lighting setup.

Include:

* one directional sun;
* dynamic shadows;
* ambient environment light;
* distance fog;
* sky or clear-color gradient;
* optional light in the cave chamber.

Do not implement a moving sun during the slice.

## 14.2 Cave treatment

Cave darkness will result from:

* terrain occlusion;
* directional shadows;
* reduced cave ambient factor;
* localized cave light or emissive object;
* slightly increased cave fog or color shift.

Do not implement voxel global illumination or light propagation.

## 14.3 Future lighting stubs

Define interfaces for:

* day/night cycle;
* sun orbit;
* moon orbit;
* moon phases;
* weather attenuation;
* cloud shadows;
* emissive voxels;
* light probes.

The fixed slice lighting should fit into these interfaces without being rewritten.

---

# 15. Water system

## 15.1 Scope

Use a static horizontal water level.

Water is rendered as a shader-driven surface, not as simulated fluid voxels.

Features:

* animated normal distortion;
* shallow and deep colors;
* transparency;
* Fresnel effect;
* depth fade;
* optional simple shoreline foam;
* underwater terrain visibility.

## 15.2 Player interaction

For the slice:

* shallow water may be entered;
* deep water acts as a boundary;
* no swimming;
* no buoyancy;
* no flowing water;
* no waves affecting physics.

A depth check can stop or reset the player before deep water becomes problematic.

## 15.3 Water geometry

Generate a simple world-covering or bounded rectangular surface at configured sea level.

Avoid coplanar overlap with terrain.

Future water bodies should be represented through connected occupancy and surface extraction, but that is deferred.

---

# 16. YAML-driven data architecture

All author-facing content is configured through YAML.

## 16.1 Required files

```text
assets/config/
    app.yaml
    performance.yaml
    player.yaml
    camera.yaml
    lighting.yaml
    water.yaml

assets/terrain/
    worlds/vertical_slice.world.yaml
    generation/vertical_slice.terrain.yaml
    caves/demo_cave.yaml
    biomes/vertical_slice.biomes.yaml
    materials/terrain.materials.yaml
    vegetation/vertical_slice.vegetation.yaml
```

## 16.2 YAML principles

Every definition includes:

```yaml
schema_version: 1
id: namespace.definition_name
```

Rust types use:

```rust
#[serde(deny_unknown_fields)]
```

where practical.

Reject:

* unknown fields;
* duplicate IDs;
* unsupported schema versions;
* invalid references;
* negative dimensions;
* reversed ranges;
* missing materials;
* invalid cave links;
* invalid chunk dimensions;
* unsupported feature types.

## 16.3 Data compilation pipeline

```text
Read YAML
→ deserialize typed structures
→ validate schema
→ resolve IDs
→ validate references
→ validate semantics
→ compile runtime representation
→ publish immutable registry
```

Simulation and rendering should not repeatedly interpret YAML strings at runtime.

## 16.4 Hot reload

Require live or controlled reload for:

* camera;
* lighting;
* water;
* terrain materials;
* biome appearance.

Terrain-generation changes should mark the world as needing regeneration and require an explicit debug action.

Invalid reloads retain the last valid configuration.

Bevy 0.19 exposes a file-watcher feature suitable for development asset reload workflows. ([Docs.rs][2])

---

# 17. Workspace architecture

Recommended workspace:

```text
roguelike-engine/
├── Cargo.toml
├── crates/
│   ├── shared/
│   ├── voxel_core/
│   ├── terrain_generation/
│   ├── terrain_meshing/
│   ├── game_data/
│   ├── physics_bridge/
│   └── game_bevy/
├── assets/
├── tools/
└── src/
    └── main.rs
```

## Dependency direction

```text
shared
    ↑
voxel_core
    ↑
terrain_generation
terrain_meshing

game_data
    independent typed definitions

physics_bridge
    wraps Avian integration

game_bevy
    depends on all lower layers
```

Important rules:

* `voxel_core` contains no Bevy dependency;
* terrain generation contains no live ECS logic;
* meshing returns plain Rust buffers;
* worker threads do not create Bevy assets;
* Bevy owns presentation and runtime entity lifecycle;
* physics types are wrapped behind project-owned interfaces.

---

# 18. Plugin organization

```rust
pub struct VerticalSlicePlugin;

impl Plugin for VerticalSlicePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            DataAssetPlugin,
            TerrainWorldPlugin,
            TerrainGenerationPlugin,
            TerrainMeshingPlugin,
            TerrainRenderingPlugin,
            PhysicsBridgePlugin,
            CharacterControllerPlugin,
            PlayerPlugin,
            OrbitCameraPlugin,
            BiomePlugin,
            VegetationPlugin,
            LightingPlugin,
            WaterPlugin,
            InteractionPlugin,
            DebugToolsPlugin,
        ));
    }
}
```

Each plugin should own a narrow responsibility.

---

# 19. Asynchronous terrain pipeline

Even though the world is small, use the lifecycle needed by future streaming.

## Chunk state

```rust
pub enum ChunkState {
    Unrequested,
    Requested,
    GeneratingDensity,
    AwaitingMeshing,
    Meshing,
    AwaitingUpload,
    AwaitingCollider,
    Ready,
    Dirty,
    Failed,
    Unloading,
}
```

## Pipeline

```text
Chunk request
→ density generation job
→ material and biome sampling
→ mesh generation job
→ main-thread mesh upload
→ collider construction
→ ready
```

## Revision safety

Every job carries:

* chunk coordinate;
* world seed;
* generation revision;
* recipe hash.

Stale results must be discarded.

## Initial budgets

```yaml
generation:
  maximum_density_jobs: 4
  maximum_mesh_jobs: 4
  maximum_mesh_uploads_per_frame: 2
  maximum_collider_builds_per_frame: 1
```

These should be tuned using profiling rather than assumed to be optimal.

---

# 20. Debug terrain editing

Include a developer-only terrain editing tool.

Operations:

```text
Subtract sphere
Add sphere
Paint material
```

This is not player gameplay.

Purpose:

* prove chunk invalidation;
* test remeshing;
* test neighbor-border synchronization;
* test collider replacement;
* test editing inside caves;
* reveal seam bugs.

## Edit process

```text
Determine affected world samples
→ modify owning and duplicate border samples
→ mark affected chunks dirty
→ regenerate meshes
→ replace colliders
→ verify revision
```

Do not add full save persistence for arbitrary edits yet, though the edit command format should be serializable.

---

# 21. Interaction proof

Place one interactive object inside the cave.

Example:

```text
Ancient beacon
Glowing crystal
Research marker
Placeholder shrine
```

Interaction flow:

```text
Player enters range
→ prompt appears
→ player presses interact
→ object changes state
→ light or material changes
→ message appears
```

This proves:

* ECS entities can coexist with voxel terrain;
* world-space interaction works;
* cave lighting can respond;
* future quest hooks have a usable event source.

The object does not require a complete inventory or quest system.

---

# 22. Advanced extension stubs

The following systems should have interfaces or data hooks but not full implementation.

## Chunk streaming

```rust
pub trait ChunkInterestProvider {
    fn desired_chunks(&self) -> BTreeSet<ChunkCoord>;
}
```

## Terrain editing persistence

```rust
pub struct ChunkDelta {
    pub coord: ChunkCoord,
    pub edits: Vec<DensityDelta>;
}
```

## Dual Contouring

Alternative `TerrainMesher` implementation.

## Hydrology

* rainfall;
* flow direction;
* river graphs;
* lake filling;
* waterfalls.

## Procedural structures

Structure placement requests should eventually produce semantic blueprints and grouped chunk patches rather than writing directly into live chunks. 

## Quest-driven AI

Terrain and structures should register semantic facts such as:

```text
Cave entrance
Shelter
Fresh water
High ground
Dangerous drop
Resource deposit
Biome region
Traversable route
Blocked route
```

These can later become inputs to the unified quest and AI systems, where quests represent desired world-state changes.

## Day/night cycle

* simulation time;
* sun and moon emitters;
* celestial trajectories;
* moon phases;
* weather-based attenuation.

## Water simulation

* connected water bodies;
* occupancy;
* flow;
* source and drain;
* waterfalls;
* buoyancy;
* swimming.

## Simulation LOD

* detailed chunks;
* regional summaries;
* distant terrain;
* horizon silhouette;
* abstract ecology.

---

# 23. Explicitly deferred systems

Do not implement during the slice:

* large-world streaming;
* full archipelago generation;
* hydraulic erosion;
* rivers;
* dynamic fluid simulation;
* volumetric clouds;
* dynamic weather;
* complete day/night cycle;
* moon phases;
* destructible gameplay terrain;
* building system;
* procedural ruins or settlements;
* NPC AI;
* quest system;
* inventory;
* combat;
* crafting;
* full save games;
* multiplayer;
* animation state machine beyond idle/run/jump;
* underwater gameplay;
* hierarchical pathfinding;
* terrain LOD seams;
* global illumination.

---

# 24. Performance budget

Target:

```text
16.67 ms total frame time at 60 FPS
```

Recommended working budget:

| Area                     |  Target budget |
| ------------------------ | -------------: |
| CPU gameplay and ECS     |         2.0 ms |
| Character and physics    |         1.5 ms |
| Terrain lifecycle work   | 1.0 ms average |
| Render preparation       |         2.0 ms |
| GPU terrain and props    |         4.0 ms |
| GPU shadows and lighting |         3.0 ms |
| Water and transparency   |         1.0 ms |
| UI/debug                 |         0.5 ms |
| Safety margin            |        1.67 ms |

These are planning limits rather than guarantees.

## Performance rules

* no voxel entities;
* no full-world scan each frame;
* no terrain remeshing unless dirty;
* no collider rebuild unless geometry changes;
* no mesh creation on worker threads through Bevy APIs;
* no individual draw call per material or voxel;
* reuse texture arrays and material handles;
* instance vegetation;
* cap chunk uploads per frame;
* cap collider creation per frame;
* use frustum culling;
* profile release builds, not only debug builds.

## Acceptance hardware

The slice must maintain 60 FPS on the RTX 3070 at:

```text
2560 × 1440
High terrain material quality
Dynamic sun shadows
Fog enabled
Water enabled
Normal vegetation density
Debug overlays disabled
```

One-percent-low frame behavior should be observed during chunk generation and regeneration, not merely average FPS.

---

# 25. Development phases

## Phase 0 — Foundation and content loading

Implement:

* Rust workspace;
* Bevy 0.19 pin;
* logging;
* application states;
* YAML asset definitions;
* schema and semantic validation;
* configuration registry;
* development file watching;
* Windows build configuration.

Deliverable:

```text
Application opens, loads configuration, and enters a blank 3D scene.
```

Exit gate:

* malformed YAML gives actionable diagnostics;
* last valid data survives failed reload;
* deterministic registry hash;
* CI passes.

---

## Phase 1 — Player and orbital camera

Implement:

* player root;
* placeholder capsule or simple model;
* camera rig;
* mouse capture;
* left/right mouse behavior;
* zoom;
* camera-relative WASD;
* character facing;
* camera smoothing;
* temporary plane.

Deliverable:

```text
The player moves naturally on a test plane with MMO-style camera controls.
```

Exit gate:

* camera never flips;
* movement remains correct at all yaw angles;
* zoom limits work;
* input is frame-rate independent.

---

## Phase 2 — Voxel core

Implement:

* chunk and sample coordinates;
* negative chunk coordinates;
* `16³` cells;
* `17³` samples;
* world/sample conversion;
* material IDs;
* chunk storage;
* deterministic density source interface;
* border equality tests.

Deliverable:

```text
Headless terrain chunks can be generated and inspected.
```

Exit gate:

* every sample index is unique;
* neighboring borders match;
* same seed produces identical hashes.

---

## Phase 3 — Surface Nets meshing

Implement:

* cell-crossing detection;
* edge intersections;
* cell vertex placement;
* topology;
* gradient normals;
* bounds;
* material attributes;
* Bevy mesh upload;
* wireframe and normals debug modes.

Deliverable:

```text
A plane, sphere, slope, cave, and overhang render correctly.
```

Exit gate:

* no cracks;
* correct winding;
* correct cave normals;
* empty and solid chunks produce no unnecessary geometry.

---

## Phase 4 — Vertical-slice terrain generation

Implement:

* coastal footprint;
* broad terrain;
* low-frequency detail;
* rocky ridge;
* cave graph;
* overhang;
* water basin;
* spawn area;
* route validation.

Deliverable:

```text
The complete deterministic 96 × 48 × 96-meter environment exists.
```

Exit gate:

* all required landmarks exist;
* cave is traversable;
* overhang is structurally plausible;
* no floating terrain;
* no accidental cave roof holes.

---

## Phase 5 — Physics and character controller

Implement:

* Avian integration;
* triangle-mesh colliders;
* capsule controller;
* grounding;
* gravity;
* walk/run;
* jump;
* slope handling;
* step handling;
* steep-slope sliding;
* camera obstruction cast.

Deliverable:

```text
The player can traverse the complete terrain route.
```

Exit gate:

* stable across seams;
* stable in cave;
* no false ceiling grounding;
* no terrain penetration;
* camera does not clip through terrain.

---

## Phase 6 — Terrain materials and biomes

Implement:

* biome rule evaluation;
* material catalog;
* four-way vertex weights;
* triplanar shader;
* sand, grass, rock, and cave material;
* slope-based rock exposure;
* wetness modifier;
* biome debug visualization.

Deliverable:

```text
The environment has readable blended terrain regions.
```

Exit gate:

* no severe cliff texture stretching;
* biome transitions do not form obvious hard bands;
* cave and exterior use coherent material logic.

---

## Phase 7 — Lighting and atmosphere

Implement:

* directional sun;
* shadows;
* ambient light;
* fog;
* sky treatment;
* cave ambient modifier;
* optional cave light;
* YAML hot reload.

Deliverable:

```text
Outdoor and cave environments have distinct readable lighting.
```

Exit gate:

* overhang casts a clear shadow;
* cave entrance transitions visibly;
* shadow artifacts are controlled;
* stable 60 FPS remains achievable.

---

## Phase 8 — Water

Implement:

* water surface;
* custom material;
* animated normals;
* shallow/deep color;
* Fresnel;
* transparency;
* depth fade;
* optional foam;
* shallow-water entry boundary logic.

Deliverable:

```text
The coastline has visually convincing animated water.
```

Exit gate:

* no major sorting problems;
* no z-fighting;
* underwater terrain remains visible near shore;
* shader is fully YAML-configurable.

---

## Phase 9 — Vegetation and props

Implement:

* deterministic biome placement;
* instanced grass;
* plants;
* shrubs;
* sparse trees;
* rocks;
* cave moss;
* culling and density controls.

Deliverable:

```text
The terrain reads as a place rather than a geometry demonstration.
```

Exit gate:

* props do not obstruct required route;
* vegetation respects slope and biome rules;
* 60 FPS target remains intact.

---

## Phase 10 — Debug editing and diagnostics

Implement:

* chunk bounds;
* density visualization;
* normal visualization;
* material and biome modes;
* collider display;
* camera cast display;
* generation metrics;
* terrain add/subtract sphere;
* material painting;
* dirty-chunk tracking.

Deliverable:

```text
Terrain problems can be inspected and isolated without external tooling.
```

Exit gate:

* cross-boundary edits remain seamless;
* colliders update correctly;
* stale job results do not overwrite newer edits.

---

## Phase 11 — Interaction proof and polish

Implement:

* interaction query;
* prompt;
* cave object;
* state change;
* sound or light response;
* minimal UI;
* simple idle/run/jump animation if available;
* final control tuning;
* final profiling.

Deliverable:

```text
A small complete playable experience.
```

---

# 26. Testing plan

## Unit tests

### Coordinates

* positive and negative chunks;
* world-to-chunk conversion;
* boundary samples;
* sample indexing;
* local-cell validity.

### Density

* sign convention;
* deterministic generation;
* constructive union;
* subtraction;
* cave SDFs;
* overhang SDFs.

### Meshing

* empty chunk;
* full chunk;
* plane;
* sphere;
* tunnel;
* overhang;
* shared border;
* normal direction.

### Data

* unknown field rejection;
* duplicate ID rejection;
* invalid reference rejection;
* range validation;
* cave graph validation;
* schema-version validation.

### Biomes

* priority;
* overlapping conditions;
* transition weights;
* cave override;
* water proximity.

## Integration tests

* generate all 108 chunk positions;
* mesh non-empty chunks;
* create render entities;
* create colliders;
* spawn player;
* complete traversal path;
* regenerate terrain;
* hot-reload lighting;
* edit terrain across boundary;
* verify no duplicate entities.

## Determinism

```text
Same seed
+ same YAML
+ same generator version
= same density hashes
+ same biome assignments
+ same mesh topology
```

Quantize floating values before mesh hashing when necessary.

## Performance tests

Track:

* terrain generation duration;
* mesh generation duration;
* collider construction duration;
* mesh upload time;
* triangle counts;
* draw calls;
* CPU frame time;
* GPU frame time;
* memory per chunk;
* regeneration hitch duration.

---

# 27. Debug controls

Suggested defaults:

```text
F1  Debug panel
F2  Chunk bounds
F3  Wireframe
F4  Biome visualization
F5  Material visualization
F6  Collider visualization
F7  Density sample visualization
F8  Regenerate same seed
F9  Regenerate next seed
F10 Freeze terrain jobs

1   Subtract terrain sphere
2   Add terrain sphere
3   Paint material

Home Recenter camera
```

All bindings should be data-driven.

---

# 28. Example configuration

## World

```yaml
schema_version: 1
id: world.vertical_slice

seed: 48129

voxel:
  cell_size_m: 1.0

chunks:
  cells: [16, 16, 16]
  world_extent: [6, 3, 6]

terrain: terrain.vertical_slice
biomes: biomes.vertical_slice
materials: materials.vertical_slice
water: water.tropical_shallow
lighting: lighting.late_morning
```

## Player

```yaml
schema_version: 1
id: player.default

capsule:
  radius_m: 0.38
  half_height_m: 0.72

movement:
  walk_speed_mps: 4.8
  run_speed_mps: 7.5
  acceleration_mps2: 26.0
  deceleration_mps2: 32.0
  rotation_speed_deg_per_s: 720.0
  maximum_walkable_slope_deg: 47.0
  step_height_m: 0.45
  ground_snap_m: 0.28
  jump_height_m: 1.15

gravity_mps2: 18.0
```

## Camera

```yaml
schema_version: 1
id: camera.mmo_default

distance:
  default_m: 8.0
  minimum_m: 2.2
  maximum_m: 16.0

pitch_degrees:
  default: -28.0
  minimum: -65.0
  maximum: -8.0

sensitivity:
  yaw: 0.18
  pitch: 0.14
  zoom: 1.0

focus_offset_m: [0.0, 1.4, 0.0]

smoothing:
  follow_half_life: 0.08
  zoom_half_life: 0.06
  obstruction_release_half_life: 0.14
```

## Performance

```yaml
schema_version: 1
id: performance.rtx3070_60fps

target_fps: 60
target_resolution: [2560, 1440]

terrain:
  maximum_density_jobs: 4
  maximum_mesh_jobs: 4
  mesh_uploads_per_frame: 2
  collider_builds_per_frame: 1

shadows:
  enabled: true
  quality: high

vegetation:
  density_multiplier: 1.0
  maximum_distance_m: 80.0

water:
  quality: high
```

---

# 29. Final acceptance scenario

A release build must allow the player to:

1. start near the shoreline;
2. orbit the camera using left mouse;
3. rotate camera and character using right mouse;
4. zoom using the wheel;
5. move using camera-relative WASD;
6. run and jump;
7. walk into shallow water;
8. traverse beach and grass biomes;
9. climb a gentle slope;
10. fail to climb an intentionally steep cliff;
11. walk beneath an overhang;
12. enter a cave;
13. experience camera obstruction correction;
14. descend into a cave chamber;
15. interact with the cave object;
16. see the object alter light or appearance;
17. exit the cave;
18. retain at least 60 FPS during normal gameplay.

---

# 30. Definition of done

The vertical slice is complete when all of the following are true.

## Voxel system

* canonical chunks are `16 × 16 × 16` cells;
* chunks contain `17 × 17 × 17` signed-density samples;
* negative coordinates work;
* shared boundaries match;
* caves and overhangs are represented volumetrically;
* chunk seams are not visible;
* terrain generation is deterministic;
* debug edits correctly remesh every affected chunk.

## Rendering

* Surface Nets terrain renders correctly;
* normals remain smooth across chunks;
* terrain uses triplanar material blending;
* beach, grassland, rock, and cave biomes are distinct;
* lighting and shadows work outside and underground;
* water is animated and transparent;
* sparse vegetation is biome-driven.

## Player

* WASD is camera-relative;
* walking, running, jumping, and falling work;
* slope and step handling are stable;
* the player can traverse the cave;
* the orbital camera works according to MMO controls;
* camera collision works in confined spaces.

## Data architecture

* all major configuration comes from typed YAML;
* definitions use stable IDs;
* invalid data fails with actionable diagnostics;
* visual settings hot reload;
* terrain settings trigger controlled regeneration;
* simulation systems use compiled runtime data rather than raw YAML.

## Architecture

* voxels are not ECS entities;
* terrain generation is independent of Bevy;
* meshing returns plain Rust data;
* Bevy asset creation occurs on the main thread;
* physics is wrapped behind project interfaces;
* advanced systems have explicit extension points;
* no deferred feature requires replacing the core density or chunk system.

## Performance

* Windows release build;
* RTX 3070;
* 2560 × 1440;
* 60 FPS during normal traversal;
* no persistent generation hitches;
* no uncontrolled per-frame allocations;
* no unnecessary remeshing or collider rebuilding.

This plan commits the project to a **one-meter, signed-density voxel world using cubic `16³` chunks, Surface Nets rendering, project-owned terrain and controller abstractions, YAML-driven content, and a deliberately authored/procedural coastal test environment**.

[1]: https://bevy.org/news/bevy-0-19/?utm_source=chatgpt.com "Bevy 0.19"
[2]: https://docs.rs/crate/bevy/latest/source/docs/cargo_features.md?utm_source=chatgpt.com "bevy 0.19.0 - Docs.rs"
