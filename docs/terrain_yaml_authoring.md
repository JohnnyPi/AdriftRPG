# Terrain YAML Authoring Guide

This guide covers how the YAML configuration system works, the geometric
budget every world must satisfy, the meaning and units of each parameter
block, and the validation that now enforces all of it. It exists because the
most expensive terrain bugs in this project have not been code bugs or data
bugs in isolation — they have been *contradictions between defs that were
individually plausible*, silently absorbed by the generator. The canonical
example: a 2,200 m island authored inside a 256 m world, which rendered as a
steep, lumpy, alpine-only cone with a clipped summit.

The rule that prevents that entire bug class: **author every def at true
world scale, and let validation reject anything that doesn't fit.** Nothing
in the pipeline rescales for you anymore without shouting.

---

## 1. How the registry finds and wires defs

The loader (`game_data/src/load.rs`) walks the **entire assets tree** and
reads every file with a `.yaml` extension, regardless of directory. Files are
dispatched to a definition type purely by the **prefix of their `id:` field**
— `world.`, `terrain.`, `island_gen.`, `biomes.`, `materials.`, `surface.`,
`water.`, `lighting.`, `sky.`, and so on. Consequences worth internalizing:

- Directory layout is convention, not mechanism. Keep defs of one type
  together (e.g. `assets/config/terrain/`) for humans; the loader doesn't care.
- Every `id` must be unique across the whole tree. Duplicates fail the load.
- Every file needs `schema_version: 1` and an `id:` with a recognized prefix.
- Defs reference each other **by id, never by path**. A world def is a bundle
  of ids: `terrain:`, `biomes:`, `materials:`, `surface:`, `water:`,
  `lighting:`, plus optional `sky:`, `landmarks:`, `structures:`,
  `island_gen:`, `resolution:`.

Renaming a file changes nothing; renaming an `id` breaks every reference to
it (which fails loudly at registry build — this is good).

## 2. The two terrain systems — pick exactly one per world

A world's terrain shape comes from one of two sources, selected by whether
the world def has an `island_gen:` field.

**Op-based worlds** (no `island_gen:`). The terrain def's `operations:` list
is the terrain: `island_mask`, `ocean_floor`, `coastal_surface`,
`coast_modifier`, `mountain_peak`, `ellipsoid`, `capsule`, `noise_perturb`,
`underwater_trench`, plus cave defs pulled in via `includes:`. Everything is
hand-placed in recipe coordinates. If the op list compiles to empty, the
engine substitutes a hardcoded fallback slice so the world isn't a void.

**Atlas worlds** (`island_gen:` present). The island generator builds the
terrain procedurally from the `island_gen.*` def: footprint lobes, volcano,
noise, hydrology, erosion, coast, beaches. The terrain def's job shrinks to
the `spawn:` point. Generated cave ops are appended automatically from the
island def's `caves:` block. The empty-ops fallback is disabled for atlas
worlds.

**Never give an atlas world an op-based terrain def.** The ops do not know
the atlas exists; you get two coastlines, two ocean floors, and coves carved
into a volcano flank. The correct shape is a minimal terrain def:

```yaml
schema_version: 1
id: terrain.island_testbed
spawn: [70.0, 0.0, 160.0]
```

## 3. Coordinate systems

There are two horizontal coordinate frames:

**Recipe coordinates** are what you write in YAML for positions: op centers,
`volcano.center`, `spawn`. **World coordinates** are what the engine
simulates in. The world def's `coord_offset: [ox, oy, oz]` converts:
`world = recipe − offset`. With the conventional `coord_offset:
[128, 0, 128]`, recipe `[128, 128]` is world origin `(0, 0)` — which is why
`volcano.center: [128, 128]` puts the volcano at the center of the world.

Vertically there is no offset in practice (`oy = 0`); elevations in YAML are
world-space meters, and **sea level is an absolute elevation**, not zero by
definition (see §5).

## 4. The world budget — the math every def must satisfy

The chunk volume defines hard walls. From the world def:

```yaml
voxel:  { cell_size_m: 1.0 }
chunks: { cells: [16, 16, 16], world_extent: [16, 10, 16] }
```

Chunk placement is **centered** (`chunk_gen::chunk_axis_range`): the first
chunk index on each axis is `-(extent / 2)` using integer division. Each axis
therefore spans:

```
min = -(extent / 2)          * cells * cell_size
max = (extent - extent / 2)  * cells * cell_size
```

For `[16, 10, 16]` × 16 cells × 1 m: **X/Z ∈ [−128, +128), Y ∈ [−80, +80)**.
Note the odd-extent asymmetry: extent 3 spans `[−16, +32)`, not `[−24, +24)`.
Terrain outside this volume does not exist — surfaces above `y_max` clip into
a flat cap at the world ceiling; bathymetry below `y_min` clamps into a wall.

### 4.1 Horizontal budget

An island's footprint extends well past `playable_diameter_m / 2`. Lobe
centers are offset up to `0.18 × R` from the island center, lobe elliptical
radii reach `0.95 × R`, and the mask falloff extends support to `1.05 ×` the
lobe radius (see `footprint.rs`). Worst case:

```
support_radius = (playable_diameter_m / 2) × 1.1775 + warp_amplitude
```

(`FOOTPRINT_SUPPORT_FACTOR` in `island_gen/params.rs` — the single source of
truth shared by validation and the legacy fit.) Requirements:

1. `support_radius ≤` the smallest horizontal half-extent of the chunk
   volume, or the coastline clips at the world edge.
2. `support_radius + ocean_padding ≤ ocean_extent_m / 2` (the atlas must
   contain the island with a guaranteed ocean ring; padding is
   `max(4 × resolution.local_m, 16)`).
3. Leave room past the coastline for the shelf: the coastline typically sits
   near `0.72–0.8 ×` of the lobe radii, and `coast.shelf_width_max_m` plus
   the deep falloff must fit between it and the chunk edge.

Worked example (`world.vs3_island`): diameter 180 → support = 90 × 1.1775 +
6 = **112 m**, against a 128 m half-extent — a guaranteed ≥16 m ocean ring —
and against a 144 m atlas half-extent with 16 m padding to spare.

### 4.2 Vertical budget

```
maximum_height_m
  + surface_noise.regional_amplitude_m
  + surface_noise.local_amplitude_m
  ≤ y_max − 2 m margin
```

And the composed volcano must respect its own declaration:

```
shield_height_m + summit_height_m + caldera_rim_height_m ≤ maximum_height_m
```

(`maximum_height_m` isn't just a cap — classifiers and cave placement read it
as the island's advertised relief, so an undeclared taller volcano skews
both.) Downward:

```
sea_level − coast.shelf_depth_max_m − 8 m deep-falloff slack ≥ y_min
```

Worked example: 50 + 4 + 1.2 = 55.2 ≤ 78; shield 30 + summit 16 + rim 2 =
48 ≤ 50; sea 2 − 16 − 8 = −22 ≥ −80. All comfortable.

### 4.3 Proportion, not just fit

A config can fit and still look wrong. Sanity ratios that held for the
original large-island tuning and should hold at any scale:

| Ratio | Healthy range | Symptom when violated |
|---|---|---|
| `shield_height / shield_radius` | 0.2 – 0.45 | too high → steep cone, all-cliff/alpine classification |
| `regional_amplitude / maximum_height` | 4 – 10 % | too high → lumpy, noise-dominated silhouette |
| island diameter × `warp_frequency` | ≥ 2 – 4 | below ~1 → coastline is one big smear, not undulation |
| `warp_amplitude / (diameter/2)` | ≤ ~8 % | too high → footprint distortion, clipping risk |

The retired `fit_to_ocean_extent` auto-rescale violated the first two
structurally: it scaled radii by `s` but heights by `√s`, a 3.4× slope
exaggeration at `s = 0.085`. It still exists for tests but is documented as a
lossy last resort; validation rejects any config that would trigger it.

## 5. Sea level and the water def

