# Rust + Bevy Voxel RPG Vertical Slice Expansion

## Progressive Refinement Plan

## 1. Expansion objective

Evolve the original vertical slice into a larger and more flexible environmental gameplay prototype without prematurely implementing the complete archipelago engine.

The expanded slice should prove:

* smooth third-person traversal over varied terrain;
* a more complete physics environment;
* scalable terrain generation beyond one small showcase area;
* several distinct elevation zones;
* sea-level water and elevated inland water;
* a small generated river with a carved channel;
* outdoor, cave, shoreline, and underwater visual transitions;
* a flexible lighting and atmosphere stack;
* multiple fog types;
* sky elements that can later participate in day/night and weather;
* stable asynchronous generation over a larger field;
* clean separation between simulation data and rendering systems.

The refinement should remain a vertical slice. It should demonstrate extensibility, not attempt to become the final world generator.

---

# 2. Preserve the existing architectural decisions

Do not replace the following systems:

```text
Signed-density terrain
16 × 16 × 16 cell chunks
17 × 17 × 17 density samples
Surface Nets initial mesher
One-meter voxel scale
YAML-driven definitions
Asynchronous density and meshing jobs
Project-owned physics abstraction
Bevy-owned presentation layer
Deterministic world-coordinate sampling
```

The expansion should introduce additional implementations and data types behind these existing interfaces.

Examples:

```text
TerrainMesher
    SurfaceNetsMesher
    FutureDualContouringMesher

WaterRenderer
    PlanarWaterRenderer
    RiverRibbonRenderer
    FutureVolumetricWaterRenderer

SkyRenderer
    GradientSkyRenderer
    AtmosphericSkyRenderer
    FutureWeatherSkyRenderer
```

The goal is gradual substitution rather than architectural replacement.

---

# 3. Expanded slice scale

## 3.1 Initial slice

The original slice uses:

```text
6 × 3 × 6 chunks
96 × 48 × 96 meters
108 possible chunk positions
```

Retain this as the compact development and regression-test world.

Call it:

```text
World profile: compact_slice
```

## 3.2 Expanded slice

Add a larger profile:

```text
16 × 6 × 16 chunks
256 × 96 × 256 meters
1,536 possible chunk positions
```

Suggested logical ranges:

```text
Chunk X: -8 through 7
Chunk Y: -2 through 3
Chunk Z: -8 through 7
```

Not every vertical chunk will contain geometry.

This expanded field is large enough for:

* a coastal bay;
* a raised inland plateau;
* two or three hills;
* a rocky ridge;
* a river source;
* a river channel;
* a small elevated pool or lake;
* a waterfall or steep cascade stub;
* grassland;
* beach;
* wetland;
* rock outcrops;
* at least one cave;
* several traversal loops;
* longer camera sightlines;
* basic chunk loading and unloading tests.

## 3.3 Do not keep all chunks active

The larger world should introduce a primitive interest-based residency system.

Recommended initial radii:

```text
Density data radius:    8 chunks
Render radius:          6 chunks
Physics radius:         4 chunks
Decoration radius:      4 chunks
High-detail radius:     3 chunks
```

For the expanded slice, the entire world may still be generated deterministically from a single recipe, but only nearby chunks should maintain render meshes and colliders.

This is the first step toward large-world streaming without implementing the full archipelago streamer.

---

# 4. Revised terrain layout

The expanded environment should form a compact drainage basin descending toward a tropical coast.

## Suggested layout

```text
North or northeast:
    upland ridge
    river spring
    elevated pool
    rocky terrain
    cave entrance

Center:
    sloped valley
    river channel
    grassland
    mixed vegetation
    exposed rock

Southwest:
    river mouth
    wetland or lagoon edge
    beach
    shallow sea

Underground:
    cave chamber
    partially flooded low section
    optional connection near the river valley
```

## Traversal routes

Create several overlapping routes:

```text
Route A:
Beach → grassland → ridge → spring

Route B:
Beach → riverbank → valley → elevated pool

Route C:
Grassland → overhang → cave → alternate exit

Route D:
Ridge → steep descent → waterfall overlook → river mouth
```

The player should be able to return to the starting area without retracing the exact route.

## 4.1 Expanded showcase island layout (data-driven)

The `world.expanded_slice` profile extends the drainage basin into a bounded tropical island authored entirely in YAML:

| Feature | Coordinates (approx.) | YAML owner |
|---------|----------------------|------------|
| Island mask / ocean floor | center `[128, 128]`, radius 92 m | `terrain.expanded_slice` (`island_mask`, `ocean_floor`) |
| Summit peak (>50 m) | `[188, 178]` | `mountain_peak` op |
| Deep ocean floor (< −20 m) | offshore `[220, 45]` | `island_mask.ocean_floor_y`, `ocean_floor` |
| Upland pool + river | `[82, 196]` | `hydrology/upland_pool.yaml`, `hydrology/demo_river.yaml` |
| Demo + flooded cave | `[24–26, 10–12]` | `caves/demo_cave.yaml`, `caves/expanded_cave.yaml` |
| Coastal fort (hybrid pad + props) | `[48, 185]` | terrain pad + `structures/coastal_fort.yaml` |
| Beach spawn | `[70, 160]` | `terrain.expanded_slice` spawn |
| Dynamic clouds | — | `sky.expanded_showcase.yaml` via world `sky:` ref |
| Landmarks / fog volumes | semantic facts | `landmarks/expanded_slice.landmarks.yaml` |
| Traversal routes A–D | waypoints | `routes/expanded_slice.routes.yaml` |

Toggle the expanded profile in the options panel (`use_expanded_profile`) if you want to compare against the compact 96 m basin; it is **on by default** at startup. Profile switches regenerate terrain automatically.

