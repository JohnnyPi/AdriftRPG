// crates/game_bevy/src/environment/surface.rs
use std::sync::Mutex;

use game_data::{CompiledSurfaceRules, CompiledTerrainMaterials};
use terrain_generation::{DensitySource, RecipeDensitySource};
use terrain_surface::{
    remap_blend_to_local_slots, resolve_blend, BiomeId, ChunkSlotPalette, ChunkSlotRemapper,
    EnvironmentSample, GeologyId, MaterialKey, MaterialLayerRegistry, MaterialVertex,
    RuleSurfaceClassifier, SoftBiomeWeights, SurfaceClassifier, SurfaceConditions,
    SurfaceContext, SurfaceGate, SurfaceGateWeights, SurfaceMeshResolver, SurfaceRamp,
    SurfaceRuleSet,
};

use super::biome_context::ChunkColumnCache;
use super::biomes::{biome_surface_tint, BiomeKind};
use crate::environment::BiomeCatalog;
use super::materials::terrain_material_key_from_paint_material;
use crate::terrain::TerrainEditStore;

const ATLAS_BIOME_MIX: f32 = 0.45;
const WETNESS_NORMALIZATION: f32 = 600.0;
const PAINT_DENSITY_MATCH_EPS: f32 = 0.001;

pub struct ChunkSurfaceResolver {
    source: RecipeDensitySource,
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
                surface_rules_from_compiled(rules),
                default_material,
            ),
            slot_remapper: Mutex::new(ChunkSlotRemapper::new()),
            edit_store,
            biome_catalog,
            source,
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
        let field = self.source.sample_density(wx_m, wy_m, wz_m);
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

        let biome_ctx = self
            .column_cache
            .context_at(&self.source, wx, wy_m, wz);

        let mut effective_moisture = biome_ctx.effective_moisture;
        let mut coast_distance_m = biome_ctx.distance_to_water;
        let mut soil_depth_m =
            fallback_soil_depth(elevation_m, self.sea_level_m, biome_ctx.slope_degrees);

        if let Some(atlas) = self.source.atlas() {
            coast_distance_m = atlas.sample_coast_distance(wx_m, wz_m);
            soil_depth_m = atlas.sample_soil_depth(wx_m, wz_m);
            let wetness = (atlas.sample_wetness(wx_m, wz_m) / WETNESS_NORMALIZATION).clamp(0.0, 1.0);
            effective_moisture = (effective_moisture * 0.5 + wetness * 0.5).clamp(0.0, 1.0);
        }

        let env = EnvironmentSample {
            elevation: biome_ctx.elevation,
            slope_degrees: terrain_surface::slope_degrees(normal),
            moisture: biome_ctx.moisture,
            effective_moisture,
            transition_noise: biome_ctx.transition_noise,
            temperature: biome_ctx.temperature,
            distance_to_water: biome_ctx.distance_to_water,
            distance_to_river: biome_ctx.distance_to_river,
            cave_depth: biome_ctx.cave_depth,
            world_y: biome_ctx.world_y,
        };
        let mut soft = terrain_surface::compute_soft_biome_weights(&env);
        if let Some(atlas) = self.source.atlas() {
            soft = merge_soft_with_atlas(soft, atlas.sample_biome_weights(wx_m, wz_m));
        }
        let biome = biome_kind_to_surface(biome_kind_from_soft(&soft));

        let water_depth_m = (self.sea_level_m - elevation_m).max(0.0);
        let wave_exposure = (1.0 - (coast_distance_m / 80.0).clamp(0.0, 1.0))
            * (1.0 - (elevation_m / (self.sea_level_m + 6.0)).clamp(0.0, 1.0));
        let cave_exposure = biome_ctx.cave_exposure;

        SurfaceContext {
            world_position: [wx_m, wy_m, wz_m],
            world_normal: normal,
            elevation_m,
            sea_level_m: self.sea_level_m,
            water_depth_m,
            slope_degrees: terrain_surface::slope_degrees(normal),
            moisture: effective_moisture,
            soil_depth_m,
            coast_distance_m,
            river_distance_m: biome_ctx.distance_to_river,
            wave_exposure,
            cave_exposure,
            mineral_deposition: cave_exposure * effective_moisture,
            biome,
            geology: resolve_geology(&self.source, cave_exposure),
            soft,
        }
    }
}

