// crates/game_bevy/src/terrain/pipeline.rs
//! Terrain chunk pipeline: density → mesh → upload → collider.
//!
//! Chunk records are SPARSE: only chunks near the interest center (or with a
//! live entity / in-flight job) are materialized in `TerrainPipelineState::
//! chunks`. Records are created on demand when a chunk enters the density
//! radius and pruned once they fall back to `Unrequested` with no entity
//! outside it. Scheduling iterates the neighborhood cube around the interest
//! center, so per-frame cost is O(density_radius³) and independent of world
//! size — required for the large island world (~1.35M potential chunks),
//! where the previous eager `Vec` + full linear scans were O(world).
//!
//! Tradeoff: empty (fully air/solid) chunks that leave the density radius are
//! pruned and re-run their density job on re-entry, matching how rendered
//! chunks already despawned and re-meshed on re-entry.

use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use std::collections::HashMap;
use std::time::Instant;
use terrain_generation::{fill_padded_samples, DensitySource, RecipeDensitySource};
use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMeshData, TerrainMesher};
use tracing::{info, warn};
use voxel_core::{CHUNK_CELLS, ChunkCoord, WorldCell};

use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;
use crate::environment::biome_context::ChunkColumnCache;
#[cfg(test)]
use crate::environment::materials::material_for_world_with_cache;
use crate::environment::surface::ChunkSurfaceResolver;
use crate::environment::BiomeInitSet;
#[cfg(test)]
use crate::environment::biomes::BiomeCatalog;
use crate::state::AppState;
use crate::terrain::material::TerrainMaterialHandle;
use crate::terrain::mesh_convert::{chunk_world_transform, mesh_from_terrain_data};
use crate::terrain::metrics::{TerrainPipelineMetrics, WorldSeedOverride};
use crate::terrain::recipe::{build_density_source_from_prefs, terrain_recipe_hash};
use crate::terrain::residency::{
    chunk_chebyshev_distance, TerrainWorldRuntime, within_density_radius, within_physics_radius,
    within_render_radius,
};
use crate::terrain::{ChunkState, TerrainChunkEntity, TerrainChunkMaterial, TerrainEditStore, TerrainRevision};
use crate::ui::{TerrainTweaks, WorldTweaks};
use physics_bridge::terrain_layers;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerrainWorldInitSet;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainRevision>()
            .init_resource::<TerrainEditStore>()
            .init_resource::<TerrainPipelineState>()
            .init_resource::<TerrainSpawnPoint>()
            .init_resource::<TerrainRecipeRevision>()
            .init_resource::<TerrainRegenPending>()
            .init_resource::<TerrainPipelineMetrics>()
            .init_resource::<WorldSeedOverride>()
            .configure_sets(OnEnter(AppState::Running), TerrainWorldInitSet)
            .add_systems(
                OnEnter(AppState::Running),
                init_terrain_world
                    .after(BiomeInitSet)
                    .in_set(TerrainWorldInitSet),
            )
            .add_systems(
                Update,
                (
                    sync_terrain_on_recipe_change,
                    manage_chunk_residency,
                    dispatch_density_jobs,
                    poll_density_jobs,
                    poll_mesh_jobs,
                    upload_chunk_meshes,
                    attach_pending_colliders,
                )
                    .chain()
                    .run_if(in_state(AppState::Running)),
            );
    }
}

#[derive(Resource, Clone, Debug)]
pub struct TerrainSpawnPoint(pub Vec3);

impl Default for TerrainSpawnPoint {
    fn default() -> Self {
        Self(Vec3::new(-30.0, 16.0, -25.0))
    }
}

#[derive(Resource, Default, Debug)]
pub struct TerrainRecipeRevision {
    pub hash: String,
}

/// Set when terrain-generation YAML changes; cleared on explicit F8 regen.
#[derive(Resource, Default, Debug)]
pub struct TerrainRegenPending {
    pub pending: bool,
    pub recipe_hash: String,
}

#[derive(Resource, Default)]
pub struct TerrainPipelineState {
    pub density_source: Option<RecipeDensitySource>,
    pub spawn_chunk: Option<ChunkCoord>,
    /// Sparse chunk records keyed by coordinate; see module docs.
    pub chunks: HashMap<ChunkCoord, ChunkRecord>,
    /// World extent in chunks; per axis, coords span `-(e/2) .. -(e/2)+e`
    /// (matching `terrain_generation::chunk_axis_range`).
    pub world_extent_chunks: [i32; 3],
    pub frozen: bool,
    pending_density: Vec<PendingDensityJob>,
    pending_mesh: Vec<PendingMeshJob>,
    upload_queue: Vec<UploadItem>,
    collider_queue: Vec<PendingCollider>,
}

