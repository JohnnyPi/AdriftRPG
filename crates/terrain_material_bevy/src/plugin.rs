// crates/terrain_material_bevy/src/plugin.rs
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use procedural_textures::TerrainMaterialRecipe;

use crate::bake::{
    TerrainProceduralMaterialState, bake_cpu_arrays, build_material_from_arrays,
    recipe_fingerprint_for, recipes_for_world, try_load_cache, write_cache,
};
use crate::material::TerrainPbrMaterial;

#[derive(Resource, Default)]
pub struct PendingTextureBake {
    pub task: Option<
        Task<(
            procedural_textures::CpuTextureArrays,
            [u8; 32],
            Vec<TerrainMaterialRecipe>,
            Vec<String>,
        )>,
    >,
}

#[derive(Resource, Clone, Default)]
pub struct ProceduralMaterialRecipeOverride {
    pub recipes: Option<Vec<TerrainMaterialRecipe>>,
    pub layer_order: Option<Vec<String>>,
}

pub struct ProceduralTerrainMaterialPlugin {
    pub materials_yaml: Option<std::path::PathBuf>,
}

impl Default for ProceduralTerrainMaterialPlugin {
    fn default() -> Self {
        Self {
            materials_yaml: None,
        }
    }
}

impl Plugin for ProceduralTerrainMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainProceduralMaterialState>()
            .init_resource::<PendingTextureBake>()
            .init_resource::<ProceduralMaterialRecipeOverride>()
            .insert_resource(ProceduralMaterialYamlPath(self.materials_yaml.clone()))
            .add_plugins(MaterialPlugin::<TerrainPbrMaterial>::default())
            .add_systems(
                Startup,
                insert_fallback_material.in_set(FallbackTerrainMaterialSet),
            )
            .add_systems(Update, (start_texture_bake, poll_texture_bake));
    }
}

#[derive(Resource, Clone)]
pub struct ProceduralMaterialYamlPath(pub Option<std::path::PathBuf>);

/// Runs during Startup before the game copies the procedural material handle.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct FallbackTerrainMaterialSet;

fn insert_fallback_material(
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<TerrainPbrMaterial>>,
    mut state: ResMut<TerrainProceduralMaterialState>,
) {
    let handles = crate::arrays::create_placeholder_array_images(&mut images, 8);
    let handle = materials.add(TerrainPbrMaterial {
        albedo_array: handles.albedo.clone(),
        normal_array: handles.normal.clone(),
        ormh_array: handles.ormh.clone(),
        settings: crate::material::TerrainMaterialSettings {
            layer_count: 8,
            debug_mode: 2,
            ..Default::default()
        },
        layer_scales: crate::material::TerrainLayerScales::default(),
        chunk_slots: crate::material::default_chunk_slots(),
    });
    state.material = handle;
    state.arrays = handles;
    state.ready = false;
}

fn resolve_bake_inputs(
    yaml_path: &ProceduralMaterialYamlPath,
    recipe_override: &ProceduralMaterialRecipeOverride,
) -> Option<(Vec<TerrainMaterialRecipe>, Vec<String>)> {
    if let Some(recipes) = recipe_override.recipes.clone() {
        let layer_order = recipe_override
            .layer_order
            .clone()
            .unwrap_or_else(|| recipes.iter().map(|recipe| recipe.id.clone()).collect());
        return Some((recipes, layer_order));
    }
    yaml_path
        .0
        .as_ref()
        .map(|path| recipes_for_world(Some(path)))
}

fn start_texture_bake(
    yaml_path: Res<ProceduralMaterialYamlPath>,
    recipe_override: Res<ProceduralMaterialRecipeOverride>,
    mut pending: ResMut<PendingTextureBake>,
    state: Res<TerrainProceduralMaterialState>,
) {
    if pending.task.is_some() || state.ready {
        return;
    }

    let Some((recipes, layer_order)) = resolve_bake_inputs(&yaml_path, &recipe_override) else {
        return;
    };
    let fingerprint = recipe_fingerprint_for(&recipes);

    if let Some(cached) = try_load_cache(fingerprint) {
        if cached.layers as usize != layer_order.len() {
            warn!(
                "terrain material cache layer count {} != palette {} — rebaking",
                cached.layers,
                layer_order.len()
            );
        } else {
            pending.task = Some(AsyncComputeTaskPool::get().spawn(async move {
                (cached, fingerprint, recipes, layer_order)
            }));
            return;
        }
    }

    pending.task = Some(AsyncComputeTaskPool::get().spawn(async move {
        let arrays = bake_cpu_arrays(&layer_order, &recipes);
        write_cache(fingerprint, &arrays);
        (arrays, fingerprint, recipes, layer_order)
    }));
}

fn poll_texture_bake(
    mut pending: ResMut<PendingTextureBake>,
    mut state: ResMut<TerrainProceduralMaterialState>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<TerrainPbrMaterial>>,
) {
    let Some(mut task) = pending.task.take() else {
        return;
    };

    if let Some((arrays, fingerprint, recipes, layer_order)) =
        bevy::tasks::block_on(bevy::tasks::futures_lite::future::poll_once(&mut task))
    {
        let ordered = crate::bake::ordered_recipes_for_palette(&layer_order, &recipes);
        if arrays.layers as usize != ordered.len() {
            error!(
                "baked texture array layer count {} != ordered palette {} — wrong textures may render",
                arrays.layers,
                ordered.len()
            );
        }
        let handles = crate::arrays::upload_texture_arrays(&arrays, &mut images);
        let material = build_material_from_arrays(&arrays, &ordered, &mut images, &mut materials);
        state.material = material;
        state.arrays = handles;
        state.layer_scales = crate::material::layer_scales_from_recipes(&ordered);
        state.recipe_fingerprint = fingerprint;
        state.ready = true;
    } else {
        pending.task = Some(task);
    }
}