pub fn surface_rules_from_compiled(rules: &CompiledSurfaceRules) -> SurfaceRuleSet {
    use game_data::{
        SurfaceBlendEntryDefinition, SurfaceClassifierDefinition, SurfaceConditionsDefinition,
        SurfaceGateDefinition,         SurfaceGateWeightDefinition, SurfaceRampDefinition,
    };

    fn map_ramp(ramp: &SurfaceRampDefinition) -> SurfaceRamp {
        SurfaceRamp {
            from: ramp.from,
            to: ramp.to,
            invert: ramp.invert,
        }
    }

    fn map_conditions(when: &SurfaceConditionsDefinition) -> SurfaceConditions {
        SurfaceConditions {
            cave_exposure_min: when.cave_exposure_min,
            water_depth_min: when.water_depth_min,
            coast_distance_max: when.coast_distance_max,
            river_distance_max: when.river_distance_max,
            slope_min: when.slope_min,
            slope_max: when.slope_max,
            elevation_min: when.elevation_min,
            elevation_max: when.elevation_max,
            elevation_above_sea_min: when.elevation_above_sea_min,
            elevation_above_sea_max: when.elevation_above_sea_max,
            moisture_min: when.moisture_min,
            moisture_max: when.moisture_max,
            geology: when.geology.clone(),
            biome: when.biome.clone(),
            soft_grassland_min: when.soft_grassland_min,
            soft_forest_min: when.soft_forest_min,
            soft_wetland_min: when.soft_wetland_min,
            soft_alpine_min: when.soft_alpine_min,
            fallback: when.fallback,
        }
    }

    fn map_gate_weights(weights: &SurfaceGateWeightDefinition) -> SurfaceGateWeights {
        SurfaceGateWeights {
            coast_distance: weights.coast_distance.as_ref().map(map_ramp),
            river_distance: weights.river_distance.as_ref().map(map_ramp),
            slope: weights.slope.as_ref().map(map_ramp),
            elevation_above_sea: weights.elevation_above_sea.as_ref().map(map_ramp),
            moisture: weights.moisture.as_ref().map(map_ramp),
            cave_exposure: weights.cave_exposure.as_ref().map(map_ramp),
            wave_exposure: weights.wave_exposure.as_ref().map(map_ramp),
            soft_alpine: weights.soft_alpine.as_ref().map(map_ramp),
            soft_wetland: weights.soft_wetland.as_ref().map(map_ramp),
            constant: weights.constant,
        }
    }

    fn map_blend(entries: &[SurfaceBlendEntryDefinition]) -> Vec<terrain_surface::SurfaceBlendEntry> {
        entries
            .iter()
            .map(|entry| terrain_surface::SurfaceBlendEntry {
                material: MaterialKey::new(entry.material.as_str()),
                weight: entry.weight,
            })
            .collect()
    }

    fn map_classifier(def: &SurfaceClassifierDefinition) -> terrain_surface::SurfaceClassifierPreset {
        terrain_surface::SurfaceClassifierPreset {
            id: def.id.clone(),
            blend: map_blend(&def.blend),
            weighted_mix: def
                .weighted_mix
                .iter()
                .map(|mix| terrain_surface::SurfaceWeightedMix {
                    classifier: mix.classifier.clone(),
                    weight: mix.weight,
                })
                .collect(),
        }
    }

    let gates = rules
        .gates
        .iter()
        .map(|gate: &SurfaceGateDefinition| SurfaceGate {
            id: gate.id.clone(),
            when: map_conditions(&gate.when),
            gate_weight: map_gate_weights(&gate.gate_weight),
            exclusive: gate.exclusive,
            blend: map_blend(&gate.blend),
            classifier: gate.classifier.clone(),
        })
        .collect();

    let classifiers = rules
        .classifiers
        .iter()
        .map(map_classifier)
        .map(|preset| (preset.id.clone(), preset))
        .collect();

    SurfaceRuleSet {
        gates,
        classifiers,
    }
}