**Startup flow:** Loading → title screen (Start / Options / Quit) → play. **Esc** or **F11** opens the options panel from the title screen or in-game.

---

# 5. Terrain generation refinement

## 5.1 Move from one surface function to composable terrain fields

Replace a single monolithic height function with a compiled terrain-field stack.

```rust
pub struct TerrainFieldStack {
    pub base_shape: Vec<Box<dyn ScalarField2d>>,
    pub modifiers: Vec<Box<dyn TerrainModifier>>,
    pub volumetric_additions: Vec<Box<dyn DensityField3d>>,
    pub volumetric_subtractions: Vec<Box<dyn DensityField3d>>,
}
```

Conceptual pipeline:

```text
Base coastal gradient
+ broad hills
+ valley basin
+ ridge fields
+ low-frequency variation
+ local roughness
- river channel
- lake basin
+ cliff masses
- caves and overhang cavities
```

Each field should have:

* a stable ID;
* its own seed;
* its own bounds;
* optional masks;
* amplitude and frequency controls;
* deterministic evaluation;
* debug visualization.

## 5.2 Introduce terrain feature masks

Use aligned masks for:

* coast;
* upland;
* valley;
* river corridor;
* cave region;
* cliff region;
* wetland;
* beach;
* vegetation exclusion;
* authored landmarks.

```rust
pub struct TerrainMask {
    pub id: TerrainMaskId,
    pub bounds: WorldAabb2,
    pub samples: Vec<f32>,
    pub resolution: UVec2,
}
```

Masks should blend with smooth falloff rather than hard boundaries.

This matches the broader terrain architecture in which large-scale surface fields are generated first and later converted into authoritative volumetric density, with caves and overhangs added in three dimensions.

## 5.3 Terrain variety targets

The expanded terrain must contain deliberate examples of:

* nearly level beach;
* gentle traversable slopes;
* rolling lowlands;
* steep but walkable paths;
* unwalkable cliff faces;
* convex hilltops;
* concave valleys;
* exposed rock shelves;
* undercut terrain;
* a cave ceiling;
* a riverbank;
* submerged shoreline;
* a raised lake shore.

The terrain should not simply become noisier. Variety should come from recognizable landforms.

---

# 6. Small river-generation and carving algorithm

The river should be generated from the surface representation before final density materialization.

It does not need full hydrology or fluid simulation.

## 6.1 River requirements

The first river should:

* begin at an elevated spring or pool;
* descend continuously toward the sea;
* avoid climbing uphill;
* remain inside the playable region;
* have a recognizable source, channel, banks, and mouth;
* widen gradually downstream;
* carve into the terrain;
* support a different water elevation at each segment;
* produce wetness and vegetation masks;
* optionally form one small waterfall or rapid.

## 6.2 Generation stages

```text
1. Generate provisional surface height.
2. Select source point.
3. Select destination at sea or lagoon.
4. Trace downhill route.
5. Resolve shallow local depressions.
6. Smooth the route.
7. Assign longitudinal water elevations.
8. Carve channel and banks.
9. Generate river surface geometry.
10. Register river semantic data.
```

## 6.3 Source selection

Choose a source candidate according to:

```text
high elevation
moderate local slope
sufficient distance from sea
inside upland mask
not inside cave or cliff exclusion
```

```rust
pub struct RiverSourceCandidate {
    pub position: Vec2,
    pub elevation: f32,
    pub suitability: f32,
}
```

For the controlled slice, the YAML recipe may define a source search region or an approximate anchor.

```yaml
river:
  source_region:
    center: [82.0, 196.0]
    radius: 24.0

  destination:
    type: nearest_sea
```

## 6.4 Downhill tracing

Use a simplified steepest-descent or D8-style traversal over a temporary surface grid.

For each river node:

1. inspect neighboring cells;
2. prefer the lowest valid neighbor;
3. add slight inertia toward the current direction;
4. penalize sharp turns;
5. reject previously visited cells unless joining an existing channel;
6. stop when sea level is reached.

Example scoring:

```text
score =
    elevation_drop × 4.0
  + direction_continuity × 1.0
  - turn_penalty × 0.8
  - revisit_penalty
  - exclusion_penalty
```

## 6.5 Depression handling

For the slice, use one of two bounded repairs:

### Shallow fill

Raise the working flow elevation slightly until an exit is found.

### Local breach

Search within a limited radius for a lower cell and carve a shallow connecting cut.

Do not implement a complete global priority-flood system yet.

Recommended limits:

```text
Maximum depression repair radius: 12–20 cells
Maximum artificial breach depth: 2–4 meters
```

If no repair is valid, reject the river seed and retry.

## 6.6 Route smoothing

The grid path will initially appear angular.

Convert it into a smoothed centerline:

```text
Grid path
→ remove redundant collinear nodes
→ Chaikin or Catmull-Rom smoothing
→ resample at fixed spacing
```

Keep the smoothed route constrained near the original downhill path.

The final centerline should be represented as:

```rust
pub struct RiverSpline {
    pub points: Vec<RiverControlPoint>,
}

pub struct RiverControlPoint {
    pub position_xz: Vec2,
    pub bed_elevation: f32,
    pub water_elevation: f32,
    pub width: f32,
    pub depth: f32,
    pub discharge: f32,
}
```

## 6.7 Channel carving

At each terrain sample, calculate the distance to the river centerline.

```rust
fn river_carve(
    distance: f32,
    half_width: f32,
    bank_width: f32,
    depth: f32,
) -> f32;
```

Use separate profiles for:

* riverbed;
* inner bank;
* outer bank;
* floodplain transition.

Conceptually:

```text
center:
    maximum excavation

inner bank:
    smooth rise

outer bank:
    shallow terrain blend

outside corridor:
    no change
```

