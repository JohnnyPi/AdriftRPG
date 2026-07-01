use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use std::time::Instant;
use terrain_generation::{iter_world_chunk_coords, RecipeDensitySource};
use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMeshData, TerrainMesher};
use tracing::info;
use voxel_core::{ChunkCoord, WorldCell, CHUNK_CELLS};

use crate::data::ConfigRegistryResource;
use crate::data::{sync_world_tweaks_from_prefs, UserSetupPrefs};
use crate::terrain::residency::{
    within_density_radius, within_physics_radius, within_render_radius, TerrainWorldRuntime,
};
use crate::ui::{TerrainTweaks, WorldTweaks};
use physics_bridge::terrain_layers;
use crate::environment::biome_context::ChunkColumnCache;
use crate::environment::materials::material_for_world_with_cache;
use crate::environment::{BiomeCatalog, BiomeInitSet};
use crate::state::AppState;
use crate::environment::surface::ChunkSurfaceResolver;
use crate::terrain::material::TerrainMaterialHandle;
use crate::terrain::mesh_convert::{chunk_world_transform, mesh_from_terrain_data};
use crate::terrain::metrics::{TerrainPipelineMetrics, WorldSeedOverride};
use crate::terrain::recipe::{build_density_source_from_prefs, terrain_recipe_hash};
use crate::terrain::{ChunkState, TerrainChunkEntity, TerrainEditStore, TerrainRevision};

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
    pub chunks: Vec<ChunkRecord>,
    pub frozen: bool,
    pending_density: Vec<PendingDensityJob>,
    pending_mesh: Vec<PendingMeshJob>,
    upload_queue: Vec<UploadItem>,
    collider_queue: Vec<PendingCollider>,
}