impl TerrainPipelineState {
    /// Clears pending work and returns chunk entities that should be
    /// despawned. All records are dropped; they re-materialize lazily at the
    /// new revision as the scheduler touches them.
    pub fn reset_for_revision(&mut self, _revision: u64) -> Vec<Entity> {
        let mut to_despawn = Vec::new();
        for chunk in self.chunks.values_mut() {
            if let Some(entity) = chunk.entity.take() {
                to_despawn.push(entity);
            }
        }
        self.chunks.clear();
        self.pending_density.clear();
        self.pending_mesh.clear();
        self.upload_queue.clear();
        self.collider_queue.clear();
        to_despawn
    }

    pub fn density_queue_len(&self) -> usize {
        self.pending_density.len()
    }

    pub fn mesh_queue_len(&self) -> usize {
        self.pending_mesh.len()
    }

    pub fn upload_queue_len(&self) -> usize {
        self.upload_queue.len()
    }

    pub fn collider_queue_len(&self) -> usize {
        self.collider_queue.len()
    }

    /// Invalidate specific chunks after terrain edits.
    pub fn invalidate_chunks(&mut self, coords: &[ChunkCoord], revision: u64) -> Vec<Entity> {
        let mut to_despawn = Vec::new();
        for coord in coords {
            if let Some(chunk) = self.chunks.get_mut(coord) {
                if let Some(entity) = chunk.entity.take() {
                    to_despawn.push(entity);
                }
                chunk.state = ChunkState::Unrequested;
                chunk.revision = revision;
            }
        }
        self.pending_density
            .retain(|job| !coords.contains(&job.coord));
        self.pending_mesh.retain(|job| !coords.contains(&job.coord));
        self.upload_queue
            .retain(|item| !coords.contains(&item.coord));
        self.collider_queue
            .retain(|pending| !to_despawn.contains(&pending.entity));
        to_despawn
    }
}

pub struct ChunkRecord {
    pub coord: ChunkCoord,
    pub state: ChunkState,
    pub revision: u64,
    pub entity: Option<Entity>,
}

struct PendingDensityJob {
    coord: ChunkCoord,
    revision: u64,
    started: Instant,
    task: Task<DensityJobResult>,
}

struct DensityJobResult {
    samples: Vec<voxel_core::TerrainSample>,
    column_cache: ChunkColumnCache,
    edit_snapshot: TerrainEditStore,
}

struct PendingMeshJob {
    coord: ChunkCoord,
    revision: u64,
    started: Instant,
    task: Task<Result<TerrainMeshData, terrain_meshing::MeshingError>>,
}

struct UploadItem {
    coord: ChunkCoord,
    revision: u64,
    mesh_data: TerrainMeshData,
}

struct PendingCollider {
    entity: Entity,
    coord: ChunkCoord,
    collider: Collider,
}

fn density_world_override(prefs: &UserSetupPrefs) -> Option<shared::StableId> {
    Some(prefs.world_stable_id())
}

fn seed_override_active(seed_override: &WorldSeedOverride, world_seed: u64) -> Option<u64> {
    if seed_override.seed != world_seed {
        Some(seed_override.seed)
    } else {
        None
    }
}

fn warn_if_atlas_validation_failed(source: &RecipeDensitySource) {
    if let Some(atlas) = source.atlas() {
        if !atlas.validation_passed {
            warn!(
                messages = ?atlas.validation_messages,
                "island atlas validation failed; terrain may not match design intent"
            );
        }
    }
}

/// Inclusive chunk-coordinate bounds of the world volume, matching
/// `terrain_generation::chunk_axis_range`.
fn world_chunk_bounds(extent: [i32; 3]) -> ([i32; 3], [i32; 3]) {
    let min = [-(extent[0] / 2), -(extent[1] / 2), -(extent[2] / 2)];
    let max = [
        min[0] + extent[0] - 1,
        min[1] + extent[1] - 1,
        min[2] + extent[2] - 1,
    ];
    (min, max)
}

