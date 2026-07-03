// crates/game_data/src/registry.rs
use std::collections::BTreeMap;

use serde::Serialize;
use shared::{DataError, DataResult, StableId};

use crate::compile::{
    CompiledApp, CompiledAtmosphere, CompiledBiomes, CompiledCamera, CompiledCave, CompiledDebug,
    CompiledFog, CompiledIslandGeneration, CompiledLandmarks, CompiledLighting,
    CompiledOptions, CompiledPerformance, CompiledPhysics, CompiledPlayer,
    CompiledRoutes, CompiledSetupSchema, CompiledSky, CompiledStructure, CompiledTerrain, CompiledTerrainMaterials, CompiledSurfaceRules, CompiledVegetation,
    CompiledWater, CompiledWaterBodyMaterial, CompiledHydrologyBody, CompiledWorld,
};
use crate::definitions::RawDefinition;
use crate::surface_registry::{
    build_surface_registry, CompiledSurfaceRegistry, MaterialDependencyIndex,
};
use crate::hash::registry_hash;
use crate::load::LoadedFile;
use crate::validate::validate_definitions;

#[derive(Clone, Debug, Serialize)]
pub struct ConfigRegistry {
    pub app: CompiledApp,
    pub performance: BTreeMap<StableId, CompiledPerformance>,
    pub player: BTreeMap<StableId, CompiledPlayer>,
    pub camera: BTreeMap<StableId, CompiledCamera>,
    pub lighting: BTreeMap<StableId, CompiledLighting>,
    pub water: BTreeMap<StableId, CompiledWater>,
    pub worlds: BTreeMap<StableId, CompiledWorld>,
    pub terrain: BTreeMap<StableId, CompiledTerrain>,
    pub caves: BTreeMap<StableId, CompiledCave>,
    pub biomes: BTreeMap<StableId, CompiledBiomes>,
    pub materials: BTreeMap<StableId, CompiledTerrainMaterials>,
    pub surface_rules: BTreeMap<StableId, CompiledSurfaceRules>,
    pub vegetation: BTreeMap<StableId, CompiledVegetation>,
    pub debug: BTreeMap<StableId, CompiledDebug>,
    pub options: BTreeMap<StableId, CompiledOptions>,
    pub physics: BTreeMap<StableId, CompiledPhysics>,
    pub water_body_materials: BTreeMap<StableId, CompiledWaterBodyMaterial>,
    pub hydrology: BTreeMap<StableId, CompiledHydrologyBody>,
    pub atmosphere: BTreeMap<StableId, CompiledAtmosphere>,
    pub fog: BTreeMap<StableId, CompiledFog>,
    pub sky: BTreeMap<StableId, CompiledSky>,
    pub landmarks: BTreeMap<StableId, CompiledLandmarks>,
    pub routes: BTreeMap<StableId, CompiledRoutes>,
    pub structures: BTreeMap<StableId, CompiledStructure>,
    pub island_gen: BTreeMap<StableId, CompiledIslandGeneration>,
    pub setup_schemas: BTreeMap<StableId, CompiledSetupSchema>,
    pub surface_registries: BTreeMap<StableId, CompiledSurfaceRegistry>,
    pub material_dependencies: BTreeMap<StableId, MaterialDependencyIndex>,
    #[serde(skip)]
    pub hash: String,
}