A simple profile can use smoothstep curves:

```rust
let bed_factor =
    1.0 - smoothstep(0.0, half_width, distance);

let bank_factor =
    1.0 - smoothstep(
        half_width,
        half_width + bank_width,
        distance,
    );

height -= bed_factor * channel_depth;
height -= bank_factor * bank_cut_depth;
```

## 6.8 River dimensions

Suggested slice ranges:

```text
Source width:       1.5–2.5 m
Mouth width:        5–8 m
Source depth:       0.3–0.7 m
Lower depth:        1.0–2.0 m
Bank transition:    2–5 m
```

The river should remain modest enough to cross at selected points.

## 6.9 Water elevation

Do not render the river as one horizontal plane.

Each control point gets a water elevation derived from the carved channel grade.

Rules:

```text
water elevation always descends or remains level
minimum water clearance above bed
maximum permitted slope per segment
steep grade becomes rapid or waterfall
```

Example:

```rust
water_y =
    max(
        downstream_water_y,
        bed_y + minimum_depth,
    );
```

Water elevation should be smoothed while preserving monotonic descent.

## 6.10 River validation

Reject or repair rivers that:

* flow uphill;
* intersect themselves excessively;
* leave the playable region;
* carve through protected spawn terrain;
* cut open the cave roof unintentionally;
* produce vertical bank walls everywhere;
* end before reaching a valid water body;
* create disconnected water surfaces.

---

# 7. Flexible water-body model

The original slice uses one static sea level. Replace this with a water-body registry.

```rust
pub struct WaterBody {
    pub id: WaterBodyId,
    pub kind: WaterBodyKind,
    pub bounds: WorldAabb,
    pub surface: WaterSurfaceDefinition,
    pub material: WaterMaterialId,
    pub flow: Option<FlowFieldId>,
    pub tags: BTreeSet<WaterTag>,
}
```

## 7.1 Water-body kinds

```rust
pub enum WaterBodyKind {
    Sea,
    Lake,
    Pond,
    River,
    Spring,
    Waterfall,
    CavePool,
}
```

## 7.2 Water surface types

```rust
pub enum WaterSurfaceDefinition {
    Horizontal {
        elevation: f32,
        polygon: Vec<Vec2>,
    },

    SplineRibbon {
        control_points: Vec<RiverControlPoint>,
    },

    PatchGrid {
        elevations: Vec<f32>,
        size: UVec2,
        origin: Vec2,
        spacing: f32,
    },
}
```

For the expanded slice, implement:

```text
Horizontal sea
Horizontal elevated lake or pond
Spline-ribbon river
Optional vertical waterfall sheet
```

Defer full patch-grid water surfaces.

## 7.3 Sea level

Sea level remains globally configured:

```yaml
sea:
  elevation_m: 0.0
  minimum_depth_m: 1.0
  maximum_visual_depth_m: 24.0
```

The sea surface should be clipped to the local coastal region or generated as a sufficiently large plane.

## 7.4 Elevated lake

Add one inland body at a separate elevation.

```yaml
water_bodies:
  - id: upland_pool
    kind: lake
    elevation_m: 31.5
    depth_m: 2.5
```

The lake basin should be carved before water placement.

## 7.5 Water occupancy query

Gameplay and physics should query a water registry rather than inspecting render meshes.

```rust
pub trait WaterQuery {
    fn water_at(&self, point: Vec3) -> Option<WaterSample>;

    fn surface_height_at(
        &self,
        position_xz: Vec2,
    ) -> Option<f32>;
}
```

```rust
pub struct WaterSample {
    pub body: WaterBodyId,
    pub surface_height: f32,
    pub depth: f32,
    pub flow_velocity: Vec3,
    pub kind: WaterBodyKind,
}
```

This supports later:

* swimming;
* buoyancy;
* current forces;
* wetness;
* drowning;
* water-aware AI;
* water sound;
* underwater fog.

---

# 8. Water rendering refinement

## 8.1 Shared water material system

Use a common parameter structure:

```rust
pub struct WaterMaterialSettings {
    pub shallow_color: LinearRgba,
    pub deep_color: LinearRgba,
    pub absorption: Vec3,
    pub roughness: f32,
    pub fresnel_power: f32,
    pub normal_strength: f32,
    pub normal_scale: f32,
    pub flow_speed: Vec2,
    pub foam_strength: f32,
    pub depth_fade_distance: f32,
}
```

Different water bodies reference different settings.

Examples:

```text
Sea:
    stronger waves
    deeper blue
    broader foam

River:
    directional flow
    lower wave amplitude
    lighter shallow water

Cave pool:
    dark tint
    minimal surface motion
    stronger reflection
```

## 8.2 River mesh

Generate a strip following the river spline.

Each cross-section contains:

* left-bank vertex;
* center-left;
* center-right;
* right-bank vertex, if foam edges are needed.

Attributes may include:

```text
distance downstream
distance from center
flow direction
water depth
foam mask
body ID
```

## 8.3 Shoreline foam

Use a depth-based or bank-distance mask.

Avoid requiring terrain-water intersection geometry to match perfectly.

## 8.4 Underwater rendering

Add a camera-state detector:

```rust
pub struct CameraWaterState {
    pub body: Option<WaterBodyId>,
    pub submerged_depth: f32,
}
```

When submerged:

* shift fog color;
* increase fog density;
* reduce sun intensity;
* reduce maximum visibility;
* apply mild color absorption;
* optionally distort the image;
* change ambient audio state.

Swimming can remain deferred while underwater presentation is tested in shallow regions or debug free-camera mode.

---

# 9. Smooth player movement refinement

The original movement target is responsive MMO-style motion. Expand this into a formal movement-state pipeline.

## 9.1 Separate intent from physical resolution