fn init_terrain_world(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    terrain_tweaks: Res<TerrainTweaks>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut spawn_point: ResMut<TerrainSpawnPoint>,
    mut recipe_revision: ResMut<TerrainRecipeRevision>,
    mut seed_override: ResMut<WorldSeedOverride>,
    mut runtime: ResMut<TerrainWorldRuntime>,
    revision: Res<TerrainRevision>,
) {
    let world_override = density_world_override(&prefs);
    let world = registry
        .0
        .effective_world(world_override.as_ref())
        .expect("world");
    seed_override.seed = prefs.seed;
    runtime.seed = prefs.seed;
    runtime.cell_size_m = world.cell_size_m;
    runtime.revision = revision.value;
    let override_seed = seed_override_active(&seed_override, world.seed);
    let source =
        build_density_source_from_prefs(&registry.0, &prefs, terrain_tweaks.field_stack_params());
    warn_if_atlas_validation_failed(&source);
    let (sx, sy, sz, spawn_report) =
        source.resolve_player_spawn(terrain_generation::PLAYER_SPAWN_MIN_CLEARANCE_M, 48.0);
    spawn_point.0 = Vec3::new(sx, sy, sz);
    let spawn_cell = WorldCell::new(sx.floor() as i32, sy.floor() as i32, sz.floor() as i32);
    pipeline.spawn_chunk = Some(spawn_cell.chunk_coord());
    runtime.interest_center = spawn_cell.chunk_coord();

    let extent = [
        world.world_extent_chunks[0] as i32,
        world.world_extent_chunks[1] as i32,
        world.world_extent_chunks[2] as i32,
    ];

    pipeline.density_source = Some(source);
    pipeline.world_extent_chunks = extent;
    pipeline.chunks = HashMap::new();
    recipe_revision.hash = terrain_recipe_hash(
        &registry.0,
        world_override.as_ref(),
        override_seed,
        Some(&prefs),
        Some(&terrain_tweaks.field_stack_params()),
    );

    info!(
        potential_chunks = extent[0] as i64 * extent[1] as i64 * extent[2] as i64,
        world = %world.id.as_str(),
        spawn = ?spawn_point.0,
        spawn_valid = spawn_report.passed,
        spawn_notes = ?spawn_report.messages,
        "terrain world initialized (sparse residency)"
    );
}

fn sync_terrain_on_recipe_change(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    terrain_tweaks: Res<TerrainTweaks>,
    recipe_revision: Res<TerrainRecipeRevision>,
    mut pending: ResMut<TerrainRegenPending>,
    seed_override: Res<WorldSeedOverride>,
) {
    let world_override = density_world_override(&prefs);
    let world = registry
        .0
        .effective_world(world_override.as_ref())
        .expect("world");
    let override_seed = seed_override_active(&seed_override, world.seed);
    let hash = terrain_recipe_hash(
        &registry.0,
        world_override.as_ref(),
        override_seed,
        Some(&prefs),
        Some(&terrain_tweaks.field_stack_params()),
    );
    if recipe_revision.hash == hash {
        return;
    }
    pending.pending = true;
    pending.recipe_hash = hash.clone();
    info!(recipe_hash = %hash, "terrain recipe changed; press F8 to regenerate");
}

/// Rebuild density from current seed override and bump terrain revision.
pub fn regen_terrain_with_seed(
    commands: &mut Commands,
    registry: &ConfigRegistryResource,
    prefs: &UserSetupPrefs,
    terrain_tweaks: &TerrainTweaks,
    pipeline: &mut TerrainPipelineState,
    recipe_revision: &mut TerrainRecipeRevision,
    revision: &mut TerrainRevision,
    seed_override: &WorldSeedOverride,
    spawn_point: &mut TerrainSpawnPoint,
    pending: &mut TerrainRegenPending,
    edit_store: &mut TerrainEditStore,
    runtime: &mut TerrainWorldRuntime,
) {
    let world_override = density_world_override(prefs);
    let world = registry
        .0
        .effective_world(world_override.as_ref())
        .expect("world");
    let override_seed = seed_override_active(seed_override, world.seed);
    revision.value += 1;
    let source =
        build_density_source_from_prefs(&registry.0, prefs, terrain_tweaks.field_stack_params());
    warn_if_atlas_validation_failed(&source);
    let (sx, sy, sz) = source.spawn_position();
    spawn_point.0 = Vec3::new(sx, sy, sz);
    let spawn_cell = WorldCell::new(sx.floor() as i32, sy.floor() as i32, sz.floor() as i32);
    pipeline.density_source = Some(source);
    pipeline.spawn_chunk = Some(spawn_cell.chunk_coord());
    pipeline.world_extent_chunks = [
        world.world_extent_chunks[0] as i32,
        world.world_extent_chunks[1] as i32,
        world.world_extent_chunks[2] as i32,
    ];
    runtime.interest_center = spawn_cell.chunk_coord();
    recipe_revision.hash = terrain_recipe_hash(
        &registry.0,
        world_override.as_ref(),
        override_seed,
        Some(prefs),
        Some(&terrain_tweaks.field_stack_params()),
    );
    edit_store.clear();
    pending.pending = false;
    pending.recipe_hash.clear();
    let to_despawn = pipeline.reset_for_revision(revision.value);
    for entity in to_despawn {
        commands.entity(entity).despawn();
    }
}