Sea level lives in the **water def** (`sea_level_m` in e.g.
`water.tropical_shallow`, currently **2.0**), and the runtime uses that value
everywhere the atlas is concerned. The island def also carries a
`sea_level_m` field; generated-cave placement reads it. **The two must be
equal** — validation enforces agreement to within 0.01 m. When retuning sea
level, change the water def and mirror it in the island def.

All elevation-dependent thresholds elsewhere are absolute elevations in the
same frame: biome rule `elevation_min/max`, beach bands, cave ceiling limits.
Moving sea level from 0 to 2 shifts what "elevation 3.5" means relative to
the waterline; re-check the biome table (§7) after any sea-level change.

## 6. `island_gen.*` parameter reference — semantics and units

**`island:`** — `playable_diameter_m` (m; drives footprint, see §4.1),
`maximum_height_m` (m absolute; declared relief ceiling), `sea_level_m` (m;
must match water def), `lobe_count` (integer; footprint blob count),
`warp_frequency` (1/m; coastline warp period = 1/frequency),
`warp_amplitude` (m; worst-case outward coastline push — counts against the
horizontal budget).

**`volcano:`** — `center` (recipe coords, §3). Shield/summit are stacked
falloffs `h(r) = H × (1 − (r/R)^exponent)`: low slope near center, steepest
at `r = R` where slope ≈ `exponent × H / R`. `caldera_*` carve a pit of
`caldera_depth_m` inside `caldera_radius_m` with a rim raised
`caldera_rim_height_m` (rim counts toward the relief total).
`radial_ridge_count` (integer), `collapse_direction_deg` (compass degrees;
also the azimuth the generated cave system and mouth follow),
`collapse_depth_m` (m; keep ~10–15 % of total relief).

**`surface_noise:`** — three amplitude tiers in meters (regional / local /
voxel), sampled at the corresponding resolution tiers. Budgeted in §4.2 and
ratio-checked in §4.3.

**`hydrology:`** — the trap block. `stream_threshold` and
`permanent_river_threshold` are **flow-accumulation cell counts on the
regional grid**, not meters or m². Grid size = `ocean_extent_m /
resolution.regional_m` per side (288 / 8 = 36 × 36 here), of which the island
occupies roughly `π × (diameter/2)² / regional_m²` cells (~400 here).
Thresholds must be far below the land-cell count to ever fire: 25 / 80 gives
a few streams and 1–3 permanent rivers at this scale. The shipped 220 / 900
values were tuned for a grid ~150× larger and produced zero rivers.
`minimum_stream_length_m` is in meters.

**`erosion:`** — `stream_power_iterations` × `maximum_step_m` bounds total
fluvial carving (keep ≤ ~10 % of relief); `m`, `n` are the stream-power
exponents; `thermal_iterations` / `thermal_transfer_rate` control talus
smoothing.

**`coast:`** — shelf widths (m, horizontal, outside the coastline — must fit
the ring between coastline and chunk edge) and shelf depths (m below sea
level — budgeted in §4.2). `deep_slope_min/max` are gradients (rise/run) for
the falloff past the shelf.

**`beaches:`** — `maximum_slope_deg` gates which coastline segments get
beaches; widths in meters (keep ≥ ~4 × voxel size or beaches become
sub-voxel noise); berm heights in meters (proportional to relief — 0.4–1.0 m
on a 50 m island).

**`caves:`** — chamber counts (the generator picks in
`[chamber_count_min, chamber_count_max]`), passage radii (m),
`minimum_cover_m` (m of rock above any chamber), `maximum_depth_m` (m below
the placement baseline; keep ≤ ~half the relief so chambers stay inside the
edifice), `overhang_enabled` (adds a sea-facing mouth on the collapse
azimuth). Setting `chamber_count_max: 0` disables generated caves — safe on
current code (the fallback-slice injection this used to trigger is fixed).

**`resolution:`** (optional on island or world def) — `regional_m`,
`local_m`, `voxel_m` field spacings. Remember: changing `regional_m` changes
the hydrology grid and therefore the meaning of the cell-count thresholds.

## 7. Making biomes land where you want them