impl TerrainPipelineState {
    /// Clears pending work and returns chunk entities that should be despawned.
    pub fn reset_for_revision(&mut self, revision: u64) -> Vec<Entity> {
        let mut to_despawn = Vec::new();
        for chunk in &mut self.chunks {
            if let Some(entity) = chunk.entity.take() {
                to_despawn.push(entity);
            }
            chunk.state = ChunkState::Unrequested;
            chunk.revision = revision;
        }
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
        for chunk in &mut self.chunks {
            if coords.contains(&chunk.coord) {
                if let Some(entity) = chunk.entity.take() {
                    to_despawn.push(entity);
                }
                chunk.state = ChunkState::Unrequested;
                chunk.revision = revision;
            }
        }
        self.pending_density.retain(|job| !coords.contains(&job.coord));
        self.pending_mesh.retain(|job| !coords.contains(&job.coord));
        self.upload_queue.retain(|item| !coords.contains(&item.coord));
        self.collider_queue.retain(|pending| {
            !to_despawn.contains(&pending.entity)
        });
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
    task: Task<Vec<voxel_core::TerrainSample>>,
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

fn init_terrain_world(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    mut world_tweaks: ResMut<WorldTweaks>,
    terrain_tweaks: Res<TerrainTweaks>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut spawn_point: ResMut<TerrainSpawnPoint>,
    mut recipe_revision: ResMut<TerrainRecipeRevision>,
    mut seed_override: ResMut<WorldSeedOverride>,
    mut runtime: ResMut<TerrainWorldRuntime>,
    revision: Res<TerrainRevision>,
) {
    sync_world_tweaks_from_prefs(&prefs, &mut world_tweaks);
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
    let source = build_density_source_from_prefs(
        &registry.0,
        &prefs,
        terrain_tweaks.field_stack_params(),
    );
    let (sx, sy, sz, spawn_report) = source.resolve_player_spawn(
        terrain_generation::PLAYER_SPAWN_MIN_CLEARANCE_M,
        48.0,
    );
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
    pipeline.chunks = iter_world_chunk_coords(extent)
        .map(|coord| ChunkRecord {
            coord,
            state: ChunkState::Unrequested,
            revision: revision.value,
            entity: None,
        })
        .collect();
    recipe_revision.hash = terrain_recipe_hash(&registry.0, world_override.as_ref(), override_seed);

    info!(
        chunk_count = pipeline.chunks.len(),
        world = %world.id.as_str(),
        spawn = ?spawn_point.0,
        spawn_valid = spawn_report.passed,
        spawn_notes = ?spawn_report.messages,
        "terrain world initialized"
    );
}

fn sync_terrain_on_recipe_change(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
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
    let hash = terrain_recipe_hash(&registry.0, world_override.as_ref(), override_seed);
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
    let source = build_density_source_from_prefs(
        &registry.0,
        prefs,
        terrain_tweaks.field_stack_params(),
    );
    let (sx, sy, sz) = source.spawn_position();
    spawn_point.0 = Vec3::new(sx, sy, sz);
    let spawn_cell = WorldCell::new(sx.floor() as i32, sy.floor() as i32, sz.floor() as i32);
    pipeline.density_source = Some(source);
    pipeline.spawn_chunk = Some(spawn_cell.chunk_coord());
    runtime.interest_center = spawn_cell.chunk_coord();
    recipe_revision.hash = terrain_recipe_hash(&registry.0, world_override.as_ref(), override_seed);
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
    let mut to_despawn = Vec::new();
    for chunk in &mut pipeline.chunks {
        if !within_render_radius(center, chunk.coord, &world_tweaks) {
            if let Some(entity) = chunk.entity.take() {
                to_despawn.push(entity);
                chunk.state = ChunkState::Unrequested;
                chunk.revision = revision.value;
            }
        }
    }
    for entity in to_despawn {
        commands.entity(entity).despawn();
    }
    pipeline
        .pending_density
        .retain(|job| within_density_radius(center, job.coord, &world_tweaks));
    pipeline
        .pending_mesh
        .retain(|job| within_density_radius(center, job.coord, &world_tweaks));
    pipeline
        .upload_queue
        .retain(|item| within_render_radius(center, item.coord, &world_tweaks));
}

fn dispatch_density_jobs(
    registry: Res<ConfigRegistryResource>,
    revision: Res<TerrainRevision>,
    runtime: Res<TerrainWorldRuntime>,
    world_tweaks: Res<WorldTweaks>,
    edit_store: Res<TerrainEditStore>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut metrics: ResMut<TerrainPipelineMetrics>,
    biomes: Option<Res<BiomeCatalog>>,
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

    let biome_catalog = biomes.as_deref().cloned().unwrap_or_default();

    let slots = max_jobs - pipeline.pending_density.len();
    let mut to_start = Vec::new();

    if let Some(spawn_chunk) = pipeline.spawn_chunk {
        if within_density_radius(runtime.interest_center, spawn_chunk, &world_tweaks) {
            if let Some(chunk) = pipeline
                .chunks
                .iter_mut()
                .find(|c| c.coord == spawn_chunk && c.state == ChunkState::Unrequested)
            {
                chunk.state = ChunkState::GeneratingDensity;
                to_start.push((chunk.coord, chunk.revision));
            }
        }
    }

    let mut candidates: Vec<_> = pipeline
        .chunks
        .iter()
        .filter(|c| {
            c.state == ChunkState::Unrequested
                && c.revision == revision.value
                && within_density_radius(runtime.interest_center, c.coord, &world_tweaks)
        })
        .map(|c| {
            let d = crate::terrain::residency::chunk_chebyshev_distance(
                runtime.interest_center,
                c.coord,
            );
            (d, c.coord, c.revision)
        })
        .collect();
    candidates.sort_by_key(|(d, _, _)| *d);

    for (d, coord, rev) in candidates {
        if to_start.len() >= slots {
            break;
        }
        let _ = d;
        if let Some(chunk) = pipeline.chunks.iter_mut().find(|c| c.coord == coord) {
            chunk.state = ChunkState::GeneratingDensity;
            to_start.push((coord, rev));
        }
    }

    let catalog_for_jobs = biome_catalog.clone();
    let edits = edit_store.clone();
    let cell_size_m = runtime.cell_size_m;
    for (coord, rev) in to_start {
        let src = source.clone();
        let catalog = catalog_for_jobs.clone();
        let edit_overlay = edits.clone();
        let started = Instant::now();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            generate_padded_samples_with_biomes(
                &src,
                &catalog,
                &edit_overlay,
                coord,
                cell_size_m,
            )
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
) {
    if pipeline.frozen {
        return;
    }
    let mut completed = Vec::new();
    pipeline.pending_density.retain_mut(|job| {
        if let Some(samples) =
            bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(&mut job.task))
        {
            completed.push((job.coord, job.revision, job.started, samples));
            false
        } else {
            true
        }
    });

    for (coord, revision, started, samples) in completed {
        metrics.record_density_ms(started.elapsed().as_secs_f32() * 1000.0);
        let Some(chunk) = pipeline.chunks.iter_mut().find(|c| c.coord == coord) else {
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
        let padded_side = CHUNK_CELLS + 3;
        let mesh_task = AsyncComputeTaskPool::get().spawn(async move {
            let resolver =
                ChunkSurfaceResolver::new(source, ox, oy, oz, padded_side, cell_size_m);
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
        let Some(chunk) = pipeline.chunks.iter_mut().find(|c| c.coord == coord) else {
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
    triplanar_handle: Res<TerrainMaterialHandle>,
) {
    if pipeline.frozen {
        return;
    }
    let upload_start = Instant::now();
    let perf = registry.0.active_performance().expect("performance");
    let mesh_budget = perf.mesh_uploads_per_frame as usize;

    let mut queue = std::mem::take(&mut pipeline.upload_queue);
    if let Some(spawn_chunk) = pipeline.spawn_chunk {
        // pop() takes from the back, so keep the spawn chunk at the end.
        queue.sort_by_key(|item| item.coord == spawn_chunk);
    }

    let material = triplanar_handle.0.clone();

    let mut uploaded = 0usize;
    for _ in 0..mesh_budget {
        let Some(item) = queue.pop() else {
            break;
        };
        let Some(chunk_idx) = pipeline.chunks.iter().position(|c| c.coord == item.coord) else {
            continue;
        };
        if pipeline.chunks[chunk_idx].revision != item.revision {
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

        let entity = commands
            .spawn((
                TerrainChunkEntity { coord: item.coord },
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material.clone()),
                chunk_world_transform(item.coord, cell_size_m),
                RigidBody::Static,
                Visibility::default(),
            ))
            .id();
        pipeline.chunks[chunk_idx].entity = Some(entity);
        pipeline.chunks[chunk_idx].state = ChunkState::Ready;
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
                let (density, material) = if let Some(override_sample) =
                    edits.0.sample_override(wx, wy, wz)
                {
                    (override_sample.density, override_sample.material)
                } else {
                    let density = source.density_at(wx_m, wy_m, wz_m);
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
    use crate::terrain::TerrainEditStore;
    use super::generate_padded_samples_with_biomes;
    use crate::environment::BiomeCatalog;
    use game_data::BiomeRuleDefinition;
    use terrain_generation::{default_vertical_slice_recipe, RecipeDensitySource};
    use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMesher};

    fn test_catalog() -> BiomeCatalog {
        BiomeCatalog {
            rules: vec![BiomeRuleDefinition::new("grassland", 0, [0.34, 0.52, 0.28])],
        }
    }

    #[test]
    fn spawn_area_chunk_meshes_with_vertices() {
        let source = RecipeDensitySource::new(default_vertical_slice_recipe(42, 2.0));
        let (sx, sy, sz) = source.spawn_position();
        let coord = voxel_core::WorldCell::new(
            sx.floor() as i32,
            sy.floor() as i32,
            sz.floor() as i32,
        )
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
        assert!(!mesh.material_ids.is_empty());
        assert!(!mesh.material_weights.is_empty());
    }
}