fn manage_chunk_residency(
    mut commands: Commands,
    runtime: Res<TerrainWorldRuntime>,
    world_tweaks: Res<WorldTweaks>,
    mut pipeline: ResMut<TerrainPipelineState>,
    revision: Res<TerrainRevision>,
) {
    let center = runtime.interest_center;

    // Cancel out-of-radius pending work first so the affected records fall
    // back to Unrequested before the prune pass below considers them.
    let mut cancelled = Vec::new();
    pipeline.pending_density.retain(|job| {
        let keep = within_density_radius(center, job.coord, &world_tweaks);
        if !keep {
            cancelled.push(job.coord);
        }
        keep
    });
    pipeline.pending_mesh.retain(|job| {
        let keep = within_density_radius(center, job.coord, &world_tweaks);
        if !keep {
            cancelled.push(job.coord);
        }
        keep
    });
    pipeline.upload_queue.retain(|item| {
        let keep = within_render_radius(center, item.coord, &world_tweaks);
        if !keep {
            cancelled.push(item.coord);
        }
        keep
    });
    reset_transient_chunk_states(&mut pipeline.chunks, &cancelled);

    // Despawn entities that left the render radius, and prune quiescent
    // records (Unrequested, no entity) outside the density radius so the map
    // stays bounded by the neighborhood rather than growing with everything
    // ever visited.
    let mut to_despawn = Vec::new();
    let revision_value = revision.value;
    pipeline.chunks.retain(|coord, chunk| {
        if !within_render_radius(center, *coord, &world_tweaks) {
            if let Some(entity) = chunk.entity.take() {
                to_despawn.push(entity);
                chunk.state = ChunkState::Unrequested;
                chunk.revision = revision_value;
            }
        }
        chunk.entity.is_some()
            || chunk.state != ChunkState::Unrequested
            || within_density_radius(center, *coord, &world_tweaks)
    });

    for entity in to_despawn.iter() {
        pipeline
            .collider_queue
            .retain(|pending| pending.entity != *entity);
    }
    for entity in to_despawn {
        commands.entity(entity).despawn();
    }
}

fn dispatch_density_jobs(
    registry: Res<ConfigRegistryResource>,
    revision: Res<TerrainRevision>,
    runtime: Res<TerrainWorldRuntime>,
    world_tweaks: Res<WorldTweaks>,
    edit_store: Res<TerrainEditStore>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut metrics: ResMut<TerrainPipelineMetrics>,
) {
    if pipeline.frozen {
        return;
    }
    let perf = registry.0.active_performance().expect("performance");
    let max_jobs = perf.maximum_density_jobs as usize;
    if pipeline.pending_density.len() >= max_jobs {
        return;
    }
    let Some(source) = pipeline.density_source.clone() else {
        return;
    };

    let slots = max_jobs - pipeline.pending_density.len();
    let mut to_start: Vec<(ChunkCoord, u64)> = Vec::new();
    let center = runtime.interest_center;
    let revision_value = revision.value;

    let mut try_start = |pipeline: &mut TerrainPipelineState, coord: ChunkCoord| -> bool {
        let record = pipeline.chunks.entry(coord).or_insert_with(|| ChunkRecord {
            coord,
            state: ChunkState::Unrequested,
            revision: revision_value,
            entity: None,
        });
        if record.state != ChunkState::Unrequested {
            return false;
        }
        record.state = ChunkState::GeneratingDensity;
        record.revision = revision_value;
        true
    };

    if let Some(spawn_chunk) = pipeline.spawn_chunk {
        if within_density_radius(center, spawn_chunk, &world_tweaks)
            && try_start(&mut pipeline, spawn_chunk)
        {
            to_start.push((spawn_chunk, revision_value));
        }
    }

    // Candidates come from the neighborhood cube around the interest center
    // (clamped to the world volume), not from scanning every record — this is
    // what keeps scheduling O(radius³) on very large worlds.
    let (min_b, max_b) = world_chunk_bounds(pipeline.world_extent_chunks);
    let r = world_tweaks.density_radius;
    let mut candidates: Vec<(i32, ChunkCoord)> = Vec::new();
    for cx in (center.x - r).max(min_b[0])..=(center.x + r).min(max_b[0]) {
        for cy in (center.y - r).max(min_b[1])..=(center.y + r).min(max_b[1]) {
            for cz in (center.z - r).max(min_b[2])..=(center.z + r).min(max_b[2]) {
                let coord = ChunkCoord::new(cx, cy, cz);
                if !within_density_radius(center, coord, &world_tweaks) {
                    continue;
                }
                if let Some(existing) = pipeline.chunks.get(&coord) {
                    if existing.state != ChunkState::Unrequested {
                        continue;
                    }
                }
                candidates.push((chunk_chebyshev_distance(center, coord), coord));
            }
        }
    }
    candidates.sort_by_key(|(d, _)| *d);

    for (_d, coord) in candidates {
        if to_start.len() >= slots {
            break;
        }
        if try_start(&mut pipeline, coord) {
            to_start.push((coord, revision_value));
        }
    }

    let edits = edit_store.clone();
    let cell_size_m = runtime.cell_size_m;
    for (coord, rev) in to_start {
        let src = source.clone();
        let edit_overlay = edits.clone();
        let started = Instant::now();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            generate_padded_samples_runtime(&src, &edit_overlay, coord, cell_size_m)
        });
        pipeline.pending_density.push(PendingDensityJob {
            coord,
            revision: rev,
            started,
            task,
        });
    }

    metrics.density_queue = pipeline.density_queue_len();
    metrics.mesh_queue = pipeline.mesh_queue_len();
    metrics.upload_queue = pipeline.upload_queue_len();
    metrics.collider_queue = pipeline.collider_queue_len();
}

