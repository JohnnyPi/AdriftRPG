# World tier scale ladder

Four worldgen recipes share one tropical-volcanic recipe stack. Only extent,
resolutions, island geometry, hydrology thresholds, and validation differ by tier.

## Scale ladder

| Tier | ID | Extent (m) | Area vs small | Linear scale | Use |
|------|-----|------------|---------------|--------------|-----|
| Smoke | `world.smoke` | 512×512 | ~0.004× | 0.0625× | Fast CI / compiler smoke |
| Small | `world.small` | 8192×8192 | 1× | 1× | Primary dev and play |
| Medium | `world.medium` | 16384×16384 | 4× | 2× | Multi-system scale stress |
| Large | `world.large` | 23104×23104 | 8× | √8 ≈ 2.828× | Performance / Milestone G prep |

Large extent: `23104 = 8192 × √8` (exactly 8× area vs small).

## Resolution formula

All tiers use a **16×16 control grid** (`width_m / 16`):

```yaml
resolutions:
  control_cell_m:  {extent_m / 16}
  regional_cell_m: {control_cell_m / 4}
  local_cell_m:    max(1, regional_cell_m / 16)
```

| Tier | control | regional | local |
|------|---------|----------|-------|
| Smoke | 32 | 8 | 1 |
| Small | 512 | 128 | 8 |
| Medium | 1024 | 256 | 16 |
| Large | 1444 | 361 | 16 |

## Shared recipes

Every tier references the same sub-recipes:

| Field | ID |
|-------|-----|
| boundary | `boundary.bounded_ocean` |
| geology | `geology.basaltic_volcanic` |
| refinement | `refinement.tropical_volcanic` |
| climate | `climate.tropical_trade_winds` |
| erosion | `erosion.tropical_volcanic` |
| coast | `coast.tropical_volcanic` |
| biomes | `biomes.tropical_land` |
| strata | `strata.tropical_volcanic` |
| caves | `caves.tropical_volcanic` (default) |

## Island geometry

`island.volcanic_small` is the 1× template. Medium and large scale all distance
parameters by 2× and √8 respectively; placement type and proportions stay fixed.

## Hydrology and validation

Per-tier hydrology and validation YAMLs live under `assets/worldgen/hydrology/` and
`assets/worldgen/validation/`. See [docs/worlds/](worlds/) for acceptance checklists.

## Presentation worlds

Player-facing profiles in `assets/terrain/worlds/` set `worldgen: world.{tier}` and
share the tropical island material catalog and expanded-slice surface stack.
