Your current direction is fundamentally sound: use **2D fields for the island’s large-scale surface**, then convert them into an authoritative **3D signed-density field** for caves, overhangs, cliffs, underwater terrain, and editing. Your existing terrain plan already recommends that hybrid rather than relying on either a heightmap alone or unrestricted 3D noise. 

What is probably making the result look artificial is not voxel resolution. At **0.5–1 meter**, you have enough geometric resolution for a convincing vertical slice. The more likely issue is that individual features are being generated independently from noise rather than emerging from—or being constrained by—the same geological and hydrological structure.

# 1. The key architectural change

Do not generate this:

```text
island noise
+ mountain noise
+ cliff noise
+ river noise
+ cave noise
+ biome colors
```

Generate this instead:

```text
Island geological skeleton
        ↓
Base elevation and bathymetry
        ↓
Drainage and watersheds
        ↓
Erosion and sediment
        ↓
Coastal classification
        ↓
Volumetric conversion
        ↓
Caves, overhangs and sea caves
        ↓
Materials and biome blending
        ↓
Voxel density sampling and meshing
```

Each later stage must read the products of earlier stages.

For example:

* Rivers must follow accumulated drainage.
* Valleys must be carved around rivers.
* Beaches must occur where slopes, sediment and exposure permit.
* Cliffs must be associated with resistant rock, steep coastal profiles or erosion scarps.
* Caves must reflect geology and depth.
* Biome color must respond to elevation, moisture, slope, exposure and substrate.
* Underwater drop-offs must be part of the island’s geological profile, not independent underwater noise.

Hydrology-based terrain methods are effective precisely because they construct river networks and terrain together rather than painting rivers onto an unrelated surface afterward. ([Purdue Computer Science][1])

---

# 2. Recommended data products

For one complete island vertical slice, generate and retain these aligned fields:

```rust
pub struct IslandAtlas {
    // Primary shape
    pub elevation: Field2D<f32>,
    pub bathymetry: Field2D<f32>,
    pub island_mask: Field2D<f32>,

    // Geological structure
    pub rock_hardness: Field2D<f32>,
    pub permeability: Field2D<f32>,
    pub volcanic_age: Field2D<f32>,
    pub fracture_density: Field2D<f32>,

    // Terrain derivatives
    pub slope: Field2D<f32>,
    pub curvature: Field2D<f32>,
    pub coast_distance: Field2D<f32>,
    pub ocean_distance: Field2D<f32>,

    // Hydrology
    pub filled_elevation: Field2D<f32>,
    pub flow_direction: Field2D<u8>,
    pub flow_accumulation: Field2D<f32>,
    pub discharge: Field2D<f32>,
    pub river_mask: Field2D<f32>,
    pub wetness: Field2D<f32>,
    pub sediment: Field2D<f32>,

    // Surface classification
    pub cliff_mask: Field2D<f32>,
    pub beach_mask: Field2D<f32>,
    pub talus_mask: Field2D<f32>,
    pub soil_depth: Field2D<f32>,
    pub biome_weights: Field2D<BiomeWeights>,

    // Explicit features
    pub volcano: VolcanoDescriptor,
    pub river_graph: RiverGraph,
    pub cave_graph: CaveGraph,
    pub overhangs: Vec<OverhangDescriptor>,
}
```

These should be generated at a coarser resolution than the final voxels.

For a vertical slice, a practical hierarchy is:

```text
Macro field:       8–16 m per sample
Hydrology field:   2–4 m per sample
Local field:       1–2 m per sample
Voxel density:     0.5–1 m per cell
```

Do not run expensive erosion independently for every voxel chunk. Generate coherent island-wide fields first, then let chunks sample them.

---

# 3. Island footprint algorithm

A realistic island should have a controlled geological form before noise is added.

## 3.1 Elliptical or warped radial support field

Start with an oriented ellipse:

```rust
fn elliptical_distance(p: Vec2, center: Vec2, radii: Vec2, angle: f32) -> f32 {
    let q = rotate(p - center, -angle);
    Vec2::new(q.x / radii.x, q.y / radii.y).length()
}
```

Convert it into a support mask:

```rust
fn island_support(d: f32, coast_start: f32, coast_end: f32) -> f32 {
    1.0 - smoothstep(coast_start, coast_end, d)
}
```

Then domain-warp the coordinates:

```rust
let warp = Vec2::new(
    noise_x.sample(p * warp_frequency),
    noise_z.sample(p * warp_frequency),
) * warp_amplitude;

let d = elliptical_distance(p + warp, center, radii, rotation);
```

Use low-frequency warping only. Excessive warp creates a melted or amoeba-like coastline.

## 3.2 Use several overlapping lobes

A single radial function often looks like a perfect video-game island. Combine 2–5 geological lobes:

```text
main volcano
older eroded ridge
secondary cone
collapsed coastal flank
offshore islet
```

Combine their influence with a smooth maximum:

```rust
fn smooth_max(a: f32, b: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (a - b) / k).clamp(0.0, 1.0);
    b.lerp(a, h) + k * h * (1.0 - h)
}
```

This produces an island with related but noncircular land masses rather than a single noise threshold.

---

# 4. Volcano and mountain algorithm

For your vertical slice, I would explicitly construct one volcanic mountain rather than asking ridged fBm to invent it.

## 4.1 Base volcanic cone

Let (r) be normalized radial distance from the volcanic center.

```rust
fn cone_profile(r: f32, exponent: f32) -> f32 {
    (1.0 - r).max(0.0).powf(exponent)
}
```

Useful values:

```text
1.0–1.5   broad shield volcano
1.8–2.8   steeper composite-looking profile
3.0+      narrow, exaggerated peak
```

For a tropical volcanic island, use a broad lower shield plus a steeper summit component:

```rust
let shield = cone_profile(r, 1.25) * 220.0;
let summit = cone_profile(r / 0.42, 2.5) * 130.0;
let height = shield + summit;
```

## 4.2 Add a caldera

Subtract a smooth crater field near the summit:

```rust
fn annular_crater(r: f32, center_r: f32, width: f32) -> f32 {
    let x = ((r - center_r) / width).abs();
    (1.0 - smoothstep(0.0, 1.0, x)).max(0.0)
}
```

Use both central depression and raised rim:

```rust
height -= central_bowl * crater_depth;
height += crater_rim * rim_height;
```

Avoid a perfectly circular caldera. Apply mild angular modulation:

```rust
radius *= 1.0 + 0.08 * noise.sample(direction * 2.0);
```

## 4.3 Radial ridges

Volcanic islands commonly need radial ridges and gullies to read convincingly.

Create 5–10 ridge splines from the summit or upper slopes. For every point, compute distance to the nearest ridge spline:

```rust
ridge = exp(-distance_to_ridge.powi(2) / (2.0 * width.powi(2)));
```

Then:

```rust
height += ridge * ridge_strength * elevation_mask;
```

Use a wider ridge near its origin and narrow it downhill.

## 4.4 Sector collapse

To break the idealized cone, remove one flank with a broad directional mask:

```rust
let angular_match = angular_gaussian(theta, collapse_direction, spread);
let radial_band = smoothstep(inner_r, outer_r, r);
height -= angular_match * radial_band * collapse_depth;
```

This can create:

* a coastal embayment,
* a steep amphitheater,
* a natural cliff sector,
* a likely river basin,
* a plausible site for landslide debris underwater.

## 4.5 Noise use

Use noise only as residual detail:

```rust
height += fbm(p * 0.002) * 12.0;
height += ridged_fbm(p * 0.007) * mountain_mask * 6.0;
height += fbm(p * 0.035) * 0.8;
```

The amplitudes should decrease strongly with frequency. If 1-meter-scale noise is moving the terrain vertically by several meters, the surface will look crumpled rather than geological.

---

# 5. Hydrology: build the river before carving it

A river should not be generated as a random spline drawn from mountain to sea. It should arise from the drainage field, though you may steer its source or mouth.

## 5.1 Resolve depressions

Run **Priority-Flood** on the provisional elevation field. It floods inward from the boundary using a priority queue and guarantees a drainable surface, avoiding digital pits that trap flow. For floating-point elevation, its expected complexity is (O(n \log n)). ([Experts@Minnesota][2])

Do not necessarily fill every depression completely. Classify depressions as:

```text
tiny numerical basin     fill
shallow wetland basin    preserve
caldera lake             preserve or provide outlet
large implausible basin  breach
```

Maintain both:

```rust
original_elevation
hydrology_elevation
```

The filled/breached version is used for routing; the original remains available for terrain shaping.

## 5.2 Flow direction

For a vertical slice, start with D8:

```text
Each cell flows to the steepest of its 8 neighbors.
```

D8 is simple and deterministic, but it can produce strongly grid-aligned channels. The usual DEM workflow is sink resolution, flow direction, accumulation and then thresholding the accumulation field into streams. ([MDPI][3])

Better options later:

* **D∞** for direction distributed between two neighbors.
* **MFD** for hillslope runoff distributed among several downslope neighbors.
* Hybrid MFD for diffuse slopes and D8 for established channels.

Flow-accumulation systems commonly support D8, MFD and D∞ because each gives different drainage behavior. ([ArcGIS Pro][4])

## 5.3 Flow accumulation

Process cells from highest to lowest hydrologic elevation:

```rust
accum[cell] = rainfall[cell];

for cell in descending_elevation_order {
    let downstream = flow_direction[cell];
    accum[downstream] += accum[cell];
}
```

Use rainfall and permeability rather than a constant of 1:

```rust
runoff =
    rainfall
    * (1.0 - permeability)
    * saturation_modifier;
```

## 5.4 Stream extraction

Instead of a single global threshold:

```rust
river = accumulation > threshold;
```

use terrain-dependent thresholds:

```rust
threshold =
    base_threshold
    * lerp(1.3, 0.7, rainfall_normalized)
    * lerp(1.4, 0.75, slope_normalized);
```

This allows small channels on steep wet slopes while avoiding many streams across flat coastal plains.

Prune streams that are:

* shorter than a minimum length,
* disconnected from meaningful outlets,
* lower than a minimum Strahler order,
* too close to stronger neighboring channels.

GRASS stream extraction similarly exposes flow accumulation thresholds and minimum stream lengths to eliminate insignificant first-order segments. ([GRASS][5])

## 5.5 Ensure one showcase river

For the vertical slice, select a river path satisfying:

```text
source elevation high enough
catchment area large enough
path reaches ocean
length within desired range
not too close to map boundary
passes through at least two biome zones
```

You can score candidate channels:

```rust
score =
    0.30 * normalized_length
  + 0.25 * normalized_discharge
  + 0.20 * elevation_drop
  + 0.15 * biome_variety
  + 0.10 * distance_from_other_major_features;
```

Select the best candidate and designate it as the primary river.

---

# 6. River channel carving

Separate river **path**, **channel shape**, **water level** and **valley incision**.

## 6.1 Channel width from discharge

```rust
width = min_width + width_scale * discharge.powf(0.45);
depth = min_depth + depth_scale * discharge.powf(0.35);
```

For a small vertical-slice island:

```text
Headwater:
width 0.8–2 m
depth 0.2–0.7 m

Middle course:
width 2–5 m
depth 0.5–1.5 m

Mouth:
width 4–10 m
depth 1–2.5 m
```

## 6.2 Cross-section

Use a smooth channel profile rather than a flat trench:

```rust
fn channel_profile(distance: f32, half_width: f32, depth: f32) -> f32 {
    let t = (distance / half_width).clamp(0.0, 1.0);
    depth * (1.0 - t * t).powf(1.5)
}
```

Then:

```rust
terrain_height -= channel_profile(distance_to_centerline, half_width, depth);
```

## 6.3 Valley profile

Create a second, much wider and shallower incision:

```rust
valley_width = channel_width * 5.0..15.0;
valley_depth = channel_depth * 1.0..3.0;
```

```rust
terrain_height -= gaussian(distance_to_river, valley_width) * valley_depth;
```

Without this, the river looks cut into otherwise unrelated terrain.

## 6.4 Longitudinal profile

The river bed must descend monotonically:

```rust
bed[i] = min(
    sampled_terrain[i] - local_channel_depth,
    bed[i - 1] - minimum_gradient * segment_length,
);
```

Smooth this profile while preserving downstream descent.

Possible minimum gradients:

```text
Upper reach:  0.02–0.12
Middle:       0.005–0.04
Lower:        0.001–0.01
```

## 6.5 Waterfalls

A waterfall candidate occurs where:

```text
discharge above threshold
local elevation drop is large
rock hardness is high
downstream pool has enough space
```

Carve:

```text
short lip
near-vertical drop
plunge pool
narrow downstream gorge
talus or boulder zone
```

Do not merely make water descend a vertical voxel wall.

---

# 7. Erosion algorithms

Your complete vertical slice does not need a long, globally simulated geomorphology model, but it needs enough erosion to tie features together.

## 7.1 Stream-power erosion

Use a simplified stream-power law:

[
E = K A^m S^n
]

where:

* (E) is erosion amount,
* (K) depends on rock erodibility,
* (A) is contributing drainage area or discharge,
* (S) is local slope,
* (m) is commonly below 1,
* (n) is commonly around 1 in simplified models.

Implementation:

```rust
let erosion =
    erodibility
    * discharge.powf(m)
    * slope.max(0.0).powf(n)
    * dt;

elevation[cell] -= erosion.min(max_erosion_step);
```

Starting values:

```yaml
stream_power:
  m: 0.45
  n: 1.0
  rate: 0.00002
  iterations: 20
  maximum_step_m: 0.25
```