fn poll_density_jobs(
    mut pipeline: ResMut<TerrainPipelineState>,
    mut metrics: ResMut<TerrainPipelineMetrics>,
    runtime: Res<TerrainWorldRuntime>,
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
) {
    if pipeline.frozen {
        return;
    }
    let mut completed = Vec::new();
    pipeline.pending_density.retain_mut(|job| {
        if let Some(result) =
            bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(&mut job.task))
        {
            completed.push((job.coord, job.revision, job.started, result));
            false
        } else {
            true
        }
    });

    for (coord, revision, started, result) in completed {
        metrics.record_density_ms(started.elapsed().as_secs_f32() * 1000.0);
        let Some(chunk) = pipeline.chunks.get_mut(&coord) else {
            continue;
        };
        if chunk.revision != revision {
            continue;
        }
        chunk.state = ChunkState::Meshing;
        let mesh_started = Instant::now();
        let source = pipeline
            .density_source
            .clone()
            .expect("density source must be initialized before meshing");
        let cell_size_m = runtime.cell_size_m;
        let (ox, oy, oz) = voxel_core::TerrainChunk::new(coord).sample_origin();
        let samples = result.samples;
        let column_cache = result.column_cache;
        let edit_snapshot = result.edit_snapshot;
        let world_id = crate::world::requested_world_id(&prefs);
        let world = registry
            .0
            .effective_world(Some(&world_id))
            .expect("world");
        let palette = registry
            .0
            .materials
            .get(&world.materials)
            .expect("materials palette")
            .clone();
        let surface_rules = registry
            .0
            .surface_rules
            .get(&world.surface)
            .expect("surface rules")
            .clone();
        let mesh_task = AsyncComputeTaskPool::get().spawn(async move {
            let resolver = ChunkSurfaceResolver::from_compiled(
                source,
                column_cache,
                ox,
                oy,
                oz,
                cell_size_m,
                edit_snapshot,
                &palette,
                &surface_rules,
            );
            let mesher = SurfaceNetsMesher;
            let input = ChunkMeshingInput {
                samples: &samples,
                chunk_cells: CHUNK_CELLS,
                surface_resolver: Some(&resolver),
            };
            mesher.build_mesh(&input)
        });
        pipeline.pending_mesh.push(PendingMeshJob {
            coord,
            revision,
            started: mesh_started,
            task: mesh_task,
        });
    }

    metrics.density_queue = pipeline.density_queue_len();
    metrics.mesh_queue = pipeline.mesh_queue_len();
}

fn poll_mesh_jobs(
    mut pipeline: ResMut<TerrainPipelineState>,
    mut metrics: ResMut<TerrainPipelineMetrics>,
) {
    if pipeline.frozen {
        return;
    }
    let mut completed = Vec::new();
    pipeline.pending_mesh.retain_mut(|job| {
        if let Some(result) =
            bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(&mut job.task))
        {
            completed.push((job.coord, job.revision, job.started, result));
            false
        } else {
            true
        }
    });

    for (coord, revision, started, result) in completed {
        metrics.record_mesh_ms(started.elapsed().as_secs_f32() * 1000.0);
        let Some(chunk) = pipeline.chunks.get_mut(&coord) else {
            continue;
        };
        if chunk.revision != revision {
            continue;
        }
        let mesh_data = match result {
            Ok(mesh_data) => mesh_data,
            Err(error) => {
                tracing::warn!(?coord, ?error, "terrain chunk meshing failed");
                chunk.state = ChunkState::Failed;
                continue;
            }
        };
        if mesh_data.positions.is_empty() || mesh_data.indices.is_empty() {
            chunk.state = ChunkState::Ready;
        } else {
            chunk.state = ChunkState::AwaitingUpload;
            pipeline.upload_queue.push(UploadItem {
                coord,
                revision,
                mesh_data,
            });
        }
    }

    metrics.mesh_queue = pipeline.mesh_queue_len();
    metrics.upload_queue = pipeline.upload_queue_len();
}

