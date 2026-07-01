# Coordinate System

Single source of truth for world-space conventions in RPGAdrift. See also [VerticalSlice.md](../VerticalSlice.md) §4 (chunk indexing) and [PhasedExpansionPlan.md](../PhasedExpansionPlan.md) Phase 0.

## Axes and handedness

RPGAdrift uses **Bevy defaults**: right-handed, **Y-up**, units in **meters**.

| Axis | Role |
|------|------|
| **+Y** | Up — gravity, height, density sign, camera `look_at` up-vector |
| **+X / +Z** | Horizontal plane — biomes, rivers, movement |
| **−Z** | Gameplay and camera forward at yaw = 0 |

Gravity is applied on **−Y**. Planar movement uses **X and Z** only.

## Voxel scale constraint

**`cell_size_m` must be exactly 1.0.** One voxel cell equals one world meter. Sub-meter voxels require a separate indexing design; until then, world YAML validation rejects other values.

Constants in `voxel_core`:

- `CHUNK_CELLS = 16` — cells per chunk axis
- `CHUNK_SAMPLES = 17` — density sample corners per chunk axis (shared at chunk boundaries)

## Coordinate spaces

### World space (runtime)

Bevy `Transform.translation`, physics, player position, mesh chunk placement, and most runtime APIs.

### Recipe space (authored)

YAML terrain operations, spawn points, and noise origins often use large positive coordinates (e.g. island center at `[128, 128]`).

**Transforms:**

```text
recipe = world + coord_offset
world  = recipe − coord_offset
```

| API | Expected space |
|-----|----------------|
| `CompiledWorld::recipe_to_world` | input: recipe → output: world |
| `RecipeDensitySource::density_at(x, y, z)` | world meters |
| `RecipeDensitySource::density_at_recipe(x, y, z)` | recipe |
| `land_surface_height(recipe, x, z)` | recipe XZ |
| `Field2D::world_to_grid(wx, wz)` | world XZ; `origin[0]` = X, `origin[1]` = Z |

### Cell index space

Integer grid aligned with world meters when `cell_size_m = 1.0`:

| Type | Meaning |
|------|---------|
| `ChunkCoord` | Chunk index (may be negative) |
| `WorldCell` | Floor of world position in meters / cell index |
| `WorldSample` | Density sample corner index |
| `LocalCell` | In-chunk cell, 0..15 |
| `LocalSample` | In-chunk sample, 0..16 |

**Chunk origin (meters):** `chunk_coord × 16`

**World cell → chunk:** `floor_div(cell_axis, 16)` (handles negatives)

**World sample → chunk:** `floor_div(sample_axis, 15)` — samples overlap at boundaries (local 16 of chunk N equals local 0 of chunk N+1).

Adjacent chunks share world-space sample positions at boundaries (see VerticalSlice §4.2).

### Chunk-local space (mesh)

Surface Nets output is in local cell units `[0..16]` per axis. Bevy mesh vertices are `local × cell_size_m`; chunk entity transform is `chunk_coord × (16 × cell_size_m)`.

## Density convention

Signed density along **Y**:

```text
density = y − surface_height
```

- `density ≤ 0` → solid
- `density > 0` → air

Ground plane helper: `plane_density(y, height) = y − height`.

## Horizontal 2D conventions

**`position_xz` is always `[X, Z]`, never `[X, Y]`.** Used for rivers, water splines, and debug overlays.

Movement input uses `Vec2(strafe, forward)` mapped to world `(X, Z)`.

## Camera and movement

- Camera sits **behind** the focus along rotated **+Z** at yaw = 0; view direction is **−Z** at yaw = 0.
- `camera_forward_xz(yaw) = (−sin(yaw), 0, −cos(yaw))`
- Movement and terrain edits use **`intent_yaw()`** (character heading + orbit offset), not smoothed render yaw.
- View direction from yaw/pitch: negate the camera-backward vector (see `camera_view_direction` in `game_bevy`).

## Rendering notes

- Terrain `UV_0` / `UV_1` carry **material IDs**, not texture coordinates.
- Planned triplanar texturing ([ProceduralTextures.md](../ProceduralTextures.md)) samples in **true world meters** using vertex world position from the model matrix.
- Shaders must use `mesh_functions::mesh_position_local_to_world`, not raw local `vertex.position`, for world-space effects.

## Quick reference: which space am I in?

| Input | Space |
|-------|-------|
| YAML spawn / terrain op center | Recipe |
| `density_at(x, y, z)` | World meters |
| `WorldCell` / `WorldSample` indices | Cell/sample grid (= meters at 1 m/cell) |
| `ChunkCoord` | Chunk index |
| `position_xz[0], [1]` | World X, Z |
| Island atlas `Field2D` origin | World X, Z |
