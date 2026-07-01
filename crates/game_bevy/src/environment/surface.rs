use terrain_generation::RecipeDensitySource;
use terrain_surface::{
    compute_soft_biome_weights, resolve_blend, BiomeId, EnvironmentSample, GeologyId,
    IslandSurfaceClassifier, MaterialLayerRegistry, SoftBiomeWeights, SurfaceClassifier,
    SurfaceContext, SurfaceMeshResolver,
};

use super::biome_context::ChunkColumnCache;
use super::biomes::BiomeKind;

pub struct ChunkSurfaceResolver {
    source: RecipeDensitySource,
    column_cache: ChunkColumnCache,
    layer_registry: MaterialLayerRegistry,
    classifier: IslandSurfaceClassifier,
    origin_x: i32,
    origin_y: i32,
    origin_z: i32,
    cell_size_m: f32,
    sea_level_m: f32,
}

impl ChunkSurfaceResolver {
    pub fn new(
        source: RecipeDensitySource,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        padded_side: usize,
        cell_size_m: f32,
    ) -> Self {
        let sea_level_m = source.recipe().sea_level;
        Self {
            column_cache: ChunkColumnCache::build(&source, origin_x, origin_z, padded_side),
            layer_registry: MaterialLayerRegistry::from_core_set(),
            classifier: IslandSurfaceClassifier,
            source,
            origin_x,
            origin_y,
            origin_z,
            cell_size_m,
            sea_level_m,
        }
    }

    fn build_context(&self, local_pos: [f32; 3], normal: [f32; 3]) -> SurfaceContext {
        let wx_m = self.origin_x as f32 * self.cell_size_m + local_pos[0] * self.cell_size_m;
        let wy_m = self.origin_y as f32 * self.cell_size_m + local_pos[1] * self.cell_size_m;
        let wz_m = self.origin_z as f32 * self.cell_size_m + local_pos[2] * self.cell_size_m;
        let wx = (wx_m / self.cell_size_m).round() as i32;
        let wz = (wz_m / self.cell_size_m).round() as i32;

        let biome_ctx = self
            .column_cache
            .context_at(&self.source, wx, wy_m, wz);

        let env = EnvironmentSample {
            elevation: biome_ctx.elevation,
            slope_degrees: terrain_surface::slope_degrees(normal),
            moisture: biome_ctx.moisture,
            effective_moisture: biome_ctx.effective_moisture,
            transition_noise: biome_ctx.transition_noise,
            temperature: biome_ctx.temperature,
            distance_to_water: biome_ctx.distance_to_water,
            distance_to_river: biome_ctx.distance_to_river,
            cave_depth: biome_ctx.cave_depth,
            world_y: biome_ctx.world_y,
        };
        let soft = compute_soft_biome_weights(&env);
        let biome = biome_kind_to_surface(biome_kind_from_soft(&soft));

        let elevation_m = wy_m;
        let water_depth_m = (self.sea_level_m - elevation_m).max(0.0);
        let coast_distance_m = biome_ctx.distance_to_water;
        let wave_exposure = (1.0 - (coast_distance_m / 80.0).clamp(0.0, 1.0))
            * (1.0 - (elevation_m / (self.sea_level_m + 6.0)).clamp(0.0, 1.0));
        let cave_exposure = if biome_ctx.cave_depth >= 2.0 {
            (biome_ctx.cave_depth / 8.0).clamp(0.0, 1.0)
        } else {
            0.0
        };

        SurfaceContext {
            world_position: [wx_m, wy_m, wz_m],
            world_normal: normal,
            elevation_m,
            sea_level_m: self.sea_level_m,
            water_depth_m,
            slope_degrees: terrain_surface::slope_degrees(normal),
            moisture: biome_ctx.effective_moisture,
            soil_depth_m: (1.5 - (elevation_m - self.sea_level_m) * 0.02).clamp(0.2, 2.0),
            coast_distance_m,
            river_distance_m: biome_ctx.distance_to_river,
            wave_exposure,
            cave_exposure,
            mineral_deposition: cave_exposure * biome_ctx.effective_moisture,
            biome,
            geology: GeologyId::Basalt,
            soft,
        }
    }
}

impl SurfaceMeshResolver for ChunkSurfaceResolver {
    fn vertex_blend(&self, position: [f32; 3], normal: [f32; 3]) -> ([u16; 4], [f32; 4]) {
        let context = self.build_context(position, normal);
        let blend = self.classifier.classify(&context);
        let resolved = resolve_blend(blend, &self.layer_registry);
        (
            [
                resolved.indices[0] as u16,
                resolved.indices[1] as u16,
                resolved.indices[2] as u16,
                resolved.indices[3] as u16,
            ],
            resolved.weights,
        )
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