fn upload_chunk_meshes(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    runtime: Res<TerrainWorldRuntime>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut metrics: ResMut<TerrainPipelineMetrics>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<terrain_material_bevy::TerrainPbrMaterial>>,
    triplanar_handle: Res<TerrainMaterialHandle>,
) {
    if pipeline.frozen {
        return;
    }
    let upload_start = Instant::now();
    let perf = registry.0.active_performance().expect("performance");
    let mesh_budget = perf.mesh_uploads_per_frame as usize;

    let mut queue = std::mem::take(&mut pipeline.upload_queue);
    let center = runtime.interest_center;
    queue.sort_by_key(|item| {
        std::cmp::Reverse(chunk_chebyshev_distance(center, item.coord))
    });
    if let Some(spawn_chunk) = pipeline.spawn_chunk {
        // pop() takes from the back, so keep the spawn chunk at the end.
        queue.sort_by_key(|item| item.coord == spawn_chunk);
    }

    let material_template = materials
        .get(&triplanar_handle.0)
        .cloned()
        .expect("terrain material");

    let mut uploaded = 0usize;
    for _ in 0..mesh_budget {
        let Some(item) = queue.pop() else {
            break;
        };
        let Some(record_revision) = pipeline.chunks.get(&item.coord).map(|c| c.revision) else {
            continue;
        };
        if record_revision != item.revision {
            continue;
        }

        let cell_size_m = runtime.cell_size_m;
        let mesh = mesh_from_terrain_data(&item.mesh_data, cell_size_m);
        let positions: Vec<Vec3> = item
            .mesh_data
            .positions
            .iter()
            .map(|p| Vec3::from_array(*p) * cell_size_m)
            .collect();
        let indices = item.mesh_data.indices.clone();
        let tri_indices: Vec<[u32; 3]> = indices
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        let collider = if tri_indices.is_empty() {
            Collider::cuboid(0.1, 0.1, 0.1)
        } else {
            Collider::trimesh(positions, tri_indices)
        };

        let chunk_material = materials.add(
            material_template.with_chunk_palette(item.mesh_data.chunk_palette),
        );

        let entity = commands
            .spawn((
                TerrainChunkEntity { coord: item.coord },
                TerrainChunkMaterial,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(chunk_material),
                chunk_world_transform(item.coord, cell_size_m),
                RigidBody::Static,
                Visibility::default(),
            ))
            .id();
        if let Some(chunk) = pipeline.chunks.get_mut(&item.coord) {
            chunk.entity = Some(entity);
            chunk.state = ChunkState::Ready;
        }
        pipeline.collider_queue.push(PendingCollider {
            entity,
            coord: item.coord,
            collider,
        });
        uploaded += 1;
    }

    if uploaded > 0 {
        metrics.record_upload_ms(upload_start.elapsed().as_secs_f32() * 1000.0);
    }

    pipeline.upload_queue = queue;
    metrics.upload_queue = pipeline.upload_queue_len();
    metrics.collider_queue = pipeline.collider_queue_len();
}

fn attach_pending_colliders(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    runtime: Res<TerrainWorldRuntime>,
    world_tweaks: Res<WorldTweaks>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut metrics: ResMut<TerrainPipelineMetrics>,
) {
    if pipeline.frozen || pipeline.collider_queue.is_empty() {
        metrics.colliders_built_this_frame = 0;
        return;
    }

    let perf = registry.0.active_performance().expect("performance");
    let budget = perf.collider_builds_per_frame as usize;
    let center = runtime.interest_center;
    let mut built = 0u32;

    pipeline.collider_queue.retain(|pending| {
        if built as usize >= budget {
            return true;
        }
        if !within_physics_radius(center, pending.coord, &world_tweaks) {
            return false;
        }
        commands.entity(pending.entity).insert((
            pending.collider.clone(),
            CollisionLayers::from(terrain_layers()),
        ));
        built += 1;
        false
    });

    metrics.colliders_built_this_frame = built;
    metrics.collider_queue = pipeline.collider_queue_len();
}

fn reset_transient_chunk_states(
    chunks: &mut HashMap<ChunkCoord, ChunkRecord>,
    cancelled: &[ChunkCoord],
) {
    for coord in cancelled {
        if let Some(chunk) = chunks.get_mut(coord) {
            if chunk.entity.is_none()
                && matches!(
                    chunk.state,
                    ChunkState::GeneratingDensity | ChunkState::Meshing | ChunkState::AwaitingUpload
                )
            {
                chunk.state = ChunkState::Unrequested;
            }
        }
    }
}

fn generate_padded_samples_runtime(
    source: &RecipeDensitySource,
    edits: &TerrainEditStore,
    coord: ChunkCoord,
    cell_size_m: f32,
) -> DensityJobResult {
    let (ox, _oy, oz) = voxel_core::TerrainChunk::new(coord).sample_origin();
    let padded_side = CHUNK_CELLS + 3;
    let column_cache = ChunkColumnCache::build(source, ox, oz, padded_side);
    let samples = fill_padded_samples(coord, |wx, wy, wz| {
        if let Some(override_sample) = edits.0.sample_override(wx, wy, wz) {
            (override_sample.density, override_sample.material)
        } else {
            (
                source.sample_density(
                    wx as f32 * cell_size_m,
                    wy as f32 * cell_size_m,
                    wz as f32 * cell_size_m,
                ),
                voxel_core::MaterialId(0),
            )
        }
    });
    DensityJobResult {
        samples,
        column_cache,
        edit_snapshot: edits.clone(),
    }
}