```rust
pub struct MovementIntent {
    pub direction: Vec2,
    pub requested_speed: MovementSpeed,
    pub jump_pressed: bool,
    pub jump_held: bool,
}
```

```rust
pub struct CharacterMotorState {
    pub velocity: Vec3,
    pub grounded: bool,
    pub ground_normal: Vec3,
    pub current_slope: f32,
    pub locomotion_state: LocomotionState,
}
```

Input should never directly modify transform position.

## 9.2 Fixed-step movement

Run physics and character motion in a fixed schedule.

Suggested starting rate:

```text
Physics step: 60 Hz
Optional substep: 120 Hz for unstable contacts
Rendering: interpolated independently
```

Character visuals should interpolate between fixed simulation states.

## 9.3 Ground movement

Support:

* acceleration;
* deceleration;
* speed-dependent turning;
* camera-relative movement;
* slope projection;
* ground adhesion;
* step climbing;
* steep-slope rejection;
* controlled sliding.

Example configurable values:

```yaml
movement:
  walk_speed: 4.8
  run_speed: 7.5
  ground_acceleration: 28.0
  ground_deceleration: 36.0
  air_acceleration: 8.0
  rotation_half_life: 0.07
  maximum_walkable_slope_deg: 47.0
  step_height_m: 0.45
```

## 9.4 Ground adhesion

Without controlled adhesion, the character may bounce or briefly become airborne when descending uneven smooth terrain.

Use a short downward shape cast after horizontal resolution.

```text
If grounded last step
AND vertical velocity is non-positive
AND ground exists within snap distance
THEN snap to valid ground
```

Suggested snap distance:

```text
0.15–0.30 meters
```

Disable snapping during intentional jump ascent.

## 9.5 Jump refinement

Add:

```text
jump buffering: 80–150 ms
coyote time:    80–120 ms
variable jump:  optional
```

These improve responsiveness without making movement arcade-like.

## 9.6 Slope movement

Project intended motion onto the ground plane.

```rust
let slope_direction =
    desired_world_direction
        .reject_from(ground_normal)
        .normalize_or_zero();
```

Apply speed modifiers only for clearly steep inclines.

Do not make every small terrain normal variation change movement speed.

## 9.7 Rotation

The character should rotate toward:

* movement direction during normal WASD travel;
* camera facing while right mouse is held;
* target-facing direction in future combat mode.

Create an explicit facing mode:

```rust
pub enum FacingMode {
    Movement,
    Camera,
    LockedDirection(Vec3),
    Target(StableEntityId),
}
```

---

# 10. Full physics refinement

“Full physics” for this stage should mean a coherent general-purpose physics layer, not every possible physical simulation.

## 10.1 Implement now

* static terrain colliders;
* kinematic or controlled player capsule;
* dynamic rigid bodies;
* fixed rigid bodies;
* sensors and trigger volumes;
* collision layers;
* friction;
* restitution;
* gravity;
* impulses;
* forces;
* mass;
* damping;
* sleeping;
* moving platforms;
* physical props;
* simple break or despawn thresholds;
* water-volume queries;
* primitive buoyancy for test objects.

## 10.2 Defer

* soft bodies;
* cloth;
* rope simulation;
* full vehicle dynamics;
* structural collapse simulation;
* fluid pressure;
* deformable bodies;
* ragdolls;
* network rollback physics.

## 10.3 Project-owned physics components

Avoid spreading backend-specific components throughout gameplay code.

```rust
pub struct PhysicsBodySpec {
    pub body_type: PhysicsBodyType,
    pub mass: f32,
    pub friction: f32,
    pub restitution: f32,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub collision_profile: CollisionProfileId,
}
```

```rust
pub enum PhysicsBodyType {
    Static,
    Dynamic,
    Kinematic,
    Sensor,
}
```

The physics bridge converts these definitions into Avian components.

## 10.4 Collision layers

Recommended initial groups:

```text
Terrain
Player
Npc
DynamicProp
StaticProp
WaterSensor
InteractionSensor
CameraProbe
ProjectileFuture
Trigger
```

Camera probes should not interact with sensors or water surfaces unless deliberately requested.

## 10.5 Moving platforms

Include one simple moving platform or raft-like test object.

The character controller should:

* detect supporting body velocity;
* inherit platform displacement;
* avoid sliding off due solely to transform order;
* remain stable when the platform stops.

This tests whether the movement controller is tied incorrectly to static terrain assumptions.

## 10.6 Dynamic props

Add:

* small rocks;
* crates;
* logs;
* one rolling object;
* one floating test object.

These should verify:

* slope interaction;
* collision with terrain;
* collision with the player;
* sleeping;
* water detection;
* basic buoyancy.

## 10.7 Primitive buoyancy

Use one or more sample points per rigid body.

For each submerged point:

```text
submersion = water_surface_y - sample_point_y

if submersion > 0:
    apply upward force
    apply flow force
    apply drag
```

This is enough for crates or logs.

Do not model wave displacement physically yet.

---

# 11. Camera refinement

## 11.1 Preserve MMO controls

Retain:

```text
Left drag:
    orbit independently

Right drag:
    rotate camera and character

Both buttons:
    move forward

Wheel:
    zoom

Home:
    recenter
```

## 11.2 Add camera states

```rust
pub enum CameraMode {
    Exploration,
    TightInterior,
    Underwater,
    DebugFree,
}
```

The modes should adjust:

* minimum distance;
* maximum distance;
* shoulder offset;
* collision sphere radius;
* field of view;
* smoothing;
* fog parameters.

## 11.3 Cave and interior adaptation

Inside tight spaces:

* shorten maximum camera distance;
* reduce lateral shoulder offset;
* increase obstruction recovery speed;
* optionally move toward centered framing.

Do not automatically switch into first-person.