fn fallback_soil_depth(elevation_m: f32, sea_level_m: f32, slope_degrees: f32) -> f32 {
    let relief = (elevation_m - sea_level_m).max(0.0);
    (1.5 - relief * 0.02 - slope_degrees * 0.02).clamp(0.2, 2.0)
}

fn merge_soft_with_atlas(
    climate: SoftBiomeWeights,
    atlas: terrain_generation::BiomeWeights,
) -> SoftBiomeWeights {
    let t = ATLAS_BIOME_MIX;
    SoftBiomeWeights {
        grassland: climate.grassland * (1.0 - t) + atlas.grassland * t,
        forest: climate.forest * (1.0 - t) + atlas.rainforest * t,
        scrub: climate.scrub * (1.0 - t),
        coastal_scrub: climate.coastal_scrub * (1.0 - t),
        wetland: climate.wetland * (1.0 - t) + atlas.wetland * t,
        beach: climate.beach * (1.0 - t) + atlas.beach * t,
        alpine: climate.alpine * (1.0 - t),
        rocky: climate.rocky * (1.0 - t) + atlas.volcanic_rock * t,
    }
    .normalize()
}

fn resolve_geology(source: &RecipeDensitySource, cave_exposure: f32) -> GeologyId {
    if cave_exposure > 0.35 && recipe_has_subtract_cavities(source.recipe()) {
        GeologyId::Limestone
    } else {
        GeologyId::Basalt
    }
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
        if let Some(vertex) = self.paint_overlay_weights(position) {
            return vertex;
        }
        let context = self.build_context(position, normal);
        let blend = self.classifier.classify(&context);
        let (globals, weights) = resolve_blend(blend, &self.layer_registry);
        let mut remapper = self.slot_remapper.lock().expect("slot remapper lock");
        let mut vertex = remap_blend_to_local_slots(globals, weights, &mut remapper);
        let kind = biome_kind_from_surface(context.biome);
        vertex.tint = biome_surface_tint(&self.biome_catalog, kind);
        vertex
    }

    fn chunk_palette(&self) -> ChunkSlotPalette {
        self.slot_remapper
            .lock()
            .expect("slot remapper lock")
            .palette_snapshot()
    }
}

pub fn biome_kind_from_surface(biome: terrain_surface::BiomeId) -> BiomeKind {
    match biome {
        terrain_surface::BiomeId::Grassland => BiomeKind::Grassland,
        terrain_surface::BiomeId::Forest => BiomeKind::Forest,
        terrain_surface::BiomeId::Scrub => BiomeKind::Scrub,
        terrain_surface::BiomeId::CoastalScrub => BiomeKind::CoastalScrub,
        terrain_surface::BiomeId::Wetland => BiomeKind::Wetland,
        terrain_surface::BiomeId::Beach => BiomeKind::Beach,
        terrain_surface::BiomeId::Alpine => BiomeKind::Alpine,
        terrain_surface::BiomeId::RockyUpland => BiomeKind::RockyUpland,
        terrain_surface::BiomeId::Cave => BiomeKind::Cave,
        terrain_surface::BiomeId::Riverbank => BiomeKind::Riverbank,
        terrain_surface::BiomeId::ShallowWater => BiomeKind::ShallowWater,
    }
}