/// System-A material path kept as a reference implementation for tests
/// exercising `surface_resolver: None` meshing. Runtime density jobs use
/// `generate_padded_samples_runtime` instead.
#[cfg(test)]
fn generate_padded_samples_with_biomes(
    source: &RecipeDensitySource,
    biomes: &BiomeCatalog,
    edits: &TerrainEditStore,
    coord: ChunkCoord,
    cell_size_m: f32,
) -> Vec<voxel_core::TerrainSample> {
    let cells = voxel_core::CHUNK_CELLS;
    let (ox, oy, oz) = voxel_core::TerrainChunk::new(coord).sample_origin();
    let padded_side = cells + 3;
    let column_cache = ChunkColumnCache::build(source, ox, oz, padded_side);
    let mut samples = Vec::with_capacity(padded_side.pow(3));
    for pz in -1..=(cells as i32 + 1) {
        for py in -1..=(cells as i32 + 1) {
            for px in -1..=(cells as i32 + 1) {
                let wx_m = (ox + px) as f32 * cell_size_m;
                let wy_m = (oy + py) as f32 * cell_size_m;
                let wz_m = (oz + pz) as f32 * cell_size_m;
                let wx = ox + px;
                let wy = oy + py;
                let wz = oz + pz;
                let (density, material) =
                    if let Some(override_sample) = edits.0.sample_override(wx, wy, wz) {
                        (override_sample.density, override_sample.material)
                    } else {
                        let density = source.sample_density(wx_m, wy_m, wz_m);
                        let material = material_for_world_with_cache(
                            biomes,
                            source,
                            Some(&column_cache),
                            wx_m,
                            wy_m,
                            wz_m,
                            density,
                        );
                        (density, material)
                    };
                samples.push(voxel_core::TerrainSample { density, material });
            }
        }
    }
    samples
}

