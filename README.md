# RPG Adrift

A data-driven, procedurally generated open-world RPG built in Rust with [Bevy](https://bevyengine.org/). RPG Adrift treats world creation as **compilation**: YAML recipes describe islands, biomes, hydrology, and materials, which are validated and transformed into streamed voxel terrain, physics, and runtime simulation.

> **Status:** Active development — vertical slice / tech demo. Gameplay systems are evolving; the focus today is deterministic world generation, voxel terrain, and a playable exploration loop.

## Highlights

- **YAML-authored worlds** — Worlds, terrain, biomes, lighting, water, vegetation, and more are defined as validated YAML and wired together by stable IDs.
- **Island atlas pipeline** — Large-scale 2D fields (elevation, hydrology, erosion, coastlines, biomes) feed a signed-density voxel runtime for caves, cliffs, overhangs, and editing.
- **Streamed voxel terrain** — 1 m cells in 16×16×16 chunks, meshed with Surface Nets, with chunk residency and LOD policies.
- **Procedural materials** — Runtime procedural texture generation and triplanar terrain shading.
- **Third-person exploration** — Character controller (Avian3D), MMO-style orbit camera, water physics, day/night sky, weather, fog, and vegetation.
- **Deterministic generation** — Seeds and baked atlas archives produce reproducible worlds.

## Architecture

```text
YAML world recipe
  → validated typed configuration (game_data)
  → world-scale 2D fields + hydrology (terrain_generation)
  → signed-density voxel sampling (voxel_core)
  → chunk meshing (terrain_meshing)
  → surface classification & materials (terrain_surface, procedural_textures)
  → Bevy runtime (game_bevy)
```

Coordinate conventions: right-handed, **Y-up**, **meters**. See [`docs/coordinate-system.md`](docs/coordinate-system.md).

## Requirements

- **Rust** — stable toolchain (CI uses latest stable with `rustfmt` and `clippy`)
- **GPU** — Vulkan, DirectX 12, or Metal (Bevy 0.19 defaults)
- **OS** — Windows (primary CI target); Linux and macOS should build with Bevy’s supported backends

## Quick Start

Clone the repository and run from the project root (so the `assets/` directory is found):

```bash
git clone https://github.com/YOUR_ORG/RPGAdrift.git
cd RPGAdrift
cargo run --release
```

Development build (faster compile, lower runtime perf):

```bash
cargo run
```

### Assets path

By default the engine searches upward from the working directory and executable location for an `assets/` folder. Override explicitly:

```bash
# Windows (PowerShell)
$env:RPG_ADRIFT_ASSETS = "E:\path\to\RPGAdrift\assets"
cargo run --release

# Linux / macOS
RPG_ADRIFT_ASSETS=/path/to/RPGAdrift/assets cargo run --release
```

User preferences (world, seed, setup overrides) are stored in `user_data/user_prefs.yaml`.

## Controls

### Main menu

| Input | Action |
|-------|--------|
| **Enter** | Start game |
| **F10** | Toggle fly camera (debug) |

### Gameplay

| Input | Action |
|-------|--------|
| **W / A / S / D** | Move (camera-relative) |
| **Shift** | Sprint |
| **Space** | Jump |
| **Left mouse (hold)** | Orbit camera |
| **Right mouse (hold)** | Steer character facing |
| **Left + Right mouse** | Move forward (optional MMO-style) |
| **Mouse wheel** | Zoom |
| **Home** | Recenter camera behind character |
| **Escape / F11** | Options panel |

### Fly camera (F10)

| Input | Action |
|-------|--------|
| **W / A / S / D** | Move horizontally |
| **Space / Ctrl** | Ascend / descend |
| **Shift** | Fast movement |
| **Left mouse (hold)** | Look |

### Debug overlays

Configurable in `assets/config/debug.yaml` (defaults below):

| Key | Overlay |
|-----|---------|
| **F1** | Debug panel |
| **F2** | Chunk bounds |
| **F3** | Wireframe |
| **F4** | Biome visualization |
| **F5** | Material IDs |
| **F6** | Colliders |
| **F7** | Density field |
| **N** | Normals |
| **F8** | Regenerate terrain |
| **F9** | Next seed |
| **1 / 2 / 3** | Terrain edit: subtract / add / paint |

## Project Structure

```text
RPGAdrift/
├── assets/              # YAML configs, shaders, baked atlases, procedural recipes
├── crates/
│   ├── game_bevy/       # Bevy app: rendering, player, camera, UI, environment
│   ├── game_data/       # YAML loading, validation, compiled config registry
│   ├── terrain_generation/  # Island atlas, hydrology, density ops (no Bevy)
│   ├── terrain_meshing/     # Surface Nets chunk meshing
│   ├── terrain_surface/     # Surface classification & biome blending
│   ├── terrain_material_bevy/  # Bevy terrain material plugin
│   ├── procedural_textures/    # Procedural texture graph & baking
│   ├── terrain_tools/       # CLI: bake-atlas, bake-materials, validate-catalog
│   ├── voxel_core/          # Chunks, density sampling, edits
│   ├── physics_bridge/      # Character controller bridge (Avian3D)
│   └── shared/              # IDs, math, errors
├── docs/                # Authoring guides and design notes
├── src/main.rs          # Binary entry point
└── user_data/           # Local user preferences (gitignored patterns may apply)
```

## World Authoring

Worlds are composed by referencing definition IDs in YAML — not file paths. The loader walks all `*.yaml` files under `assets/` and builds a registry.

Example app config (`assets/config/app.yaml`):

```yaml
schema_version: 1
id: app.default

world: world.small
player: player.default
camera: camera.mmo_default
performance: performance.default
```

Two terrain modes exist:

- **Worldgen worlds** — `world.small`, `world.medium`, and `world.large` via the Milestone A compiler (recommended).
- **Legacy atlas worlds** — Procedural islands via `island_gen.*` (deprecated for new work).
- **Op-based worlds** — Hand-authored terrain operation lists for bespoke layouts.

See [docs/world_tiers.md](docs/world_tiers.md) for the scale ladder.

See the authoring guides:

- [`docs/terrain_yaml_authoring.md`](docs/terrain_yaml_authoring.md) — Parameters, validation, and world scale rules
- [`docs/terrain_feature_catalog.md`](docs/terrain_feature_catalog.md) — Feature definitions
- [`docs/ProceduralTextures.md`](docs/ProceduralTextures.md) — Material and texture recipes
- [`PhasedExpansionPlan.md`](PhasedExpansionPlan.md) — Long-term generator roadmap

## CLI Tools

The `terrain_tools` crate provides offline utilities:

```bash
# Bake a procedural island atlas for faster loads
cargo run -p terrain_tools --bin bake-atlas -- --world world.small

# Bake procedural terrain materials
cargo run -p terrain_tools --bin bake-materials -- --help

# Validate material catalog definitions
cargo run -p terrain_tools --bin validate-catalog -- --help
```

Baked atlases are written under `assets/terrain/baked/` by default.

## Development

```bash
# Format
cargo fmt --all

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Test
cargo test --workspace

# Release build
cargo build --release
```

Enable Bevy asset hot-reload during development:

```bash
cargo run -p game_bevy --features dev_hot_reload
```

Logging uses `tracing`. Filter with `RUST_LOG`:

```bash
RUST_LOG=info cargo run
```

CI (`.github/workflows/ci.yml`) runs on **Windows** and checks format, clippy, tests, and release build.

## Documentation

| Document | Description |
|----------|-------------|
| [`docs/coordinate-system.md`](docs/coordinate-system.md) | Axes, voxel scale, recipe vs world space |
| [`docs/terrain_yaml_authoring.md`](docs/terrain_yaml_authoring.md) | How to author and validate worlds |
| [`docs/CameraImplementation.md`](docs/CameraImplementation.md) | Camera rig and input design |
| [`PhasedExpansionPlan.md`](PhasedExpansionPlan.md) | Generator expansion phases |

## Tech Stack

- **[Bevy 0.19](https://bevyengine.org/)** — ECS game engine
- **[Avian3D](https://github.com/Jondolf/avian)** — Physics
- **[bevy_egui](https://github.com/mvlabat/bevy_egui)** — Debug and options UI
- **Serde / YAML** — Data definitions
- **Rayon, blake3** — Parallelism and hashing

## Contributing

This project is in early development. Before opening a PR:

1. Run `cargo fmt`, `cargo clippy`, and `cargo test --workspace`.
2. Keep YAML defs at true world scale — validation rejects contradictory geometry.
3. Prefer extending existing crates and conventions over parallel implementations.

## License

No license file is included yet. All rights reserved until a license is added.
