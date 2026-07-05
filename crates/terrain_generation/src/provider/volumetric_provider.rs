//! Composed world density provider: atlas surface + volumetric carves.

use std::sync::Arc;

use crate::caves::sdf::CaveSubtractOps;
use crate::compiler::CompiledWorld;
use crate::contract::coordinates::{WorldPosition, WorldXZ};
use crate::contract::density::{
    ColumnSample, GeologySample, SurfaceSample, WorldDensityProvider, surface_density,
};
use crate::contract::metadata::WorldMetadata;
use crate::hydrology::realize::CompiledHydrologyProducts;
use crate::provider::atlas_provider::AtlasWorldProvider;
use crate::recipe::RiverCarveContext;
use crate::river::{river_carve_offset, river_channel_at};
use crate::water_body::RiverSpline;

pub struct VolumetricWorldProvider {
    base: AtlasWorldProvider,
    cave_ops: CaveSubtractOps,
    river_carve: Option<RiverCarveContext>,
    use_carved_elevation: bool,
    hydrology_products: Option<CompiledHydrologyProducts>,
}

impl VolumetricWorldProvider {
    pub fn from_compiled(world: &CompiledWorld) -> Self {
        let mut base = AtlasWorldProvider::from_compiled(world);
        let use_carved_elevation = world
            .atlas
            .fields
            .get_scalar(crate::fields::key::FieldKey::CarvedElevation)
            .is_some();

        if use_carved_elevation {
            if let Some(carved) = world
                .atlas
                .fields
                .get_scalar(crate::fields::key::FieldKey::CarvedElevation)
            {
                base.set_surface_elevation(carved.as_ref().clone());
            }
        }

        let cave_ops = world
            .atlas
            .graphs
            .cave_subtract_ops
            .clone()
            .unwrap_or_default();

        let river_carve = world
            .atlas
            .graphs
            .hydrology
            .as_ref()
            .and_then(|g| g.primary_river.as_ref())
            .map(|river| RiverCarveContext {
                spline: river.clone(),
                bank_width_m: 6.0,
            });

        let hydrology_products = world.atlas.graphs.hydrology_products.clone();

        Self {
            base,
            cave_ops,
            river_carve,
            use_carved_elevation,
            hydrology_products,
        }
    }

    pub fn cave_ops(&self) -> &CaveSubtractOps {
        &self.cave_ops
    }

    pub fn hydrology_products(&self) -> Option<&CompiledHydrologyProducts> {
        self.hydrology_products.as_ref()
    }

    pub fn into_arc(self) -> Arc<dyn WorldDensityProvider> {
        Arc::new(self)
    }
}

impl WorldDensityProvider for VolumetricWorldProvider {
    fn world_metadata(&self) -> &WorldMetadata {
        self.base.world_metadata()
    }

    fn sample_density(&self, position: WorldPosition) -> f32 {
        let horizontal = position.horizontal();
        let mut elev = self.base.sample_surface(horizontal).elevation_m;
        if !self.use_carved_elevation {
            if let Some(ref river) = self.river_carve {
                elev = apply_river_carve_to_elevation(horizontal, elev, river);
            }
        }
        let mut density = surface_density(position, elev);
        density = self.cave_ops.apply_subtract(density, position);
        density
    }

    fn sample_surface(&self, horizontal: WorldXZ) -> SurfaceSample {
        let mut surface = self.base.sample_surface(horizontal);
        if !self.use_carved_elevation {
            if let Some(ref river) = self.river_carve {
                surface.elevation_m =
                    apply_river_carve_to_elevation(horizontal, surface.elevation_m, river);
            }
        }
        surface
    }

    fn sample_geology(&self, position: WorldPosition) -> GeologySample {
        self.base.sample_geology(position)
    }

    fn sample_column(&self, horizontal: WorldXZ) -> ColumnSample {
        self.base.sample_column(horizontal)
    }

    fn primary_river(&self) -> Option<&RiverSpline> {
        self.base.primary_river()
    }
}

fn apply_river_carve_to_elevation(
    horizontal: WorldXZ,
    elevation_m: f32,
    river: &RiverCarveContext,
) -> f32 {
    let wx = horizontal.x() as f32;
    let wz = horizontal.z() as f32;
    let (dist, half_width, depth) = river_channel_at(&river.spline, wx, wz);
    let carve = river_carve_offset(dist, half_width, river.bank_width_m, depth);
    if carve > 0.0 {
        elevation_m - carve
    } else {
        elevation_m
    }
}
