# Large world test guide

**Presentation ID:** `world.large`  
**Worldgen ID:** `world.large`  
**Extent:** 23104×23104 m (8× area vs small)

## Purpose

Largest single-island tier (~23 km) before archipelago/streaming milestone (Milestone G).
Primary performance and integration testbed at maximum scale.

## Terrain / worldgen features

- Full scaled volcanic island at √8 linear factor (`island.volcanic_large`)
- ≥3 cave systems; multi-watershed coherence at max scale
- Long-range climate and coast gradients

## Engine / runtime features

- Maximum chunk volume (~1444³ chunks on X/Z/Y from compiled extent)
- Horizon skirt and distant impostor validation (`impostor_start_m: 1200`)
- Reduced residency radii in presentation YAML for initial perf headroom
- Future hooks: landmark grid, structure placement, vegetation density scaling

## Acceptance checklist

- [ ] `validation.large` passes
- [ ] Compile completes (document expected time on your machine)
- [ ] Player can traverse beach → summit → cave without holes
- [ ] Chunk residency keeps up at sprint speed from center to coast
- [ ] Record compile time and peak RSS for regression tracking

## Implementation

- Worldgen: [`assets/worldgen/worlds/large.world.yaml`](../../assets/worldgen/worlds/large.world.yaml)
- Presentation: [`assets/terrain/worlds/large_world.yaml`](../../assets/terrain/worlds/large_world.yaml)
- Tests: `milestone_d` (`large_world_compiles_with_validation`, `#[ignore]` by default)

Manual compile benchmark:

```bash
cargo test -p terrain_generation large_world_compiles_with_validation -- --ignored --nocapture
```

## Performance knobs

If frame time or compile time is excessive, adjust in [`large_world.yaml`](../../assets/terrain/worlds/large_world.yaml):

- `residency.density_radius`, `render_radius`, `physics_radius`
- `lod.terrain` distance bands
- Run with a smaller render radius in performance settings
