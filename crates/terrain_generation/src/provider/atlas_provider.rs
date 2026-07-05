//! Runtime WorldDensityProvider backed by a compiled WorldAtlas.

use std::sync::Arc;

use crate::DensitySource;
use glam::Vec3;

use crate::compiler::CompiledWorld;
use crate::contract::coordinates::{WorldPosition, WorldXZ};
use crate::contract::density::{
    ColumnSample, GeologySample, SurfaceSample, WorldDensityProvider, surface_density,
};
use crate::contract::metadata::WorldMetadata;
use crate::fields::key::FieldKey;
use crate::geology::columns::ColumnSampler;
use crate::geology::material::BedrockId;
use crate::hydrology::HydrologyGraph;
use crate::strata::material::StrataMaterialId;
use crate::water_body::RiverSpline;
use crate::biomes::id::BiomeBlendCell;
use crate::world::graphs::BiomeGrid;

pub struct AtlasWorldProvider {
    metadata: WorldMetadata,
    surface_elevation: Arc<crate::fields::scalar::ScalarField>,
    coast_distance: Arc<crate::fields::scalar::ScalarField>,
    land_mask: Arc<crate::fields::scalar::ScalarField>,
    hardness: Arc<crate::fields::scalar::ScalarField>,
    erodibility: Arc<crate::fields::scalar::ScalarField>,
    permeability: Arc<crate::fields::scalar::ScalarField>,
    fracture: Arc<crate::fields::scalar::ScalarField>,
    age: Arc<crate::fields::scalar::ScalarField>,
    island_id: Arc<crate::fields::scalar::ScalarField>,
    bedrock: Arc<crate::fields::typed::CategoricalField<u8>>,
    temperature: Option<Arc<crate::fields::scalar::ScalarField>>,
    rainfall: Option<Arc<crate::fields::scalar::ScalarField>>,
    humidity: Option<Arc<crate::fields::scalar::ScalarField>>,
    flow_accumulation: Option<Arc<crate::fields::scalar::ScalarField>>,
    soil_depth: Option<Arc<crate::fields::scalar::ScalarField>>,
    regolith_depth: Option<Arc<crate::fields::scalar::ScalarField>>,
    weathering_depth: Option<Arc<crate::fields::scalar::ScalarField>>,
    deposit_mask: Option<Arc<crate::fields::scalar::ScalarField>>,
    wave_exposure: Option<Arc<crate::fields::scalar::ScalarField>>,
    primary_biome: Option<Arc<crate::fields::typed::CategoricalField<u8>>>,
    column_sampler: Option<ColumnSampler>,
    biome_grid: Option<BiomeGrid>,
    hydrology: Option<Arc<HydrologyGraph>>,
}

impl AtlasWorldProvider {
    pub fn from_compiled(world: &CompiledWorld) -> Self {
        let fields = &world.atlas.fields;
        let surface_elevation = fields
            .get_scalar(FieldKey::CoastalElevation)
            .or_else(|| fields.get_scalar(FieldKey::ErodedElevation))
            .or_else(|| fields.get_scalar(FieldKey::FinalElevation))
            .expect("surface elevation");

        let soil_depth = fields.get_scalar(FieldKey::SoilDepth);
        let regolith_depth = fields.get_scalar(FieldKey::RegolithDepth);
        let weathering_depth = fields.get_scalar(FieldKey::WeatheringDepth);
        let deposit_mask = fields.get_scalar(FieldKey::DepositMask);

        let column_sampler = match (
            soil_depth.as_ref(),
            regolith_depth.as_ref(),
            weathering_depth.as_ref(),
            deposit_mask.as_ref(),
        ) {
            (Some(soil), Some(regolith), Some(weathering), Some(deposit)) => Some(ColumnSampler {
                regolith_depth: regolith.as_ref().clone(),
                weathering_depth: weathering.as_ref().clone(),
                soil_depth: soil.as_ref().clone(),
                deposit: deposit.as_ref().clone(),
                recipe: default_strata_recipe(),
            }),
            _ => None,
        };

        Self {
            metadata: world.atlas.metadata.clone(),
            surface_elevation,
            coast_distance: fields
                .get_scalar(FieldKey::CoastDistance)
                .expect("coast distance"),
            land_mask: fields.get_scalar(FieldKey::LandMask).expect("land mask"),
            hardness: fields.get_scalar(FieldKey::RockHardness).expect("hardness"),
            erodibility: fields
                .get_scalar(FieldKey::Erodibility)
                .expect("erodibility"),
            permeability: fields
                .get_scalar(FieldKey::Permeability)
                .expect("permeability"),
            fracture: fields
                .get_scalar(FieldKey::FractureIntensity)
                .expect("fracture"),
            age: fields.get_scalar(FieldKey::IslandAge).expect("age"),
            island_id: fields.get_scalar(FieldKey::IslandId).expect("island id"),
            bedrock: fields.get_categorical(FieldKey::Bedrock).expect("bedrock"),
            temperature: fields.get_scalar(FieldKey::Temperature),
            rainfall: fields.get_scalar(FieldKey::Rainfall),
            humidity: fields.get_scalar(FieldKey::Humidity),
            flow_accumulation: fields.get_scalar(FieldKey::FlowAccumulation),
            soil_depth,
            regolith_depth,
            weathering_depth,
            deposit_mask,
            wave_exposure: fields.get_scalar(FieldKey::WaveExposureCoastal),
            primary_biome: fields.get_categorical(FieldKey::PrimaryBiome),
            column_sampler,
            biome_grid: world.atlas.graphs.biome.clone(),
            hydrology: world
                .atlas
                .graphs
                .hydrology
                .as_ref()
                .map(|g| Arc::new(g.clone())),
        }
    }

