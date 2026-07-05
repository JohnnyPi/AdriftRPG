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
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use terrain_generation::{
    DensitySource, RecipeDensitySource, WorldDensityProvider, WorldPosition, fill_padded_samples,
    resolve_spawn_from_provider,
};
use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMeshData, TerrainMesher};
use tracing::{info, warn};
use voxel_core::{CHUNK_CELLS, ChunkCoord, WorldCell};

use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;
use crate::environment::biome_context::ChunkColumnCache;
#[cfg(test)]
use crate::environment::materials::material_for_world_with_cache;
use crate::environment::surface::ChunkSurfaceResolver;
use crate::environment::{BiomeCatalog, BiomeInitSet};
use crate::interest::WorldSimulationLodProvider;
use crate::lod::{LodPolicy, LodRuntimeState, mesh_cell_stride, terrain_lod_with_hysteresis};
use crate::state::AppState;
use crate::terrain::material::TerrainMaterialHandle;
use crate::terrain::mesh_convert::{chunk_world_transform, mesh_from_terrain_data};
use crate::terrain::metrics::{TerrainPipelineMetrics, WorldSeedOverride};
use crate::terrain::recipe::terrain_recipe_hash;
use crate::terrain::residency::{
    TerrainWorldRuntime, chunk_chebyshev_distance, within_density_radius, within_physics_radius,
    within_render_radius,
};
use crate::terrain::{
    ChunkState, TerrainChunkEntity, TerrainChunkMaterial, TerrainChunkPalette, TerrainEditStore,
    TerrainRevision,
};
use crate::terrain::{build_density_source_from_prefs, validate_density_source_buildable};
use crate::ui::{TerrainTweaks, WorldTweaks};
use crate::worldgen::{ActiveCompiledWorld, WorldCompilationConfig};
use game_data::TerrainColliderLodDefinition;
use physics_bridge::terrain_layers;
use terrain_surface::{
    DEFAULT_REGION_CHUNKS, MaterialKey, MaterialRegionCoord, MaterialRegionPaletteCache,
    SurfaceCoverage,
};

const MAX_TASK_POLLS_PER_FRAME: usize = 64;

#[derive(Clone)]
pub(crate) enum DensitySamplingContext {
    Legacy(Arc<RecipeDensitySource>),
    Compiled {
        provider: Arc<dyn WorldDensityProvider>,
        cell_size_m: f32,
    },
}

fn try_poll_task<T>(task: &mut Task<T>) -> Option<T> {
    bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(task))
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerrainWorldInitSet;

pub struct TerrainPlugin;

#[derive(Resource, Default)]
struct ProceduralAtlasInit {
    pending: Option<Task<ProceduralAtlasBuildResult>>,
}

struct ProceduralAtlasBuildResult {
    source: RecipeDensitySource,
    spawn: Vec3,
    spawn_chunk: ChunkCoord,
    spawn_report: terrain_generation::SpawnValidationReport,
    recipe_hash: String,
    world_extent: [i32; 3],
}

struct ResolvedSpawn {
    position: Vec3,
    chunk: ChunkCoord,
    report: terrain_generation::SpawnValidationReport,
}

/// Cached edit-store snapshot; rebuilt when [`TerrainRevision`] changes.
#[derive(Resource, Default)]
struct TerrainEditSnapshot {
    revision: u64,
    store: Arc<TerrainEditStore>,
}

impl TerrainEditSnapshot {
    fn get(&mut self, revision: u64, edit_store: &TerrainEditStore) -> Arc<TerrainEditStore> {
        if self.revision != revision {
            self.store = Arc::new(edit_store.clone());
            self.revision = revision;
        }
        Arc::clone(&self.store)
    }
}

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
            .init_resource::<ProceduralAtlasInit>()
            .init_resource::<TerrainEditSnapshot>()
            .init_resource::<TerrainRegionPaletteCache>()
            .add_plugins(super::horizon_skirt::HorizonSkirtPlugin)
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
                    crate::worldgen::apply_compiled_world_to_pipeline,
                    poll_procedural_atlas_init,
                    sync_terrain_on_recipe_change,
                    manage_chunk_residency,
                    retry_failed_chunks,
                    dispatch_density_jobs,
                    poll_density_jobs,
                    promote_density_ready_to_mesh,
                    poll_mesh_jobs,
                    upload_chunk_meshes,
                    attach_pending_colliders,
                    sync_chunk_lod_tiers,
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
    pub horizon_skirt_reset: bool,
}

#[derive(Resource, Default)]
pub struct TerrainPipelineState {
    pub density_source: Option<Arc<RecipeDensitySource>>,
    pub world_density_provider: Option<Arc<dyn WorldDensityProvider>>,
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
    /// Cached density results kept until the chunk leaves the density radius.
    density_cache: HashMap<ChunkCoord, CachedDensity>,
}

impl TerrainPipelineState {
    pub fn has_density_cached(&self, coord: ChunkCoord) -> bool {
        self.density_cache.contains_key(&coord)
    }