Biome rules (`biomes.*`) are absolute-threshold classifiers over elevation
(m), slope (degrees), moisture, and distances (m). The island's relief must
be authored *into* these bands or one rule eats the whole map.

**`world.island_testbed`** uses `biomes.expanded_slice` (relief ~50 m).
**`world.island_large`** uses `biomes.island_large` — the same rule set with
elevation and distance thresholds scaled by relief ratio 7.2 and diameter
ratio 12.2 (zero runtime cost; separate YAML files).

`biomes.expanded_slice` land bands:

| Rule | Elevation | Slope | Other |
|---|---|---|---|
| beach | −1 … 14 | — | ≤ 35 m from water |
| coastal_scrub | 2 … 16 | — | ≤ 80 m from water, moisture ≥ 0.30 |
| scrub | 4 … 14 | — | moisture ≥ 0.38 |
| forest | 4 … 18 | ≤ 25° | moisture ≥ 0.55 |
| rocky_upland | 12 … 27 | ≥ 18° | — |
| mountain_alpine | ≥ 28 | ≥ 12° | — |

The "alpine-only island" failure is now legible: the distorted volcano put
essentially all land above 28 m elevation at ≥ 12° slope, so
`mountain_alpine` matched everywhere. A 48 m-relief island with gentle lower
flanks distributes across all six bands. When retuning relief, re-derive:
what fraction of the surface sits in each elevation × slope cell?

## 8. Validation — what is enforced, and where

`terrain_generation::validate_island_world_budget(island, world,
water_sea_level)` returns human-readable messages (empty = coherent) and
encodes §4 and §5: footprint vs. chunk extents, atlas fit (rejecting
anything `fit_to_ocean_extent` would rescale, with the scale factors it
would have applied), relief + noise vs. ceiling, composed relief vs.
declaration, shelf vs. floor, and sea-level checks. It runs:

- at runtime, in `game_bevy` `build_density_source` /
  `build_density_source_from_prefs` (panics at startup with the message list);
- in diagnostics/tests, in `build_atlas_density_source` (panics);
- in the regression test `island_worlds_load_with_scale_appropriate_biomes`, which
  loads the shipped assets — so a YAML retune that breaks the budget fails CI
  before anyone sees a lumpy cone.

## 9. Retuning checklist

When changing any island or world number, walk this in order: (1) recompute
the chunk bounds if `chunks:` changed (§4, mind odd extents); (2) recompute
`support_radius` and check both horizontal constraints; (3) sum relief +
noise against the ceiling, shelf against the floor; (4) check the §4.3
proportion ratios; (5) if `regional_m` or the diameter changed, re-derive
land-cell count and re-set hydrology thresholds; (6) confirm island and
water `sea_level_m` agree; (7) run `cargo test` (budget regression test) and
an elevation diagnostic (`elev_diag` / `vs3_elevation_diag`) and confirm
peak, coastline radius, and land fraction match your derivation; (8) only
then look at it in-game.

## 10. Known sharp edges

`Default` for `IslandGenParams` mirrors `island_gen.island_testbed`; keep them
in sync when retuning or diagnostics drift from the shipped world. Inland lakes
use `hydrology_bodies` on the world def plus matching `waterbody.*` render
materials (`hydrology.upland_pool` → `waterbody.upland_pool`). Ocean render
colors come from `waterbody.sea` (aligned with `water.tropical_shallow`);
physics hydrology uses `water.sea` / `water.river.island` stable ids.
Setup UI schemas follow `setup.{world_suffix}` (`setup.testbed`,
`setup.large`). The atlas
blend width in `game_bevy/terrain/recipe.rs` reuses the demo river's
`bank_width_m` (3.5 m fallback) — a semantic borrow worth cleaning up
eventually. Landmarks and structures are positioned in recipe coordinates
against a *specific* terrain surface; re-derive their positions after any
relief change (landmarks for `world.island_testbed` are deferred until Stage 7
for exactly this reason). And `island_gen/mod.rs` (atlas pass ordering)
remains unaudited — if generation output contradicts this guide's model,
suspect that file first and get it reviewed.