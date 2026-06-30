use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use std::time::Instant;
use terrain_generation::{iter_world_chunk_coords, RecipeDensitySource};
use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMeshData, TerrainMesher};
use tracing::info;
use voxel_core::{ChunkCoord, WorldCell, CHUNK_CELLS};

use crate::data::ConfigRegistryResource;
use crate::environment::materials::material_for_world;
use crate::environment::BiomeCatalog;
use crate::state::AppState;
use crate::terrain::material::TerrainTriplanarMaterial;
use crate::terrain::mesh_convert::{chunk_world_transform, mesh_from_terrain_data};
use crate::terrain::metrics::{TerrainPipelineMetrics, WorldSeedOverride};
use crate::terrain::recipe::{build_density_source, terrain_recipe_hash};
use crate::terrain::{ChunkState, TerrainChunkEntity, TerrainRevision};

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainRevision>()
            .init_resource::<TerrainPipelineState>()
            .init_resource::<TerrainSpawnPoint>()
            .init_resource::<TerrainRecipeRevision>()
            .init_resource::<TerrainPipelineMetrics>()
            .init_resource::<WorldSeedOverride>()
            .add_systems(OnEnter(AppState::Running), init_terrain_world)
            .add_systems(
                Update,
                (
                    sync_terrain_on_recipe_change,
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
    collider: Collider,
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
    mut pipeline: ResMut<TerrainPipelineState>,
    mut spawn_point: ResMut<TerrainSpawnPoint>,
    mut recipe_revision: ResMut<TerrainRecipeRevision>,
    mut seed_override: ResMut<WorldSeedOverride>,
    revision: Res<TerrainRevision>,
) {
    let world = registry.0.active_world().expect("world");
    seed_override.seed = world.seed;
    let override_seed = seed_override_active(&seed_override, world.seed);
    let source = build_density_source(&registry.0, override_seed);
    let (sx, sy, sz) = source.spawn_position();
    spawn_point.0 = Vec3::new(sx, sy, sz);
    pipeline.spawn_chunk = Some(
        WorldCell::new(sx.floor() as i32, sy.floor() as i32, sz.floor() as i32).chunk_coord(),
    );

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
    recipe_revision.hash = terrain_recipe_hash(&registry.0, override_seed);

    info!(chunk_count = pipeline.chunks.len(), "terrain world initialized");
}

fn sync_terrain_on_recipe_change(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut recipe_revision: ResMut<TerrainRecipeRevision>,
    mut revision: ResMut<TerrainRevision>,
    seed_override: Res<WorldSeedOverride>,
) {
    let world = registry.0.active_world().expect("world");
    let override_seed = seed_override_active(&seed_override, world.seed);
    let hash = terrain_recipe_hash(&registry.0, override_seed);
    if recipe_revision.hash == hash {
        return;
    }
    recipe_revision.hash = hash.clone();
    revision.value += 1;
    let source = build_density_source(&registry.0, override_seed);
    let (sx, sy, sz) = source.spawn_position();
    pipeline.density_source = Some(source);
    pipeline.spawn_chunk = Some(
        WorldCell::new(sx.floor() as i32, sy.floor() as i32, sz.floor() as i32).chunk_coord(),
    );
    let to_despawn = pipeline.reset_for_revision(revision.value);
    for entity in to_despawn {
        commands.entity(entity).despawn();
    }
    info!(revision = revision.value, recipe_hash = %hash, "terrain recipe changed; regenerating");
}

/// Rebuild density from current seed override and bump terrain revision.
pub fn regen_terrain_with_seed(
    commands: &mut Commands,
    registry: &ConfigRegistryResource,
    pipeline: &mut TerrainPipelineState,
    recipe_revision: &mut TerrainRecipeRevision,
    revision: &mut TerrainRevision,
    seed_override: &WorldSeedOverride,
    spawn_point: &mut TerrainSpawnPoint,
) {
    let world = registry.0.active_world().expect("world");
    let override_seed = seed_override_active(seed_override, world.seed);
    revision.value += 1;
    let source = build_density_source(&registry.0, override_seed);
    let (sx, sy, sz) = source.spawn_position();
    spawn_point.0 = Vec3::new(sx, sy, sz);
    pipeline.density_source = Some(source);
    pipeline.spawn_chunk = Some(
        WorldCell::new(sx.floor() as i32, sy.floor() as i32, sz.floor() as i32).chunk_coord(),
    );
    recipe_revision.hash = terrain_recipe_hash(&registry.0, override_seed);
    let to_despawn = pipeline.reset_for_revision(revision.value);
    for entity in to_despawn {
        commands.entity(entity).despawn();
    }
}

fn dispatch_density_jobs(
    registry: Res<ConfigRegistryResource>,
    revision: Res<TerrainRevision>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut metrics: ResMut<TerrainPipelineMetrics>,
    biomes: Res<BiomeCatalog>,
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
    let mut to_start = Vec::new();

    if let Some(spawn_chunk) = pipeline.spawn_chunk {
        if let Some(chunk) = pipeline
            .chunks
            .iter_mut()
            .find(|c| c.coord == spawn_chunk && c.state == ChunkState::Unrequested)
        {
            chunk.state = ChunkState::GeneratingDensity;
            to_start.push((chunk.coord, chunk.revision));
        }
    }

    for chunk in &mut pipeline.chunks {
        if to_start.len() >= slots {
            break;
        }
        if chunk.state != ChunkState::Unrequested || chunk.revision != revision.value {
            continue;
        }
        chunk.state = ChunkState::GeneratingDensity;
        to_start.push((chunk.coord, chunk.revision));
    }

    let biome_catalog = biomes.clone();
    for (coord, rev) in to_start {
        let src = source.clone();
        let catalog = biome_catalog.clone();
        let started = Instant::now();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            generate_padded_samples_with_biomes(&src, &catalog, coord)
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
        let mesh_task = AsyncComputeTaskPool::get().spawn(async move {
            let mesher = SurfaceNetsMesher;
            let input = ChunkMeshingInput {
                samples: &samples,
                chunk_cells: CHUNK_CELLS,
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
        if mesh_data.positions.is_empty() {
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
    mut pipeline: ResMut<TerrainPipelineState>,
    mut metrics: ResMut<TerrainPipelineMetrics>,
    mut meshes: ResMut<Assets<Mesh>>,
    triplanar_handle: Option<Res<crate::terrain::material::TerrainMaterialHandle>>,
    mut triplanar_materials: ResMut<Assets<TerrainTriplanarMaterial>>,
) {
    if pipeline.frozen {
        return;
    }
    let upload_start = Instant::now();
    let perf = registry.0.active_performance().expect("performance");
    let mesh_budget = perf.mesh_uploads_per_frame as usize;

    let mut queue = std::mem::take(&mut pipeline.upload_queue);
    if let Some(spawn_chunk) = pipeline.spawn_chunk {
        queue.sort_by_key(|item| item.coord != spawn_chunk);
    }

    let material = triplanar_handle
        .as_ref()
        .map(|h| h.0.clone())
        .unwrap_or_else(|| triplanar_materials.add(TerrainTriplanarMaterial::default_catalog()));

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

        let mesh = mesh_from_terrain_data(&item.mesh_data);
        let positions: Vec<Vec3> = item
            .mesh_data
            .positions
            .iter()
            .map(|p| Vec3::from_array(*p))
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
                chunk_world_transform(item.coord),
                RigidBody::Static,
                Visibility::default(),
            ))
            .id();
        pipeline.chunks[chunk_idx].entity = Some(entity);
        pipeline.chunks[chunk_idx].state = ChunkState::Ready;
        pipeline.collider_queue.push(PendingCollider { entity, collider });
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
    mut pipeline: ResMut<TerrainPipelineState>,
    mut metrics: ResMut<TerrainPipelineMetrics>,
) {
    if pipeline.frozen || pipeline.collider_queue.is_empty() {
        metrics.colliders_built_this_frame = 0;
        return;
    }

    let perf = registry.0.active_performance().expect("performance");
    let budget = perf.collider_builds_per_frame as usize;
    let mut built = 0u32;

    pipeline.collider_queue.retain(|pending| {
        if built as usize >= budget {
            return true;
        }
        commands.entity(pending.entity).insert(pending.collider.clone());
        built += 1;
        false
    });

    metrics.colliders_built_this_frame = built;
    metrics.collider_queue = pipeline.collider_queue_len();
}

fn generate_padded_samples_with_biomes(
    source: &RecipeDensitySource,
    biomes: &BiomeCatalog,
    coord: ChunkCoord,
) -> Vec<voxel_core::TerrainSample> {
    let cells = voxel_core::CHUNK_CELLS;
    let (ox, oy, oz) = voxel_core::TerrainChunk::new(coord).sample_origin();
    let sea_level = source.recipe().sea_level;
    let mut samples = Vec::with_capacity((cells + 3).pow(3));
    for pz in -1..=(cells as i32 + 1) {
        for py in -1..=(cells as i32 + 1) {
            for px in -1..=(cells as i32 + 1) {
                let wx = ox + px;
                let wy = oy + py;
                let wz = oz + pz;
                let density = source.density_at(wx as f32, wy as f32, wz as f32);
                let material = material_for_world(
                    biomes,
                    sea_level,
                    wx as f32,
                    wy as f32,
                    wz as f32,
                    density,
                );
                samples.push(voxel_core::TerrainSample { density, material });
            }
        }
    }
    samples
}