impl ConfigRegistry {
    pub fn from_loaded_files(files: &[LoadedFile]) -> DataResult<Self> {
        let definitions: Vec<RawDefinition> = files.iter().map(|file| file.definition.clone()).collect();
        validate_definitions(&definitions).into_result()?;

        let mut performance = BTreeMap::new();
        let mut player = BTreeMap::new();
        let mut camera = BTreeMap::new();
        let mut lighting = BTreeMap::new();
        let mut water = BTreeMap::new();
        let mut worlds = BTreeMap::new();
        let mut terrain = BTreeMap::new();
        let mut caves = BTreeMap::new();
        let mut biomes = BTreeMap::new();
        let mut materials = BTreeMap::new();
        let mut surface_rules = BTreeMap::new();
        let mut vegetation = BTreeMap::new();
        let mut debug = BTreeMap::new();
        let mut options = BTreeMap::new();
        let mut physics = BTreeMap::new();
        let mut water_body_materials = BTreeMap::new();
        let mut hydrology = BTreeMap::new();
        let mut atmosphere = BTreeMap::new();
        let mut fog = BTreeMap::new();
        let mut sky = BTreeMap::new();
        let mut landmarks = BTreeMap::new();
        let mut routes = BTreeMap::new();
        let mut structures = BTreeMap::new();
        let mut island_gen = BTreeMap::new();
        let mut setup_schemas = BTreeMap::new();
        let mut app: Option<CompiledApp> = None;

        for definition in &definitions {
            match definition {
                RawDefinition::App(def) => {
                    app = Some(def.into());
                }
                RawDefinition::Performance(def) => {
                    performance.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Player(def) => {
                    player.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Camera(def) => {
                    camera.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Lighting(def) => {
                    lighting.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Water(def) => {
                    water.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::World(def) => {
                    worlds.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::TerrainGeneration(def) => {
                    terrain.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Biomes(def) => {
                    biomes.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::TerrainMaterials(def) => {
                    materials.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::SurfaceRules(def) => {
                    surface_rules.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Vegetation(def) => {
                    vegetation.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Cave(def) => {
                    caves.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Debug(def) => {
                    debug.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Options(def) => {
                    options.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Physics(def) => {
                    physics.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::WaterBodyMaterial(def) => {
                    water_body_materials.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Hydrology(def) => {
                    hydrology.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Atmosphere(def) => {
                    atmosphere.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Fog(def) => {
                    fog.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Sky(def) => {
                    sky.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Landmarks(def) => {
                    landmarks.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Routes(def) => {
                    routes.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Structure(def) => {
                    structures.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::IslandGeneration(def) => {
                    island_gen.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::SetupSchema(def) => {
                    setup_schemas.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::River(_) => {}
                RawDefinition::TextureRecipe(_)
                | RawDefinition::SurfaceMaterial(_)
                | RawDefinition::MaterialCatalog(_)
                | RawDefinition::Overlay(_) => {}
            }
        }

        let app = app.ok_or_else(|| DataError::InvalidValue {
            context: "registry".to_string(),
            message: "missing required app definition".to_string(),
        })?;

        resolve_app_references(&app, &performance, &player, &camera, &worlds)?;

        let mut surface_registries = BTreeMap::new();
        let mut material_dependencies = BTreeMap::new();
        let mut catalog_ids: std::collections::BTreeSet<StableId> = std::collections::BTreeSet::new();
        for def in &definitions {
            if let RawDefinition::MaterialCatalog(cat) = def {
                catalog_ids.insert(cat.header.id.clone());
            }
        }
        for world in worlds.values() {
            if let Some(ref cat_id) = world.material_catalog {
                catalog_ids.insert(cat_id.clone());
            }
        }
        for cat_id in catalog_ids {
            let palette_def = worlds
                .values()
                .find(|w| w.material_catalog.as_ref() == Some(&cat_id))
                .and_then(|w| materials.get(&w.materials));
            let palette_yaml = palette_def.and_then(|compiled: &CompiledTerrainMaterials| {
                definitions.iter().find_map(|d| {
                    if let RawDefinition::TerrainMaterials(m) = d {
                        if m.header.id == compiled.id {
                            return Some(m);
                        }
                    }
                    None
                })
            });
            let (registry, deps) =
                build_surface_registry(&definitions, Some(&cat_id), palette_yaml)?;
            surface_registries.insert(cat_id.clone(), registry);
            material_dependencies.insert(cat_id, deps);
        }

        let mut registry = Self {
            app,
            performance,
            player,
            camera,
            lighting,
            water,
            worlds,
            terrain,
            caves,
            biomes,
            materials,
            surface_rules,
            vegetation,
            debug,
            options,
            physics,
            water_body_materials,
            hydrology,
            atmosphere,
            fog,
            sky,
            landmarks,
            routes,
            structures,
            island_gen,
            setup_schemas,
            surface_registries,
            material_dependencies,
            hash: String::new(),
        };
        registry.hash = registry_hash(&registry);
        Ok(registry)
    }

    pub fn active_world(&self) -> DataResult<&CompiledWorld> {
        self.worlds.get(&self.app.world).ok_or_else(|| DataError::UnknownReference {
            reference: self.app.world.clone(),
            context: "active world".to_string(),
        })
    }

    pub fn world_by_id(&self, id: &StableId) -> DataResult<&CompiledWorld> {
        self.worlds.get(id).ok_or_else(|| DataError::UnknownReference {
            reference: id.clone(),
            context: "world profile".to_string(),
        })
    }

    pub fn effective_world<'a>(&'a self, override_id: Option<&StableId>) -> DataResult<&'a CompiledWorld> {
        if let Some(id) = override_id {
            self.world_by_id(id)
        } else {
            self.active_world()
        }
    }

    pub fn active_options(&self) -> DataResult<&CompiledOptions> {
        self.options
            .get(&StableId::new("options.default"))
            .or_else(|| self.options.values().next())
            .ok_or_else(|| DataError::UnknownReference {
                reference: StableId::new("options.default"),
                context: "active options".to_string(),
            })
    }

    pub fn active_setup_schema(&self) -> DataResult<&CompiledSetupSchema> {
        self.setup_schemas
            .get(&StableId::new("setup.large"))
            .or_else(|| self.setup_schemas.get(&StableId::new("setup.default")))
            .or_else(|| self.setup_schemas.values().next())
            .ok_or_else(|| DataError::UnknownReference {
                reference: StableId::new("setup.large"),
                context: "active setup schema".to_string(),
            })
    }

    /// Setup UI schema for a world profile (`setup.{world_suffix}` convention).
    pub fn effective_setup_schema(&self, world: &CompiledWorld) -> Option<&CompiledSetupSchema> {
        let suffix = world.id.as_str().strip_prefix("world.")?;
        self.setup_schemas
            .get(&StableId::new(format!("setup.{suffix}")))
            .or_else(|| self.active_setup_schema().ok())
    }

    pub fn effective_setup_schema_for_id(&self, world_id: &StableId) -> Option<&CompiledSetupSchema> {
        self.world_by_id(world_id)
            .ok()
            .and_then(|world| self.effective_setup_schema(world))
    }

    pub fn island_generation_for_world(&self, world: &CompiledWorld) -> Option<&CompiledIslandGeneration> {
        world
            .island_gen
            .as_ref()
            .and_then(|id| self.island_gen.get(id))
    }

    pub fn world_profiles(&self) -> impl Iterator<Item = &CompiledWorld> {
        self.worlds.values()
    }

    pub fn active_physics(&self) -> DataResult<&CompiledPhysics> {
        self.physics
            .get(&StableId::new("physics.default"))
            .or_else(|| self.physics.values().next())
            .ok_or_else(|| DataError::UnknownReference {
                reference: StableId::new("physics.default"),
                context: "active physics".to_string(),
            })
    }

    pub fn water_body_material(&self, id: &StableId) -> Option<&CompiledWaterBodyMaterial> {
        self.water_body_materials.get(id)
    }

    pub fn active_atmosphere(&self) -> Option<&CompiledAtmosphere> {
        self.atmosphere
            .get(&StableId::new("atmosphere.default"))
            .or_else(|| self.atmosphere.values().next())
    }

    pub fn active_fog(&self) -> Option<&CompiledFog> {
        self.fog
            .get(&StableId::new("fog.default"))
            .or_else(|| self.fog.values().next())
    }

    pub fn active_sky(&self) -> Option<&CompiledSky> {
        self.sky
            .get(&StableId::new("sky.default"))
            .or_else(|| self.sky.values().next())
    }

    pub fn effective_sky(&self, world: &CompiledWorld) -> Option<&CompiledSky> {
        world
            .sky
            .as_ref()
            .and_then(|id| self.sky.get(id))
            .or_else(|| self.active_sky())
    }

    pub fn effective_landmarks(&self, world: &CompiledWorld) -> Option<&CompiledLandmarks> {
        world
            .landmarks
            .as_ref()
            .and_then(|id| self.landmarks.get(id))
    }

    pub fn world_structures(&self, world: &CompiledWorld) -> Vec<&CompiledStructure> {
        world
            .structures
            .iter()
            .filter_map(|id| self.structures.get(id))
            .collect()
    }

    pub fn world_ocean_extent_m(&self, world: &CompiledWorld) -> f32 {
        world.effective_ocean_extent_m()
    }

    pub fn routes_for_world(&self, world: &CompiledWorld) -> Option<&CompiledRoutes> {
        // Routes follow the world id's suffix by convention:
        // `world.island_testbed` looks up `routes.island_testbed`. Returns
        // None when no routes have been authored for the world. (The old
        // hardcoded `routes.expanded_slice` fallback — including its
        // "any routes at all" branch — died with the op-based worlds.)
        let suffix = world.id.as_str().strip_prefix("world.")?;
        self.routes.get(&StableId::new(format!("routes.{suffix}")))
    }

    pub fn active_player(&self) -> DataResult<&CompiledPlayer> {
        self.player.get(&self.app.player).ok_or_else(|| DataError::UnknownReference {
            reference: self.app.player.clone(),
            context: "active player".to_string(),
        })
    }

    pub fn active_camera(&self) -> DataResult<&CompiledCamera> {
        self.camera.get(&self.app.camera).ok_or_else(|| DataError::UnknownReference {
            reference: self.app.camera.clone(),
            context: "active camera".to_string(),
        })
    }

    pub fn active_performance(&self) -> DataResult<&CompiledPerformance> {
        self.performance
            .get(&self.app.performance)
            .ok_or_else(|| DataError::UnknownReference {
                reference: self.app.performance.clone(),
                context: "active performance profile".to_string(),
            })
    }

    pub fn active_lighting(&self) -> DataResult<&CompiledLighting> {
        let world = self.active_world()?;
        self.lighting.get(&world.lighting).ok_or_else(|| DataError::UnknownReference {
            reference: world.lighting.clone(),
            context: "active lighting".to_string(),
        })
    }

    pub fn active_water(&self) -> DataResult<&CompiledWater> {
        let world = self.active_world()?;
        self.water.get(&world.water).ok_or_else(|| DataError::UnknownReference {
            reference: world.water.clone(),
            context: "active water".to_string(),
        })
    }
}

fn resolve_app_references(
    app: &CompiledApp,
    performance: &BTreeMap<StableId, CompiledPerformance>,
    player: &BTreeMap<StableId, CompiledPlayer>,
    camera: &BTreeMap<StableId, CompiledCamera>,
    worlds: &BTreeMap<StableId, CompiledWorld>,
) -> DataResult<()> {
    if !worlds.contains_key(&app.world) {
        return Err(DataError::UnknownReference {
            reference: app.world.clone(),
            context: "app.world".to_string(),
        });
    }
    if !player.contains_key(&app.player) {
        return Err(DataError::UnknownReference {
            reference: app.player.clone(),
            context: "app.player".to_string(),
        });
    }
    if !camera.contains_key(&app.camera) {
        return Err(DataError::UnknownReference {
            reference: app.camera.clone(),
            context: "app.camera".to_string(),
        });
    }
    if !performance.contains_key(&app.performance) {
        return Err(DataError::UnknownReference {
            reference: app.performance.clone(),
            context: "app.performance".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::load::load_registry_from_directory;
    use std::path::PathBuf;

    fn workspace_assets() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .canonicalize()
            .expect("workspace assets directory")
    }

    #[test]
    fn registry_hash_is_deterministic() {
        let first = load_registry_from_directory(workspace_assets()).expect("first load");
        let second = load_registry_from_directory(workspace_assets()).expect("second load");
        assert_eq!(first.hash, second.hash);
    }

    #[test]
    fn expanded_showcase_sky_enables_clouds() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry");
        let sky = registry
            .sky
            .get(&shared::StableId::new("sky.expanded_showcase"))
            .expect("expanded sky");
        assert!(sky.clouds_enabled);
    }

    #[test]
    fn atmosphere_fog_sky_yaml_are_compiled() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry");
        let atmo = registry.active_atmosphere().expect("atmosphere");
        assert_eq!(atmo.id.as_str(), "atmosphere.default");
        assert!(atmo.sun_illuminance_lux > 0.0);
        let fog = registry.active_fog().expect("fog");
        assert!(fog.distance_end_m > fog.distance_start_m);
        let sky = registry.active_sky().expect("sky");
        assert!(sky.shader.contains("sky.wgsl"));
        assert!(sky.night_zenith_color[2] > sky.night_zenith_color[0]);
        assert!(atmo.moon_enabled);
        assert!(atmo.moon_angular_radius > 0.0);
    }
}
