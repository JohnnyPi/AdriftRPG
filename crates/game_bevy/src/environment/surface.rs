// crates/game_bevy/src/environment/surface.rs
use std::sync::{Arc, Mutex};

use game_data::{CompiledSurfaceRules, CompiledTerrainMaterials};
use terrain_generation::{
    DensitySource, RecipeDensitySource, WorldDensityProvider, WorldPosition, WorldXZ,
};
use terrain_surface::{
    ChunkSlotPalette, ChunkSlotRemapper, EnvironmentSample, GeologyId, MaterialKey,
    MaterialLayerRegistry, MaterialVertex, RuleSurfaceClassifier, SurfaceClassifier,
    SurfaceContext, SurfaceMeshResolver, SurfaceRuleSet, compute_overlay_state,
    compiler_biome_to_presentation, merge_soft_biome_sources, overlay_response_for_material_name,
    remap_blend_to_local_slots, resolve_blend, soft_weights_from_blend_cell,
    soft_weights_from_primary_u8,
};

use super::biome_context::ChunkColumnCache;
use super::biomes::{biome_surface_tint, biome_surface_tint_from_soft};
use super::materials::terrain_material_key_from_paint_material;
use crate::environment::BiomeCatalog;
use crate::terrain::TerrainEditStore;

const ATLAS_BIOME_MIX: f32 = 0.30;
const COMPILER_BIOME_MIX: f32 = 0.85;

fn wetness_normalization(source: &RecipeDensitySource) -> f32 {
    source
        .atlas()
        .map(|atlas| atlas.max_wetness())
        .unwrap_or_else(terrain_surface::default_wetness_normalization)
        .max(1.0)
}
const PAINT_DENSITY_MATCH_EPS: f32 = 0.001;

enum TerrainDensityBackend {
    Recipe(RecipeDensitySource),
    Compiled(Arc<dyn WorldDensityProvider>),
}

impl TerrainDensityBackend {
    fn sample_density_m(&self, wx_m: f32, wy_m: f32, wz_m: f32) -> f32 {
        match self {
            Self::Recipe(source) => source.sample_density(wx_m, wy_m, wz_m),
            Self::Compiled(provider) => {
                provider.sample_density(WorldPosition::new(wx_m as f64, wy_m as f64, wz_m as f64))
            }
        }
    }

    fn atlas(&self) -> Option<&terrain_generation::IslandAtlas> {
        match self {
            Self::Recipe(source) => source.atlas(),
            Self::Compiled(_) => None,
        }
    }

    fn recipe_source(&self) -> Option<&RecipeDensitySource> {
        match self {
            Self::Recipe(source) => Some(source),
            Self::Compiled(_) => None,
        }
    }

    fn provider(&self) -> Option<&dyn WorldDensityProvider> {
        match self {
            Self::Recipe(_) => None,
            Self::Compiled(provider) => Some(provider.as_ref()),
        }
    }
}

pub struct ChunkSurfaceResolver {
    backend: TerrainDensityBackend,
    use_provider_context: bool,
    column_cache: ChunkColumnCache,
    layer_registry: MaterialLayerRegistry,
    classifier: RuleSurfaceClassifier,
    slot_remapper: Mutex<ChunkSlotRemapper>,
    edit_store: TerrainEditStore,
    biome_catalog: BiomeCatalog,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    cell_size_m: f32,
    sea_level_m: f32,
}

