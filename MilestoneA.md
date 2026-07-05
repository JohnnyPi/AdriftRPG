# Milestone A — Single-Island World Compiler

## Expanded implementation plan for Phases 0–7

### Milestone objective

Build a deterministic, data-driven **world compiler** that transforms composable YAML recipes into a validated, finite `WorldAtlas` containing:

* a bounded deep-ocean basin,
* exactly one volcanic island,
* a coherent island footprint and volcanic skeleton,
* macro elevation and bathymetry,
* geological age, bedrock, hardness, and erodibility,
* seam-free regional surface refinement,
* and a runtime-facing density provider that hides all compiler internals.

The key architectural decision is to treat this as a compiler rather than a collection of runtime noise functions:

```text
YAML source assets
        ↓
Parsed source definitions
        ↓
Resolved and validated world recipe
        ↓
Ordered compiler passes
        ↓
WorldAtlas + IslandBlueprint
        ↓
CompiledWorldArtifact
        ↓
WorldDensityProvider
        ↓
Runtime voxel/chunk generation
```

This follows the existing project direction: hierarchical generation, aligned 2D fields, typed YAML, explicit passes, and eventual density-field materialization. 

Bevy 0.19 is the current target and provides the asset-loading, task-pool, asset-event, and generated-mesh APIs needed for the engine adapter, but the actual terrain compiler should remain a pure Rust library with no Bevy dependency. Bevy 0.19 was released on June 19, 2026. ([Bevy][1])

---

# 1. Final architectural boundaries

## 1.1 Crate responsibilities

```text
crates/
├── terrain_generation/
│   ├── pure Rust world compiler
│   ├── field storage and sampling
│   ├── island algorithms
│   ├── geology
│   ├── regional refinement
│   └── compiled density provider
│
├── game_data/
│   ├── YAML source definitions
│   ├── schema versions
│   ├── ID and reference types
│   ├── migrations
│   └── semantic validation
│
├── terrain_storage/
│   ├── compiled atlas serialization
│   ├── tile cache
│   ├── manifests
│   └── content hashes
│
└── game_bevy/
    ├── Bevy AssetLoader adapters
    ├── compilation task scheduling
    ├── diagnostic presentation
    ├── hot-reload handling
    └── runtime chunk requests
```

`terrain_generation` should accept ordinary Rust structures and return ordinary Rust structures. It should not query ECS resources, spawn entities, create Bevy assets, or depend on frame timing.

## 1.2 Authoring, compilation, and runtime types

Keep three distinct representations.

### Source definitions

Close to the YAML layout:

```rust
pub struct WorldRecipeSource {
    pub schema_version: u32,
    pub id: WorldRecipeId,
    pub seed: u64,
    pub extent: ExtentSource,
    pub boundary: RecipeRef<BoundaryRecipeId>,
    pub island: RecipeRef<IslandRecipeId>,
    pub geology: RecipeRef<GeologyRecipeId>,
    pub refinement: RecipeRef<RefinementRecipeId>,
}
```

### Compiled definitions

All references resolved, units normalized, ranges checked, and derived constants precomputed:

```rust
pub struct CompiledWorldRecipe {
    pub id: WorldRecipeId,
    pub seed: u64,
    pub version: GeneratorVersion,
    pub extent: WorldExtent,
    pub boundary: CompiledBoundaryRecipe,
    pub islands: Vec<CompiledIslandRecipe>,
    pub geology: CompiledGeologyRecipe,
    pub refinement: CompiledRefinementRecipe,
    pub recipe_hash: RecipeHash,
}
```

### Generated products

```rust
pub struct CompiledWorld {
    pub manifest: WorldManifest,
    pub atlas: WorldAtlas,
    pub islands: Vec<IslandBlueprint>,
}
```

The runtime should consume only `CompiledWorld` or a thinner runtime artifact.

---

# 2. Cross-cutting implementation rules

## 2.1 Coordinate convention

Lock this down before adding any new generator.

Recommended convention:

```text
World axis:
    +X = east
    +Y = up
    +Z = north

Horizontal compiler coordinates:
    DVec2(x, z)

Vertical units:
    meters

Sea level:
    elevation 0.0 meters

World origin:
    center of the finite world

Field row/column:
    x increases across columns
    z increases across rows

Grid sample:
    explicit sample position, never inferred ambiguously
```

Use `f64` for world-space coordinates and parameter calculations, especially for large finite worlds. Store most atlas samples as `f32` unless a specific field requires more precision.

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldXZ(pub glam::DVec2);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldPosition(pub glam::DVec3);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElevationMeters(pub f32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellSizeMeters(pub f64);
```

Do not pass naked `Vec2`, `f32`, or `u32` values through compiler APIs where units or coordinate domains could be confused.

## 2.2 Deterministic seeds

Do not maintain one global random-number generator whose state depends on pass order.

Derive every stochastic stream from stable inputs:

```text
world seed
+ generator version
+ pass identifier
+ island identifier
+ tile coordinate
+ feature identifier
+ iteration
```

```rust
pub fn derive_seed(
    world_seed: u64,
    namespace: &str,
    coordinate: Option<TileCoord>,
    local_id: u64,
) -> u64 {
    let mut hasher = blake3::Hasher::new();

    hasher.update(&world_seed.to_le_bytes());
    hasher.update(namespace.as_bytes());

    if let Some(coord) = coordinate {
        hasher.update(&coord.x.to_le_bytes());
        hasher.update(&coord.z.to_le_bytes());
    }

    hasher.update(&local_id.to_le_bytes());

    let bytes = hasher.finalize();
    u64::from_le_bytes(bytes.as_bytes()[0..8].try_into().unwrap())
}
```

BLAKE3’s Rust implementation provides fixed 32-byte hashes and incremental hashing, making it suitable for stable recipe and cache identities. ([Docs.rs][2])

## 2.3 Pure pass semantics

Every generation pass should have:

```text
declared inputs
declared outputs
validated configuration
deterministic seed namespace
temporary working buffers
explicit commit point
pass report
```

Avoid passes that mutate arbitrary portions of the atlas.

```rust
pub trait WorldgenPass: Send + Sync {
    fn key(&self) -> PassKey;

    fn inputs(&self) -> &'static [FieldKey];

    fn outputs(&self) -> &'static [FieldKey];

    fn execute(
        &self,
        context: &PassContext<'_>,
        output: &mut PassOutput,
    ) -> Result<PassReport, WorldgenError>;
}
```

`PassOutput` should be committed only after validation succeeds.

## 2.4 Stable parallelism

Field generation can use Rayon or Bevy tasks, but parallel execution must not alter output.

Safe operations include:

* sampling independent cells,
* generating independent regional windows,
* computing field statistics in deterministic chunks,
* generating separate island descriptors.

Avoid nondeterministic floating-point reductions. Merge windows, histograms, and accumulated values in a fixed coordinate order.

Rayon exposes parallel iterators and non-overlapping parallel slice chunks, which fit field and tile generation well. ([Docs.rs][3])

---

# Phase 0 — World-generation contract

## Goal

Formalize the stable interface between:

* authored recipes,
* compiler passes,
* compiled world products,
* runtime chunk generation,
* and future systems such as hydrology, caves, erosion, and terrain editing.

This phase should remove runtime knowledge of `footprint.rs`, `volcano.rs`, noise generators, YAML definitions, or generation-pass ordering.

---

## 0.1 Runtime-facing density interface

Extend the existing `DensitySource` and `RecipeDensitySource`, but make the final interface independent of recipes.

```rust
pub trait WorldDensityProvider: Send + Sync + 'static {
    fn world_metadata(&self) -> &WorldMetadata;

    fn sample_density(&self, position: WorldPosition) -> f32;

    fn sample_surface(&self, horizontal: WorldXZ) -> SurfaceSample;

    fn sample_geology(&self, position: WorldPosition) -> GeologySample;

    fn sample_column(&self, horizontal: WorldXZ) -> ColumnSample;
}
```

Suggested samples:

```rust
#[derive(Clone, Copy, Debug)]
pub struct SurfaceSample {
    pub elevation_m: f32,
    pub slope: f32,
    pub macro_normal: glam::Vec3,
    pub land_mask: f32,
    pub coast_distance_m: f32,
    pub island_id: Option<IslandId>,
}