    /// Clears pending work and returns chunk entities that should be
    /// despawned. All records are dropped; they re-materialize lazily at the
    /// new revision as the scheduler touches them.
    pub fn reset_for_revision(&mut self) -> Vec<Entity> {
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
        self.density_cache.clear();
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

    pub fn density_sampling_context(&self, cell_size_m: f32) -> Option<DensitySamplingContext> {
        if let Some(provider) = self.world_density_provider.as_ref() {
            return Some(DensitySamplingContext::Compiled {
                provider: Arc::clone(provider),
                cell_size_m,
            });
        }
        self.density_source
            .as_ref()
            .map(|source| DensitySamplingContext::Legacy(Arc::clone(source)))
    }

    /// Invalidate specific chunks after terrain edits.
    pub fn invalidate_chunks(&mut self, coords: &[ChunkCoord], revision: u64) -> Vec<Entity> {
        let coord_set: HashSet<ChunkCoord> = coords.iter().copied().collect();
        let mut to_despawn = Vec::new();
        for coord in coords {
            if let Some(chunk) = self.chunks.get_mut(coord) {
                if let Some(entity) = chunk.entity.take() {
                    to_despawn.push(entity);
                }
                chunk.state = ChunkState::Unrequested;
                chunk.revision = revision;
                chunk.failed_at = None;
            }
            self.density_cache.remove(coord);
        }
        self.pending_density
            .retain(|job| !coord_set.contains(&job.coord));
        self.pending_mesh
            .retain(|job| !coord_set.contains(&job.coord));
        self.upload_queue
            .retain(|item| !coord_set.contains(&item.coord));
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
    pub failed_at: Option<Instant>,
    pub lod_tier: u8,
    pub mesh_resolution_scale: f32,
    pub collider_lod: TerrainColliderLodDefinition,
}

struct CachedDensity {
    revision: u64,
    samples: Arc<[voxel_core::TerrainSample]>,
    column_cache: Arc<ChunkColumnCache>,
    edit_snapshot: Arc<TerrainEditStore>,
}

struct PendingDensityJob {
    coord: ChunkCoord,
    revision: u64,
    started: Instant,
    task: Task<DensityJobResult>,
}

struct DensityJobResult {
    samples: Arc<[voxel_core::TerrainSample]>,
    column_cache: Arc<ChunkColumnCache>,
}

struct PendingMeshJob {
    coord: ChunkCoord,
    revision: u64,
    started: Instant,
    task: Task<Result<MeshJobResult, terrain_meshing::MeshingError>>,
}

struct MeshJobResult {
    mesh_data: TerrainMeshData,
    collider: Option<Collider>,
}

struct MeshPromoteJob {
    coord: ChunkCoord,
    samples: Arc<[voxel_core::TerrainSample]>,
    column_cache: Arc<ChunkColumnCache>,
    edit_snapshot: Arc<TerrainEditStore>,
    needs_collider: bool,
    cell_stride: u32,
}

#[derive(Resource)]
struct TerrainRegionPaletteCache {
    cache: MaterialRegionPaletteCache,
    coverage: HashMap<MaterialRegionCoord, SurfaceCoverage>,
}

impl Default for TerrainRegionPaletteCache {
    fn default() -> Self {
        Self {
            cache: MaterialRegionPaletteCache::new(DEFAULT_REGION_CHUNKS),
            coverage: HashMap::new(),
        }
    }
}

#[derive(Resource, Default, Clone)]
struct UploadItem {
    coord: ChunkCoord,
    revision: u64,
    mesh_data: TerrainMeshData,
    collider: Option<Collider>,
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

fn world_extent_chunks_from_metadata(
    extent: &terrain_generation::WorldExtent,
    cell_size_m: f32,
    spawn_chunk: Option<ChunkCoord>,
) -> [i32; 3] {
    let chunk_span_m = voxel_core::CHUNK_CELLS as f32 * cell_size_m;
    let horizontal = (extent.width_m / chunk_span_m as f64).ceil() as i32;
    let mut vertical =
        ((extent.vertical_max_m - extent.vertical_min_m) / chunk_span_m as f64).ceil() as i32;
    let depth = (extent.depth_m / chunk_span_m as f64).ceil() as i32;
    // Chunk Y is centered on sea level; YAML vertical span can underestimate volcanic peaks.
    if let Some(spawn) = spawn_chunk {
        let margin_chunks = 8;
        let peak_vertical = (spawn.y.abs() + margin_chunks) * 2 + 1;
        vertical = vertical.max(peak_vertical);
    }
    [horizontal.max(1), vertical.max(1), depth.max(1)]
}

pub(crate) fn finish_compiled_world_install(
    provider: Arc<dyn WorldDensityProvider>,
    pipeline: &mut TerrainPipelineState,
    spawn_point: &mut TerrainSpawnPoint,
    recipe_revision: &mut TerrainRecipeRevision,
    runtime: &mut TerrainWorldRuntime,
    recipe_hash: String,
    cell_size_m: f32,
) {
    let metadata = provider.world_metadata().clone();
    let extent = metadata.extent;
    let (sx, sy, sz, spawn_report) = resolve_spawn_from_provider(
        provider.as_ref(),
        0.0,
        0.0,
        terrain_generation::PLAYER_SPAWN_MIN_CLEARANCE_M,
        256.0,
    );
    let spawn_cell = WorldCell::new(
        (sx / cell_size_m).floor() as i32,
        (sy / cell_size_m).floor() as i32,
        (sz / cell_size_m).floor() as i32,
    );
    spawn_point.0 = Vec3::new(sx, sy, sz);
    let spawn_chunk_coord = spawn_cell.chunk_coord();
    pipeline.spawn_chunk = Some(spawn_chunk_coord);
    runtime.interest_center = spawn_chunk_coord;
    pipeline.world_density_provider = Some(provider);
    pipeline.density_source = None;
    pipeline.world_extent_chunks =
        world_extent_chunks_from_metadata(&extent, cell_size_m, Some(spawn_chunk_coord));
    pipeline.chunks = HashMap::new();
    pipeline.frozen = false;
    recipe_revision.hash = recipe_hash;
    info!(
        world = %metadata.world_id,
        spawn = ?spawn_point.0,
        spawn_valid = spawn_report.passed,
        spawn_notes = ?spawn_report.messages,
        extent_chunks = ?pipeline.world_extent_chunks,
        "compiled world installed into terrain pipeline"
    );
}

fn finish_terrain_world_init(
    source: RecipeDensitySource,
    world: &game_data::CompiledWorld,
    pipeline: &mut TerrainPipelineState,
    spawn_point: &mut TerrainSpawnPoint,
    recipe_revision: &mut TerrainRecipeRevision,
    runtime: &mut TerrainWorldRuntime,
    recipe_hash: String,
    resolved_spawn: Option<ResolvedSpawn>,
) {
    let (spawn_pos, spawn_chunk, spawn_report) = if let Some(resolved) = resolved_spawn {
        (resolved.position, resolved.chunk, resolved.report)
    } else {
        let (sx, sy, sz, report) =
            source.resolve_player_spawn(terrain_generation::PLAYER_SPAWN_MIN_CLEARANCE_M, 48.0);
        let spawn_cell = WorldCell::new(sx.floor() as i32, sy.floor() as i32, sz.floor() as i32);
        (Vec3::new(sx, sy, sz), spawn_cell.chunk_coord(), report)
    };
    spawn_point.0 = spawn_pos;
    pipeline.spawn_chunk = Some(spawn_chunk);
    runtime.interest_center = spawn_chunk;

    let extent = [
        world.world_extent_chunks[0] as i32,
        world.world_extent_chunks[1] as i32,
        world.world_extent_chunks[2] as i32,
    ];

    pipeline.density_source = Some(Arc::new(source));
    pipeline.world_density_provider = None;
    pipeline.world_extent_chunks = extent;
    pipeline.chunks = HashMap::new();
    pipeline.frozen = false;
    recipe_revision.hash = recipe_hash;

    info!(
        potential_chunks = extent[0] as i64 * extent[1] as i64 * extent[2] as i64,
        world = %world.id.as_str(),
        spawn = ?spawn_point.0,
        spawn_valid = spawn_report.passed,
        spawn_notes = ?spawn_report.messages,
        "terrain world initialized (sparse residency)"
    );
}

fn init_terrain_world(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    terrain_tweaks: Res<TerrainTweaks>,
    worldgen_config: Res<WorldCompilationConfig>,
    compiled_world: Option<Res<ActiveCompiledWorld>>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut spawn_point: ResMut<TerrainSpawnPoint>,
    mut recipe_revision: ResMut<TerrainRecipeRevision>,
    mut seed_override: ResMut<WorldSeedOverride>,
    mut runtime: ResMut<TerrainWorldRuntime>,
    revision: Res<TerrainRevision>,
    mut procedural_init: ResMut<ProceduralAtlasInit>,
) {
    let world_override = density_world_override(&prefs);
    let world = registry
        .0
        .effective_world(world_override.as_ref())
        .expect("world");

    let worldgen_recipe = world.worldgen.as_ref().map(|id| id.as_str());
    let use_worldgen = worldgen_recipe.is_some() && worldgen_config.enabled;

    if use_worldgen {
        runtime.cell_size_m = world.cell_size_m;
        runtime.revision = revision.value;
        seed_override.seed = prefs.seed;
        runtime.seed = prefs.seed;
        let expected_id = worldgen_recipe.unwrap_or_default();
        if let Some(active) = compiled_world {
            if active.world_id == expected_id {
                let cell_size_m = runtime.cell_size_m.max(0.001);
                finish_compiled_world_install(
                    Arc::clone(&active.provider),
                    &mut pipeline,
                    &mut spawn_point,
                    &mut recipe_revision,
                    &mut runtime,
                    active.recipe_hash.clone(),
                    cell_size_m,
                );
                return;
            }
        }
        pipeline.frozen = true;
        pipeline.density_source = None;
        pipeline.world_density_provider = None;
        pipeline.chunks.clear();
        info!(
            worldgen = expected_id,
            presentation = %world.id.as_str(),
            "waiting for compiled worldgen atlas"
        );
        return;
    }

    seed_override.seed = prefs.seed;
    runtime.seed = prefs.seed;
    runtime.cell_size_m = world.cell_size_m;
    runtime.revision = revision.value;
    let override_seed = seed_override_active(&seed_override, world.seed);
    if let Err(err) = validate_density_source_buildable(
        &registry.0,
        world_override.as_ref(),
        override_seed,
        terrain_tweaks.field_stack_params(),
    ) {
        warn!(
            error = %err,
            "density source probe failed; continuing with prefs-backed build"
        );
    }
    let procedural_only = world.island_gen.is_some() && world.island_atlas_baked.is_none();
    if procedural_only {
        pipeline.frozen = true;
        pipeline.density_source = None;
        pipeline.chunks.clear();
        let registry = registry.0.clone();
        let prefs = prefs.clone();
        let field_stack = terrain_tweaks.field_stack_params();
        let world_extent = [
            world.world_extent_chunks[0] as i32,
            world.world_extent_chunks[1] as i32,
            world.world_extent_chunks[2] as i32,
        ];
        let world_id = world.id.clone();
        procedural_init.pending = Some(AsyncComputeTaskPool::get().spawn(async move {
            let source = build_density_source_from_prefs(&registry, &prefs, field_stack.clone());
            let hash = terrain_recipe_hash(
                &registry,
                Some(&world_id),
                override_seed,
                Some(&prefs),
                Some(&field_stack),
            );
            let (sx, sy, sz, spawn_report) =
                source.resolve_player_spawn(terrain_generation::PLAYER_SPAWN_MIN_CLEARANCE_M, 48.0);
            let spawn_cell =
                WorldCell::new(sx.floor() as i32, sy.floor() as i32, sz.floor() as i32);
            ProceduralAtlasBuildResult {
                source,
                spawn: Vec3::new(sx, sy, sz),
                spawn_chunk: spawn_cell.chunk_coord(),
                spawn_report,
                recipe_hash: hash,
                world_extent,
            }
        }));
        info!(world = %world.id.as_str(), "procedural atlas build started on background thread");
        return;
    }

    let source =
        build_density_source_from_prefs(&registry.0, &prefs, terrain_tweaks.field_stack_params());
    warn_if_atlas_validation_failed(&source);
    finish_terrain_world_init(
        source,
        world,
        &mut pipeline,
        &mut spawn_point,
        &mut recipe_revision,
        &mut runtime,
        terrain_recipe_hash(
            &registry.0,
            world_override.as_ref(),
            override_seed,
            Some(&prefs),
            Some(&terrain_tweaks.field_stack_params()),
        ),
        None,
    );
}

fn poll_procedural_atlas_init(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    mut procedural_init: ResMut<ProceduralAtlasInit>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut spawn_point: ResMut<TerrainSpawnPoint>,
    mut recipe_revision: ResMut<TerrainRecipeRevision>,
    mut runtime: ResMut<TerrainWorldRuntime>,
) {
    let Some(mut task) = procedural_init.pending.take() else {
        return;
    };
    if let Some(result) = try_poll_task(&mut task) {
        warn_if_atlas_validation_failed(&result.source);
        let world = registry
            .0
            .effective_world(Some(&prefs.world_stable_id()))
            .expect("world");
        pipeline.world_extent_chunks = result.world_extent;
        finish_terrain_world_init(
            result.source,
            world,
            &mut pipeline,
            &mut spawn_point,
            &mut recipe_revision,
            &mut runtime,
            result.recipe_hash,
            Some(ResolvedSpawn {
                position: result.spawn,
                chunk: result.spawn_chunk,
                report: result.spawn_report,
            }),
        );
        info!(world = %world.id.as_str(), "procedural atlas build completed");
    } else {
        procedural_init.pending = Some(task);
    }
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
    // Milestone A worlds track recipe identity via the compiled atlas hash, not the
    // legacy terrain-recipe payload used for island YAML / op stacks.
    if world.worldgen.is_some() {
        return;
    }
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
    edit_store.clear();
    pending.pending = false;
    pending.recipe_hash.clear();
    pending.horizon_skirt_reset = true;
    let to_despawn = pipeline.reset_for_revision();
    for entity in to_despawn {
        commands.entity(entity).despawn();
    }
    pipeline.world_extent_chunks = [
        world.world_extent_chunks[0] as i32,
        world.world_extent_chunks[1] as i32,
        world.world_extent_chunks[2] as i32,
    ];

    if world.worldgen.is_some() {
        pipeline.density_source = None;
        pipeline.world_density_provider = None;
        pipeline.frozen = true;
        pipeline.chunks.clear();
        recipe_revision.hash.clear();
        info!(
            worldgen = ?world.worldgen,
            "world profile uses Milestone A worldgen; waiting for compiled atlas"
        );
        return;
    }

    let source =
        build_density_source_from_prefs(&registry.0, prefs, terrain_tweaks.field_stack_params());
    warn_if_atlas_validation_failed(&source);
    let (sx, sy, sz, _) =
        source.resolve_player_spawn(terrain_generation::PLAYER_SPAWN_MIN_CLEARANCE_M, 48.0);
    spawn_point.0 = Vec3::new(sx, sy, sz);
    let spawn_cell = WorldCell::new(sx.floor() as i32, sy.floor() as i32, sz.floor() as i32);
    pipeline.density_source = Some(Arc::new(source));
    pipeline.spawn_chunk = Some(spawn_cell.chunk_coord());
    runtime.interest_center = spawn_cell.chunk_coord();
    recipe_revision.hash = terrain_recipe_hash(
        &registry.0,
        world_override.as_ref(),
        override_seed,
        Some(prefs),
        Some(&terrain_tweaks.field_stack_params()),
    );
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
    // back appropriately before the prune pass below considers them.
    let mut cancelled = Vec::new();
    pipeline.pending_density.retain(|job| {
        let keep = within_density_radius(center, job.coord, &world_tweaks);
        if !keep {
            cancelled.push(job.coord);
        }
        keep
    });
    pipeline.pending_mesh.retain(|job| {
        let keep = within_render_radius(center, job.coord, &world_tweaks);
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
    let mut cached_coords: HashSet<ChunkCoord> = pipeline.density_cache.keys().copied().collect();
    reset_transient_chunk_states(&mut pipeline.chunks, &cancelled, &cached_coords);

    // Evict density cache and records outside the density radius.
    pipeline
        .density_cache
        .retain(|coord, _| within_density_radius(center, *coord, &world_tweaks));
    cached_coords.retain(|coord| pipeline.density_cache.contains_key(coord));

    // Despawn entities that left the render radius; keep density cache so
    // re-entry only remeshes.
    let mut to_despawn = Vec::new();
    let revision_value = revision.value;
    let mut despawn_coords = Vec::new();
    for (coord, chunk) in &pipeline.chunks {
        if !within_density_radius(center, *coord, &world_tweaks) {
            continue;
        }
        if !within_render_radius(center, *coord, &world_tweaks) && chunk.entity.is_some() {
            despawn_coords.push(*coord);
        }
    }
    for coord in despawn_coords {
        let Some(chunk) = pipeline.chunks.get_mut(&coord) else {
            continue;
        };
        if let Some(entity) = chunk.entity.take() {
            to_despawn.push(entity);
            chunk.state = if cached_coords.contains(&coord) {
                ChunkState::DensityReady
            } else {
                ChunkState::Unrequested
            };
            chunk.revision = revision_value;
        }
    }

    pipeline.chunks.retain(|coord, chunk| {
        if !within_density_radius(center, *coord, &world_tweaks) {
            chunk.state = ChunkState::Unrequested;
            chunk.entity = None;
            chunk.failed_at = None;
            return false;
        }
        true
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

const FAILED_CHUNK_RETRY: Duration = Duration::from_secs(5);

fn retry_failed_chunks(
    runtime: Res<TerrainWorldRuntime>,
    world_tweaks: Res<WorldTweaks>,
    mut pipeline: ResMut<TerrainPipelineState>,
) {
    if pipeline.frozen {
        return;
    }
    let center = runtime.interest_center;
    let now = Instant::now();
    for chunk in pipeline.chunks.values_mut() {
        if chunk.state != ChunkState::Failed {
            continue;
        }
        let Some(failed_at) = chunk.failed_at else {
            chunk.state = ChunkState::Unrequested;
            continue;
        };
        if !within_density_radius(center, chunk.coord, &world_tweaks) {
            continue;
        }
        if now.duration_since(failed_at) >= FAILED_CHUNK_RETRY {
            chunk.state = ChunkState::Unrequested;
            chunk.failed_at = None;
        }
    }
}

fn dispatch_density_jobs(
    registry: Res<ConfigRegistryResource>,
    revision: Res<TerrainRevision>,
    runtime: Res<TerrainWorldRuntime>,
    policy: Res<LodPolicy>,
    world_tweaks: Res<WorldTweaks>,
    edit_store: Res<TerrainEditStore>,
    mut edit_snapshot: ResMut<TerrainEditSnapshot>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut metrics: ResMut<TerrainPipelineMetrics>,
) {
    if pipeline.frozen {
        return;
    }
    let Ok(perf) = registry.0.active_performance() else {
        return;
    };
    let max_jobs = perf.maximum_density_jobs as usize;
    if pipeline.pending_density.len() >= max_jobs {
        return;
    }
    let Some(density_ctx) = pipeline.density_sampling_context(runtime.cell_size_m) else {
        return;
    };

    let slots = max_jobs - pipeline.pending_density.len();
    let mut to_start: Vec<(ChunkCoord, u64)> = Vec::new();
    let center = runtime.interest_center;
    let revision_value = revision.value;

    let try_start = |pipeline: &mut TerrainPipelineState, coord: ChunkCoord| -> bool {
        let record = pipeline.chunks.entry(coord).or_insert_with(|| ChunkRecord {
            coord,
            state: ChunkState::Unrequested,
            revision: revision_value,
            entity: None,
            failed_at: None,
            lod_tier: 0,
            mesh_resolution_scale: 1.0,
            collider_lod: TerrainColliderLodDefinition::Full,
        });
        if record.state != ChunkState::Unrequested {
            return false;
        }
        record.state = ChunkState::GeneratingDensity;
        record.revision = revision_value;
        record.failed_at = None;
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
                if !WorldSimulationLodProvider::should_simulate_chunk(center, coord, &policy) {
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
    candidates.sort_by(|(da, a), (db, b)| {
        da.cmp(db)
            .then_with(|| a.x.cmp(&b.x))
            .then_with(|| a.y.cmp(&b.y))
            .then_with(|| a.z.cmp(&b.z))
    });

    for (_d, coord) in candidates {
        if to_start.len() >= slots {
            break;
        }
        if try_start(&mut pipeline, coord) {
            to_start.push((coord, revision_value));
        }
    }

    if !to_start.is_empty() {
        let edits = edit_snapshot.get(revision.value, &edit_store);
        let cell_size_m = runtime.cell_size_m;
        for (coord, rev) in to_start {
            let edit_overlay = Arc::clone(&edits);
            let started = Instant::now();
            let task = match &density_ctx {
                DensitySamplingContext::Legacy(source) => {
                    let src = Arc::clone(source);
                    AsyncComputeTaskPool::get().spawn(async move {
                        generate_padded_samples_runtime(&src, &edit_overlay, coord, cell_size_m)
                    })
                }
                DensitySamplingContext::Compiled {
                    provider,
                    cell_size_m,
                } => {
                    let provider = Arc::clone(provider);
                    let cell = *cell_size_m;
                    AsyncComputeTaskPool::get().spawn(async move {
                        generate_padded_samples_from_provider(&provider, &edit_overlay, coord, cell)
                    })
                }
            };
            pipeline.pending_density.push(PendingDensityJob {
                coord,
                revision: rev,
                started,
                task,
            });
        }
    }

    metrics.density_queue = pipeline.density_queue_len();
    metrics.mesh_queue = pipeline.mesh_queue_len();
    metrics.upload_queue = pipeline.upload_queue_len();
    metrics.collider_queue = pipeline.collider_queue_len();
}

fn poll_density_jobs(
    revision: Res<TerrainRevision>,
    edit_store: Res<TerrainEditStore>,
    mut edit_snapshot: ResMut<TerrainEditSnapshot>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut metrics: ResMut<TerrainPipelineMetrics>,
) {
    if pipeline.frozen {
        return;
    }
    let mut completed = Vec::new();
    let mut polls = 0usize;
    pipeline.pending_density.retain_mut(|job| {
        if polls >= MAX_TASK_POLLS_PER_FRAME {
            return true;
        }
        polls += 1;
        if let Some(result) = try_poll_task(&mut job.task) {
            completed.push((job.coord, job.revision, job.started, result));
            false
        } else {
            true
        }
    });

    if completed.is_empty() {
        metrics.density_queue = pipeline.density_queue_len();
        metrics.mesh_queue = pipeline.mesh_queue_len();
        return;
    }

    let edit_snapshot = edit_snapshot.get(revision.value, &edit_store);
    for (coord, revision, started, result) in completed {
        metrics.record_density_ms(started.elapsed().as_secs_f32() * 1000.0);
        pipeline.density_cache.insert(
            coord,
            CachedDensity {
                revision,
                samples: result.samples,
                column_cache: result.column_cache,
                edit_snapshot: Arc::clone(&edit_snapshot),
            },
        );
        let Some(chunk) = pipeline.chunks.get_mut(&coord) else {
            continue;
        };
        if chunk.revision != revision {
            pipeline.density_cache.remove(&coord);
            continue;
        }
        chunk.state = ChunkState::DensityReady;
    }

    metrics.density_queue = pipeline.density_queue_len();
    metrics.mesh_queue = pipeline.mesh_queue_len();
}

fn promote_density_ready_to_mesh(
    registry: Res<ConfigRegistryResource>,
    revision: Res<TerrainRevision>,
    runtime: Res<TerrainWorldRuntime>,
    world_tweaks: Res<WorldTweaks>,
    prefs: Res<UserSetupPrefs>,
    biomes: Res<BiomeCatalog>,
    policy: Res<LodPolicy>,
    mut lod_runtime: ResMut<LodRuntimeState>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut metrics: ResMut<TerrainPipelineMetrics>,
) {
    if pipeline.frozen {
        return;
    }
    let Ok(perf) = registry.0.active_performance() else {
        return;
    };
    let max_mesh_jobs = perf.maximum_mesh_jobs as usize;
    if pipeline.pending_mesh.len() >= max_mesh_jobs {
        return;
    }
    let Some(density_ctx) = pipeline.density_sampling_context(runtime.cell_size_m) else {
        return;
    };

    let center = runtime.interest_center;
    let revision_value = revision.value;
    let cell_size_m = runtime.cell_size_m;

    let mut candidates: Vec<(i32, ChunkCoord)> = pipeline
        .chunks
        .iter()
        .filter(|(coord, chunk)| {
            chunk.state == ChunkState::DensityReady
                && chunk.revision == revision_value
                && within_render_radius(center, **coord, &world_tweaks)
                && pipeline.density_cache.contains_key(*coord)
        })
        .map(|(coord, _)| (chunk_chebyshev_distance(center, *coord), *coord))
        .collect();
    let spawn_chunk = pipeline.spawn_chunk;
    candidates.sort_by(|(da, a), (db, b)| {
        let a_spawn = Some(*a) == spawn_chunk;
        let b_spawn = Some(*b) == spawn_chunk;
        match (a_spawn, b_spawn) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => da
                .cmp(db)
                .then_with(|| a.x.cmp(&b.x))
                .then_with(|| a.y.cmp(&b.y))
                .then_with(|| a.z.cmp(&b.z)),
        }
    });

    let slots = max_mesh_jobs - pipeline.pending_mesh.len();
    if slots == 0 || candidates.is_empty() {
        metrics.mesh_queue = pipeline.mesh_queue_len();
        return;
    }

    let world_id = crate::world::requested_world_id(&prefs);
    let Ok(world) = registry.0.effective_world(Some(&world_id)) else {
        return;
    };
    let Some(palette) = registry.0.materials.get(&world.materials).cloned() else {
        return;
    };
    let Some(surface_rules) = registry.0.surface_rules.get(&world.surface).cloned() else {
        return;
    };
    let palette = Arc::new(palette);
    let surface_rules = Arc::new(surface_rules);
    let biome_catalog = Arc::new(biomes.clone());

    for (_d, coord) in candidates.into_iter().take(slots) {
        let (lod_tier, mesh_scale, collider_lod) =
            terrain_lod_with_hysteresis(&policy, &mut lod_runtime, center, coord);
        let needs_collider = within_physics_radius(center, coord, &world_tweaks)
            && collider_lod != TerrainColliderLodDefinition::None;
        let cell_stride = mesh_cell_stride(mesh_scale);

        let cached_job = pipeline.density_cache.get(&coord).and_then(|cached| {
            if cached.revision != revision_value {
                return None;
            }
            Some((
                Arc::clone(&cached.samples),
                Arc::clone(&cached.column_cache),
                Arc::clone(&cached.edit_snapshot),
            ))
        });
        let Some((samples, column_cache, edit_snapshot)) = cached_job else {
            continue;
        };
        let Some(chunk) = pipeline.chunks.get_mut(&coord) else {
            continue;
        };
        if chunk.state != ChunkState::DensityReady {
            continue;
        }
        chunk.lod_tier = lod_tier;
        chunk.mesh_resolution_scale = mesh_scale;
        chunk.collider_lod = collider_lod;
        chunk.state = ChunkState::Meshing;

        let job = MeshPromoteJob {
            coord,
            samples,
            column_cache,
            edit_snapshot,
            needs_collider,
            cell_stride,
        };

        let (ox, oy, oz) = voxel_core::TerrainChunk::new(job.coord).sample_origin();
        let mesh_started = Instant::now();
        let palette_job = Arc::clone(&palette);
        let surface_rules_job = Arc::clone(&surface_rules);
        let biome_catalog_job = Arc::clone(&biome_catalog);
        let cell_stride = job.cell_stride;
        let samples = Arc::clone(&job.samples);
        let column_cache = Arc::clone(&job.column_cache);
        let edit_snapshot = Arc::clone(&job.edit_snapshot);
        let needs_collider = job.needs_collider;
        let mesh_task = match &density_ctx {
            DensitySamplingContext::Legacy(source) => {
                let src = Arc::clone(source);
                AsyncComputeTaskPool::get().spawn(async move {
                    let resolver = ChunkSurfaceResolver::from_compiled(
                        Arc::unwrap_or_clone(src),
                        Arc::unwrap_or_clone(column_cache),
                        ox,
                        oy,
                        oz,
                        cell_size_m,
                        Arc::unwrap_or_clone(edit_snapshot),
                        &palette_job,
                        &surface_rules_job,
                        (*biome_catalog_job).clone(),
                    );
                    build_mesh_with_resolver(
                        &samples,
                        needs_collider,
                        &resolver,
                        cell_size_m,
                        cell_stride,
                    )
                })
            }
            DensitySamplingContext::Compiled { provider, .. } => {
                let provider = Arc::clone(provider);
                AsyncComputeTaskPool::get().spawn(async move {
                    let resolver = ChunkSurfaceResolver::from_world_provider(
                        provider,
                        Arc::unwrap_or_clone(column_cache),
                        ox,
                        oy,
                        oz,
                        cell_size_m,
                        Arc::unwrap_or_clone(edit_snapshot),
                        &palette_job,
                        &surface_rules_job,
                        (*biome_catalog_job).clone(),
                    );
                    build_mesh_with_resolver(
                        &samples,
                        needs_collider,
                        &resolver,
                        cell_size_m,
                        cell_stride,
                    )
                })
            }
        };
        pipeline.pending_mesh.push(PendingMeshJob {
            coord: job.coord,
            revision: revision_value,
            started: mesh_started,
            task: mesh_task,
        });
    }

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
    let mut polls = 0usize;
    pipeline.pending_mesh.retain_mut(|job| {
        if polls >= MAX_TASK_POLLS_PER_FRAME {
            return true;
        }
        polls += 1;
        if let Some(result) = try_poll_task(&mut job.task) {
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
            Ok(job) => job,
            Err(error) => {
                tracing::warn!(?coord, ?error, "terrain chunk meshing failed");
                chunk.state = ChunkState::Failed;
                chunk.failed_at = Some(Instant::now());
                continue;
            }
        };
        if mesh_data.mesh_data.positions.is_empty() || mesh_data.mesh_data.indices.is_empty() {
            chunk.state = ChunkState::Ready;
        } else {
            chunk.state = ChunkState::AwaitingUpload;
            pipeline.upload_queue.push(UploadItem {
                coord,
                revision,
                mesh_data: mesh_data.mesh_data,
                collider: mesh_data.collider,
            });
        }
    }

    metrics.mesh_queue = pipeline.mesh_queue_len();
    metrics.upload_queue = pipeline.upload_queue_len();
}

fn record_chunk_region_coverage(
    region_cache: &mut TerrainRegionPaletteCache,
    layer_order: &[shared::StableId],
    coord: ChunkCoord,
    palette: &terrain_surface::ChunkSlotPalette,
) {
    let region = region_cache.cache.region_for_chunk(coord.x, coord.z);
    let coverage = region_cache.coverage.entry(region).or_default();
    for local in 0..palette.slot_count() {
        if let Some(global) = palette.global_for_local(local) {
            let global = global as usize;
            if let Some(id) = layer_order.get(global) {
                coverage.record(MaterialKey::new(id.as_str()), 1.0);
            }
        }
    }
}

fn upload_chunk_meshes(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    runtime: Res<TerrainWorldRuntime>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut metrics: ResMut<TerrainPipelineMetrics>,
    mut meshes: ResMut<Assets<Mesh>>,
    _materials: ResMut<Assets<terrain_material_bevy::TerrainPbrMaterial>>,
    triplanar_handle: Res<TerrainMaterialHandle>,
    mut region_cache: ResMut<TerrainRegionPaletteCache>,
    prefs: Res<UserSetupPrefs>,
) {
    if pipeline.frozen {
        return;
    }
    let upload_start = Instant::now();
    let Ok(perf) = registry.0.active_performance() else {
        return;
    };
    let mesh_budget = perf.mesh_uploads_per_frame as usize;

    let mut queue = std::mem::take(&mut pipeline.upload_queue);
    if queue.is_empty() {
        pipeline.upload_queue = queue;
        return;
    }

    let center = runtime.interest_center;
    let spawn_chunk = pipeline.spawn_chunk;
    queue.sort_by(|a, b| {
        let a_spawn = Some(a.coord) == spawn_chunk;
        let b_spawn = Some(b.coord) == spawn_chunk;
        match (a_spawn, b_spawn) {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => {
                let da = chunk_chebyshev_distance(center, a.coord);
                let db = chunk_chebyshev_distance(center, b.coord);
                db.cmp(&da)
                    .then_with(|| a.coord.x.cmp(&b.coord.x))
                    .then_with(|| a.coord.y.cmp(&b.coord.y))
                    .then_with(|| a.coord.z.cmp(&b.coord.z))
            }
        }
    });

    let world_id = crate::world::requested_world_id(&prefs);
    let layer_order = registry
        .0
        .effective_world(Some(&world_id))
        .ok()
        .and_then(|world| registry.0.materials.get(&world.materials))
        .map(|materials_def| materials_def.layer_order.clone());

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
        let chunk_palette = item.mesh_data.chunk_palette;
        let mesh = mesh_from_terrain_data(item.mesh_data, cell_size_m);

        let material_template = triplanar_handle.0.clone();

        let entity = commands
            .spawn((
                TerrainChunkEntity { coord: item.coord },
                TerrainChunkMaterial,
                TerrainChunkPalette(chunk_palette),
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material_template),
                chunk_world_transform(item.coord, cell_size_m),
                RigidBody::Static,
                Visibility::default(),
            ))
            .id();
        if let Some(chunk) = pipeline.chunks.get_mut(&item.coord) {
            chunk.entity = Some(entity);
            chunk.state = ChunkState::Ready;
        }
        if let Some(collider) = item.collider {
            pipeline.collider_queue.push(PendingCollider {
                entity,
                coord: item.coord,
                collider,
            });
        }
        if let Some(layer_order) = layer_order.as_deref() {
            record_chunk_region_coverage(
                &mut region_cache,
                layer_order,
                item.coord,
                &chunk_palette,
            );
        }
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

    let Ok(perf) = registry.0.active_performance() else {
        metrics.colliders_built_this_frame = 0;
        return;
    };
    let budget = perf.collider_builds_per_frame as usize;
    let center = runtime.interest_center;
    let mut built = 0u32;
    let render_resident: HashSet<ChunkCoord> = pipeline
        .chunks
        .iter()
        .filter(|(coord, chunk)| {
            chunk.entity.is_some() && within_render_radius(center, **coord, &world_tweaks)
        })
        .map(|(coord, _)| *coord)
        .collect();

    pipeline.collider_queue.retain(|pending| {
        if built as usize >= budget {
            return true;
        }
        if !within_physics_radius(center, pending.coord, &world_tweaks) {
            return render_resident.contains(&pending.coord);
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
    density_cached: &HashSet<ChunkCoord>,
) {
    for coord in cancelled {
        if let Some(chunk) = chunks.get_mut(coord) {
            if chunk.entity.is_some() {
                continue;
            }
            match chunk.state {
                ChunkState::GeneratingDensity => {
                    chunk.state = ChunkState::Unrequested;
                }
                ChunkState::Meshing | ChunkState::AwaitingUpload => {
                    if density_cached.contains(coord) {
                        chunk.state = ChunkState::DensityReady;
                    } else {
                        chunk.state = ChunkState::Unrequested;
                    }
                }
                _ => {}
            }
        }
    }
}

fn sync_chunk_lod_tiers(
    policy: Res<LodPolicy>,
    mut lod_runtime: ResMut<LodRuntimeState>,
    runtime: Res<TerrainWorldRuntime>,
    world_tweaks: Res<WorldTweaks>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut commands: Commands,
) {
    if pipeline.frozen {
        return;
    }
    let center = runtime.interest_center;
    let mut remesh = Vec::new();
    let mut strip_colliders = Vec::new();

    for (coord, chunk) in &pipeline.chunks {
        if chunk.state != ChunkState::Ready {
            continue;
        }
        if !within_render_radius(center, *coord, &world_tweaks) {
            continue;
        }
        let (target_tier, target_scale, collider_lod) =
            terrain_lod_with_hysteresis(&policy, &mut lod_runtime, center, *coord);
        let tier_changed = chunk.lod_tier != target_tier
            || (chunk.mesh_resolution_scale - target_scale).abs() > 0.01;
        if !tier_changed {
            continue;
        }
        if collider_lod == TerrainColliderLodDefinition::None {
            if let Some(entity) = chunk.entity {
                strip_colliders.push(entity);
            }
        }
        remesh.push(*coord);
    }

    for entity in strip_colliders {
        commands.entity(entity).remove::<Collider>();
    }
    for coord in remesh {
        if let Some(chunk) = pipeline.chunks.get_mut(&coord) {
            chunk.state = ChunkState::DensityReady;
        }
    }
}

fn build_chunk_collider(mesh_data: &TerrainMeshData, cell_size_m: f32) -> Option<Collider> {
    if mesh_data.positions.is_empty() || mesh_data.indices.is_empty() {
        return None;
    }
    let positions: Vec<Vec3> = mesh_data
        .positions
        .iter()
        .map(|p| Vec3::from_array(*p) * cell_size_m)
        .collect();
    let tri_indices: Vec<[u32; 3]> = mesh_data
        .indices
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    Some(Collider::trimesh(positions, tri_indices))
}

fn generate_padded_samples_from_provider(
    provider: &Arc<dyn WorldDensityProvider>,
    edits: &Arc<TerrainEditStore>,
    coord: ChunkCoord,
    cell_size_m: f32,
) -> DensityJobResult {
    let (ox, _oy, oz) = voxel_core::TerrainChunk::new(coord).sample_origin();
    let padded_side = CHUNK_CELLS + 3;
    let column_cache =
        ChunkColumnCache::build_from_provider(provider.as_ref(), ox, oz, padded_side, cell_size_m);
    let samples = fill_padded_samples(coord, |wx, wy, wz| {
        if let Some(override_sample) = edits.0.sample_override(wx, wy, wz) {
            (override_sample.density, override_sample.material)
        } else {
            (
                provider.sample_density(WorldPosition::new(
                    (wx as f32 * cell_size_m) as f64,
                    (wy as f32 * cell_size_m) as f64,
                    (wz as f32 * cell_size_m) as f64,
                )),
                voxel_core::MaterialId(0),
            )
        }
    });
    DensityJobResult {
        samples: Arc::from(samples),
        column_cache: Arc::new(column_cache),
    }
}

fn build_mesh_with_resolver(
    samples: &Arc<[voxel_core::TerrainSample]>,
    needs_collider: bool,
    resolver: &ChunkSurfaceResolver,
    cell_size_m: f32,
    cell_stride: u32,
) -> Result<MeshJobResult, terrain_meshing::MeshingError> {
    let mesher = SurfaceNetsMesher;
    let input = ChunkMeshingInput {
        samples,
        chunk_cells: CHUNK_CELLS,
        cell_stride,
        surface_resolver: Some(resolver),
    };
    let mesh_data = mesher.build_mesh(&input)?;
    let collider = if needs_collider {
        build_chunk_collider(&mesh_data, cell_size_m)
    } else {
        None
    };
    Ok(MeshJobResult {
        mesh_data,
        collider,
    })
}

fn generate_padded_samples_runtime(
    source: &Arc<RecipeDensitySource>,
    edits: &Arc<TerrainEditStore>,
    coord: ChunkCoord,
    cell_size_m: f32,
) -> DensityJobResult {
    let (ox, _oy, oz) = voxel_core::TerrainChunk::new(coord).sample_origin();
    let padded_side = CHUNK_CELLS + 3;
    let column_cache = ChunkColumnCache::build(source.as_ref(), ox, oz, padded_side);
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
        samples: Arc::from(samples),
        column_cache: Arc::new(column_cache),
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
    use super::{
        ChunkRecord, generate_padded_samples_runtime, generate_padded_samples_with_biomes,
        reset_transient_chunk_states,
    };
    use crate::environment::biomes::BiomeCatalog;
    use crate::terrain::{ChunkState, TerrainEditStore};
    use bevy::prelude::Entity;
    use game_data::BiomeRuleDefinition;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;
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

    fn single_record(
        coord: ChunkCoord,
        state: ChunkState,
        entity: Option<Entity>,
    ) -> HashMap<ChunkCoord, ChunkRecord> {
        let mut chunks = HashMap::new();
        chunks.insert(
            coord,
            ChunkRecord {
                coord,
                state,
                revision: 1,
                entity,
                failed_at: None,
                lod_tier: 0,
                mesh_resolution_scale: 1.0,
                collider_lod: game_data::TerrainColliderLodDefinition::Full,
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
                cell_stride: 1,
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
        let coord = WorldCell::new(0, surface_y.floor() as i32, 0).chunk_coord();
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
            &Arc::new(source),
            &Arc::new(TerrainEditStore::default()),
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
        let cache = HashSet::new();
        reset_transient_chunk_states(&mut chunks, &[coord], &cache);
        assert_eq!(chunks[&coord].state, ChunkState::Unrequested);
    }

    #[test]
    fn cancelled_mesh_with_density_cache_returns_density_ready() {
        let coord = ChunkCoord::new(3, 0, 0);
        let mut chunks = single_record(coord, ChunkState::Meshing, None);
        let cached = HashSet::from([coord]);
        reset_transient_chunk_states(&mut chunks, &[coord], &cached);
        assert_eq!(chunks[&coord].state, ChunkState::DensityReady);
    }

    #[test]
    fn chunks_with_entities_are_not_reset_on_cancellation() {
        let coord = ChunkCoord::new(3, 0, 0);
        let mut chunks = single_record(coord, ChunkState::Ready, Some(Entity::from_bits(1)));
        let cache = HashSet::new();
        reset_transient_chunk_states(&mut chunks, &[coord], &cache);
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
    fn mesh_promote_candidate_sort_is_deterministic_at_equal_distance() {
        let spawn_chunk = Some(ChunkCoord::new(1, 0, 0));
        let candidates: Vec<(i32, ChunkCoord)> = vec![
            (2, ChunkCoord::new(2, 0, 0)),
            (2, ChunkCoord::new(0, 2, 0)),
            (2, ChunkCoord::new(0, 0, 2)),
            (1, ChunkCoord::new(1, 0, 0)),
            (2, ChunkCoord::new(-2, 0, 0)),
        ];

        let sort_candidates = |candidates: &mut [(i32, ChunkCoord)]| {
            candidates.sort_by(|(da, a), (db, b)| {
                let a_spawn = Some(*a) == spawn_chunk;
                let b_spawn = Some(*b) == spawn_chunk;
                match (a_spawn, b_spawn) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => da
                        .cmp(db)
                        .then_with(|| a.x.cmp(&b.x))
                        .then_with(|| a.y.cmp(&b.y))
                        .then_with(|| a.z.cmp(&b.z)),
                }
            });
        };

        let mut first = candidates.clone();
        sort_candidates(&mut first);
        for _ in 0..32 {
            let mut shuffled = candidates.clone();
            shuffled.reverse();
            sort_candidates(&mut shuffled);
            assert_eq!(
                first, shuffled,
                "equal-distance mesh candidates must sort identically regardless of input order"
            );
        }
    }

    #[test]
    fn upload_queue_sort_is_deterministic_at_equal_distance() {
        let center = ChunkCoord::new(0, 0, 0);
        let spawn_chunk = Some(ChunkCoord::new(1, 0, 0));
        let coords = [
            ChunkCoord::new(2, 0, 0),
            ChunkCoord::new(0, 2, 0),
            ChunkCoord::new(1, 0, 0),
            ChunkCoord::new(0, 0, 2),
        ];

        let sort_coords = |coords: &mut [ChunkCoord]| {
            coords.sort_by(|a, b| {
                let a_spawn = Some(*a) == spawn_chunk;
                let b_spawn = Some(*b) == spawn_chunk;
                match (a_spawn, b_spawn) {
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    _ => {
                        let da = crate::terrain::residency::chunk_chebyshev_distance(center, *a);
                        let db = crate::terrain::residency::chunk_chebyshev_distance(center, *b);
                        db.cmp(&da)
                            .then_with(|| a.x.cmp(&b.x))
                            .then_with(|| a.y.cmp(&b.y))
                            .then_with(|| a.z.cmp(&b.z))
                    }
                }
            });
        };

        let mut first = coords;
        sort_coords(&mut first);
        let mut reversed = [
            ChunkCoord::new(0, 0, 2),
            ChunkCoord::new(1, 0, 0),
            ChunkCoord::new(0, 2, 0),
            ChunkCoord::new(2, 0, 0),
        ];
        sort_coords(&mut reversed);
        assert_eq!(first, reversed);
        assert_eq!(first.last().copied(), spawn_chunk);
    }

    #[test]
    fn collider_queue_keeps_render_resident_chunks_outside_physics_radius() {
        use crate::terrain::TerrainPipelineState;

        let coord = ChunkCoord::new(6, 0, 0);
        let entity = Entity::from_bits(42);
        let mut pipeline = TerrainPipelineState::default();
        pipeline.chunks.insert(
            coord,
            ChunkRecord {
                coord,
                state: ChunkState::Ready,
                revision: 1,
                entity: Some(entity),
                failed_at: None,
                lod_tier: 0,
                mesh_resolution_scale: 1.0,
                collider_lod: game_data::TerrainColliderLodDefinition::Full,
            },
        );

        let center = ChunkCoord::new(0, 0, 0);
        let world_tweaks = crate::ui::WorldTweaks::default();
        let outside_physics =
            !crate::terrain::residency::within_physics_radius(center, coord, &world_tweaks);
        let still_render_resident = pipeline
            .chunks
            .get(&coord)
            .and_then(|chunk| chunk.entity)
            .is_some()
            && crate::terrain::residency::within_render_radius(center, coord, &world_tweaks);

        assert!(
            outside_physics,
            "test coord must sit outside physics radius"
        );
        assert!(
            still_render_resident,
            "test coord must remain inside render radius"
        );

        let retain = |chunks: &HashMap<ChunkCoord, ChunkRecord>, pending_coord: ChunkCoord| {
            if !crate::terrain::residency::within_physics_radius(
                center,
                pending_coord,
                &world_tweaks,
            ) {
                return chunks
                    .get(&pending_coord)
                    .and_then(|chunk| chunk.entity)
                    .is_some()
                    && crate::terrain::residency::within_render_radius(
                        center,
                        pending_coord,
                        &world_tweaks,
                    );
            }
            true
        };

        assert!(
            retain(&pipeline.chunks, coord),
            "pending collider should be retained while chunk entity is render-resident"
        );
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
            &voxel_core::TerrainEditCommand::SubtractSphere { center, radius_m },
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
