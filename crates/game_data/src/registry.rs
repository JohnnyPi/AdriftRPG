use std::collections::BTreeMap;

use shared::{DataError, DataResult, StableId};

use crate::compile::{
    CompiledApp, CompiledBiomes, CompiledCamera, CompiledCave, CompiledDebug, CompiledLighting,
    CompiledPerformance, CompiledPlayer, CompiledTerrain, CompiledTerrainMaterials,
    CompiledVegetation, CompiledWater, CompiledWorld,
};
use crate::definitions::RawDefinition;
use crate::hash::registry_hash;
use crate::load::LoadedFile;
use crate::validate::validate_definitions;

#[derive(Clone, Debug)]
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
    pub vegetation: BTreeMap<StableId, CompiledVegetation>,
    pub debug: BTreeMap<StableId, CompiledDebug>,
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
        let mut vegetation = BTreeMap::new();
        let mut debug = BTreeMap::new();
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
                RawDefinition::Vegetation(def) => {
                    vegetation.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Cave(def) => {
                    caves.insert(def.header.id.clone(), def.into());
                }
                RawDefinition::Debug(def) => {
                    debug.insert(def.header.id.clone(), def.into());
                }
            }
        }

        let app = app.ok_or_else(|| DataError::InvalidValue {
            context: "registry".to_string(),
            message: "missing required app definition".to_string(),
        })?;

        resolve_app_references(&app, &performance, &player, &camera, &worlds)?;

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
            vegetation,
            debug,
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

    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        append_stable_id(&mut bytes, &self.app.id);
        append_stable_id(&mut bytes, &self.app.world);
        append_stable_id(&mut bytes, &self.app.player);
        append_stable_id(&mut bytes, &self.app.camera);
        append_stable_id(&mut bytes, &self.app.performance);

        for (id, world) in &self.worlds {
            append_stable_id(&mut bytes, id);
            bytes.extend(world.seed.to_le_bytes());
            bytes.extend(world.cell_size_m.to_le_bytes());
            for value in world.chunk_cells {
                bytes.extend(value.to_le_bytes());
            }
            for value in world.world_extent_chunks {
                bytes.extend(value.to_le_bytes());
            }
            append_stable_id(&mut bytes, &world.terrain);
            append_stable_id(&mut bytes, &world.biomes);
            append_stable_id(&mut bytes, &world.materials);
            append_stable_id(&mut bytes, &world.water);
            append_stable_id(&mut bytes, &world.lighting);
        }

        for (id, player) in &self.player {
            append_stable_id(&mut bytes, id);
            bytes.extend(player.walk_speed_mps.to_le_bytes());
            bytes.extend(player.run_speed_mps.to_le_bytes());
            bytes.extend(player.gravity_mps2.to_le_bytes());
        }

        for (id, camera) in &self.camera {
            append_stable_id(&mut bytes, id);
            bytes.extend(camera.distance_default_m.to_le_bytes());
            bytes.extend(camera.distance_minimum_m.to_le_bytes());
            bytes.extend(camera.distance_maximum_m.to_le_bytes());
        }

        for (id, lighting) in &self.lighting {
            append_stable_id(&mut bytes, id);
            bytes.push(lighting.fog_enabled as u8);
            bytes.extend(lighting.fog_start_m.to_le_bytes());
            bytes.extend(lighting.fog_end_m.to_le_bytes());
        }

        for (id, water) in &self.water {
            append_stable_id(&mut bytes, id);
            bytes.extend(water.sea_level_m.to_le_bytes());
            bytes.extend(water.transparency.to_le_bytes());
        }

        for (id, performance) in &self.performance {
            append_stable_id(&mut bytes, id);
            bytes.extend(performance.target_fps.to_le_bytes());
            bytes.extend(performance.target_resolution[0].to_le_bytes());
            bytes.extend(performance.target_resolution[1].to_le_bytes());
        }

        bytes
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

fn append_stable_id(bytes: &mut Vec<u8>, id: &StableId) {
    let value = id.as_str();
    bytes.extend((value.len() as u32).to_le_bytes());
    bytes.extend(value.as_bytes());
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
}