#[derive(Clone, Copy, Debug)]
pub struct GeologySample {
    pub bedrock: BedrockId,
    pub hardness: f32,
    pub erodibility: f32,
    pub permeability: f32,
    pub volcanic_age: f32,
    pub fracture_intensity: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ColumnSample {
    pub surface: SurfaceSample,
    pub regolith_depth_m: f32,
    pub weathering_depth_m: f32,
    pub base_bedrock: BedrockId,
}
```

Do not require the runtime to repeatedly sample twelve atlas fields individually. Provide composite sampling APIs that minimize lookups.

## 0.2 Density convention

Choose one convention and encode it in tests:

```text
density < 0 = solid
density = 0 = surface
density > 0 = air or water volume
```

Basic surface density:

```rust
pub fn surface_density(
    position: WorldPosition,
    surface_height_m: f32,
) -> f32 {
    position.0.y as f32 - surface_height_m
}
```

Later caves, structures, and terrain edits can compose with this value without changing the provider interface.

## 0.3 Compiler contract

```rust
pub trait WorldCompiler {
    fn validate(
        &self,
        recipe: &CompiledWorldRecipe,
    ) -> Result<ValidationReport, WorldgenError>;

    fn compile(
        &self,
        recipe: &CompiledWorldRecipe,
        options: &CompileOptions,
    ) -> Result<CompiledWorld, WorldgenError>;
}
```

`CompileOptions` should control tooling behavior rather than world design:

```rust
pub struct CompileOptions {
    pub output_directory: Option<PathBuf>,
    pub write_debug_maps: bool,
    pub retain_intermediate_fields: bool,
    pub enable_parallelism: bool,
    pub cache_policy: CachePolicy,
}
```

World shape and algorithms remain in the recipe.

## 0.4 Compiler artifact

```rust
pub struct WorldManifest {
    pub world_id: WorldId,
    pub recipe_id: WorldRecipeId,
    pub recipe_hash: RecipeHash,
    pub generator_version: GeneratorVersion,
    pub seed: u64,
    pub extent: WorldExtent,
    pub sea_level_m: f32,
    pub field_descriptors: BTreeMap<FieldKey, FieldDescriptor>,
    pub pass_reports: Vec<PassReport>,
}
```

The manifest is the basis for:

* save compatibility,
* cache invalidation,
* bug reports,
* deterministic reproduction,
* and later multiplayer world verification.

## 0.5 Phase 0 tests

* Coordinate conversion round trips.
* Field cell to world position conversions.
* Density sign convention.
* Same recipe and seed produce the same manifest hash.
* Runtime density provider can be mocked without loading YAML.
* `terrain_generation` builds without Bevy.
* Runtime chunk generation does not import compiler implementation modules.

### Exit gate

```text
A runtime system can sample a compiled test island entirely through
WorldDensityProvider without knowing how the world was generated.
```

---

# Phase 1 — Typed YAML world recipes

## Goal

Create a strict, composable authoring system with:

* schema versions,
* typed definitions,
* cross-file references,
* migrations,
* semantic validation,
* canonical recipe hashing,
* and useful source diagnostics.

---

## 1.1 Recommended asset organization

Migrate incrementally toward:

```text
assets/worldgen/
├── worlds/
│   └── small.world.yaml
│
├── boundaries/
│   └── bounded_ocean.boundary.yaml
│
├── islands/
│   ├── volcanic_small.island.yaml
│   └── volcanic_medium.island.yaml
│
├── geology/
│   └── basaltic_volcanic.geology.yaml
│
├── refinement/
│   └── tropical_volcanic.refinement.yaml
│
├── materials/
│   └── volcanic_rocks.materials.yaml
│
└── validation/
    └── single_island.validation.yaml
```

Allow `assets/terrain/` paths during the migration, but resolve everything into logical IDs rather than preserving filesystem paths in compiled recipes.

## 1.2 Root recipe example

```yaml
schema_version: 1
id: world.small
seed: 4815162342

extent:
  width_m: 131072
  depth_m: 131072
  sea_level_m: 0.0

resolutions:
  control_cell_m: 512
  regional_cell_m: 64
  local_cell_m: 8

boundary: boundary.bounded_ocean
islands:
  - island.volcanic_small

geology: geology.basaltic_volcanic
refinement: refinement.tropical_volcanic
validation: validation.small
```

## 1.3 Tagged algorithm definitions

Prefer explicitly tagged enums:

```yaml
footprint:
  type: warped_ellipse
  major_radius_m: 25000
  minor_radius_m: 18500
  rotation_deg: 18
  warp:
    amplitude_m: 2200
    wavelength_m: 11000
```

```rust
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FootprintSource {
    Ellipse(EllipseFootprintSource),
    WarpedEllipse(WarpedEllipseFootprintSource),
    SplineHull(SplineHullFootprintSource),
    ImportedMask(ImportedMaskFootprintSource),
}
```

Use `#[serde(deny_unknown_fields)]` on author-facing structures so misspelled properties fail instead of silently disappearing. Serde documents that unknown fields are otherwise ignored for self-describing formats. Also note that Serde does not support combining `flatten` with `deny_unknown_fields`, so avoid flatten-heavy polymorphic layouts for strict worldgen definitions. ([Serde][4])

## 1.4 Three validation layers

### Parse validation

Catches:

* malformed YAML,
* invalid types,
* missing fields,
* unknown fields,
* invalid enum variants.

### Reference validation

Catches:

* missing referenced IDs,
* duplicate IDs,
* reference cycles,
* incorrect reference categories,
* incompatible schema versions.

### Semantic validation

Catches:

* negative world dimensions,
* island radius larger than safe interior,
* regional spacing finer than local spacing,
* falloff width exceeding world extent,
* invalid hardness or erodibility ranges,
* window stride larger than window size,
* zero-length splines,
* unsupported pass combinations.

## 1.5 Source diagnostics

Use structured errors with file names and source spans.

```rust
#[derive(Debug, thiserror::Error, miette::Diagnostic)]
#[error("invalid island radius")]
#[diagnostic(
    code(worldgen::invalid_radius),
    help("reduce the major radius or enlarge the world extent")
)]
pub struct InvalidRadiusDiagnostic {
    #[source_code]
    pub source: miette::NamedSource<String>,

    #[label("this radius leaves no required ocean margin")]
    pub span: miette::SourceSpan,
}
```

`miette` provides named source text, source spans, labels, and diagnostic derives suited to compiler-style error reporting. `thiserror` provides an implementation-oriented `std::error::Error` derive. ([Docs.rs][5])

## 1.6 Generated schemas

Derive a machine-readable schema for editor assistance and CI validation:

```rust
#[derive(
    Clone,
    Debug,
    Deserialize,
    Serialize,
    schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct IslandRecipeSource {
    // ...
}
```

Schemars derives JSON Schema from Rust types and generally respects corresponding Serde attributes. ([Docs.rs][6])

Even though the authoring files are YAML, the generated JSON Schema can still support:

* IDE completion,
* documentation generation,
* validation tooling,
* generated reference pages.

## 1.7 Schema migrations

Do not deserialize every old schema directly into the current type.

Use:

```text
YAML bytes
→ inspect schema_version
→ deserialize version-specific source
→ migrate to current source
→ semantic validation
→ compile
```

```rust
pub enum AnyWorldRecipeSource {
    V1(WorldRecipeSourceV1),
    V2(WorldRecipeSourceV2),
}

impl AnyWorldRecipeSource {
    pub fn migrate(self) -> Result<WorldRecipeSource, MigrationError> {
        match self {
            Self::V1(source) => migrate_v1_to_current(source),
            Self::V2(source) => Ok(source.into()),
        }
    }
}
```

## 1.8 Canonical hashing

Recipe hashing should occur after:

* migrations,
* reference resolution,
* normalization of units,
* stable sorting of maps and lists where order is semantically irrelevant,
* expansion of defaults.

Do not hash raw YAML bytes. Comments, whitespace, property ordering, and path spelling should not invalidate the world.

## 1.9 Bevy integration

Bevy 0.19’s `AssetLoader` reads through an asynchronous `Reader`, receives a `LoadContext`, and returns a typed asset. `LoadContext` also tracks dependencies. ([Docs.rs][7])

Recommended split:

```text
AssetLoader:
    YAML bytes → typed source asset

Resolver/compiler system:
    waits for all referenced source assets
    resolves IDs
    validates recipe
    launches compilation job
```

Do not perform a long world compilation inside the `AssetLoader`.

```rust
#[derive(Asset, TypePath, Debug)]
pub struct WorldRecipeAsset {
    pub source: WorldRecipeSource,
    pub source_path: PathBuf,
}
```

Bevy file watching can be enabled for development, allowing changed source assets to trigger recompilation or invalidation. ([Docs.rs][8])

## 1.10 Phase 1 tests

* Unknown property fails.
* Missing reference fails with source location.
* Circular reference fails.
* Schema V1 migrates to current.
* Equivalent YAML produces the same recipe hash.
* Changed algorithm parameter changes the recipe hash.
* Reordered maps do not change the recipe hash.
* Invalid physical units fail semantic validation.
* Generated schema matches representative YAML fixtures.

### Exit gate

```text
A root world YAML and all referenced files resolve into one immutable,
validated, canonical CompiledWorldRecipe with a stable content hash.
```

---

# Phase 2 — Field framework and atlas

## Goal

Create reusable field types and a `WorldAtlas` capable of supporting all later generation systems.

The current `Field2D` should become a general field foundation rather than an elevation-specific container.

---

## 2.1 Field descriptor

Every field needs explicit spatial metadata:

```rust
#[derive(Clone, Debug)]
pub struct FieldDescriptor {
    pub key: FieldKey,
    pub origin_world: WorldXZ,
    pub cell_size_m: f64,
    pub width: u32,
    pub height: u32,
    pub sample_layout: SampleLayout,
    pub value_kind: FieldValueKind,
}
```

```rust
pub enum SampleLayout {
    CellCenter,
    CellCorner,
    VertexGrid,
}
```

Do not infer whether values represent centers or corners from array dimensions.

## 2.2 Base storage

```rust
#[derive(Clone)]
pub struct Field2D<T> {
    pub descriptor: FieldDescriptor,
    pub values: Vec<T>,
}
```

Use row-major indexing consistently:

```rust
impl<T> Field2D<T> {
    #[inline]
    pub fn index(&self, x: u32, z: u32) -> usize {
        x as usize
            + self.descriptor.width as usize * z as usize
    }
}
```

## 2.3 Typed field wrappers

```rust
pub struct ScalarField(pub Field2D<f32>);
pub struct MaskField(pub Field2D<f32>);
pub struct CategoricalField<T>(pub Field2D<T>);
pub struct VectorField(pub Field2D<glam::Vec2>);
pub struct IndexField(pub Field2D<u32>);
```

A mask remains continuous in `[0, 1]`; do not reduce it to a boolean unless the consuming pass explicitly thresholds it.

## 2.4 Atlas structure

Grow `IslandAtlas` into `WorldAtlas` now, even while only one island is generated.

```rust
pub struct WorldAtlas {
    pub metadata: WorldMetadata,
    pub fields: FieldRegistry,
    pub graphs: GraphRegistry,
    pub islands: Vec<IslandBlueprint>,
}
```

Initial fields:

```text
boundary_distance
boundary_mask
ocean_basin
island_influence
island_id
island_age
base_elevation
bathymetry
coast_distance
bedrock
rock_hardness
erodibility
permeability
fracture_intensity
regional_residual
final_elevation
```

## 2.5 Field registry

```rust
pub struct FieldRegistry {
    scalar: BTreeMap<FieldKey, Arc<ScalarField>>,
    masks: BTreeMap<FieldKey, Arc<MaskField>>,
    categorical: BTreeMap<FieldKey, Arc<dyn ErasedField>>,
}
```

Use stable `FieldKey` values rather than string lookups inside hot sampling loops.

## 2.6 Sampling policies

Implement these explicitly:

```rust
pub enum ScalarSampling {
    Nearest,
    Bilinear,
    Bicubic,
}

pub enum CategoricalSampling {
    Nearest,
    Majority4,
}
```

Avoid interpolating categorical IDs.

Bilinear sampling:

```rust
pub fn sample_bilinear(
    field: &ScalarField,
    position: WorldXZ,
) -> Option<f32> {
    // Convert world coordinates to continuous grid coordinates,
    // sample four neighbors, and interpolate.
    todo!()
}
```

## 2.7 Resampling

Every resampling operation should specify:

* source and target descriptors,
* interpolation policy,
* out-of-bounds policy,
* anti-alias policy when downsampling.

```rust
pub enum OutOfBoundsPolicy {
    Clamp,
    Constant(f32),
    Mirror,
    Error,
}
```

Use ocean-depth constants for padded marine context, not ordinary edge clamping.

## 2.8 Tiles and halos

Do not immediately tile every small field. Introduce a storage abstraction:

```rust
pub enum FieldStorage<T> {
    Dense(Field2D<T>),
    Tiled(TiledField2D<T>),
}
```

```rust
pub struct TileSpec {
    pub interior_size: UVec2,
    pub halo: u32,
}
```

The halo contains neighboring samples needed for:

* derivatives,
* normal calculation,
* filtering,
* local convolution,
* window refinement,
* future hydrology.

The interior is the only region committed back to the atlas.

## 2.9 Memory planning

A dense `4096 × 4096` `f32` field uses approximately 64 MiB:

```text
4096 × 4096 × 4 bytes = 67,108,864 bytes
```

Twelve simultaneously resident fields would approach 768 MiB before temporary buffers.

Therefore classify fields as:

```text
Persistent:
    needed by later passes or runtime

Intermediate:
    released after downstream use

Derived:
    recomputed cheaply when needed

Cached:
    persisted on disk but not always resident
```

## 2.10 Debug export

Each field should support:

* grayscale image export,
* false-color export,
* histogram,
* minimum and maximum,
* percentile range,
* NaN and infinity count,
* difference image,
* seam heatmap.

Do not normalize every debug image independently without recording the applied range, or visual comparisons will be misleading.

## 2.11 Phase 2 tests

* World-to-grid and grid-to-world conversions.
* Corner and center sampling behavior.
* Bilinear interpolation at exact sample locations.
* Nearest sampling for categorical values.
* Dense and tiled storage return identical samples.
* Tile halos match neighboring tile interiors.
* Serialization round trip.
* Field exports preserve orientation.
* NaN detection catches invalid output.
* Resampling a constant field remains constant.

### Exit gate

```text
Scalar, mask, categorical, and vector fields can be generated,
resampled, tiled, serialized, visualized, and sampled by world position.
```

---

# Phase 3 — Bounded ocean basin

## Goal

Guarantee a finite world whose entire perimeter is deep ocean, with a smooth transition toward a shelf-capable interior.

This pass must establish a hard invariant that no later island configuration can violate.

---

## 3.1 Boundary-distance field

For a rectangular world centered at the origin:

```rust
pub fn distance_to_rect_edge(
    p: glam::DVec2,
    half_extent: glam::DVec2,
) -> f64 {
    let dx = half_extent.x - p.x.abs();
    let dz = half_extent.y - p.y.abs();
    dx.min(dz)
}
```

Inside the world this returns:

```text
0 at the boundary
positive toward the interior
```

Store both:

```text
boundary_distance_m
boundary_interior_mask
```

## 3.2 Normalized interior mask

```rust
pub fn smoothstep01(value: f32) -> f32 {
    let t = value.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn boundary_mask(
    distance_to_edge_m: f32,
    transition_width_m: f32,
) -> f32 {
    smoothstep01(distance_to_edge_m / transition_width_m)
}
```

The mask should be exactly zero on the perimeter.

## 3.3 Basin profile

Separate three concepts:

```text
edge abyss:
    guaranteed deepest boundary zone

deep-ocean basin:
    broad negative elevation around the island region

shelf-capable interior:
    region where later island and shelf fields may raise terrain
```

Example:

```rust
pub fn base_ocean_elevation(
    boundary_mask: f32,
    edge_depth_m: f32,
    interior_depth_m: f32,
) -> f32 {
    edge_depth_m
        + boundary_mask * (interior_depth_m - edge_depth_m)
}
```

With:

```text
edge_depth_m     = -6000
interior_depth_m = -1500
```

## 3.4 Low-frequency basin variation

Variation may be added only after it is masked:

```rust
let variation =
    ocean_noise.sample(world_xz) * recipe.variation_amplitude_m;

let basin =
    base_ocean
    + variation * boundary_mask.powf(2.0);
```

This ensures noise cannot raise the boundary.

## 3.5 Safety margin

Calculate the maximum permitted island influence radius:

```text
safe island radius =
minimum world half-extent
- required deep-ocean band
- required boundary transition
- regional refinement halo
```

Reject recipes that exceed it before generation begins.

## 3.6 Boundary validation

Check every perimeter sample:

```text
elevation < required perimeter depth
land mask = 0
island influence = 0
finite values only
```

Also validate several inward rings:

* median depth should generally become shallower inward,
* no isolated positive spikes,
* no shelf or island field reaches the protected boundary band.

Do not require strict per-cell monotonicity because limited low-frequency variation is desirable.

## 3.7 Phase 3 outputs

```text
boundary_distance
boundary_mask
protected_boundary_mask
ocean_basin
```

### Exit gate

```text
Every boundary sample is deep ocean, no land influence reaches the
protected edge band, and the interior remains available for island placement.
```

---

# Phase 4 — Island seeds and archipelago skeleton

## Goal

Replace the single hard-coded ellipse with a scalable island-description model that initially produces exactly one island but can later generate chains, arcs, clusters, and hotspot tracks.

---

## 4.1 Island descriptor

```rust
pub struct IslandSeed {
    pub id: IslandId,
    pub center: WorldXZ,
    pub rotation_rad: f32,

    pub major_radius_m: f32,
    pub minor_radius_m: f32,

    pub age_myr: f32,
    pub uplift: f32,
    pub volcanic_activity: f32,

    pub root_seed: u64,
    pub volcanic_centers: Vec<VolcanicCenterSeed>,
    pub ridge_seeds: Vec<RidgeSeed>,
}
```

Even for one island, the atlas should contain:

```rust
pub islands: Vec<IslandBlueprint>
```

The current milestone validates `islands.len() == 1`.

## 4.2 Footprint as a distance field

Begin with an ellipse signed-distance approximation in island-local coordinates:

```rust
pub fn ellipse_influence(
    world: WorldXZ,
    seed: &IslandSeed,
) -> f32 {
    let local = rotate_into_island_space(
        world.0 - seed.center.0,
        -seed.rotation_rad,
    );

    let normalized = glam::DVec2::new(
        local.x / seed.major_radius_m as f64,
        local.y / seed.minor_radius_m as f64,
    );

    let r = normalized.length();
    1.0 - r as f32
}
```

The raw influence is:

```text
positive inside
zero near coast
negative outside
```

Apply a profile curve later rather than mixing elevation into the footprint pass.

## 4.3 Domain warping

Use low-frequency coordinate displacement to avoid an obviously elliptical coastline:

```rust
let warped_position = position + warp_vector(position, warp_recipe);
let influence = ellipse_influence(warped_position, island);
```

Keep warp amplitude much smaller than island radius. Large warps produce detached lobes, narrow necks, and accidental offshore fragments.

FastNoise Lite and the Rust `noise` crate provide coherent noise, fBm, displacement, and related functions, but wrap any third-party implementation behind your own `Noise2D` trait so generator behavior is not coupled to one crate. ([GitHub][9])

```rust
pub trait Noise2D: Send + Sync {
    fn sample(&self, position: glam::DVec2) -> f64;
}
```

## 4.4 Skeleton graph

The skeleton is not yet terrain. It is a semantic graph of island-forming features:

```rust
pub struct IslandSkeleton {
    pub volcanic_centers: Vec<VolcanicCenter>,
    pub ridges: Vec<StructuralRidge>,
    pub saddles: Vec<StructuralSaddle>,
    pub calderas: Vec<CalderaDescriptor>,
}
```

```rust
pub struct VolcanicCenter {
    pub id: VolcanicCenterId,
    pub position: WorldXZ,
    pub age_myr: f32,
    pub radius_m: f32,
    pub target_height_m: f32,
    pub shape: VolcanoShape,
}
```

For the first island, allow:

* one principal volcanic center,
* zero to three secondary vents,
* radial or asymmetric ridge seeds,
* optional summit caldera.

## 4.5 Age-driven descriptors

Island age should affect descriptors, not directly run a complete geology simulation.

Example mapping:

```text
younger island:
    greater volcanic activity
    higher peak-height target
    steeper macro slope
    smaller coastal-plain fraction
    narrower provisional shelf
    less weathering

older island:
    lower height target
    broader footprint
    weaker summit prominence
    wider coastal plain
    deeper weathering
    lower rock hardness near surface
```

Store the age field separately so later systems can interpret it differently.

## 4.6 Future multi-island compatibility

Create placement policies now:

```rust
pub enum IslandPlacement {
    Explicit(Vec<ExplicitIslandSource>),
    SingleCentered(SingleIslandSource),
    HotspotTrack(HotspotTrackSource),
    VolcanicArc(VolcanicArcSource),
}
```

For Milestone A, compile only `SingleCentered` and a one-entry `Explicit` list.

## 4.7 Phase 4 outputs

```text
island descriptors
island skeleton
island influence
island_id
volcanic_age
volcanic-center masks
ridge influence masks
```

## 4.8 Phase 4 tests

* Seed remains inside allowed world bounds.
* Footprint does not reach protected edge band.
* Same seed produces identical skeleton.
* Different root seed changes warp and secondary features.
* Footprint remains one connected component unless explicitly allowed.
* Minimum land area and coastline length are satisfied.
* No island lobe falls below configured minimum area.
* Single-island recipe produces exactly one island ID.

### Exit gate

```text
The compiler produces one reproducible volcanic island blueprint with
a non-elliptical connected footprint and an inspectable structural skeleton.
```

---

# Phase 5 — Macro elevation and bathymetry

## Goal

Generate all broad terrain forms without local noise:

* volcanic mass,
* major ridges,
* summit and caldera,
* broad slopes,
* coastal plains,
* nearshore shelf,
* continental or volcanic slope,
* deep ocean.

The result should be recognizable at a distant-camera scale before regional detail is added.

---

## 5.1 Frequency responsibility

Macro elevation should represent wavelengths approximately from:

```text
several kilometers
to the full island diameter
```

It should not contain:

* small gullies,
* meter-scale rock roughness,
* river channels,
* local erosion scars,
* vegetation-scale variation.

## 5.2 Footprint elevation profile

Transform island influence into elevation with a configurable curve:

```rust
pub fn footprint_height(
    influence: f32,
    peak_height_m: f32,
    exponent: f32,
) -> f32 {
    influence
        .max(0.0)
        .powf(exponent)
        * peak_height_m
}
```

A larger exponent concentrates elevation toward the center. A smaller exponent produces broader uplands.

## 5.3 Volcanic profiles

Support several profile families:

```rust
pub enum VolcanoProfile {
    Shield {
        exponent: f32,
        summit_rounding: f32,
    },
    Stratovolcano {
        lower_slope: f32,
        upper_slope: f32,
    },
    Composite {
        shoulder_height: f32,
        summit_exponent: f32,
    },
}
```

Use semantic curves rather than noise to establish the primary form.

## 5.4 Calderas

A caldera should be a broad structural depression with a rim:

```rust
let depression =
    -caldera.depth_m
    * radial_bell(distance / caldera.radius_m);

let rim =
    caldera.rim_height_m
    * ring_profile(
        distance,
        caldera.radius_m,
        caldera.rim_width_m,
    );
```

Apply the caldera only to the summit region. Do not subtract a generic crater from the entire island.

## 5.5 Structural ridges

Represent major ridges as spline-distance fields:

```rust
pub struct StructuralRidge {
    pub path: Vec<WorldXZ>,
    pub half_width_m: f32,
    pub height_m: f32,
    pub taper: RidgeTaper,
}
```

At each sample:

```text
distance to spline
→ cross-section profile
→ longitudinal taper
→ ridge elevation contribution
```

Radial volcanic ridges should generally:

* begin near volcanic centers,
* taper toward the coast,
* vary in length,
* avoid perfectly even angular spacing,
* respect the island footprint.

## 5.6 Combining forms

Avoid hard `max` operations where they create abrupt derivative discontinuities.

Use smooth maximum:

```rust
pub fn smooth_max(a: f32, b: f32, k: f32) -> f32 {
    -smooth_min(-a, -b, k)
}
```

Combine:

```text
base footprint
smooth-max primary volcano
smooth-max secondary vents
add structural ridges
subtract caldera
apply coastal-plain shaping
```

## 5.7 Coastal plains

Use a coast-distance band and slope suppression:

```text
near coast
+ sufficiently low elevation
+ not inside cliff-designated structural zone
→ reduce gradient toward a plain target
```

Do not flatten all coastlines. Store a `coastal_plain_potential` mask for later climate and sediment passes.

## 5.8 Bathymetry

Bathymetry should be driven primarily by distance from the provisional coast.

Recommended zones:

```text
shore transition
inner shelf
outer shelf
shelf break
island flank
abyssal basin
boundary abyss
```

```rust
pub struct BathymetryProfile {
    pub inner_shelf_width_m: f32,
    pub outer_shelf_width_m: f32,
    pub shelf_break_depth_m: f32,
    pub flank_depth_m: f32,
    pub abyss_depth_m: f32,
}
```

Use signed coast distance:

```text
positive inland
zero at coastline
negative offshore
```

Map offshore distance through a piecewise smooth curve.

## 5.9 Coastline iteration

The coastline depends on elevation, while shelf construction depends on the coastline. Resolve this with a small fixed iteration:

```text
1. Generate footprint-based provisional elevation.
2. Extract sea-level coastline.
3. Compute coast-distance field.
4. Build shelf and bathymetry.
5. Recalculate final macro elevation.
6. Re-extract coast and validate.
```

Two or three deterministic iterations should be enough. Do not run an unconstrained convergence loop.

## 5.10 Phase 5 outputs

```text
base_elevation
macro_elevation
bathymetry
land_mask
ocean_mask
coast_distance
shelf_mask
coastal_plain_potential
caldera_mask
ridge_mask
```

## 5.11 Macro validation

Measure:

* island area,
* coastline length,
* maximum elevation,
* mean elevation,
* peak prominence,
* shelf area,
* coast-to-edge distance,
* number of disconnected land components,
* extreme-slope fraction,
* caldera depth and rim continuity.

### Exit gate

```text
With all regional noise disabled, the island is already recognizable as
a coherent volcanic landform with a summit, ridges, coast, shelf, and deep ocean.
```

---

# Phase 6 — Structural geology

## Goal

Generate authoritative geological fields that constrain later:

* erosion,
* cave placement,
* river incision,
* cliffs,
* soil depth,
* resources,
* terrain damage,
* material rendering.

Do not continue deriving geology from altitude and random noise at runtime.

---

## 6.1 Core geology fields

Add:

```text
bedrock_type
volcanic_age
rock_hardness
erodibility
permeability
fracture_intensity
weathering_intensity
regolith_potential
structural_constraint
```

Possible initial bedrock types:

```rust
pub enum BedrockId {
    DenseBasalt,
    VesicularBasalt,
    VolcanicTuff,
    Breccia,
    AshDeposit,
    IntrusiveRock,
    MarineSediment,
}
```

## 6.2 Material definitions

```yaml
materials:
  dense_basalt:
    hardness: 0.88
    erodibility: 0.18
    permeability: 0.20
    fracture_susceptibility: 0.42

  volcanic_tuff:
    hardness: 0.30
    erodibility: 0.78
    permeability: 0.55
    fracture_susceptibility: 0.68

  ash_deposit:
    hardness: 0.08
    erodibility: 0.93
    permeability: 0.35
    fracture_susceptibility: 0.20
```

These values are simulation coefficients, not literal laboratory units. Document that distinction.

## 6.3 Geological construction history

Build geology from structural events:

```text
old primary shield flow
newer summit flow
secondary vent flow
caldera collapse deposit
ash or tuff sector
coastal marine sediment
fault or fracture corridor
```

```rust
pub struct GeologicalEvent {
    pub event_id: GeologicalEventId,
    pub age_myr: f32,
    pub material: BedrockId,
    pub influence: GeologicalInfluence,
    pub priority: i16,
}
```

Each event writes:

* material candidates,
* age,
* hardness modifiers,
* fracture modifiers.

Resolve overlapping events by:

```text
youngest covering event
explicit priority
or configured blend rule
```

This produces meaningful geological regions rather than arbitrary categorical noise.

## 6.4 Volcanic age field

Age may vary radially or by flow sector:

```text
summit and recent vent:
    youngest

outer shield:
    older

deep eroded valleys:
    expose older units

caldera deposits:
    age of collapse event
```

Do not assume one uniform age for the entire island.

## 6.5 Hardness and erodibility

Derive effective coefficients from:

```text
base material property
× weathering modifier
× fracture modifier
× age modifier
× local alteration modifier
```

```rust
pub fn effective_erodibility(
    material: &MaterialProperties,
    weathering: f32,
    fracture: f32,
) -> f32 {
    (
        material.erodibility
        * (1.0 + weathering * 0.6)
        * (1.0 + fracture * 0.4)
    )
    .clamp(0.0, 1.0)
}
```

Keep hardness and erodibility distinct. They may be correlated but are not interchangeable gameplay coefficients.

## 6.6 Fracture zones

Generate:

* radial volcanic fractures,
* caldera ring faults,
* ridge-axis fractures,
* optional regional fault corridors.

Fracture fields should later influence:

* cave and lava-tube probability,
* groundwater,
* rockfall,
* cliff weathering,
* erosion.

## 6.7 Structural constraints

Create two preservation fields:

```text
value constraint:
    resist changes to absolute elevation

gradient constraint:
    resist changes to local shape or slope
```

Examples:

```text
summit landmark:
    high value and high gradient constraint

broad volcanic shield:
    low value, moderate gradient constraint

soft ash field:
    low value and low gradient constraint

future authored settlement plateau:
    high value constraint, moderate gradient constraint
```

This is the basis for later erosion without erasing required macro forms.

## 6.8 Vertical geology contract

Although this milestone stores most geology as 2D fields, define the vertical interface now:

```rust
pub trait GeologicalColumnProvider {
    fn sample_material(
        &self,
        horizontal: WorldXZ,
        depth_below_surface_m: f32,
    ) -> BedrockId;
}
```

Initial implementation may use:

```text
surface regolith
weathered bedrock
primary bedrock
deep intrusive bedrock
```

Later caves and voxelization can replace this with more sophisticated strata without changing the runtime sampling interface.

## 6.9 Phase 6 outputs

```text
bedrock_type
volcanic_age
rock_hardness
erodibility
permeability
fracture_intensity
weathering_intensity
regolith_potential
value_constraint
gradient_constraint
```

## 6.10 Phase 6 tests

* Every land cell has valid bedrock.
* Ocean cells use a defined marine category or no-data value.
* Coefficients remain within configured ranges.
* Young flow units replace older units according to rules.
* Caldera ring faults form a continuous or intentionally broken ring.
* Same material recipe produces stable coefficients.
* Geology boundaries do not create NaNs or invalid interpolations.
* Runtime column sampling agrees with atlas surface geology.

### Exit gate

```text
The island possesses inspectable geological regions and continuous physical
property fields that can constrain erosion, caves, materials, and damage.
```

---

# Phase 7 — Regional refinement

## Goal

Add medium- and high-frequency terrain detail without:

* changing the island’s identity,
* moving the coastline substantially,
* destroying structural ridges,
* creating visible tile seams,
* introducing chunk-aligned patterns,
* or inventing river systems before hydrology exists.

---

## 7.1 Frequency separation

Store:

```text
final elevation =
macro elevation
+ regional residual
+ later hydrology-aware local detail
```

For Milestone A, Phase 7 should produce only the regional residual and conservative local roughness.

Suggested wavelength responsibilities:

```text
macro:
    approximately 8 km to full island diameter

regional:
    approximately 250 m to 8 km

local:
    approximately 8 m to 250 m
```

The exact bands depend on world and voxel scale, but they must not overlap arbitrarily.

## 7.2 Window specification

```rust
pub struct RegionalWindowSpec {
    pub interior_size: UVec2,
    pub overlap: UVec2,
    pub halo: u32,
    pub cell_size_m: f64,
}
```

Example:

```yaml
window:
  interior_samples: [512, 512]
  stride_samples: [320, 320]
  halo_samples: 32
  blend: raised_cosine
```

The generator receives a padded context window but commits only its interior contribution.

## 7.3 Window input

Each window should receive immutable snapshots of:

```text
macro elevation
island influence
distance to coast
volcanic age
bedrock
hardness
erodibility
fracture intensity
ridge mask
caldera mask
constraints
```

Do not allow a window to inspect or mutate neighboring window output.

## 7.4 Residual generator interface

```rust
pub trait RegionalRefiner: Send + Sync {
    fn refine(
        &self,
        context: &RegionalContext<'_>,
        output: &mut RegionalPatch,
    ) -> Result<(), RefinementError>;
}
```

The output should be a residual, not a complete replacement elevation field.

```rust
pub struct RegionalPatch {
    pub residual: Field2D<f32>,
    pub confidence: Option<Field2D<f32>>,
}
```

## 7.5 Geology-aware detail

Examples:

```text
dense basalt:
    sharper ridges
    lower small-scale amplitude
    more persistent cliff bands

tuff and ash:
    broader soft forms
    greater incision potential
    smoother exposed surfaces

fractured basalt:
    anisotropic grooves
    localized roughness
    higher break-up frequency
```

Use geological fields to modulate:

* amplitude,
* ridge sharpness,
* anisotropy,
* octave count,
* roughness,
* orientation.

## 7.6 Noise usage

Use noise as residual structure, not as the world’s author.

Possible components:

```text
low-amplitude fBm
ridged multifractal bands
directional ridge noise
domain-warped detail
cellular distance for fractured patterns
```

The Rust `noise` crate exposes Perlin, OpenSimplex, fBm, displacement, and multiple combinators. Its OpenSimplex implementation is documented as a slower but higher-quality gradient-noise option than 2D Perlin. ([Docs.rs][10])

Wrap these components into compiled samplers:

```rust
pub enum CompiledNoiseNode {
    Gradient(GradientNoise),
    Fbm(FbmNoise),
    Ridged(RidgedNoise),
    Warp(DomainWarp),
    Multiply(Box<Self>, Box<Self>),
    Add(Box<Self>, Box<Self>),
    Mask {
        source: Box<Self>,
        mask: FieldKey,
    },
}
```

Do not interpret arbitrary executable expression trees from YAML. Compile a controlled algorithm vocabulary.

## 7.7 Coast preservation

Regional residual amplitude should fade near the coast unless the recipe explicitly enables cliffs or coastal roughness.

```rust
let coast_preservation =
    smoothstep(
        recipe.coast_preserve_start_m,
        recipe.coast_preserve_end_m,
        coast_distance_m,
    );

residual *= coast_preservation;
```

This prevents small noise changes from turning one coastline into thousands of tiny islets.

## 7.8 Structural preservation

Apply constraint suppression:

```rust
let allowed_change =
    1.0 - value_constraint.clamp(0.0, 1.0);

let allowed_gradient_change =
    1.0 - gradient_constraint.clamp(0.0, 1.0);

residual *= allowed_change;
```

For ridge zones, orient detail along the ridge rather than simply suppressing it.

## 7.9 Edge-neutral patches

Each patch should be approximately neutral at its border.

Use both:

1. an overlap blend weight,
2. an optional residual border fade.

Raised-cosine weight:

```rust
pub fn raised_cosine_weight(t: f32) -> f32 {
    0.5 - 0.5 * (std::f32::consts::PI * t).cos()
}
```

The two-dimensional weight is the product of X and Z weights.

## 7.10 Blending

Maintain two global buffers:

```text
weighted residual sum
weight sum
```

For each patch:

```text
sum[cell] += residual[cell] × weight[cell]
weights[cell] += weight[cell]
```

Final:

```text
regional residual = sum / weights
```

Merge patches in stable tile-coordinate order even when generation occurs in parallel.

## 7.11 Seam validation

For every shared window overlap, calculate:

```text
maximum absolute elevation difference
mean absolute elevation difference
gradient discontinuity
normal discontinuity
```

Produce a seam heatmap.

The stricter invariant is not that raw patch outputs match; it is that the blended committed field has no detectable boundary aligned to the window grid.

## 7.12 Avoid premature hydrology

Before the hydrology milestone, regional refinement may add:

* ridges,
* shoulders,
* terraces,
* rough escarpments,
* broad shallow depressions,
* geological texture.

It should not add:

* permanent river channels,
* dendritic drainage networks,
* deltas,
* alluvial fans,
* detailed floodplains.

Those must be produced after global drainage is known.

## 7.13 Phase 7 outputs

```text
regional_residual
regional_detail_amplitude
refined_elevation
regional_window_manifest
seam_metrics
```

## 7.14 Phase 7 tests

* One-window and multi-window generation match in the interior.
* Different thread counts produce the same quantized field hash.
* No NaN or infinite samples.
* Constant input with zero amplitude remains unchanged.
* Coast displacement remains below configured tolerance.
* Maximum peak position remains within tolerance.
* Protected landmarks remain within value and gradient tolerances.
* Seam metrics stay below threshold.
* Window order does not change the result.
* Tile-size changes do not create large morphological changes.

### Exit gate

```text
The island contains varied regional terrain detail, but its coastline,
summit, volcanic ridges, geology, and macro silhouette remain intact,
with no visible or measurable window seams.
```

---

# 3. Compiler orchestration

## 3.1 Pass sequence

```text
Resolve and validate YAML
        ↓
Create atlas descriptors
        ↓
Generate boundary distance and ocean basin
        ↓
Generate one island seed and skeleton
        ↓
Generate island influence
        ↓
Generate macro elevation
        ↓
Extract provisional coastline
        ↓
Generate shelf and bathymetry
        ↓
Generate structural geology
        ↓
Generate regional windows
        ↓
Blend regional residual
        ↓
Validate complete Milestone A world
        ↓
Freeze manifest and compiled artifact
```

## 3.2 Compile state

```rust
pub enum CompileStage {
    ResolveRecipe,
    ValidateRecipe,
    AllocateAtlas,
    Boundary,
    IslandSkeleton,
    MacroTerrain,
    Bathymetry,
    Geology,
    RegionalRefinement,
    FinalValidation,
    Persist,
    Complete,
    Failed,
}
```

## 3.3 Reports

```rust
pub struct PassReport {
    pub pass: PassKey,
    pub elapsed: Duration,
    pub seed: u64,
    pub cache_status: CacheStatus,
    pub outputs: Vec<FieldKey>,
    pub metrics: BTreeMap<MetricKey, f64>,
    pub warnings: Vec<WorldgenWarning>,
}
```

Report useful metrics rather than only logging “pass complete.”

---

# 4. Bevy 0.19 execution model

## 4.1 Asset loading

Bevy loads source definitions as assets. When all dependencies are available, create a compilation request.

```rust
#[derive(Message)]
pub struct RequestWorldCompilation {
    pub recipe: Handle<WorldRecipeAsset>,
}
```

## 4.2 Background compilation

Work that does not need to finish for the next frame belongs on Bevy’s `AsyncComputeTaskPool`. ([Docs.rs][11])

```rust
#[derive(Component)]
pub struct WorldCompilationTask {
    pub task: bevy::tasks::Task<Result<CompiledWorld, WorldgenError>>,
}
```

```rust
fn begin_world_compilation(
    mut commands: Commands,
    requests: MessageReader<RequestWorldCompilation>,
    recipes: Res<Assets<WorldRecipeAsset>>,
) {
    let pool = AsyncComputeTaskPool::get();

    for request in requests.read() {
        let Some(recipe_asset) = recipes.get(&request.recipe) else {
            continue;
        };

        let source = recipe_asset.source.clone();

        let task = pool.spawn(async move {
            compile_world_from_source(source)
        });

        commands.spawn(WorldCompilationTask { task });
    }
}
```

Poll tasks without blocking the main thread. Bevy’s async-compute example specifically warns against repeatedly blocking around `poll_once`. ([Docs.rs][12])

## 4.3 Runtime installation

Once compilation succeeds:

```rust
#[derive(Resource)]
pub struct ActiveCompiledWorld {
    pub world: Arc<CompiledWorld>,
    pub provider: Arc<dyn WorldDensityProvider>,
}
```

Dense field data should remain in resources or dedicated stores, not as one ECS entity per sample.

## 4.4 Debug rendering

For Phase 0–7 visualization:

* create low-resolution debug meshes,
* show field textures on planes,
* draw island skeleton splines with gizmos,
* draw window boundaries,
* toggle geology and mask overlays.

Bevy 0.19’s `Mesh` API uses `Mesh::new` with `RenderAssetUsages` and attribute/index insertion methods. ([Docs.rs][13])

Do not build final voxel terrain meshes in this milestone unless needed for a minimal visual verification.

---

# 5. Recommended module layout

```text
crates/terrain_generation/src/
├── lib.rs
│
├── contract/
│   ├── coordinates.rs
│   ├── density.rs
│   ├── metadata.rs
│   ├── manifest.rs
│   └── version.rs
│
├── compiler/
│   ├── mod.rs
│   ├── context.rs
│   ├── pass.rs
│   ├── scheduler.rs
│   ├── report.rs
│   ├── cache_key.rs
│   └── validation.rs
│
├── fields/
│   ├── mod.rs
│   ├── descriptor.rs
│   ├── dense.rs
│   ├── tiled.rs
│   ├── scalar.rs
│   ├── mask.rs
│   ├── categorical.rs
│   ├── vector.rs
│   ├── sampling.rs
│   ├── resampling.rs
│   ├── distance.rs
│   └── serialization.rs
│
├── world/
│   ├── atlas.rs
│   ├── extent.rs
│   └── metadata.rs
│
├── boundary/
│   ├── mod.rs
│   ├── rectangle.rs
│   ├── distance.rs
│   ├── falloff.rs
│   └── validation.rs
│
├── islands/
│   ├── seed.rs
│   ├── placement.rs
│   ├── footprint.rs
│   ├── skeleton.rs
│   ├── ridge.rs
│   ├── volcano.rs
│   └── validation.rs
│
├── macro_terrain/
│   ├── elevation.rs
│   ├── caldera.rs
│   ├── coastal_plain.rs
│   ├── coast_distance.rs
│   ├── bathymetry.rs
│   └── blending.rs
│
├── geology/
│   ├── material.rs
│   ├── event.rs
│   ├── age.rs
│   ├── hardness.rs
│   ├── erodibility.rs
│   ├── fracture.rs
│   ├── columns.rs
│   └── constraints.rs
│
├── regional/
│   ├── window.rs
│   ├── context.rs
│   ├── generator.rs
│   ├── frequency.rs
│   ├── noise_graph.rs
│   ├── blending.rs
│   └── seams.rs
│
└── diagnostics/
    ├── export.rs
    ├── histogram.rs
    ├── metrics.rs
    ├── difference.rs
    └── heatmap.rs
```

---

# 6. Testing strategy

## Unit tests

Test:

* coordinate conversion,
* interpolation,
* seed derivation,
* curves,
* distance fields,
* smooth combinations,
* recipe validation,
* geological property derivation.

## Property-based tests

Useful invariants:

```text
boundary mask is always within [0, 1]
hardness and erodibility are within valid ranges
field sampling never indexes outside storage
same seed produces same descriptor
tile halos match neighboring tiles
weights are positive wherever output is committed
```

## Golden-world tests

Maintain a few small deterministic worlds:

```text
tiny centered shield volcano
asymmetric caldera island
broad old volcanic island
extreme but valid narrow island
```

Save:

* manifest,
* selected quantized field hashes,
* metrics,
* small debug images.

Hash quantized fields rather than raw floating-point bytes if cross-compiler or cross-CPU stability becomes important.

## Regression metrics

Track:

```text
land area
coastline length
maximum elevation
mean elevation
shelf area
number of land components
hardness histogram
erodibility histogram
seam maximum
seam mean
compile time
peak memory
```

A changed image is not automatically a regression, but a changed metric should be deliberate.

---

# 7. Milestone A completion criteria

Milestone A is complete when one YAML root recipe can reliably produce:

```text
1 validated world recipe
1 bounded finite world
1 connected volcanic island
1 coherent volcanic skeleton
1 macro elevation field
1 bathymetry and shelf field
1 structural geology model
1 seam-free refined elevation field
1 compiled manifest
1 runtime WorldDensityProvider
```

## Required acceptance tests

### Determinism

```text
Same source + same seed + same generator version
→ identical compiled recipe hash
→ identical island descriptors
→ identical quantized atlas hashes
```

### Boundary

```text
100% of perimeter samples are below required deep-ocean elevation.
No land lies inside the protected boundary band.
```

### Single-island topology

```text
Exactly one connected primary landmass.
Optional tiny rocks are disabled for this milestone.
```

### Macro coherence

```text
The primary summit, ridge network, caldera, coastal plain,
shelf, and ocean basin are visible with regional detail disabled.
```

### Geology

```text
Every land sample has valid bedrock, age, hardness,
erodibility, permeability, and fracture values.
```

### Regional seams

```text
No window-aligned seam exceeds the configured elevation
or gradient discontinuity threshold.
```

### Runtime isolation

```text
Voxel or chunk code samples only WorldDensityProvider.
It does not reference YAML, compiler passes, atlas mutation,
footprint generators, volcano generators, or noise recipes.
```

### Diagnostics

Every compilation produces:

```text
manifest
pass timing report
validation report
field summary
boundary map
macro elevation map
bathymetry map
geology map
regional residual map
final elevation map
seam heatmap
```

---

# 8. Recommended implementation order inside the milestone

1. **Lock coordinate, seed, density, and manifest contracts.**
2. **Implement strict source loading, migration, resolution, and validation.**
3. **Generalize `Field2D` and create `WorldAtlas`.**
4. **Implement bounded-ocean invariants before island generation.**
5. **Replace the ellipse with `IslandSeed` plus a warped distance-field footprint.**
6. **Create the semantic volcanic skeleton.**
7. **Generate macro elevation without local noise.**
8. **Generate shelf and bathymetry from provisional coast distance.**
9. **Generate geological events and derived physical fields.**
10. **Implement overlapping regional residual windows.**
11. **Add seam metrics and complete-world validation.**
12. **Expose the compiled result through `WorldDensityProvider`.**

The important sequencing rule is:

> Do not optimize the visual noise before the compiler contract, atlas, bounded ocean, island topology, and macro geology are stable.

Those foundations determine whether future hydrology, erosion, caves, biomes, structures, saves, and streaming can be added as compiler passes rather than requiring another terrain rewrite.

[1]: https://bevy.org/news/bevy-0-19/?utm_source=chatgpt.com "Bevy 0.19"
[2]: https://docs.rs/blake3/latest/blake3/struct.Hash.html?utm_source=chatgpt.com "Hash in blake3 - Rust"
[3]: https://docs.rs/rayon/latest/rayon/slice/trait.ParallelSliceMut.html?utm_source=chatgpt.com "ParallelSliceMut in rayon::slice - Rust"
[4]: https://serde.rs/container-attrs.html?utm_source=chatgpt.com "Container attributes"
[5]: https://docs.rs/miette/latest/miette/struct.NamedSource.html?utm_source=chatgpt.com "NamedSource in miette - Rust"
[6]: https://docs.rs/schemars/latest/schemars/derive.JsonSchema.html?utm_source=chatgpt.com "JsonSchema in schemars - Rust"
[7]: https://docs.rs/bevy/latest/bevy/asset/struct.LoadContext.html?utm_source=chatgpt.com "LoadContext in bevy::asset - Rust"
[8]: https://docs.rs/crate/bevy/latest/source/examples/asset/processing/asset_processing.rs?utm_source=chatgpt.com "bevy 0.19.0 - Docs.rs"
[9]: https://github.com/Auburn/FastNoiseLite?utm_source=chatgpt.com "Auburn/FastNoiseLite: Fast Portable Noise Library"
[10]: https://docs.rs/noise/latest/noise/struct.OpenSimplex.html?utm_source=chatgpt.com "OpenSimplex in noise - Rust"
[11]: https://docs.rs/bevy/latest/bevy/tasks/struct.AsyncComputeTaskPool.html?utm_source=chatgpt.com "AsyncComputeTaskPool in bevy::tasks - Rust"
[12]: https://docs.rs/bevy/latest/src/async_compute/async_compute.rs.html?utm_source=chatgpt.com "async_compute.rs - source"
[13]: https://docs.rs/crate/bevy/latest/source/examples/3d/generate_custom_mesh.rs?utm_source=chatgpt.com "bevy 0.19.0 - Docs.rs"