## 11.4 Camera collision stability

Improve the sphere-cast system with:

* obstruction hysteresis;
* minimum hold duration;
* separate inward and outward smoothing;
* ignoring the player collider;
* ignoring vegetation;
* optional multi-hit inspection.

The camera should move inward quickly and recover outward more slowly.

---

# 12. Lighting architecture

Move from a fixed sun setup to a lighting-state resource.

```rust
pub struct EnvironmentLightingState {
    pub sun: DirectionalLightState,
    pub moon: DirectionalLightState,
    pub ambient: AmbientLightState,
    pub sky: SkyState,
    pub fog: FogState,
    pub exposure: ExposureState,
}
```

The first expanded slice may still use a mostly fixed time, but all lighting should be driven through this state.

## 12.1 Sun

Support:

* directional light;
* configurable direction;
* intensity;
* color temperature or color;
* cascade configuration;
* shadow distance;
* shadow bias;
* normal bias;
* optional contact-shadow approximation.

Example YAML:

```yaml
sun:
  azimuth_deg: 132.0
  elevation_deg: 48.0
  illuminance: 85000.0
  color_temperature_k: 5600.0
  shadows: true
  shadow_distance_m: 220.0
  cascades: 4
```

## 12.2 Moon stub

Add a disabled or low-intensity moon definition now.

```yaml
moon:
  enabled: false
  illuminance: 0.15
  phase: 1.0
```

This prepares the system for the later physically inspired moon-phase model.

## 12.3 Ambient lighting

Ambient illumination should differ by environment.

Recommended sources:

```text
Outdoor sky ambient
Cave ambient multiplier
Underwater ambient
Local light contributions
```

Avoid implementing cave darkness solely by lowering the global ambient light.

Instead, use a cave-region or sky-visibility value.

## 12.4 Sky visibility

Create a scalar query or volume tag:

```rust
pub struct SkyVisibility(pub f32);
```

Possible initial approximation:

* outside terrain: `1.0`;
* deep cave: `0.0–0.15`;
* cave entrance: blended;
* under overhang: `0.3–0.7`.

It may initially be authored or derived from cave volumes.

Later it can be replaced by probes or voxel occlusion queries.

## 12.5 Local lights

Implement:

* point lights;
* spotlights;
* emissive material visuals;
* light groups or profiles;
* range;
* intensity;
* flicker profile;
* shadow toggle.

Add examples:

* cave beacon;
* glowing fungus;
* lantern near the river;
* optional underwater emissive object.

## 12.6 Exposure

Add controlled exposure adaptation.

Outdoor-to-cave transitions should not become instantly black or washed out.

Use limits:

```text
minimum exposure
maximum exposure
adaptation speed brighter
adaptation speed darker
```

Adaptation should remain subtle enough that lighting design still matters.

---

# 13. Sky elements

## 13.1 Sky progression

Implement in layers.

### Stage 1: gradient sky

* zenith color;
* horizon color;
* ground-haze color;
* sun-direction brightening.

### Stage 2: atmospheric sky

* approximate Rayleigh color;
* approximate Mie haze;
* sun disc;
* horizon brightening;
* time-controlled sun direction.

### Stage 3: additional celestial elements

* moon disc;
* stars;
* simple cloud layer;
* night gradient.

The expanded vertical slice should complete Stages 1 and 2 and include data hooks for Stage 3.

## 13.2 Sun disc

Render the sun visually in the same direction as the directional light.

The rendered sun and light direction must never diverge.

## 13.3 Moon

Add a renderable moon object or shader parameter even if it is disabled in the default daytime profile.

Store:

```rust
pub struct CelestialBodyState {
    pub direction: Vec3,
    pub angular_radius: f32,
    pub brightness: f32,
    pub phase: f32,
}
```

## 13.4 Stars

Stars may be a simple sky texture or procedural field.

They should fade according to:

* sun elevation;
* sky brightness;
* fog;
* clouds later.

Do not create individual ECS entities for stars.

## 13.5 Simple cloud layer

Clouds are optional for this refinement, but the sky system should support a lightweight layer:

* scrolling texture or procedural noise;
* altitude;
* opacity;
* direction;
* speed;
* sun-light attenuation stub.

Do not implement volumetric clouds yet.

---

# 14. Fog architecture

Replace one global distance-fog setting with layered fog contributors.

```rust
pub struct FogStack {
    pub global_distance: Option<DistanceFog>,
    pub height: Option<HeightFog>,
    pub local_volumes: Vec<LocalFogVolume>,
    pub underwater: Option<UnderwaterFog>,
}
```

## 14.1 Distance fog

Purpose:

* soften the edge of the active terrain field;
* increase depth perception;
* prevent distant terrain from appearing excessively sharp.

Parameters:

```text
start distance
full-opacity distance
density curve
color
sun scattering strength
```

## 14.2 Height fog

Useful for:

* coastal haze;
* humid valleys;
* low-lying wetlands;
* reduced visibility near sea level.

```rust
pub struct HeightFog {
    pub base_height: f32,
    pub density: f32,
    pub falloff: f32,
    pub color: LinearRgba,
}
```

The valley and coast may have denser fog than the upland ridge.

## 14.3 Local fog volumes

Use bounded volumes for:

* cave chamber haze;
* waterfall mist;
* wetland mist;
* river-mouth humidity;
* dust or spores.

```rust
pub struct LocalFogVolume {
    pub shape: FogVolumeShape,
    pub density: f32,
    pub color: LinearRgba,
    pub noise_strength: f32,
    pub priority: i16,
}
```

Initial volume shapes:

```text
Box
Sphere
Capsule
```

These may initially affect camera postprocessing rather than participating in true volumetric lighting.

## 14.4 Underwater fog

