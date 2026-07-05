//! Bevy integration for Milestone A world compiler.

use std::path::PathBuf;
use std::sync::Arc;

use bevy::prelude::*;
use game_data::{WorldgenLoadError, load_worldgen_bundle, resolve_world_bundle};
use terrain_generation::{
    CompileOptions, CompiledWorld, VolumetricWorldProvider, WorldDensityProvider,
    compile_world_from_bundle,
};

use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;
use crate::data::assets_root;
use crate::state::AppState;
use crate::terrain::{
    TerrainPipelineState, TerrainRecipeRevision, TerrainSpawnPoint, TerrainWorldInitSet,
    TerrainWorldRuntime,
};

/// Request background compilation of a world recipe by ID.
#[derive(Message)]
pub struct RequestWorldCompilation {
    pub world_id: String,
    pub assets_root: PathBuf,
}

#[derive(Component)]
pub struct WorldCompilationTask {
    pub task: bevy::tasks::Task<Result<CompiledWorld, terrain_generation::WorldgenError>>,
}

#[derive(Resource)]
pub struct ActiveCompiledWorld {
    pub world_id: String,
    pub world: Arc<CompiledWorld>,
    pub provider: Arc<dyn WorldDensityProvider>,
    pub recipe_hash: String,
    pub hydrology_products: Option<terrain_generation::CompiledHydrologyProducts>,
}

#[derive(Resource)]
pub struct WorldCompilationConfig {
    pub enabled: bool,
    pub world_id: String,
    pub assets_root: PathBuf,
}

impl Default for WorldCompilationConfig {
    fn default() -> Self {
        Self {
            // Enable when the active world YAML sets `worldgen: world.*`.
            enabled: false,
            world_id: String::new(),
            assets_root: assets_root().join("worldgen"),
        }
    }
}

#[derive(Resource, Default)]
struct WorldgenHotReloadState {
    last_bundle_fingerprint: Option<String>,
}

pub struct WorldgenPlugin;

impl Plugin for WorldgenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldCompilationConfig>()
            .init_resource::<WorldgenHotReloadState>()
            .add_message::<RequestWorldCompilation>()
            .add_systems(Startup, bootstrap_world_compilation)
            .add_systems(
                OnEnter(AppState::Running),
                sync_worldgen_config_from_prefs.before(TerrainWorldInitSet),
            )
            .add_systems(
                Update,
                (
                    sync_worldgen_config_from_prefs,
                    begin_world_compilation,
                    poll_world_compilation,
                    watch_worldgen_yaml_changes,
                    draw_worldgen_debug_gizmos,
                )
                    .run_if(in_state(AppState::Running)),
            );
    }
}

fn bootstrap_world_compilation(mut commands: Commands, config: Res<WorldCompilationConfig>) {
    if !config.enabled || config.world_id.is_empty() {
        return;
    }
    commands.write_message(RequestWorldCompilation {
        world_id: config.world_id.clone(),
        assets_root: config.assets_root.clone(),
    });
}

/// Enable Milestone A worldgen when the active presentation world declares `worldgen:`.
pub fn sync_worldgen_config_from_prefs(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    mut config: ResMut<WorldCompilationConfig>,
    mut requests: MessageWriter<RequestWorldCompilation>,
    mut last: Local<Option<(String, String)>>,
) {
    let Ok(world) = registry.0.effective_world(Some(&prefs.world_stable_id())) else {
        return;
    };
    let Some(worldgen_id) = world.worldgen.as_ref() else {
        if config.enabled {
            config.enabled = false;
            config.world_id.clear();
            *last = None;
        }
        return;
    };
    let recipe_id = worldgen_id.as_str().to_string();
    let presentation_id = prefs.world_id.clone();
    config.enabled = true;
    config.world_id = recipe_id.clone();
    let key = (presentation_id, recipe_id);
    if last.as_ref() != Some(&key) {
        *last = Some(key);
        requests.write(RequestWorldCompilation {
            world_id: config.world_id.clone(),
            assets_root: config.assets_root.clone(),
        });
    }
}

fn begin_world_compilation(
    mut commands: Commands,
    mut requests: MessageReader<RequestWorldCompilation>,
) {
    let pool = bevy::tasks::AsyncComputeTaskPool::get();
    for request in requests.read() {
        let world_id = request.world_id.clone();
        let assets_root = request.assets_root.clone();
        let task = pool.spawn(async move {
            let bundle = load_worldgen_bundle(&assets_root).map_err(map_load_error)?;
            let resolved = resolve_world_bundle(&world_id, &bundle)
                .map_err(terrain_generation::WorldgenError::GameData)?;
            compile_world_from_bundle(&resolved, &CompileOptions::default())
        });
        commands.spawn(WorldCompilationTask { task });
    }
}

