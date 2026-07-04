// crates/terrain_generation/tests/route_validation.rs
//
// Validates the island_testbed atlas world: generated features exist (river,
// caves), the atlas passes its own design validation, terrain invariants hold
// (seams, foundations, no void shafts / floating voxels), and authored routes
// are traversable.
//
// History: this file previously probed hardcoded landmark coordinates of the
// op-based world.vertical_slice / world.expanded_slice terrains (ridge, fort
// pad, authored cave mouth, IslandMask/trench ops). Those worlds and their
// authored geometry were removed in the two-world condensation; the
// world-agnostic invariants were kept and retargeted here, and the landmark
// probes were replaced by feature-existence checks against the generated
// island.

use game_data::load_registry_from_directory;
use std::path::PathBuf;
use terrain_generation::{
    CombineOp, FOUNDATION_DEPTH_M, PLAYER_SPAWN_MIN_CLEARANCE_M, RecipeDensitySource, RecipeOp,
    SPAWN_FLOOR_EPSILON_M, build_atlas_density_source, outside_declared_cavities,
};
use voxel_core::CHUNK_CELLS;

const TESTBED_WORLD: &str = "world.island_testbed";
const LARGE_WORLD: &str = "world.island_large";

fn workspace_assets() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .canonicalize()
        .expect("assets")
}

/// Atlas density source for the testbed world at its authored seed (48129),
/// so feature assertions and route waypoints refer to a deterministic island.
fn testbed_source() -> RecipeDensitySource {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let world = registry
        .world_by_id(&shared::StableId::new(TESTBED_WORLD))
        .expect("testbed world");
    build_atlas_density_source(
        &registry,
        &shared::StableId::new(TESTBED_WORLD),
        world.seed,
        None,
        None,
    )
}

fn recipe_xz(source: &RecipeDensitySource, rx: f32, rz: f32) -> (f32, f32) {
    (
        rx - source.recipe().coord_offset[0],
        rz - source.recipe().coord_offset[2],
    )
}