Underwater fog should be selected from the active `WaterBody`.

Parameters may differ between:

* clear upland pool;
* silty river;
* ocean;
* cave pool.

## 14.5 Fog transition rules

Blend fog over time and space.

Avoid abrupt changes when:

* entering the cave;
* crossing the water surface;
* leaving a local fog volume;
* moving between beach and upland.

---

# 15. Biome and surface-material expansion

Expand the initial set:

```text
Beach
Coastal grassland
Wet riverbank
Upland grassland
Rocky ridge
Cave
Shallow sea
Freshwater
Wetland
```

## 15.1 Additional classification inputs

Add:

* normalized river distance;
* normalized lake distance;
* floodplain mask;
* water-body type;
* local drainage;
* sky visibility;
* terrain exposure;
* wetness;
* sediment value.

```rust
pub struct BiomeSampleContext {
    pub elevation: f32,
    pub slope_degrees: f32,
    pub distance_to_sea: f32,
    pub distance_to_fresh_water: f32,
    pub river_influence: f32,
    pub floodplain: f32,
    pub cave_cover: f32,
    pub moisture: f32,
    pub exposure: f32,
    pub transition_noise: f32,
}
```

## 15.2 Wetness

Represent wetness separately from material identity.

```rust
pub struct SurfaceModifiers {
    pub wetness: f32,
    pub moss: f32,
    pub sediment: f32,
    pub snow_future: f32,
}
```

Wetness may affect:

* roughness;
* darkening;
* specularity;
* vegetation;
* footstep audio later;
* traction later.

## 15.3 River materials

Add:

* mud;
* gravel;
* river stone;
* wet rock;
* sediment-rich sand.

These may share textures through parameter variation rather than requiring wholly separate material sets.

---

# 16. Vegetation refinement

Vegetation should respond to the new water and terrain fields.

## Placement inputs

```text
Biome
Slope
Material
Wetness
Distance to river
Distance to sea
Floodplain value
Sky visibility
Exposure
Spacing
```

## New vegetation examples

```text
Beach:
    sparse grasses
    driftwood
    coastal shrubs

Riverbank:
    reeds
    ferns
    moisture-loving plants

Wetland:
    dense low plants
    shallow-water vegetation

Upland:
    shorter grass
    sparse shrubs
    exposed-rock plants

Cave:
    moss
    fungus
```

Vegetation should not be generated in the river navigation channel or on required crossing points.

---

# 17. Environmental audio hooks

Full audio design may remain deferred, but register environmental zones now.

```rust
pub struct EnvironmentAudioProfile {
    pub ambient_loop: AudioProfileId,
    pub reverb: ReverbProfileId,
    pub water_loop: Option<AudioProfileId>,
    pub wind_profile: Option<AudioProfileId>,
}
```

Profiles:

```text
Coast
River
Upland
Cave
Underwater
Waterfall mist
```

The river spline should support audio emitters based on flow intensity.

---

# 18. Expanded YAML organization

Add:

```text
assets/config/
    physics.yaml
    atmosphere.yaml
    fog.yaml
    sky.yaml
    environment_transitions.yaml

assets/terrain/
    worlds/
        compact_slice.world.yaml
        expanded_slice.world.yaml

    generation/
        compact_slice.terrain.yaml
        expanded_slice.terrain.yaml

    hydrology/
        demo_river.yaml
        upland_pool.yaml

    water/
        sea.water.yaml
        river.water.yaml
        freshwater.water.yaml
        cave_pool.water.yaml

    lighting/
        tropical_day.lighting.yaml
        cave.lighting.yaml

    fog/
        coastal_haze.fog.yaml
        cave_haze.fog.yaml
        underwater.fog.yaml
```

Example river definition:

```yaml
schema_version: 1
id: river.demo_upland

source:
  region_center: [82.0, 196.0]
  region_radius_m: 24.0
  minimum_elevation_m: 24.0

destination:
  type: nearest_water_body
  required_kind: sea

routing:
  grid_spacing_m: 2.0
  direction_inertia: 0.65
  maximum_turn_deg: 80.0
  depression_repair_radius_cells: 12
  maximum_breach_depth_m: 3.0

channel:
  source_width_m: 1.8
  mouth_width_m: 6.5
  source_depth_m: 0.4
  mouth_depth_m: 1.6
  bank_width_m: 3.5

water:
  minimum_depth_m: 0.25
  maximum_segment_slope: 0.08
  waterfall_threshold_m: 2.5
```

---

# 19. Runtime resources

Recommended resources:

```rust
pub struct TerrainWorldRuntime {
    pub world_id: WorldDefinitionId,
    pub seed: u64,
    pub bounds: WorldAabb,
    pub revision: u64,
}

pub struct TerrainFeatureRegistry {
    pub rivers: BTreeMap<RiverId, RiverSpline>,
    pub water_bodies: BTreeMap<WaterBodyId, WaterBody>,
    pub caves: BTreeMap<CaveId, CaveDescriptor>,
}

pub struct EnvironmentState {
    pub lighting: EnvironmentLightingState,
    pub fog: FogStack,
    pub active_profile: EnvironmentProfileId,
}

pub struct PhysicsWorldSettings {
    pub gravity: Vec3,
    pub fixed_timestep: f32,
    pub maximum_substeps: u8,
}
```

---

# 20. Revised plugin organization

```rust
pub struct ExpandedVerticalSlicePlugin;

impl Plugin for ExpandedVerticalSlicePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            DataAssetPlugin,
            TerrainWorldPlugin,
            TerrainGenerationPlugin,
            TerrainFeaturePlugin,
            RiverGenerationPlugin,
            WaterBodyPlugin,
            TerrainMeshingPlugin,
            TerrainRenderingPlugin,
            ChunkResidencyPlugin,
            PhysicsBridgePlugin,
            CharacterMotorPlugin,
            DynamicPropPlugin,
            OrbitCameraPlugin,
            BiomePlugin,
            VegetationPlugin,
            EnvironmentLightingPlugin,
            SkyPlugin,
            FogPlugin,
            WaterRenderingPlugin,
            EnvironmentAudioStubPlugin,
            DebugToolsPlugin,
        ));
    }
}
```