#[cfg(test)]
mod pipeline_tests {
    use bevy::prelude::Entity;
    use std::collections::HashMap;
    use super::{
        generate_padded_samples_runtime, generate_padded_samples_with_biomes, reset_transient_chunk_states,
        ChunkRecord,
    };
    use crate::environment::biomes::BiomeCatalog;
    use crate::terrain::{ChunkState, TerrainEditStore};
    use game_data::BiomeRuleDefinition;
    use terrain_generation::{
        DensitySource, IslandGenParams, RecipeDensitySource, TerrainRecipe, build_island_atlas,
        default_vertical_slice_recipe,
    };
    use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMesher};
    use voxel_core::{CHUNK_CELLS, ChunkCoord, MaterialId, TerrainChunk, WorldCell};

    fn test_catalog() -> BiomeCatalog {
        BiomeCatalog {
            rules: vec![BiomeRuleDefinition::new("grassland", 0, [0.34, 0.52, 0.28])],
        }
    }

    fn single_record(coord: ChunkCoord, state: ChunkState, entity: Option<Entity>) -> HashMap<ChunkCoord, ChunkRecord> {
        let mut chunks = HashMap::new();
        chunks.insert(
            coord,
            ChunkRecord {
                coord,
                state,
                revision: 1,
                entity,
            },
        );
        chunks
    }

    #[test]
    fn spawn_area_chunk_meshes_with_vertices() {
        let source = RecipeDensitySource::new(default_vertical_slice_recipe(42, 2.0));
        let (sx, sy, sz) = source.spawn_position();
        let coord =
            voxel_core::WorldCell::new(sx.floor() as i32, sy.floor() as i32, sz.floor() as i32)
                .chunk_coord();
        let samples = generate_padded_samples_with_biomes(
            &source,
            &test_catalog(),
            &TerrainEditStore::default(),
            coord,
            1.0,
        );
        let mesher = SurfaceNetsMesher;
        let mesh = mesher
            .build_mesh(&ChunkMeshingInput {
                samples: &samples,
                chunk_cells: voxel_core::CHUNK_CELLS,
                surface_resolver: None,
            })
            .expect("mesh");
        assert!(
            !mesh.positions.is_empty(),
            "spawn chunk should produce terrain geometry"
        );
        assert!(!mesh.material_vertices.is_empty());
    }

    #[test]
    fn biome_meshing_path_uses_sample_density_microdetail() {
        let mut params = IslandGenParams::default();
        params.surface_noise.voxel_amplitude_m = 1.0;
        let atlas = build_island_atlas(&params);
        let source = RecipeDensitySource::new(TerrainRecipe {
            seed: params.seed,
            sea_level: params.island.sea_level_m,
            spawn_x: 0.0,
            spawn_z: 0.0,
            coord_offset: [0.0, 0.0, 0.0],
            ops: Vec::new(),
        })
        .with_atlas(atlas, 3.5);
        let probe_x = 8.0;
        let probe_z = 8.0;
        let surface_y = source.terrain_surface_height_at(probe_x, probe_z);
        let chunk_y = (surface_y.floor() as i32).div_euclid(CHUNK_CELLS as i32);
        let coord = WorldCell::new(0, surface_y.floor() as i32, 0)
            .chunk_coord();
        assert_eq!(coord.y, chunk_y);
        let samples = generate_padded_samples_with_biomes(
            &source,
            &test_catalog(),
            &TerrainEditStore::default(),
            coord,
            1.0,
        );

        let (ox, oy, oz) = TerrainChunk::new(coord).sample_origin();
        let padded_side = CHUNK_CELLS + 3;
        let mut verified = false;
        'scan: for pz in -1..=(CHUNK_CELLS as i32 + 1) {
            for py in -1..=(CHUNK_CELLS as i32 + 1) {
                for px in -1..=(CHUNK_CELLS as i32 + 1) {
                    let wx = (ox + px) as f32;
                    let wy = (oy + py) as f32;
                    let wz = (oz + pz) as f32;
                    let base = source.density_at(wx, wy, wz);
                    if base.abs() > 2.0 {
                        continue;
                    }
                    let sampled = source.sample_density(wx, wy, wz);
                    if (sampled - base).abs() <= 0.01 {
                        continue;
                    }
                    let idx = (pz + 1) as usize * padded_side * padded_side
                        + (py + 1) as usize * padded_side
                        + (px + 1) as usize;
                    assert!((samples[idx].density - sampled).abs() < 0.001);
                    verified = true;
                    break 'scan;
                }
            }
        }
        assert!(
            verified,
            "expected at least one microdetail-adjusted padded sample near surface y={surface_y}"
        );
    }

    #[test]
    fn runtime_density_path_skips_per_sample_material_classification() {
        let source = RecipeDensitySource::new(default_vertical_slice_recipe(42, 2.0));
        let coord = ChunkCoord::new(0, 1, 0);
        let result = generate_padded_samples_runtime(
            &source,
            &TerrainEditStore::default(),
            coord,
            1.0,
        );
        assert!(!result.samples.is_empty());
        assert!(
            result
                .samples
                .iter()
                .all(|sample| sample.material == MaterialId(0)),
            "runtime path should not classify per-sample materials"
        );
    }

    #[test]
    fn cancelled_out_of_radius_jobs_reset_to_unrequested() {
        let coord = ChunkCoord::new(3, 0, 0);
        let mut chunks = single_record(coord, ChunkState::AwaitingUpload, None);
        reset_transient_chunk_states(&mut chunks, &[coord]);
        assert_eq!(chunks[&coord].state, ChunkState::Unrequested);
    }

    #[test]
    fn chunks_with_entities_are_not_reset_on_cancellation() {
        let coord = ChunkCoord::new(3, 0, 0);
        let mut chunks = single_record(coord, ChunkState::Ready, Some(Entity::from_bits(1)));
        reset_transient_chunk_states(&mut chunks, &[coord]);
        assert_eq!(chunks[&coord].state, ChunkState::Ready);
    }

    #[test]
    fn world_chunk_bounds_match_axis_range_convention() {
        // extent 16 spans -8..=7; extent 10 spans -5..=4 (chunk_axis_range).
        let (min, max) = super::world_chunk_bounds([16, 10, 16]);
        assert_eq!(min, [-8, -5, -8]);
        assert_eq!(max, [7, 4, 7]);
    }

    #[test]
    fn subtract_sphere_boundary_sample_matches_sample_density_field() {
        let mut params = IslandGenParams::default();
        params.surface_noise.voxel_amplitude_m = 1.0;
        let atlas = build_island_atlas(&params);
        let source = RecipeDensitySource::new(TerrainRecipe {
            seed: params.seed,
            sea_level: params.island.sea_level_m,
            spawn_x: 0.0,
            spawn_z: 0.0,
            coord_offset: [0.0, 0.0, 0.0],
            ops: Vec::new(),
        })
        .with_atlas(atlas, 3.5);

        let wx = 12;
        let wz = 12;
        let wy = source
            .terrain_surface_height_at(wx as f32, wz as f32)
            .floor() as i32;
        let center = [wx as f32, wy as f32, wz as f32];
        let radius_m = 3.0;

        let mut store = TerrainEditStore::default();
        store.0.apply_command(
            &voxel_core::TerrainEditCommand::SubtractSphere {
                center,
                radius_m,
            },
            |ix, iy, iz| source.sample_density(ix as f32, iy as f32, iz as f32),
            |_ix, _iy, _iz, _d| MaterialId(0),
        );

        let boundary_x = (center[0] + radius_m).floor() as i32;
        let edited = store
            .0
            .sample_override(boundary_x, wy, wz)
            .expect("boundary sample should be touched by sphere");
        let field = source.sample_density(boundary_x as f32, wy as f32, wz as f32);
        assert!(
            (edited.density - field).abs() < 1e-4,
            "t=0 boundary should preserve procedural density (edited={}, field={})",
            edited.density,
            field
        );
    }
}