Analytical and simulation-based stream-power methods are used specifically to produce drainage-consistent mountain and valley morphology. ([Inria Côte d'Azur][6])

## 7.2 Constraint-aware erosion

Protect required features:

```rust
actual_erosion =
    proposed_erosion
    * (1.0 - landmark_constraint)
    * (1.0 - peak_preservation)
    * erodibility;
```

This lets you maintain:

* volcano silhouette,
* cave entrance cover,
* traversal routes,
* designated cliff,
* intended beach,
* river mouth.

Constraint maps and rainfall-driven graph erosion have been proposed specifically to improve terrain variety while retaining designer control. ([arXiv][7])

## 7.3 Thermal erosion

After fluvial incision, relax slopes exceeding the local talus angle:

```rust
if slope > talus_angle {
    let excess = height_difference - allowed_difference;
    transfer = excess * relaxation_rate;
}
```

Different materials should use different talus angles:

```text
Loose sand:        28–34°
Wet soil:          25–35°
Volcanic ash:      28–36°
Weathered basalt:  35–42°
Solid basalt:      55°+ before cliff failure
```

This creates:

* talus at cliff bases,
* softened soil slopes,
* sharp but believable rock faces,
* sediment available for beaches and river mouths.

## 7.4 Hydraulic droplet erosion

Droplet erosion can add small gullies, but it should be a finishing pass, not the foundation. GPU hydraulic erosion models can simulate water movement, erosion, transport and deposition efficiently, but they remain more expensive and less controllable than a drainage-first approach. ([Inria HAL][8])

Use it only on:

* the volcanic upper slopes,
* wet windward terrain,
* the primary river basin,
* a few medium-resolution tiles.

Avoid millions of droplets over the whole high-resolution island.

---

# 8. Cliff generation

Cliffs should come from several causes.

## 8.1 Structural cliffs

Create explicit fault or lava-flow escarpments as splines or arcs.

```rust
cliff_offset =
    cliff_height
    * smoothstep(-width, width, signed_distance_to_fault);
```

Add broken segments and varying height, but retain a coherent escarpment.

## 8.2 Coastal cliffs

Classify a coast as cliff where:

```text
land slope high
rock hardness high
wave exposure high
sediment supply low
```

```rust
cliff_score =
    slope_score
    * hardness_score
    * exposure_score
    * (1.0 - sediment_score);
```

Carve undercut near sea level:

```rust
undercut_mask =
    cliff_score
    * vertical_band(y, sea_level - 2.0, sea_level + 1.5)
    * horizontal_coast_band;
```

Subtract a shallow cavity from the 3D density field.

## 8.3 River gorges

Where discharge and rock hardness are both high:

```text
narrow valley
steep sidewalls
limited floodplain
exposed rock
```

Where erodibility is high:

```text
broader valley
gentler banks
more sediment
```

## 8.4 Meshing implications

If cliffs remain rounded even when the density field is correct, the problem may be the mesher. Dual Contouring places vertices from Hermite intersection data and is designed to preserve sharp features better than simpler averaging methods. ([Computer Science | Rice University][9])

A good progression is:

```text
Surface Nets for first slice
→ constrained Surface Nets for important cliffs
→ Dual Contouring when sharp terrain is essential
```

---

# 9. Beaches

A beach is not just “sand within five meters of sea level.”

Calculate a beach suitability score:

```rust
beach_score =
    low_slope
    * available_sediment
    * moderate_exposure
    * low_cliff_score
    * suitable_coastal_curvature;
```

## 9.1 Inputs

Use:

* slope,
* accumulated sediment,
* distance from coast,
* wave exposure,
* river-mouth proximity,
* coastal curvature,
* rock hardness.

## 9.2 Beach profile

A typical profile should include:

```text
shallow underwater apron
foreshore slope
berm
back-beach transition
```

For a stylized but believable beach:

```text
Underwater slope: 1:20 to 1:8
Foreshore slope:  1:12 to 1:5
Berm height:      0.5–2 m above sea level
Beach width:      5–35 m
```

Blend the coast toward a target beach profile only where suitability is high:

```rust
height = lerp(original_height, beach_profile_height, beach_mask);
```

Do not flatten the entire shoreline. Alternate between:

* sandy beach,
* cobble beach,
* mangrove or wetland,
* rocky shore,
* cliff,
* river mouth.

---

# 10. Underwater terrain and sudden drop-offs

The underwater terrain should continue the island’s geological story.

## 10.1 Distance-based coastal profile

Compute signed distance from the shoreline:

```text
positive inland
negative offshore
```

Create basic depth from offshore distance:

```rust
fn shelf_depth(distance: f32, shelf_width: f32, shelf_depth: f32) -> f32 {
    let t = (distance / shelf_width).clamp(0.0, 1.0);
    -shelf_depth * smoothstep(0.0, 1.0, t)
}
```

## 10.2 Shelf break

After the shallow shelf, transition into a steeper slope:

```rust
if distance < shelf_width {
    depth = shallow_profile(distance);
} else {
    let beyond = distance - shelf_width;
    depth = -shelf_depth - beyond * deep_slope;
}
```

Vary shelf width by coastline sector:

```text
Sheltered beach:     broad shelf
River mouth:         broad sedimentary shelf
Young volcanic coast: narrow shelf
Collapsed flank:     abrupt drop-off
Reef coast:          shallow platform then reef wall
```

## 10.3 Sudden drop-offs

Add explicit submarine scarps:

```rust
drop = smoothstep(start_distance, end_distance, offshore_distance)
     * directional_sector_mask
     * drop_depth;
```

Good vertical-slice examples:

* one reef wall,
* one volcanic flank drop,
* one landslide scar,
* one shallow lagoon or cove.

## 10.4 Submarine landslide debris

Below a collapsed volcanic flank, add hummocky deposits:

```rust
depth += debris_lobes(p) * 2.0..12.0;
```

Use sparse warped ellipsoidal mounds, not high-frequency noise across the whole seabed.

---

# 11. Volumetric density conversion

Once the final surface height is ready:

```rust
fn base_density(p: Vec3, surface_y: f32) -> f32 {
    p.y - surface_y
}
```

Using:

```text
density < 0 = solid
density > 0 = air or water
```

Then compose volumetric features:

```rust
density = solid_union(density, added_rock_mass);
density = solid_subtract(density, cave_volume);
density = solid_subtract(density, cliff_undercut);
density = solid_subtract(density, sea_cave);
density = solid_union(density, natural_arch_mass);
```

Use constructive operations:

```rust
fn union(a: f32, b: f32) -> f32 {
    a.min(b)
}

fn intersection(a: f32, b: f32) -> f32 {
    a.max(b)
}

fn subtract(solid: f32, cavity: f32) -> f32 {
    solid.max(-cavity)
}
```

---

# 12. Cave generation

Do not use thresholded 3D noise alone. It generally produces sponge terrain and disconnected cavities.

A researched cave-generation pipeline uses an explicit network generator followed by noise-perturbed implicit volumes rather than relying solely on random 3D density. One published method uses an L-system for passage structure, metaball-based volume construction and cellular automata refinement. ([julian.togelius.com][10])

For your vertical slice, use a simpler graph-plus-SDF version.

## 12.1 Build a cave graph

Define nodes:

```rust
pub enum CaveNodeKind {
    Entrance,
    Chamber,
    Junction,
    Shaft,
    Pool,
    Squeeze,
    Terminus,
}
```

Example graph:

```text
cliff entrance
    ↓
entrance chamber
    ↓
main descending passage
    ├── small side chamber
    └── lower chamber
            ↓
         water pool
```

Keep it small:

```text
4–8 chambers
5–12 passage segments
20–80 m total length
1–2 vertical changes
```

## 12.2 Place the entrance intentionally

The entrance should satisfy:

```text
visible from a plausible approach
sufficient rock cover behind it
not beneath river bed unless intended
not immediately flooded
slope suitable for opening
cave network remains inside island mass
```

Good positions:

* cliff base,
* cliff wall above beach,
* upper river gorge,
* collapsed lava tube,
* caldera wall.

## 12.3 Passage SDF

Use a capsule along each graph edge:

```rust
fn capsule_sdf(p: Vec3, a: Vec3, b: Vec3, radius: f32) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    let closest = a + ab * t;
    p.distance(closest) - radius
}
```

Perturb the wall:

```rust
let cave =
    capsule_sdf(p, a, b, radius)
    + fbm3(p * wall_frequency) * wall_roughness;
```

Carve where `cave < 0`.

## 12.4 Chamber SDF

Use warped ellipsoids:

```rust
fn ellipsoid_sdf(p: Vec3, center: Vec3, radii: Vec3) -> f32 {
    ((p - center) / radii).length() - 1.0
}
```

Combine overlapping chambers and passages smoothly.

## 12.5 Floor bias

Pure capsules have rounded bottoms that are awkward to walk on. Modify the vertical coordinate or intersect with a floor field:

```rust
cave_density += floor_flattening(p, centerline, desired_floor_y);
```

Alternatively, after carving:

```text
sample cave interior
find local floor
flatten only within navigation corridor
leave walls and ceiling irregular
```

## 12.6 Cave noise constraints

Allow 3D noise only where:

```text
depth below surface > 4–8 m
distance from entrance network < 5–15 m
geology supports cavities
ceiling thickness remains above minimum
```

This preserves intentional connectivity.

---

# 13. Overhang with cave entrance

Generate this as an authored procedural feature, not a lucky density accident.

## 13.1 Choose a cliff anchor

Select a point where:

```text
slope > 45°
rock hardness > 0.6
vertical cliff extent > 6 m
there is walkable terrain below
sufficient solid volume lies behind the face
```

## 13.2 Add a projecting rock shelf

Use an elongated ellipsoid or rounded box:

```rust
shelf_mass = ellipsoid_sdf(
    p,
    anchor + outward * 2.0 + Vec3::Y * 3.0,
    Vec3::new(7.0, 2.5, 5.0),
);
```

Union it with the terrain.

## 13.3 Carve underneath

```rust
undercut = ellipsoid_sdf(
    p,
    anchor + outward * 1.5,
    Vec3::new(5.0, 2.0, 4.0),
);
```

Subtract it.

## 13.4 Carve the entrance

Connect the undercut cavity directly to the first cave passage capsule.

## 13.5 Validate

Raycast or flood-fill to confirm:

* exterior air reaches the entrance chamber,
* the entrance is not too narrow,
* the overhang has minimum thickness,
* the player can stand beneath it,
* no accidental hole opens through the roof.

---

# 14. Materials and geology

Assign materials according to geology and depth, not only biome.

Example volcanic profile:

```text
0–0.3 m: leaf litter, sand or exposed rock
0.3–2 m: soil / weathered ash
2–8 m: weathered basalt
8 m+: dense basalt
localized: ash, tuff, fractured basalt, alluvium
```

```rust
fn material_at(
    depth: f32,
    slope: f32,
    biome: &BiomeWeights,
    geology: Geology,
    sediment: f32,
) -> MaterialId
```

Rules:

```text
High slope:
    reduce soil depth
    expose rock

River floodplain:
    alluvium
    wet soil
    gravel near channel

Beach:
    sand above and below sea level
    exposed basalt where sediment is low

Cliff:
    exposed bedrock
    talus at base

Cave:
    underlying geology
    moisture staining
    sediment on floors
```

---

# 15. Biome coloring without hard bands

The “biome colors look procedural” problem often comes from using hard categories such as:

```text
height < 5 = sand
height < 60 = grass
height > 60 = rock
```

Instead calculate suitability weights.

## 15.1 Core environmental inputs

For each surface point:

```text
elevation
slope
aspect
rainfall
wetness
distance from river
distance from ocean
salt exposure
soil depth
rock exposure
temperature
```

## 15.2 Suitability functions

```rust
fn range_weight(value: f32, min: f32, max: f32, fade: f32) -> f32 {
    smoothstep(min - fade, min, value)
        * (1.0 - smoothstep(max, max + fade, value))
}
```

Example rainforest:

```rust
rainforest =
    range_weight(rainfall, 0.65, 1.0, 0.15)
    * range_weight(elevation, 10.0, 180.0, 30.0)
    * range_weight(slope, 0.0, 38.0, 10.0)
    * soil_depth_weight;
```

Example exposed volcanic rock:

```rust
volcanic_rock =
    high_slope
    .max(high_elevation)
    .max(low_soil_depth)
    * basalt_geology;
```

Normalize the top 3–4 weights and pass them to the terrain shader.

Pipeline-based biome-generation systems commonly combine terrain, climate and elevation inputs rather than deriving biomes from elevation alone. ([cgvr.cs.uni-bremen.de][11])

## 15.3 Break contour-line boundaries

Add low-amplitude selection noise to biome weights:

```rust
weight *= 1.0 + noise.sample(p * 0.015) * 0.15;
```

Do not perturb underlying hydrology or terrain height at this stage.

## 15.4 Use triplanar materials

For cliffs, caves and overhang undersides, conventional top-down UVs will stretch. Use world-space triplanar projection weighted by the absolute normal components.

Bevy meshes can carry arbitrary per-vertex attributes in addition to positions and normals, allowing biome weights or material indices to be supplied to a custom terrain material. ([Docs.rs][12])

---

# 16. Density detail at 0.5–1 meter

At your voxel scale, detail amplitudes should be restrained.

For 1-meter voxels:

```yaml
density_detail:
  macro_roughness:
    wavelength_m: 16–40
    amplitude_m: 0.8–2.5

  rock_roughness:
    wavelength_m: 4–12
    amplitude_m: 0.2–0.8

  micro_roughness:
    wavelength_m: 1.5–4
    amplitude_m: 0.05–0.25
```

For 0.5-meter voxels, you can roughly halve the final wavelength and amplitude ranges.

Avoid adding micro-noise directly to every material boundary. Sand, soil, river sediment and beaches should be smoother than exposed rock.

Use material-aware roughness:

```rust
detail_amplitude =
    match material {
        Sand => 0.05,
        Soil => 0.12,
        WeatheredRock => 0.35,
        Basalt => 0.50,
        RiverSediment => 0.08,
    };
```

---

# 17. Surface extraction

For the first slice:

## Surface Nets

Advantages:

* straightforward implementation,
* relatively low vertex count,
* smooth caves and terrain,
* good enough for most natural surfaces.

Weakness:

* averages intersections,
* can overly soften cliffs and corners.

## Dual Contouring

Advantages:

* better retention of sharp cliffs,
* better for construction and ruins,
* handles Hermite normals and edge intersections,
* supports simplification strategies. ([Computer Science | Rice University][9])

My recommendation:

```text
Use Surface Nets now.
Store accurate density gradients and edge intersections.
Keep the mesher interface replaceable.
Upgrade to Dual Contouring after generation quality is correct.
```

Do not try to solve unrealistic landforms through the mesher. The density field has to be structurally correct first.

---

# 18. Complete generation order

Here is the concrete order I recommend.

## Phase A — Macro geology

1. Create island extent and sea level.
2. Place main volcano center.
3. Add broad shield profile.
4. Add summit cone.
5. Add caldera.
6. Add radial ridge splines.
7. Add one collapsed flank.
8. Add secondary geological lobes.
9. Apply low-frequency domain warp.
10. Add restrained regional fBm.

Output:

```text
base elevation
island mask
volcanic age
rock hardness
fracture fields
```

## Phase B — Bathymetry

1. Compute coast distance.
2. Create shallow coastal profile.
3. Vary shelf width by coastal sector.
4. Add shelf break.
5. Add collapsed-flank drop-off.
6. Add one reef platform or underwater ledge.
7. Add sparse submarine debris.
8. Blend into deep-ocean floor.

Output:

```text
bathymetry
shelf mask
drop-off mask
reef suitability
```

## Phase C — Hydrology

1. Generate provisional rainfall.
2. Run Priority-Flood.
3. Preserve designated caldera or wetland basin.
4. Calculate D8 or MFD flow.
5. Accumulate runoff.
6. Extract stream graph.
7. Select primary river.
8. Smooth primary centerline.
9. Generate monotonic bed profile.
10. Carve channel and valley.

Output:

```text
river graph
flow accumulation
discharge
wetness
river mask
```

## Phase D — Erosion

1. Run 10–30 stream-power iterations.
2. Apply geological erodibility.
3. Protect required landmarks.
4. Transport simplified sediment downstream.
5. Deposit at low-gradient reaches and river mouth.
6. Run thermal relaxation.
7. Optionally run local droplet erosion.
8. Recalculate slope and curvature.

Output:

```text
eroded elevation
sediment
talus
soil depth
```

## Phase E — Coastline features

1. Calculate wave exposure.
2. Classify cliff sectors.
3. Classify beach sectors.
4. Generate beach profiles.
5. Add rocky shoreline transitions.
6. Add coastal undercuts.
7. Add river-mouth delta or estuary.
8. Recalculate shoreline and coast distance.

Output:

```text
beaches
cliffs
rocky shores
river mouth
```

## Phase F — Convert to density

1. Convert elevation/bathymetry to base SDF.
2. Add rock shelf for overhang.
3. Carve overhang cavity.
4. Place cave graph.
5. Carve passages and chambers.
6. Carve entrance.
7. Add one sea cave if desired.
8. Validate roof thickness and connectivity.

Output:

```text
authoritative density field
```

## Phase G — Surface and biome classification

1. Calculate surface normals.
2. Resolve geology by depth.
3. Calculate soil depth.
4. Calculate moisture and river influence.
5. Generate biome suitability weights.
6. Assign 3–4 dominant material weights.
7. Add local surface variation.
8. Generate meshes and collision.

---

# 19. Suggested vertical-slice feature budget

Keep the island compact enough to regenerate quickly while still containing every required feature.

A good initial target:

```text
Island playable diameter:   1.5–3 km
Maximum elevation:          250–500 m
Sea depth in slice:         80–250 m
Primary river length:       600–1,800 m
Main cave length:           40–120 m
Major cliff section:        80–250 m
Main beach:                 100–350 m
Voxel resolution nearby:    0.5–1 m
```

This is large enough for a convincing island but small enough to repeatedly regenerate while tuning algorithms.

Feature checklist:

```text
1 main volcano or mountain
1 caldera or summit depression
5–8 principal ridges
1 primary drainage basin
1 permanent river
1 waterfall or rapid
1 sandy beach
1 rocky beach or shore
1 major sea cliff
1 coastal shelf
1 sudden submarine drop-off
1 reef or shallow platform
1 overhang
1 visible cave entrance
1 connected cave system
6–10 blended biome/material regimes
```

---

# 20. Useful starting parameters

```yaml
island:
  playable_diameter_m: 2200
  maximum_height_m: 360
  sea_level_m: 0

volcano:
  shield_radius_m: 950
  shield_exponent: 1.3
  shield_height_m: 230

  summit_radius_m: 360
  summit_exponent: 2.4
  summit_height_m: 135

  caldera_radius_m: 90
  caldera_depth_m: 38
  caldera_rim_height_m: 12

  radial_ridges:
    count: 7
    width_m: [35, 110]
    height_m: [8, 32]
    length_m: [350, 900]

surface_noise:
  regional:
    wavelength_m: [100, 450]
    amplitude_m: 14

  local:
    wavelength_m: [15, 80]
    amplitude_m: 3.5

  voxel:
    wavelength_m: [2, 12]
    amplitude_m: 0.35

hydrology:
  routing: d8
  rainfall_base: 1.0
  stream_threshold: 220
  permanent_river_threshold: 900
  minimum_stream_length_m: 60

erosion:
  stream_power_iterations: 22
  m: 0.48
  n: 1.0
  maximum_step_m: 0.2
  thermal_iterations: 12
  thermal_transfer_rate: 0.16

coast:
  shelf_width_m: [60, 300]
  shelf_depth_m: [8, 28]
  deep_slope: [0.18, 0.65]

beaches:
  maximum_slope_deg: 13
  width_m: [8, 35]
  berm_height_m: [0.6, 1.8]

caves:
  chamber_count: [4, 7]
  passage_radius_m: [1.4, 3.5]
  chamber_radius_m: [3.5, 9]
  minimum_cover_m: 4
  maximum_depth_m: 45
  wall_noise_amplitude_m: 0.4
```

Treat these as calibration values, not final physical constants.

---

# 21. Validation tests

Automated validation will save far more time than visual seed inspection alone.

## Island validation

```text
All map edges are underwater.
Island land area is within target range.
Highest point is within volcano region.
No disconnected one-voxel land specks.
```

## River validation

```text
Primary river reaches ocean.
River bed never rises downstream.
No channel segment passes above its containing terrain.
Minimum water depth is maintained.
Maximum bank slope is within bounds except at waterfalls.
```

## Cave validation

```text
Entrance connects to every required chamber.
Minimum passage width supports player collider.
Minimum roof thickness is respected.
No accidental surface holes exist.
No chamber is completely submerged unless intended.
```

## Coast validation

```text
Beach slopes stay within profile range.
Cliff and beach masks do not strongly overlap.
Shelf depth is continuous at chunk boundaries.
Sudden drop-offs do not produce isolated spikes.
```

## Density validation

```text
Shared chunk samples are bit-identical.
No NaN or infinity values.
Density gradient remains valid around surface crossings.
No nonmanifold surfaces from microscopic isolated solids.
```

---

# 22. Debug views you should add immediately

The quickest way to tune this is to visualize each field separately.

Create toggles for:

```text
Raw elevation
Geological base only
Residual noise only
Slope
Curvature
Rock hardness
Filled elevation
Flow direction
Flow accumulation
River hierarchy
Stream-power erosion amount
Sediment
Beach suitability
Cliff suitability
Shelf depth
Cave SDF
Final density
Biome weights
Material weights
```

Also add:

```text
Disable all local noise
Disable erosion
Disable cave carving
Show feature spline centerlines
Show voxel density zero crossings
Show cave roof thickness
```

If the terrain only looks plausible after all noise layers are turned on, the macro structure is still too weak.

---

# 23. The most important practical rules

1. **Generate one coherent geological island before adding detail.**
2. **Use noise to perturb structure, not replace structure.**
3. **Calculate the river from drainage before carving it.**
4. **Carve a valley around the channel, not only the channel itself.**
5. **Generate beaches from slope, sediment and exposure.**
6. **Continue landform logic underwater.**
7. **Use explicit cave graphs with SDF passages.**
8. **Author the showcase overhang procedurally from primitives.**
9. **Blend biome weights rather than assigning hard color bands.**
10. **Run expensive simulation on island fields, not independently in voxel chunks.**
11. **Keep surface generation and volumetric modifications separate.**
12. **Validate each required feature before accepting a seed.**

The best immediate milestone is not “make every possible island.” It is:

> Produce one deterministic island seed whose geological, hydrological, coastal and volumetric features all pass validation, then expose enough YAML parameters to create controlled variations of that island.

That gives you a convincing complete vertical slice without prematurely building a generalized planet-scale terrain simulator.

[1]: https://www.cs.purdue.edu/cgvlab/www/resources/papers/Genevaux-ACM_Trans_Graph-2013-Terrain_Generation_Using_Procedural_Models_Based_on_Hydrology.pdf?utm_source=chatgpt.com "Terrain generation using procedural models based on hydrology"
[2]: https://experts.umn.edu/en/publications/priority-flood-an-optimal-depression-filling-and-watershed-labeli/?utm_source=chatgpt.com "Priority-flood: An optimal depression-filling and watershed ..."
[3]: https://www.mdpi.com/2220-9964/10/3/186?utm_source=chatgpt.com "Setting the Flow Accumulation Threshold Based on ..."
[4]: https://pro.arcgis.com/en/pro-app/3.6/tool-reference/spatial-analyst/flow-accumulation.htm?utm_source=chatgpt.com "Flow Accumulation (Spatial Analyst) - ArcGIS Pro"
[5]: https://grass.osgeo.org/grass-stable/manuals/r.stream.extract.html?utm_source=chatgpt.com "r.stream.extract - GRASS 8.5 Documentation"
[6]: https://www-sop.inria.fr/reves/Basilic/2024/TGSC24/Analytical_Terrains_EG.pdf?utm_source=chatgpt.com "Physically-based analytical erosion for fast terrain generation"
[7]: https://arxiv.org/abs/2210.14496?utm_source=chatgpt.com "Visually Improved Erosion Algorithm for the Procedural Generation of Tile-based Terrain"
[8]: https://inria.hal.science/inria-00402079/PDF/FastErosion_PG07.pdf?utm_source=chatgpt.com "Fast Hydraulic Erosion Simulation and Visualization on GPU"
[9]: https://www.cs.rice.edu/~jwarren/papers/dualcontour.pdf?utm_source=chatgpt.com "Dual Contouring of Hermite Data - Rice Computer Science"
[10]: https://julian.togelius.com/Mark2015Procedural.pdf?utm_source=chatgpt.com "Procedural Generation of 3D Caves for Games on the GPU"
[11]: https://cgvr.cs.uni-bremen.de/papers/cgi20/AutoBiomes.pdf?utm_source=chatgpt.com "procedural generation of multi-biome landscapes"
[12]: https://docs.rs/bevy/latest/bevy/prelude/struct.Mesh.html?utm_source=chatgpt.com "Mesh in bevy::prelude - Rust"