    pub fn sample_strata_material(
        &self,
        horizontal: WorldXZ,
        depth_below_surface_m: f32,
    ) -> StrataMaterialId {
        self.column_sampler
            .as_ref()
            .map(|s| s.sample_material(horizontal, depth_below_surface_m))
            .unwrap_or(StrataMaterialId::Basalt)
    }

    pub fn biome_grid(&self) -> Option<&BiomeGrid> {
        self.biome_grid.as_ref()
    }

    pub fn hydrology_graph(&self) -> Option<&HydrologyGraph> {
        self.hydrology.as_deref()
    }

    pub fn set_surface_elevation(&mut self, field: crate::fields::scalar::ScalarField) {
        self.surface_elevation = Arc::new(field);
    }

    pub fn into_arc(self) -> Arc<dyn WorldDensityProvider> {
        Arc::new(self)
    }
}

fn default_strata_recipe() -> game_data::CompiledStrataRecipe {
    game_data::CompiledStrataRecipe {
        id: "default".into(),
        layers: vec![],
        deposits: vec![],
    }
}

impl WorldDensityProvider for AtlasWorldProvider {
    fn world_metadata(&self) -> &WorldMetadata {
        &self.metadata
    }

    fn sample_density(&self, position: WorldPosition) -> f32 {
        let surface = self.sample_surface(position.horizontal());
        surface_density(position, surface.elevation_m)
    }

    fn sample_surface(&self, horizontal: WorldXZ) -> SurfaceSample {
        let elevation_m = self.surface_elevation.sample_at_world(horizontal);
        let land_mask = self.land_mask.sample_at_world(horizontal).clamp(0.0, 1.0);
        let coast_distance_m = self.coast_distance.sample_at_world(horizontal);
        let island_id = if land_mask > 0.05 {
            Some(self.island_id.sample_at_world(horizontal).round() as u32)
        } else {
            None
        };
        SurfaceSample {
            elevation_m,
            slope: estimate_slope(&self.surface_elevation, horizontal),
            macro_normal: Vec3::Y,
            land_mask,
            coast_distance_m,
            island_id,
        }
    }

    fn sample_geology(&self, position: WorldPosition) -> GeologySample {
        let h = position.horizontal();
        let bedrock_id = BedrockId::from_u8(self.bedrock.get(
            sample_grid_x(&self.bedrock.0, h),
            sample_grid_z(&self.bedrock.0, h),
        ));
        GeologySample {
            bedrock: bedrock_id,
            hardness: self.hardness.sample_at_world(h),
            erodibility: self.erodibility.sample_at_world(h),
            permeability: self.permeability.sample_at_world(h),
            volcanic_age: self.age.sample_at_world(h),
            fracture_intensity: self.fracture.sample_at_world(h),
        }
    }