impl ChunkSurfaceResolver {
    pub fn from_compiled(
        source: RecipeDensitySource,
        column_cache: ChunkColumnCache,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        cell_size_m: f32,
        edit_store: TerrainEditStore,
        palette: &CompiledTerrainMaterials,
        rules: &CompiledSurfaceRules,
        biome_catalog: BiomeCatalog,
    ) -> Self {
        let layer_order: Vec<MaterialKey> = palette
            .layer_order
            .iter()
            .map(|key| MaterialKey::new(key.as_str()))
            .collect();
        let default_material = palette
            .layer_order
            .first()
            .map(|key| MaterialKey::new(key.as_str()))
            .unwrap_or_else(|| MaterialKey::new("grass"));
        let sea_level_m = source.recipe().sea_level;
        Self {
            column_cache,
            layer_registry: MaterialLayerRegistry::from_layer_order(&layer_order),
            classifier: RuleSurfaceClassifier::new(
                SurfaceRuleSet::from_compiled(rules),
                default_material,
            ),
            slot_remapper: Mutex::new(ChunkSlotRemapper::new()),
            edit_store,
            biome_catalog,
            backend: TerrainDensityBackend::Recipe(source),
            use_provider_context: false,
            origin_x,
            origin_y,
            origin_z,
            cell_size_m,
            sea_level_m,
        }
    }

    pub fn from_world_provider(
        provider: Arc<dyn WorldDensityProvider>,
        column_cache: ChunkColumnCache,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        cell_size_m: f32,
        edit_store: TerrainEditStore,
        palette: &CompiledTerrainMaterials,
        rules: &CompiledSurfaceRules,
        biome_catalog: BiomeCatalog,
    ) -> Self {
        let layer_order: Vec<MaterialKey> = palette
            .layer_order
            .iter()
            .map(|key| MaterialKey::new(key.as_str()))
            .collect();
        let default_material = palette
            .layer_order
            .first()
            .map(|key| MaterialKey::new(key.as_str()))
            .unwrap_or_else(|| MaterialKey::new("grass"));
        let sea_level_m = provider.world_metadata().extent.sea_level_m;
        Self {
            column_cache,
            layer_registry: MaterialLayerRegistry::from_layer_order(&layer_order),
            classifier: RuleSurfaceClassifier::new(
                SurfaceRuleSet::from_compiled(rules),
                default_material,
            ),
            slot_remapper: Mutex::new(ChunkSlotRemapper::new()),
            edit_store,
            biome_catalog,
            backend: TerrainDensityBackend::Compiled(provider),
            use_provider_context: true,
            origin_x,
            origin_y,
            origin_z,
            cell_size_m,
            sea_level_m,
        }
    }

    fn try_paint_override_at(&self, wx: i32, wy: i32, wz: i32) -> Option<MaterialVertex> {
        let sample = self.edit_store.0.sample_override(wx, wy, wz)?;
        let wx_m = wx as f32 * self.cell_size_m;
        let wy_m = wy as f32 * self.cell_size_m;
        let wz_m = wz as f32 * self.cell_size_m;
        let field = self.backend.sample_density_m(wx_m, wy_m, wz_m);
        if (sample.density - field).abs() > PAINT_DENSITY_MATCH_EPS {
            return None;
        }
        let material_key = terrain_material_key_from_paint_material(sample.material);
        let global = self.layer_registry.layer_or_fallback(&material_key);
        let mut remapper = self.slot_remapper.lock().expect("slot remapper lock");
        Some(remap_blend_to_local_slots(
            [global, 0, 0, 0],
            [1.0, 0.0, 0.0, 0.0],
            &mut remapper,
        ))
    }

    fn paint_overlay_weights(&self, local_pos: [f32; 3]) -> Option<MaterialVertex> {
        let bx = local_pos[0].floor() as i32;
        let by = local_pos[1].floor() as i32;
        let bz = local_pos[2].floor() as i32;
        for dx in 0..=1 {
            for dy in 0..=1 {
                for dz in 0..=1 {
                    let wx = self.origin_x + bx + dx;
                    let wy = self.origin_y + by + dy;
                    let wz = self.origin_z + bz + dz;
                    if let Some(vertex) = self.try_paint_override_at(wx, wy, wz) {
                        return Some(vertex);
                    }
                }
            }
        }
        None
    }

