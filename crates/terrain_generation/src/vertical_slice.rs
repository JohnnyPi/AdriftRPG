use crate::recipe::{default_vertical_slice_recipe, RecipeDensitySource, TerrainRecipe};

/// Legacy wrapper retained for tests; delegates to [`RecipeDensitySource`].
#[derive(Clone, Debug)]
pub struct VerticalSliceDensitySource {
    inner: RecipeDensitySource,
    pub sea_level: f32,
}

impl VerticalSliceDensitySource {
    pub fn new(seed: u64, sea_level: f32) -> Self {
        Self {
            sea_level,
            inner: RecipeDensitySource::new(default_vertical_slice_recipe(seed, sea_level)),
        }
    }

    pub fn from_recipe(recipe: TerrainRecipe) -> Self {
        let sea_level = recipe.sea_level;
        Self {
            sea_level,
            inner: RecipeDensitySource::new(recipe),
        }
    }

    pub fn density_at(&self, x: f32, y: f32, z: f32) -> f32 {
        self.inner.density_at(x, y, z)
    }

    pub fn surface_height_at(&self, x: f32, z: f32) -> f32 {
        self.inner.surface_height_at(x, z)
    }

    pub fn spawn_position(&self) -> (f32, f32, f32) {
        self.inner.spawn_position()
    }

    pub fn recipe_source(&self) -> &RecipeDensitySource {
        &self.inner
    }
}

impl crate::DensitySource for VerticalSliceDensitySource {
    fn sample_density(&self, world_x: f32, world_y: f32, world_z: f32) -> f32 {
        self.inner.sample_density(world_x, world_y, world_z)
    }
}

// Keep AnalyticShape from original file below