fn biome_kind_from_soft(soft: &SoftBiomeWeights) -> BiomeKind {
    let channels = [
        (BiomeKind::Grassland, soft.grassland),
        (BiomeKind::Forest, soft.forest),
        (BiomeKind::Scrub, soft.scrub),
        (BiomeKind::CoastalScrub, soft.coastal_scrub),
        (BiomeKind::Wetland, soft.wetland),
        (BiomeKind::Beach, soft.beach),
        (BiomeKind::Alpine, soft.alpine),
        (BiomeKind::RockyUpland, soft.rocky),
    ];
    channels
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(k, _)| k)
        .unwrap_or(BiomeKind::Grassland)
}

fn biome_kind_to_surface(kind: BiomeKind) -> BiomeId {
    match kind {
        BiomeKind::Grassland => BiomeId::Grassland,
        BiomeKind::Forest => BiomeId::Forest,
        BiomeKind::Scrub => BiomeId::Scrub,
        BiomeKind::CoastalScrub => BiomeId::CoastalScrub,
        BiomeKind::Wetland => BiomeId::Wetland,
        BiomeKind::Beach => BiomeId::Beach,
        BiomeKind::Alpine => BiomeId::Alpine,
        BiomeKind::RockyUpland => BiomeId::RockyUpland,
        BiomeKind::Cave => BiomeId::Cave,
        BiomeKind::Riverbank => BiomeId::Riverbank,
        BiomeKind::ShallowWater | BiomeKind::DeepWater | BiomeKind::OffshoreShelf => {
            BiomeId::Beach
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::biome_context::ChunkColumnCache;
    use shared::StableId;
    use terrain_generation::{default_vertical_slice_recipe, RecipeDensitySource};
    use terrain_generation::fill_padded_samples;
    use terrain_meshing::{ChunkMeshingInput, SurfaceNetsMesher, TerrainMesher};
    use voxel_core::{MaterialId, TerrainEditCommand, CHUNK_CELLS, TerrainChunk, WorldCell};

    fn test_palette() -> CompiledTerrainMaterials {
        CompiledTerrainMaterials {
            id: StableId::new("materials.test"),
            materials: vec![],
            layer_order: vec![
                StableId::new("grass"),
                StableId::new("sand"),
                StableId::new("rock"),
                StableId::new("cave_stone"),
                StableId::new("wet_rock"),
                StableId::new("forest_floor"),
                StableId::new("scrub"),
                StableId::new("flowstone"),
                StableId::new("limestone"),
            ],
            key_to_layer: [
                (StableId::new("grass"), 0),
                (StableId::new("sand"), 1),
                (StableId::new("rock"), 2),
                (StableId::new("cave_stone"), 3),
                (StableId::new("wet_rock"), 4),
                (StableId::new("forest_floor"), 5),
                (StableId::new("scrub"), 6),
                (StableId::new("flowstone"), 7),
                (StableId::new("limestone"), 8),
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
                        material: StableId::new("wet_rock"),
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

        let coord = WorldCell::new(wx.floor() as i32, surface_y.floor() as i32, wz.floor() as i32)
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
            surface_rules_from_compiled(rules),
            MaterialKey::new("grass"),
        );
        let island = IslandSurfaceClassifier;

        let mut cliff = parity_context();
        cliff.slope_degrees = 72.0;
        cliff.moisture = 0.2;
        cliff.soil_depth_m = 0.1;
        assert!(blend_has_material(&rule_classifier.classify(&cliff), "rock"));
        assert!(blend_has_material(&island.classify(&cliff), "rock"));

        let mut coast = parity_context();
        coast.coast_distance_m = 10.0;
        coast.elevation_m = 0.5;
        coast.wave_exposure = 0.2;
        assert!(blend_has_material(&rule_classifier.classify(&coast), "sand"));
        assert!(blend_has_material(&island.classify(&coast), "sand"));

        let mut cave = parity_context();
        cave.cave_exposure = 0.8;
        cave.geology = terrain_surface::GeologyId::Limestone;
        cave.mineral_deposition = 0.5;
        assert!(blend_has_material(&rule_classifier.classify(&cave), "limestone"));
        assert!(blend_has_material(&island.classify(&cave), "limestone"));
    }
}
