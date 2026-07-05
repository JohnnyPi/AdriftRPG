# Terrain Feature Catalog

Bridge between **authored op-based** terrain (`terrain.*` YAML) and **procedural island_gen**
modules. Use `world.small` as the reference layout when tuning scaled tiers.
See [world_tiers.md](world_tiers.md) and [worlds/small.md](worlds/small.md).

## Island footprint

| Authored op | Key params | island_gen module | Validation | Visual acceptance |
|-------------|------------|-------------------|------------|-------------------|
| `island_mask` | `center`, `radius_m`, `falloff_m`, `domain_warp` | `footprint.rs` lobes + warp | Support radius ≤ world half-extent | Smooth coastline ring; no clip at chunk edge |
| `coastal_surface` | `origin`, `scale`, `ridge_*`, noise freqs | `volcano.rs` + `surface_noise` | Peak + noise ≤ vertical budget | Gentle shield slopes, readable ridges |

## Relief

| Authored op | Key params | island_gen module | Validation | Visual acceptance |
|-------------|------------|-------------------|------------|-------------------|
| `mountain_peak` | `base_radius_m`, `peak_height_m`, `steepness` | `volcano.rs` shield/summit stack | Composed height ≤ `maximum_height_m` | Central cone visible from beach |
| `noise_perturb` | `scale`, `amplitude` | `surface_noise` voxel tier | — | Micro-variation without noise-dominated silhouette |

## Hydrology

| Authored op / body | Key params | island_gen module | Validation | Visual acceptance |
|--------------------|------------|-------------------|------------|-------------------|
| `valley_basin` + `river_carve` | polyline `origin`/`scale`, spline source | `hydrology.rs` + `carving.rs` | Monotonic `water_elevation`; mouth at sea | Ribbon flush with carved channel |
| `hydrology.*` lake | `center` (recipe), `elevation_m`, `radius_m` | Authored only (no procedural lakes) | Recipe→world coord conversion | Disc centered in depression |
| `underwater_trench` | `points`, `width_m` | `bathymetry.rs` deep features | Below sea level | Visible shelf channel offshore |

## Coast

| Authored op | Key params | island_gen module | Validation | Visual acceptance |
|-------------|------------|-------------------|------------|-------------------|
| `coast_modifier` (cove) | `center`, `radius_m`, `depth_m` | `beaches` + coast distance | — | Sandy cove, berm rises inland |
| `ocean_floor` | `base_depth_m`, `variation_m` | `bathymetry.rs` | Shelf fits inside world edge | Seabed visible under shallow water |

## Cavities

| Authored op | Key params | island_gen module | Validation | Visual acceptance |
|-------------|------------|-------------------|------------|-------------------|
| `includes: caves.*` | ellipsoid/capsule subtract ops | `caves` block in island_gen | Chambers inside relief | Walk-in entrance on collapse flank |
| `ellipsoid` subtract | `center`, `radii` | Lake basins / caldera | — | Upland pool sits in bowl |

## Runtime systems

| Concern | Authored path | Procedural path |
|---------|---------------|-----------------|
| Density | `compile_terrain_recipe` + optional `with_river_carve` | `build_island_atlas` + `with_atlas` |
| Water render | `generate_river_spline` ribbon + lake discs | Atlas `river_graph` + lake discs |
| Ocean | Camera-snapped 3×3 tile grid (`water/mod.rs`) | Same |
| Atlas build | N/A | Runtime async when `island_atlas_baked` absent |

## World scale reference

| World | Mode | Horizontal span | Island size |
|-------|------|-----------------|-------------|
| `world.small` | Worldgen compiler | 8192 m | ~5600 m |
| `world.medium` | Worldgen compiler | 16384 m | ~11200 m |
| `world.large` | Worldgen compiler | 23104 m | ~15800 m |
| `world.smoke` | Worldgen compiler (CI) | 512 m | ~190 m |