    fn sample_column(&self, horizontal: WorldXZ) -> ColumnSample {
        let surface = self.sample_surface(horizontal);
        let geology = self.sample_geology(WorldPosition::new(
            horizontal.x(),
            surface.elevation_m as f64,
            horizontal.z(),
        ));
        let temperature = self
            .temperature
            .as_ref()
            .map(|f| f.sample_at_world(horizontal))
            .unwrap_or(0.5);
        let rainfall = self
            .rainfall
            .as_ref()
            .map(|f| f.sample_at_world(horizontal))
            .unwrap_or(0.0);
        let humidity = self
            .humidity
            .as_ref()
            .map(|f| f.sample_at_world(horizontal))
            .unwrap_or(0.5);
        let wetness = self
            .flow_accumulation
            .as_ref()
            .map(|f| {
                let acc = f.sample_at_world(horizontal);
                (acc / 100.0).clamp(0.0, 1.0)
            })
            .unwrap_or(0.0);
        let deposit_presence = self
            .deposit_mask
            .as_ref()
            .map(|f| {
                if f.sample_at_world(horizontal) > 0.5 {
                    1.0
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);
        let soil_depth_m = self
            .soil_depth
            .as_ref()
            .map(|f| f.sample_at_world(horizontal) + deposit_presence * 0.25)
            .unwrap_or(0.0);
        let regolith_depth_m = self
            .regolith_depth
            .as_ref()
            .map(|f| f.sample_at_world(horizontal))
            .unwrap_or(if surface.land_mask > 0.5 { 0.5 } else { 0.0 });
        let weathering_depth_m = self
            .weathering_depth
            .as_ref()
            .map(|f| f.sample_at_world(horizontal))
            .unwrap_or(if surface.land_mask > 0.5 { 1.5 } else { 0.0 });
        let wave_exposure = self
            .wave_exposure
            .as_ref()
            .map(|f| f.sample_at_world(horizontal))
            .unwrap_or(0.0);
        let primary_biome = self
            .primary_biome
            .as_ref()
            .map(|f| {
                f.get(
                    sample_grid_x(&f.0, horizontal),
                    sample_grid_z(&f.0, horizontal),
                )
            })
            .unwrap_or(0);
        ColumnSample {
            surface,
            regolith_depth_m,
            weathering_depth_m,
            base_bedrock: geology.bedrock,
            temperature,
            rainfall,
            humidity,
            wetness: wetness.max(deposit_presence * 0.35),
            soil_depth_m,
            wave_exposure,
            primary_biome,
        }
    }

    fn primary_river(&self) -> Option<&RiverSpline> {
        self.hydrology
            .as_ref()
            .and_then(|g| g.primary_river.as_ref())
    }

    fn sample_biome_blend(&self, horizontal: WorldXZ) -> Option<BiomeBlendCell> {
        let grid = self.biome_grid.as_ref()?;
        let field = self.primary_biome.as_ref()?;
        let x = sample_grid_x(&field.0, horizontal);
        let z = sample_grid_z(&field.0, horizontal);
        if x >= grid.width || z >= grid.height {
            return None;
        }
        let idx = (z * grid.width + x) as usize;
        grid.cells.get(idx).copied()
    }
}

fn sample_grid_x(field: &crate::fields::dense::DenseField2D<u8>, h: WorldXZ) -> u32 {
    let d = &field.descriptor;
    let lx = (h.x() - d.origin_x()) / d.cell_size_m;
    (lx.round() as i64).clamp(0, d.width.saturating_sub(1) as i64) as u32
}

fn sample_grid_z(field: &crate::fields::dense::DenseField2D<u8>, h: WorldXZ) -> u32 {
    let d = &field.descriptor;
    let lz = (h.z() - d.origin_z()) / d.cell_size_m;
    (lz.round() as i64).clamp(0, d.height.saturating_sub(1) as i64) as u32
}

fn estimate_slope(field: &crate::fields::scalar::ScalarField, world: WorldXZ) -> f32 {
    let cell = field.descriptor.cell_size_m as f32;
    let e0 = field.sample_at_world(world);
    let ex = field.sample_at_world(WorldXZ::new(world.x() + cell as f64, world.z()));
    let ez = field.sample_at_world(WorldXZ::new(world.x(), world.z() + cell as f64));
    let dx = (ex - e0) / cell;
    let dz = (ez - e0) / cell;
    (dx * dx + dz * dz).sqrt().atan().to_degrees()
}

/// Adapter wrapping legacy RecipeDensitySource as WorldDensityProvider.
pub struct RecipeDensityProviderAdapter {
    inner: crate::recipe::RecipeDensitySource,
    metadata: WorldMetadata,
    sea_level_m: f32,
}

impl RecipeDensityProviderAdapter {
    pub fn new(
        inner: crate::recipe::RecipeDensitySource,
        metadata: WorldMetadata,
        sea_level_m: f32,
    ) -> Self {
        Self {
            inner,
            metadata,
            sea_level_m,
        }
    }

    fn find_surface_elevation(&self, horizontal: WorldXZ) -> f32 {
        let mut lo = self.sea_level_m - 500.0;
        let mut hi = self.sea_level_m + 3000.0;
        for _ in 0..24 {
            let mid = (lo + hi) * 0.5;
            let d = self.sample_density(WorldPosition::new(
                horizontal.x(),
                mid as f64,
                horizontal.z(),
            ));
            if d < 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo + hi) * 0.5
    }
}

impl WorldDensityProvider for RecipeDensityProviderAdapter {
    fn world_metadata(&self) -> &WorldMetadata {
        &self.metadata
    }

    fn sample_density(&self, position: WorldPosition) -> f32 {
        self.inner.sample_density(
            position.0.x as f32,
            position.0.y as f32,
            position.0.z as f32,
        )
    }

    fn sample_surface(&self, horizontal: WorldXZ) -> SurfaceSample {
        let elev = self.find_surface_elevation(horizontal);
        SurfaceSample {
            elevation_m: elev,
            slope: 0.0,
            macro_normal: Vec3::Y,
            land_mask: if elev > self.sea_level_m { 1.0 } else { 0.0 },
            coast_distance_m: 0.0,
            island_id: Some(0),
        }
    }

    fn sample_geology(&self, _position: WorldPosition) -> GeologySample {
        GeologySample::default()
    }

    fn sample_column(&self, horizontal: WorldXZ) -> ColumnSample {
        let surface = self.sample_surface(horizontal);
        ColumnSample {
            surface,
            ..Default::default()
        }
    }
}