fn assert_route_traversable(source: &RecipeDensitySource, waypoints: &[[f32; 2]]) {
    for window in waypoints.windows(2) {
        let x0 = window[0][0];
        let z0 = window[0][1];
        let x1 = window[1][0];
        let z1 = window[1][1];
        let steps = 16;
        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let x = x0 + (x1 - x0) * t;
            let z = z0 + (z1 - z0) * t;
            let surface = source.surface_height_at_recipe(x, z);
            let headroom = source.density_at_recipe(x, surface + 2.0, z);
            assert!(
                headroom > 0.0,
                "route blocked at ({x},{z}): surface={surface} headroom density={headroom}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Generated feature existence (the design-intent checks)
// ---------------------------------------------------------------------------

/// The atlas's own validation must pass. On failure this prints the design
/// messages (e.g. "Primary river missing") instead of leaving them as
/// easily-missed runtime warnings.
#[test]
fn testbed_atlas_passes_design_validation() {
    let source = testbed_source();
    let atlas = source.atlas().expect("atlas");
    assert!(
        atlas.validation_passed,
        "island atlas design validation failed:\n{}",
        atlas.validation_messages.join("\n")
    );
}

#[test]
fn testbed_has_primary_river() {
    let source = testbed_source();
    let atlas = source.atlas().expect("atlas");
    assert!(
        atlas.river_graph.is_some(),
        "no primary river spline; hydrology thresholds too high for this island scale? messages:\n{}",
        atlas.validation_messages.join("\n")
    );

    let mut river_samples = 0usize;
    for x in (-100..=100).step_by(2) {
        for z in (-100..=100).step_by(2) {
            if atlas.river_mask.sample_bilinear(x as f32, z as f32) > 0.5 {
                river_samples += 1;
            }
        }
    }
    assert!(
        river_samples >= 5,
        "river mask nearly empty ({river_samples} samples > 0.5); river not carved into fields"
    );
}

/// Cave chambers/passages are appended to the compiled recipe as ops. The
/// testbed terrain def authors no ops of its own, so every ellipsoid/capsule
/// op in the recipe is a generated cave element.
#[test]
fn testbed_recipe_includes_generated_caves() {
    let source = testbed_source();
    let cave_ops = source
        .recipe()
        .ops
        .iter()
        .filter(|op| matches!(op, RecipeOp::Ellipsoid { .. } | RecipeOp::Capsule { .. }))
        .count();
    assert!(
        cave_ops >= 4,
        "expected >=4 generated cave ops (chamber_count_min), got {cave_ops}"
    );
}

/// Somewhere inside a generated cave chamber there must be air beneath the
/// terrain surface — otherwise the caves are sealed and undiscoverable.
#[test]
fn testbed_island_has_subsurface_cave_air() {
    let source = testbed_source();
    let mut found = false;
    for op in &source.recipe().ops {
        let RecipeOp::Ellipsoid {
            center,
            combine: CombineOp::Subtract,
            ..
        } = op
        else {
            continue;
        };
        if source.density_at_recipe(center[0], center[1], center[2]) > 0.0 {
            found = true;
            break;
        }
        let (wx, wz) = recipe_xz(&source, center[0], center[2]);
        let surface = source.terrain_surface_height_at(wx, wz);
        for depth in [2.0f32, 4.0, 6.0, 8.0, 10.0, 14.0] {
            if source.density_at(wx, surface - depth, wz) > 0.0 {
                found = true;
                break;
            }
        }
        if found {
            break;
        }
    }
    assert!(
        found,
        "no sub-surface air found in generated cave chambers; caves sealed or not carved"
    );
}

#[test]
fn testbed_peak_exceeds_35m() {
    let source = testbed_source();
    let mut peak = f32::MIN;
    for rx in (236..=276).step_by(2) {
        for rz in (236..=276).step_by(2) {
            let (wx, wz) = recipe_xz(&source, rx as f32, rz as f32);
            peak = peak.max(source.terrain_surface_height_at(wx, wz));
        }
    }
    assert!(
        peak > 35.0,
        "volcano summit region should exceed 35 m (composed relief ~48 m), got {peak}"
    );
}

#[test]
fn testbed_offshore_is_submerged() {
    let source = testbed_source();
    let sea = source.recipe().sea_level;
    // Both probes lie beyond the island's footprint support radius (~112 m).
    for (rx, rz) in [(240.0f32, 60.0f32), (16.0, 16.0)] {
        let (wx, wz) = recipe_xz(&source, rx, rz);
        let surface = source.terrain_surface_height_at(wx, wz);
        assert!(
            surface < sea,
            "offshore at recipe ({rx},{rz}) should be below sea level {sea}, got {surface}"
        );
    }
}

// ---------------------------------------------------------------------------
// Terrain invariants (retargeted from the op-based worlds)
// ---------------------------------------------------------------------------

/// Outdoor columns outside generated cavities must have bedrock within the
/// foundation depth below the surface.
#[test]
fn outdoor_columns_have_foundation_bedrock() {
    let source = testbed_source();
    let recipe = source.recipe();
    let sea = recipe.sea_level;
    let mut violations = 0usize;

    for wx in (-100..=100).step_by(4) {
        for wz in (-100..=100).step_by(4) {
            let xf = wx as f32;
            let zf = wz as f32;
            let surface = source.foundation_surface_at(xf, zf);
            if surface < sea + 1.0 {
                continue;
            }
            let bedrock_y = surface - FOUNDATION_DEPTH_M;
            let rx = xf + recipe.coord_offset[0];
            let rz = zf + recipe.coord_offset[2];
            if !outside_declared_cavities(recipe, rx, bedrock_y, rz) {
                continue;
            }
            let density = source.density_at(xf, bedrock_y, zf);
            if density > 0.0 {
                violations += 1;
                if violations <= 5 {
                    eprintln!("missing bedrock at ({xf},{zf}) y={bedrock_y:.1}");
                }
            }
        }
    }

    assert_eq!(
        violations, 0,
        "found {violations} outdoor columns missing foundation bedrock"
    );
}

/// No shallow outdoor void shafts (fully-air columns in the y=0..6 band)
/// on island land outside generated cavities.
#[test]
fn no_shallow_outdoor_void_shafts_on_island() {
    let source = testbed_source();
    let recipe = source.recipe();
    let sea = recipe.sea_level;
    let mut violations = 0usize;

    for wx in (-100..=100).step_by(2) {
        for wz in (-100..=100).step_by(2) {
            let xf = wx as f32;
            let zf = wz as f32;
            let surface = source.terrain_surface_height_at(xf, zf);
            if surface < sea + 1.0 {
                continue;
            }
            let rx = xf + recipe.coord_offset[0];
            let rz = zf + recipe.coord_offset[2];
            if !outside_declared_cavities(recipe, rx, 3.0, rz) {
                continue;
            }
            if source.column_is_void(xf, zf, 0.0, 6.0) {
                violations += 1;
                if violations <= 5 {
                    eprintln!("outdoor void shaft at ({xf},{zf}) from y=0..6");
                }
            }
        }
    }

    assert_eq!(
        violations, 0,
        "found {violations} shallow outdoor void shafts on island land"
    );
}

#[test]
fn no_floating_voxel_islands_in_probe_grid() {
    let source = testbed_source();
    let mut floaters = 0usize;
    for x in (-60..=60).step_by(4) {
        for z in (-60..=60).step_by(4) {
            for y in 1..55 {
                let yf = y as f32;
                let below = source.density_at(x as f32, yf - 1.0, z as f32);
                let here = source.density_at(x as f32, yf, z as f32);
                let above = source.density_at(x as f32, yf + 1.0, z as f32);
                if here <= 0.0 && below > 0.0 && above > 0.0 {
                    floaters += 1;
                }
            }
        }
    }
    assert!(
        floaters < 15,
        "expected almost no floating solid voxels, found {floaters}"
    );
}

#[test]
fn chunk_border_sample_indices_are_consistent() {
    let source = testbed_source();
    let coord = voxel_core::ChunkCoord::new(1, 1, 0);
    let samples =
        terrain_generation::generate_padded_samples(&source, coord, voxel_core::MaterialId(0));
    let cells = CHUNK_CELLS as usize;
    let stride = cells + 3;
    let idx = |x: i32, y: i32, z: i32| -> usize {
        (z + 1) as usize * stride * stride + (y + 1) as usize * stride + (x + 1) as usize
    };
    let boundary = idx(cells as i32, 8, 8);
    assert!(samples[boundary].density.is_finite());
}

/// Shared chunk faces must sample identical density values (mesh seam
/// prerequisite) — now exercised against the atlas sampling path.
#[test]
fn chunk_face_density_is_continuous() {
    let source = testbed_source();
    let cells = CHUNK_CELLS as i32;
    let pairs = [
        (
            voxel_core::ChunkCoord::new(0, 1, 0),
            voxel_core::ChunkCoord::new(1, 1, 0),
            0i32,
        ),
        (
            voxel_core::ChunkCoord::new(1, 0, 0),
            voxel_core::ChunkCoord::new(1, 1, 0),
            1i32,
        ),
        (
            voxel_core::ChunkCoord::new(1, 1, 0),
            voxel_core::ChunkCoord::new(1, 1, 1),
            2i32,
        ),
    ];

    for (a, b, axis) in pairs {
        let (a_origin, b_origin) = (
            voxel_core::TerrainChunk::new(a).sample_origin(),
            voxel_core::TerrainChunk::new(b).sample_origin(),
        );
        for u in 0..=cells {
            for v in 0..=cells {
                for y in 0..=cells {
                    let (ax, ay, az, bx, by, bz) = match axis {
                        0 => (
                            a_origin.0 + cells,
                            a_origin.1 + y,
                            a_origin.2 + u,
                            b_origin.0,
                            b_origin.1 + y,
                            b_origin.2 + u,
                        ),
                        1 => (
                            a_origin.0 + u,
                            a_origin.1 + cells,
                            a_origin.2 + v,
                            b_origin.0 + u,
                            b_origin.1,
                            b_origin.2 + v,
                        ),
                        _ => (
                            a_origin.0 + u,
                            a_origin.1 + y,
                            a_origin.2 + cells,
                            b_origin.0 + u,
                            b_origin.1 + y,
                            b_origin.2,
                        ),
                    };
                    let da = source.density_at(ax as f32, ay as f32, az as f32);
                    let db = source.density_at(bx as f32, by as f32, bz as f32);
                    assert!(
                        (da - db).abs() < 0.0001,
                        "density seam mismatch on axis {axis} at ({ax},{ay},{az}) vs ({bx},{by},{bz}): {da} != {db}"
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Spawn and routes
// ---------------------------------------------------------------------------

#[test]
fn testbed_spawn_resolves_on_natural_terrain() {
    let source = testbed_source();
    let wx = source.recipe().spawn_x - source.recipe().coord_offset[0];
    let wz = source.recipe().spawn_z - source.recipe().coord_offset[2];
    let terrain = source.terrain_surface_height_at(wx, wz);
    let composite = source.surface_height_at(wx, wz);

    let (_x, foot_y, _z, report) = source.resolve_player_spawn(PLAYER_SPAWN_MIN_CLEARANCE_M, 48.0);
    assert!(
        report.passed,
        "spawn validation failed: {:?}",
        report.messages
    );
    assert!(
        foot_y <= terrain + SPAWN_FLOOR_EPSILON_M + 1.5,
        "foot y={foot_y} should sit on terrain y={terrain}, not composite y={composite}"
    );
    assert!(
        composite >= terrain,
        "composite surface should not be below terrain"
    );
}

#[test]
fn testbed_routes_from_yaml_are_traversable() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let world = registry
        .world_by_id(&shared::StableId::new(TESTBED_WORLD))
        .expect("testbed world");
    let routes = registry
        .routes_for_world(world)
        .expect("routes.island_testbed should be authored for the testbed world");
    assert!(!routes.routes.is_empty(), "routes def contains no routes");
    let source = testbed_source();
    for route in &routes.routes {
        assert_route_traversable(&source, &route.waypoints);
    }
}

fn large_source() -> RecipeDensitySource {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let world = registry
        .world_by_id(&shared::StableId::new(LARGE_WORLD))
        .expect("large world");
    build_atlas_density_source(
        &registry,
        &shared::StableId::new(LARGE_WORLD),
        world.seed,
        None,
        None,
    )
}

#[test]
fn large_routes_from_yaml_are_traversable() {
    let registry = load_registry_from_directory(workspace_assets()).expect("registry");
    let world = registry
        .world_by_id(&shared::StableId::new(LARGE_WORLD))
        .expect("large world");
    let routes = registry
        .routes_for_world(world)
        .expect("routes.island_large should be authored for the large world");
    assert!(!routes.routes.is_empty(), "routes def contains no routes");
    let source = large_source();
    for route in &routes.routes {
        assert_route_traversable(&source, &route.waypoints);
    }
}