Keep the river algorithm independent from rendering.

```text
RiverGenerationPlugin:
    generates spline and carve data

TerrainGenerationPlugin:
    applies the channel to the terrain surface

WaterBodyPlugin:
    registers water occupancy and elevations

WaterRenderingPlugin:
    constructs visible river and lake surfaces
```

---

# 21. Progressive implementation phases

## Refinement Phase A — Movement and physics foundation

Implement:

* fixed-step physics schedule;
* input intent;
* smoothed character motor;
* acceleration and deceleration;
* ground snapping;
* slope projection;
* jump buffer;
* coyote time;
* stable moving-platform handling;
* collision layers;
* dynamic test props.

Deliverable:

```text
The player and physical props behave consistently on a terrain test course.
```

Exit gate:

* movement is frame-rate independent;
* character does not bounce on ordinary slopes;
* no false grounding on cave ceilings;
* moving-platform support is stable;
* dynamic objects sleep correctly;
* camera collision remains independent.

---

## Refinement Phase B — Expanded terrain field

Implement:

* `expanded_slice` world profile;
* basic chunk interest provider;
* render, physics, and decoration radii;
* larger terrain bounds;
* ridge and valley fields;
* longer traversal routes;
* sparse chunk residency;
* chunk generation prioritization.

Deliverable:

```text
A 256 × 96 × 256-meter terrain can be explored without loading every render mesh and collider simultaneously.
```

Exit gate:

* no visible chunk cracks;
* generation is prioritized near the player;
* stale job results are discarded;
* collider creation remains budgeted;
* sustained movement does not produce severe frame spikes.

---

## Refinement Phase C — Water-body registry

Implement:

* `WaterBodyId`;
* sea body;
* elevated lake body;
* water queries;
* camera submersion state;
* body-specific water material profiles;
* simple lake basin carving.

Deliverable:

```text
The world contains ocean water and an elevated freshwater pool at different elevations.
```

Exit gate:

* water queries return correct bodies and depths;
* elevated water does not affect sea-level logic;
* camera transitions underwater correctly;
* water rendering does not z-fight with terrain.

---

## Refinement Phase D — River routing and carving

Implement:

* provisional height grid;
* source selection;
* downhill route tracing;
* limited depression repair;
* route smoothing;
* channel-width interpolation;
* terrain carving;
* bank generation;
* river water elevations;
* spline-ribbon mesh.

Deliverable:

```text
A deterministic river flows from the upland pool to the sea through a visibly carved channel.
```

Exit gate:

* river reaches the sea;
* river never flows materially uphill;
* water remains above the channel bed;
* banks are traversable at intended crossings;
* no accidental cave rupture;
* repeated seeds produce identical river data.

---

## Refinement Phase E — Lighting-state architecture

Implement:

* environment lighting resource;
* YAML-driven sun;
* cascaded shadow configuration;
* ambient profiles;
* local point and spot lights;
* sky-visibility approximation;
* exposure adaptation;
* cave lighting transition.

Deliverable:

```text
Beach, upland, overhang, cave entrance, and cave chamber remain visually distinct and readable.
```

Exit gate:

* sun direction matches the sky sun;
* overhangs cast stable shadows;
* cave ambience is not controlled solely by global light;
* exposure transitions do not flash;
* local lights can be hot-reloaded.

---

## Refinement Phase F — Sky system

Implement:

* gradient sky;
* approximate atmospheric scattering;
* sun disc;
* horizon haze;
* moon data stub;
* star data stub;
* optional simple scrolling cloud layer.

Deliverable:

```text
The sky feels integrated with lighting rather than being a flat clear color.
```

Exit gate:

* sky orientation follows the sun;
* horizon color blends with fog;
* no visible sky seam;
* sky parameters reload without rebuilding terrain.

---

## Refinement Phase G — Fog stack

Implement:

* distance fog;
* height fog;
* cave fog volume;
* waterfall or river mist volume;
* underwater fog;
* transition blending.

Deliverable:

```text
Coast, upland, cave, mist, and underwater areas use distinct but smoothly transitioning atmospheric states.
```

Exit gate:

* fog hides the active-field edge;
* height fog does not fill high ridges excessively;
* entering water does not cause a one-frame flash;
* local volumes compose predictably;
* debug overlays show active fog contributors.

---

## Refinement Phase H — Water physics and interaction proof

Implement:

* water sensors;
* simple rigid-body buoyancy;
* flow force along the river;
* wetness state stub;
* shallow-water movement modifier;
* deep-water boundary or reset;
* floating crate or log test.

Deliverable:

```text
Physical objects float and drift, while the player can enter shallow water without full swimming.
```

Exit gate:

* objects do not accelerate without limit;
* flow follows the river direction;
* water body transitions are stable;
* shallow-water movement is deterministic.

---

## Refinement Phase I — Biome and vegetation expansion

Implement:

* river-distance and wetness inputs;
* wetland biome;
* riverbank materials;
* freshwater vegetation;
* wet-rock modifiers;
* floodplain mask;
* deterministic placements.

Deliverable:

```text
The river, lake, coast, upland, and cave have visually distinct ecological treatment.
```

Exit gate:

* vegetation respects water and slope;
* crossings remain unobstructed;
* wetness transitions are not hard bands;
* vegetation remains within performance budget.

---

## Refinement Phase J — Final integration and polish

Implement:

* environmental audio hooks;
* route signage or landmarks;
* waterfall overlook;
* cave object interaction;
* debug profile switching;
* compact and expanded world presets;
* final profiling;
* automated traversal tests.

Deliverable:

```text
A cohesive 10–20 minute exploration slice demonstrating terrain, water, physics, movement, lighting, sky, fog, and environmental transitions.
```

---

# 22. Debug tooling additions

Add controls or panels for:

```text
World profile switch
Chunk residency view
Physics body view
Character ground probe
Character velocity
Slope classification
Water-body bounds
Water surface elevations
River spline
River flow arrows
River carve profile
Fog contributors
Sky visibility
Sun direction
Shadow cascades
Exposure value
Biome and wetness
```

Suggested controls:

```text
F11  Environment debug panel
F12  Water and river debug view

Ctrl+F2  Toggle physics bodies
Ctrl+F3  Toggle water-body bounds
Ctrl+F4  Toggle river spline
Ctrl+F5  Toggle fog volumes
Ctrl+F6  Toggle lighting probes
Ctrl+F7  Toggle residency rings
```

---

# 23. Testing additions

## River unit tests

* downhill selection;
* deterministic tie-breaking;
* path termination;
* no repeated loop;
* depression repair;
* channel width interpolation;
* monotonic water elevation;
* bed below water surface;
* distance-to-spline query.

## Water tests

* sea query;
* elevated lake query;
* overlapping body priority;
* river surface interpolation;
* underwater camera detection;
* boundary tolerance.

## Character tests

* flat-ground acceleration;
* slope projection;
* ground snapping;
* jump buffer;
* coyote time;
* steep-slope rejection;
* moving platform;
* riverbank traversal;
* shallow-water modifier.

## Physics tests

* rigid body falls and rests;
* friction changes slope behavior;
* restitution affects bounce;
* sensor does not physically block;
* buoyant body stabilizes;
* flow force points downstream.

## Lighting tests

* sun and sky directions agree;
* cave profile blends;
* local light profile reload;
* exposure remains within configured range.

## Fog tests

* contributor priority;
* underwater override;
* smooth transition;
* height-fog falloff;
* local-volume boundary.

## Streaming tests

* chunks load in priority order;
* colliders unload outside radius;
* water registry remains available when render chunks unload;
* river continuity survives chunk boundaries;
* player cannot outrun required collision generation under normal speeds.

---

# 24. Performance targets

Maintain:

```text
Target: 60 FPS
Resolution: 2560 × 1440
GPU: RTX 3070
```

Expanded working budget:

| Area                            |         Target |
| ------------------------------- | -------------: |
| Gameplay and ECS                |         2.0 ms |
| Character and physics           |         2.0 ms |
| Terrain residency and lifecycle | 1.0 ms average |
| Render preparation              |         2.0 ms |
| Terrain and vegetation GPU      |         3.5 ms |
| Shadows and lighting GPU        |         3.0 ms |
| Sky, fog, and water GPU         |         1.5 ms |
| UI and diagnostics              |         0.5 ms |
| Margin                          |        1.17 ms |

## Required profiling cases

```text
Standing at beach with long sightline
Walking rapidly along river
Entering cave
Emerging from cave
Looking across water
Underwater debug camera
Chunk loading at maximum run speed
Terrain regeneration
River debug overlay enabled
Multiple dynamic props near water
```

---

# 25. Explicit deferrals after expansion

Even after this refinement, defer:

* full world-scale hydrology;
* rainfall-driven river discharge;
* seasonal rivers;
* erosion simulation at runtime;
* tides;
* ocean currents;
* swimming;
* boats;
* dynamic waves affecting physics;
* fluid voxels;
* water diversion;
* flooding;
* sediment transport;
* volumetric clouds;
* complete day/night cycle;
* moon phases;
* weather fronts;
* global illumination;
* large-scale horizon LOD;
* complete archipelago streaming;
* structural collapse;
* destructible gameplay terrain;
* NPC navigation across the full world.

The expanded slice should leave clean interfaces for these systems but should not implement them prematurely.

---

# 26. Final acceptance criteria

The refinement is complete when:

## World

* The player can explore a deterministic `256 × 96 × 256`-meter terrain.
* Terrain contains coastline, upland, valley, cave, overhang, river, and lake.
* Nearby chunks load and unload through residency rules.

## Movement

* Camera-relative movement is smooth and responsive.
* Slopes, steps, jumping, falling, and moving platforms behave consistently.
* Character motion remains stable across chunk seams.

## Physics

* Static, dynamic, kinematic, and sensor bodies coexist.
* Props roll, collide, sleep, float, and respond to water flow.
* Gameplay code is not tightly coupled to the physics backend.

## Water

* Sea and inland water exist at different elevations.
* The river descends from inland water to the sea.
* The river visibly carves the terrain.
* Water queries are independent from water rendering.
* Underwater fog and camera state function.

## Lighting and sky

* Sun, shadows, ambient light, local lights, and exposure are data-driven.
* Sky rendering matches the sun direction.
* Cave and exterior lighting transition smoothly.
* Moon, stars, and clouds have stable extension points.

## Fog

* Distance, height, local, cave, mist, and underwater fog can coexist.
* Fog transitions are smooth.
* Fog helps conceal the terrain residency boundary.

## Performance

* The expanded slice remains near the 60 FPS target on the target hardware.
* Chunk generation, mesh upload, and collider creation remain budgeted.
* Normal traversal does not produce severe recurring stalls.

## Architecture

* Compact and expanded worlds use the same systems.
* River generation is independent from rendering.
* Water simulation queries are independent from shaders.
* Lighting state is independent from individual light entities.
* Physics abstractions remain project-owned.
* Every major parameter is validated and data-driven.
