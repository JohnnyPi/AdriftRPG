# Medium world test guide

**Presentation ID:** `world.medium`  
**Worldgen ID:** `world.medium`  
**Extent:** 16384×16384 m (4× area vs small)

## Purpose

16 km world for scale and multi-system stress testing (Milestones C–D). Confirms that
scaled recipes produce richer networks without breaking coherence.

## Terrain / worldgen features

- 2× island geometry → longer watersheds, higher stream order
- ≥2 cave systems
- Erosion at regional scale; sediment across larger drainages
- Extended reef/lagoon/coast bands

## Engine / runtime features

- Chunk streaming at 1024×1024 chunk extent (4× chunk count vs small on X/Z)
- LOD falloff at greater view distances (`impostor_start_m: 800`)
- Default residency radii — validates prefetch under higher chunk demand
- Worldgen atlas memory footprint baseline

## Acceptance checklist

- [ ] `validation.medium` passes after compile
- [ ] ≥2 cave systems, ≥1 traversable
- [ ] Multiple permanent rivers; disconnected river fraction ≤5%
- [ ] Stable frame time with default render radius (record baseline)
- [ ] No seam artifacts at regional refinement boundaries

## Implementation

- Worldgen: [`assets/worldgen/worlds/medium.world.yaml`](../../assets/worldgen/worlds/medium.world.yaml)
- Presentation: [`assets/terrain/worlds/medium_world.yaml`](../../assets/terrain/worlds/medium_world.yaml)
- Tests: `milestone_d` (`medium_produces_multiple_cave_systems`)

Compile locally:

```bash
cargo test -p terrain_generation medium_produces_multiple_cave_systems
```
