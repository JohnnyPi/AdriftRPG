# Smoke world test guide

**Worldgen ID:** `world.smoke`  
**Extent:** 512×512 m  
**Audience:** CI and compiler developers only (not in the player world picker)

## Purpose

Prove all 18 compiler passes run quickly and deterministically on a minimal tropical
volcanic island.

## Engine systems tested

- Recipe resolve and `recipe_content_hash` stability
- Full pass pipeline (`boundary` → `caves` → `water_carve`)
- `AtlasWorldProvider` and `VolumetricWorldProvider` sampling
- Cave graph presence, hydrology graph, carved elevation field

## Terrain features

- Single centered mini shield volcano (`island.volcanic_smoke`)
- Primary river, ≥1 traversable cave system
- Reduced hydrology thresholds (`hydrology.smoke`)

## Acceptance checklist

- [ ] `cargo test -p terrain_generation` smoke-tier tests compile in under 10 s
- [ ] `pass_reports.len() == 18`
- [ ] ≥1 traversable cave system; primary river exists
- [ ] Deterministic elevation field across two compiles

## Implementation

- Worldgen: [`assets/worldgen/worlds/smoke.world.yaml`](../../assets/worldgen/worlds/smoke.world.yaml)
- Tests: `milestone_a`, `milestone_b`, `milestone_c`, `milestone_d`, `dual_pipeline`

Run large-tier compile benchmarks separately:

```bash
cargo test -p terrain_generation large_world_compiles_with_validation -- --ignored
```