    fn build_context(&self, local_pos: [f32; 3], normal: [f32; 3]) -> SurfaceContext {
        let wx_m = self.origin_x as f32 * self.cell_size_m + local_pos[0] * self.cell_size_m;
        let wy_m = self.origin_y as f32 * self.cell_size_m + local_pos[1] * self.cell_size_m;
        let wz_m = self.origin_z as f32 * self.cell_size_m + local_pos[2] * self.cell_size_m;
        let wx = (wx_m / self.cell_size_m).round() as i32;
        let wz = (wz_m / self.cell_size_m).round() as i32;
        let elevation_m = wy_m;

        let biome_ctx = if self.use_provider_context {
            let provider = self.backend.provider().expect("compiled provider");
            self.column_cache
                .context_at_provider(provider, wx, wy_m, wz)
        } else {
            let source = self.backend.recipe_source().expect("recipe source");
            self.column_cache.context_at(source, wx, wy_m, wz)
        };

        let mut effective_moisture = biome_ctx.effective_moisture;
        let mut coast_distance_m = biome_ctx.distance_to_water;
        let mut slope_degrees = biome_ctx.slope_degrees;
        let mut soil_depth_m =
            fallback_soil_depth(elevation_m, self.sea_level_m, biome_ctx.slope_degrees);
        let mut wave_exposure = (1.0 - (coast_distance_m / 80.0).clamp(0.0, 1.0))
            * (1.0 - (elevation_m / (self.sea_level_m + 6.0)).clamp(0.0, 1.0));

        if self.use_provider_context {
            if let Some(provider) = self.backend.provider() {
                let column = provider.sample_column(WorldXZ::new(wx_m as f64, wz_m as f64));
                if column.soil_depth_m > 0.0 {
                    soil_depth_m = column.soil_depth_m;
                }
                if column.wave_exposure > 0.0 {
                    wave_exposure = column.wave_exposure;
                }
                coast_distance_m = column.surface.coast_distance_m;
                slope_degrees = column.surface.slope;
            }
        } else if let Some(atlas) = self.backend.atlas() {
            coast_distance_m = atlas.sample_coast_distance(wx_m, wz_m);
            slope_degrees = atlas.slope_at(wx_m, wz_m);
            soil_depth_m = atlas.sample_soil_depth(wx_m, wz_m);
            let wetness = terrain_surface::normalize_wetness(
                atlas.sample_wetness(wx_m, wz_m),
                wetness_normalization(self.backend.recipe_source().expect("recipe source")),
            );
            effective_moisture = (effective_moisture * 0.5 + wetness * 0.5).clamp(0.0, 1.0);
        }

        let env = EnvironmentSample {
            elevation: biome_ctx.elevation,
            slope_degrees,
            moisture: biome_ctx.moisture,
            effective_moisture,
            transition_noise: biome_ctx.transition_noise,
            temperature: biome_ctx.temperature,
            distance_to_water: coast_distance_m,
            distance_to_river: biome_ctx.distance_to_river,
            cave_depth: biome_ctx.cave_depth,
            world_y: biome_ctx.world_y,
        };
        let climate_soft = terrain_surface::compute_soft_biome_weights(&env);
        let mut soft = climate_soft;
        let mut biome = soft.primary_biome();

        if self.use_provider_context {
            if let Some(provider) = self.backend.provider() {
                let horizontal = WorldXZ::new(wx_m as f64, wz_m as f64);
                if let Some(blend) = provider.sample_biome_blend(horizontal) {
                    let compiler_soft = soft_weights_from_blend_cell(blend);
                    soft = merge_soft_biome_sources(compiler_soft, climate_soft, COMPILER_BIOME_MIX);
                    biome = compiler_biome_to_presentation(blend.primary);
                } else {
                    let column = provider.sample_column(horizontal);
                    if column.primary_biome != 0 {
                        let compiler_soft = soft_weights_from_primary_u8(column.primary_biome);
                        soft = merge_soft_biome_sources(
                            compiler_soft,
                            climate_soft,
                            COMPILER_BIOME_MIX,
                        );
                        biome = compiler_biome_to_presentation(
                            terrain_generation::CompilerBiomeId::from_u8(column.primary_biome),
                        );
                    }
                }
            }
        } else if let Some(atlas) = self.backend.atlas() {
            soft = terrain_surface::merge_soft_with_atlas(
                climate_soft,
                atlas.sample_biome_weights(wx_m, wz_m),
                ATLAS_BIOME_MIX,
            );
            biome = soft.primary_biome();
        }

        let water_depth_m = (self.sea_level_m - elevation_m).max(0.0);
        let cave_exposure = biome_ctx.cave_exposure;

        SurfaceContext {
            world_position: [wx_m, wy_m, wz_m],
            world_normal: normal,
            elevation_m,
            sea_level_m: self.sea_level_m,
            water_depth_m,
            slope_degrees,
            moisture: effective_moisture,
            soil_depth_m,
            coast_distance_m,
            river_distance_m: biome_ctx.distance_to_river,
            wave_exposure,
            cave_exposure,
            mineral_deposition: cave_exposure * effective_moisture,
            biome,
            geology: self.resolve_geology(cave_exposure),
            soft,
        }
    }