fn poll_world_compilation(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut WorldCompilationTask)>,
) {
    for (entity, mut task) in &mut tasks {
        if let Some(result) =
            bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(&mut task.task))
        {
            match result {
                Ok(world) => {
                    let recipe_hash = world.manifest.recipe_hash.hex();
                    let world_id = world.manifest.world_id.clone();
                    let provider = VolumetricWorldProvider::from_compiled(&world).into_arc();
                    let hydrology_products = world.atlas.graphs.hydrology_products.clone();
                    commands.insert_resource(ActiveCompiledWorld {
                        world_id,
                        world: Arc::new(world),
                        provider,
                        recipe_hash,
                        hydrology_products,
                    });
                }
                Err(error) => {
                    bevy::log::error!("world compilation failed: {error}");
                }
            }
            commands.entity(entity).despawn();
        }
    }
}

/// Install compiled-world density into the terrain pipeline once compilation completes.
pub fn apply_compiled_world_to_pipeline(
    registry: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
    config: Res<WorldCompilationConfig>,
    active: Option<Res<ActiveCompiledWorld>>,
    mut pipeline: ResMut<TerrainPipelineState>,
    mut spawn_point: ResMut<TerrainSpawnPoint>,
    mut recipe_revision: ResMut<TerrainRecipeRevision>,
    mut runtime: ResMut<TerrainWorldRuntime>,
) {
    if !config.enabled {
        return;
    }
    let Ok(world) = registry.0.effective_world(Some(&prefs.world_stable_id())) else {
        return;
    };
    let Some(expected) = world.worldgen.as_ref() else {
        return;
    };
    let Some(active) = active else {
        return;
    };
    if active.world_id != expected.as_str() {
        return;
    }
    if pipeline.world_density_provider.is_some()
        && !pipeline.frozen
        && recipe_revision.hash == active.recipe_hash
    {
        return;
    }
    if recipe_revision.hash == active.recipe_hash && pipeline.world_density_provider.is_some() {
        pipeline.frozen = false;
        return;
    }
    let cell_size_m = runtime.cell_size_m.max(0.001);
    crate::terrain::finish_compiled_world_install(
        Arc::clone(&active.provider),
        &mut pipeline,
        &mut spawn_point,
        &mut recipe_revision,
        &mut runtime,
        active.recipe_hash.clone(),
        cell_size_m,
    );
}

fn watch_worldgen_yaml_changes(
    config: Res<WorldCompilationConfig>,
    mut reload: ResMut<WorldgenHotReloadState>,
    mut requests: MessageWriter<RequestWorldCompilation>,
) {
    if !config.enabled {
        return;
    }
    let fingerprint = worldgen_bundle_fingerprint(&config.assets_root);
    if reload.last_bundle_fingerprint.is_none() {
        reload.last_bundle_fingerprint = Some(fingerprint);
        return;
    }
    if reload.last_bundle_fingerprint.as_ref() == Some(&fingerprint) {
        return;
    }
    reload.last_bundle_fingerprint = Some(fingerprint);
    requests.write(RequestWorldCompilation {
        world_id: config.world_id.clone(),
        assets_root: config.assets_root.clone(),
    });
}

fn worldgen_bundle_fingerprint(root: &std::path::Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    for subdir in [
        "worlds",
        "boundaries",
        "islands",
        "geology",
        "refinement",
        "validation",
    ] {
        let dir = root.join(subdir);
        if !dir.exists() {
            continue;
        }
        let mut entries: Vec<_> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        entries.sort();
        for path in entries {
            path.to_string_lossy().hash(&mut hasher);
            if let Ok(bytes) = std::fs::read(&path) {
                bytes.hash(&mut hasher);
            }
        }
    }
    format!("{:016x}", hasher.finish())
}

fn draw_worldgen_debug_gizmos(
    config: Res<WorldCompilationConfig>,
    active: Option<Res<ActiveCompiledWorld>>,
    mut gizmos: Gizmos,
) {
    if !config.enabled {
        return;
    }
    let Some(active) = active else {
        return;
    };
    let extent = active.world.manifest.extent;
    let hw = extent.half_width() as f32;
    let hd = extent.half_depth() as f32;
    let y = extent.sea_level_m + 2.0;
    let corners = [
        Vec3::new(-hw, y, -hd),
        Vec3::new(hw, y, -hd),
        Vec3::new(hw, y, hd),
        Vec3::new(-hw, y, hd),
    ];
    for window in corners.windows(2) {
        gizmos.line(window[0], window[1], Color::srgb(0.2, 0.7, 1.0));
    }
    gizmos.line(corners[3], corners[0], Color::srgb(0.2, 0.7, 1.0));
}

fn map_load_error(error: WorldgenLoadError) -> terrain_generation::WorldgenError {
    terrain_generation::WorldgenError::Validation(error.to_string())
}