    fn resolve_geology(&self, cave_exposure: f32) -> GeologyId {
        if cave_exposure <= 0.35 {
            return GeologyId::Basalt;
        }
        if self.backend.atlas().is_some() {
            return GeologyId::Basalt;
        }
        if let Some(source) = self.backend.recipe_source() {
            if recipe_has_subtract_cavities(source.recipe()) {
                return GeologyId::Limestone;
            }
        }
        GeologyId::Basalt
    }
}

fn fallback_soil_depth(elevation_m: f32, sea_level_m: f32, slope_degrees: f32) -> f32 {
    let relief = (elevation_m - sea_level_m).max(0.0);
    (1.5 - relief * 0.02 - slope_degrees * 0.02).clamp(0.2, 2.0)
}

fn recipe_has_subtract_cavities(recipe: &terrain_generation::TerrainRecipe) -> bool {
    use terrain_generation::CombineOp;
    recipe.ops.iter().any(|op| match op {
        terrain_generation::RecipeOp::Ellipsoid { combine, .. }
        | terrain_generation::RecipeOp::Capsule { combine, .. } => *combine == CombineOp::Subtract,
        _ => false,
    })
}

impl SurfaceMeshResolver for ChunkSurfaceResolver {
    fn vertex_blend(&self, position: [f32; 3], normal: [f32; 3]) -> MaterialVertex {
        if let Some(mut vertex) = self.paint_overlay_weights(position) {
            let context = self.build_context(position, normal);
            vertex.tint = biome_surface_tint(&self.biome_catalog, context.biome);
            return vertex;
        }
        let context = self.build_context(position, normal);
        let blend = self.classifier.classify(&context);
        let dominant = blend.materials[0].clone();
        let (globals, weights) = resolve_blend(blend, &self.layer_registry);
        let mut remapper = self.slot_remapper.lock().expect("slot remapper lock");
        let mut vertex = remap_blend_to_local_slots(globals, weights, &mut remapper);
        vertex.tint = biome_surface_tint_from_soft(&self.biome_catalog, &context.soft);
        apply_vertex_overlay(&mut vertex, &context, &dominant);
        vertex
    }

    fn chunk_palette(&self) -> ChunkSlotPalette {
        self.slot_remapper
            .lock()
            .expect("slot remapper lock")
            .palette_snapshot()
    }
}

fn apply_vertex_overlay(
    vertex: &mut MaterialVertex,
    context: &SurfaceContext,
    dominant_material: &MaterialKey,
) {
    let responses = overlay_response_for_material_name(dominant_material.as_str());
    let overlay = compute_overlay_state(context, &responses, 0.0, 0.0);
    vertex.overlay = [overlay.wetness, overlay.moss];
    let moss_green = 1.0 + overlay.moss * 0.12;
    vertex.tint[1] = (vertex.tint[1] * moss_green).min(1.2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::biome_context::ChunkColumnCache;
    use shared::StableId;
    use terrain_generation::fill_padded_samples;
    use terrain_generation::{RecipeDensitySource, default_vertical_slice_recipe};
    use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMesher};
    use voxel_core::{CHUNK_CELLS, MaterialId, TerrainChunk, TerrainEditCommand, WorldCell};

    fn test_palette() -> CompiledTerrainMaterials {
        CompiledTerrainMaterials {
            id: StableId::new("materials.test"),
            materials: vec![],
            layer_order: vec![
                StableId::new("grass"),
                StableId::new("sand"),
                StableId::new("rock"),
                StableId::new("forest_floor"),
                StableId::new("scree"),
                StableId::new("weathered_cliff"),
                StableId::new("volcanic_ash"),
                StableId::new("limestone"),
            ],
            key_to_layer: [
                (StableId::new("grass"), 0),
                (StableId::new("sand"), 1),
                (StableId::new("rock"), 2),
                (StableId::new("forest_floor"), 3),
                (StableId::new("scree"), 4),
                (StableId::new("weathered_cliff"), 5),
                (StableId::new("volcanic_ash"), 6),
                (StableId::new("limestone"), 7),
            ]
            .into_iter()
            .collect(),
        }
    }

    fn test_rules() -> CompiledSurfaceRules {
        use game_data::{
            SurfaceBlendEntryDefinition, SurfaceConditionsDefinition, SurfaceGateDefinition,
        };
        CompiledSurfaceRules {
            id: StableId::new("surface.test"),
            gates: vec![
                SurfaceGateDefinition {
                    id: "underwater".to_string(),
                    when: SurfaceConditionsDefinition {
                        water_depth_min: Some(0.05),
                        ..Default::default()
                    },
                    gate_weight: Default::default(),
                    exclusive: true,
                    blend: vec![SurfaceBlendEntryDefinition {
                        material: StableId::new("rock"),
                        weight: 1.0,
                    }],
                    classifier: None,
                },
                SurfaceGateDefinition {
                    id: "land".to_string(),
                    when: SurfaceConditionsDefinition {
                        fallback: true,
                        ..Default::default()
                    },
                    gate_weight: Default::default(),
                    exclusive: true,
                    blend: vec![SurfaceBlendEntryDefinition {
                        material: StableId::new("grass"),
                        weight: 1.0,
                    }],
                    classifier: None,
                },
            ],
            classifiers: vec![],
        }
    }

    fn test_catalog() -> BiomeCatalog {
        use game_data::BiomeRuleDefinition;
        BiomeCatalog {
            rules: vec![
                BiomeRuleDefinition::new("grassland", 0, [0.34, 0.52, 0.28]),
                BiomeRuleDefinition::new("beach", 1, [0.86, 0.78, 0.58]),
            ],
        }
    }

    #[test]
    fn vertex_blend_applies_biome_surface_tint() {
        let source = RecipeDensitySource::new(default_vertical_slice_recipe(42, 2.0));
        let top = source.terrain_surface_height_at(-22.0, -18.0);
        let column_cache = ChunkColumnCache::build(&source, 0, 0, 5);
        let resolver = ChunkSurfaceResolver::from_compiled(
            source,
            column_cache,
            0,
            0,
            0,
            1.0,
            TerrainEditStore::default(),
            &test_palette(),
            &test_rules(),
            test_catalog(),
        );
        let vertex = resolver.vertex_blend([-22.0, top, -18.0], [0.0, 1.0, 0.0]);
        assert!(
            vertex.tint.iter().any(|&c| (c - 1.0).abs() > 0.05),
            "expected biome tint to differ from white, got {:?}",
            vertex.tint
        );
    }

    #[test]
    fn vertex_blend_runs_on_open_terrain() {
        let source = RecipeDensitySource::new(default_vertical_slice_recipe(42, 2.0));
        let top = source.terrain_surface_height_at(-22.0, -18.0);
        let column_cache = ChunkColumnCache::build(&source, 0, 0, 5);
        let resolver = ChunkSurfaceResolver::from_compiled(
            source,
            column_cache,
            0,
            0,
            0,
            1.0,
            TerrainEditStore::default(),
            &test_palette(),
            &test_rules(),
            test_catalog(),
        );
        let vertex = resolver.vertex_blend([-22.0, top, -18.0], [0.0, 1.0, 0.0]);
        assert!(vertex.weights.iter().sum::<f32>() > 0.99);
        assert!(vertex.weights.iter().any(|&w| w > 0.2));
    }

    #[test]
    fn painted_material_reaches_mesh_vertices() {
        let source = RecipeDensitySource::new(default_vertical_slice_recipe(42, 2.0));
        let wx = -22.0;
        let wz = -18.0;
        let surface_y = source.terrain_surface_height_at(wx, wz);
        let paint_material = MaterialId(2);
        let rock_global = 2u32;

        let mut edits = crate::terrain::TerrainEditStore::default();
        let affected = edits.0.apply_command(
            &TerrainEditCommand::PaintMaterial {
                center: [wx, surface_y, wz],
                radius_m: 2.5,
                material: paint_material,
            },
            |ix, iy, iz| source.sample_density(ix as f32, iy as f32, iz as f32),
            |_ix, _iy, _iz, _d| MaterialId(0),
        );
        assert!(!affected.is_empty());

        let coord = WorldCell::new(
            wx.floor() as i32,
            surface_y.floor() as i32,
            wz.floor() as i32,
        )
        .chunk_coord();
        let (ox, oy, oz) = TerrainChunk::new(coord).sample_origin();
        let samples = fill_padded_samples(coord, |ix, iy, iz| {
            if let Some(sample) = edits.0.sample_override(ix, iy, iz) {
                (sample.density, sample.material)
            } else {
                (
                    source.sample_density(ix as f32, iy as f32, iz as f32),
                    MaterialId(0),
                )
            }
        });
        let column_cache = ChunkColumnCache::build(&source, ox, oz, CHUNK_CELLS + 3);
        let resolver = ChunkSurfaceResolver::from_compiled(
            source,
            column_cache,
            ox,
            oy,
            oz,
            1.0,
            edits,
            &test_palette(),
            &test_rules(),
            test_catalog(),
        );
        let mesh = SurfaceNetsMesher
            .build_mesh(&ChunkMeshingInput {
                samples: &samples,
                chunk_cells: CHUNK_CELLS,
                cell_stride: 1,
                surface_resolver: Some(&resolver),
            })
            .expect("mesh");

        assert!(
            mesh.material_vertices.iter().any(|vertex| {
                vertex_dominates_global_material(vertex, &mesh.chunk_palette, rock_global)
            }),
            "painted rock should dominate at least one mesh vertex"
        );
    }

    fn vertex_dominates_global_material(
        vertex: &MaterialVertex,
        palette: &ChunkSlotPalette,
        global_layer: u32,
    ) -> bool {
        vertex
            .local_indices
            .iter()
            .zip(vertex.weights.iter())
            .any(|(&local, &weight)| {
                weight > 0.5 && palette.global_for_local(local) == Some(global_layer)
            })
    }

    fn blend_has_material(blend: &terrain_surface::SurfaceMaterialBlend, name: &str) -> bool {
        blend
            .materials
            .iter()
            .any(|material| material.as_str() == name && blend.weights.iter().sum::<f32>() > 0.0)
            && blend
                .materials
                .iter()
                .zip(blend.weights.iter())
                .any(|(material, weight)| material.as_str() == name && *weight > 0.05)
    }

    fn parity_context() -> terrain_surface::SurfaceContext {
        use terrain_surface::{BiomeId, GeologyId, SoftBiomeWeights};
        terrain_surface::SurfaceContext {
            world_position: [0.0, 10.0, 0.0],
            world_normal: [0.0, 1.0, 0.0],
            elevation_m: 10.0,
            sea_level_m: 0.0,
            water_depth_m: 0.0,
            slope_degrees: 5.0,
            moisture: 0.5,
            soil_depth_m: 1.2,
            coast_distance_m: 100.0,
            river_distance_m: 100.0,
            wave_exposure: 0.0,
            cave_exposure: 0.0,
            mineral_deposition: 0.0,
            biome: BiomeId::Grassland,
            geology: GeologyId::Basalt,
            soft: SoftBiomeWeights {
                grassland: 1.0,
                ..Default::default()
            },
        }
    }

    #[test]
    fn expanded_slice_rule_classifier_matches_island_scenarios() {
        use game_data::load_registry_from_directory;
        use std::path::PathBuf;
        use terrain_surface::{IslandSurfaceClassifier, SurfaceClassifier};

        let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .canonicalize()
            .expect("assets");
        let registry = load_registry_from_directory(&assets).expect("registry");
        let rules = registry
            .surface_rules
            .get(&StableId::new("surface.expanded_slice"))
            .expect("expanded slice surface rules");
        let rule_classifier = RuleSurfaceClassifier::new(
            SurfaceRuleSet::from_compiled(rules),
            MaterialKey::new("grass"),
        );
        let island = IslandSurfaceClassifier;

        let mut cliff = parity_context();
        cliff.slope_degrees = 72.0;
        cliff.moisture = 0.2;
        cliff.soil_depth_m = 0.1;
        assert!(blend_has_material(
            &rule_classifier.classify(&cliff),
            "rock"
        ));
        assert!(blend_has_material(
            &rule_classifier.classify(&cliff),
            "scree"
        ));
        assert!(blend_has_material(&island.classify(&cliff), "rock"));

        let mut coast = parity_context();
        coast.coast_distance_m = 10.0;
        coast.elevation_m = 0.5;
        coast.wave_exposure = 0.2;
        assert!(blend_has_material(
            &rule_classifier.classify(&coast),
            "sand"
        ));
        assert!(blend_has_material(
            &rule_classifier.classify(&coast),
            "weathered_cliff"
        ));
        assert!(blend_has_material(&island.classify(&coast), "sand"));

        let mut alpine = parity_context();
        alpine.elevation_m = 45.0;
        alpine.slope_degrees = 38.0;
        alpine.soft.grassland = 0.1;
        alpine.soft.alpine = 0.7;
        assert!(blend_has_material(
            &rule_classifier.classify(&alpine),
            "scree"
        ));

        let mut cave = parity_context();
        cave.cave_exposure = 0.8;
        cave.geology = terrain_surface::GeologyId::Limestone;
        cave.mineral_deposition = 0.5;
        assert!(blend_has_material(
            &rule_classifier.classify(&cave),
            "limestone"
        ));
        assert!(blend_has_material(&island.classify(&cave), "limestone"));
    }

    #[test]
    fn biome_rule_id_round_trip_for_land_biomes() {
        use terrain_surface::BiomeId;

        let land_biomes = [
            BiomeId::Grassland,
            BiomeId::Forest,
            BiomeId::Scrub,
            BiomeId::CoastalScrub,
            BiomeId::Wetland,
            BiomeId::Beach,
            BiomeId::Alpine,
            BiomeId::RockyUpland,
            BiomeId::Cave,
            BiomeId::Riverbank,
            BiomeId::ShallowWater,
            BiomeId::DeepWater,
            BiomeId::OffshoreShelf,
        ];
        for id in land_biomes {
            assert_eq!(
                BiomeId::from_rule_id(id.as_rule_id()),
                id,
                "round-trip failed for {id:?}"
            );
        }
    }
}
