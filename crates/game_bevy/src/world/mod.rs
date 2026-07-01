mod preview;
mod profile;

pub use preview::{generate_map_preview, hash_prefs, MapPreviewState};
pub use profile::{requested_world_id, WorldProfilePlugin};

use bevy::prelude::*;
use terrain_generation::RecipeDensitySource;

use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;
use crate::terrain::{TerrainPipelineState, TerrainSpawnPoint, TerrainWorldInitSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WorldSemanticTag {
    CaveEntrance,
    Shelter,
    FreshWater,
    HighGround,
    DangerousDrop,
    ResourceDeposit,
    BiomeRegion,
    TraversableRoute,
    BlockedRoute,
}

#[derive(Clone, Debug)]
pub struct WorldSemanticFact {
    #[allow(dead_code)]
    pub tag: WorldSemanticTag,
    pub position: Vec3,
    pub label: String,
    /// When true, a decorative route-sign prop is spawned (snapped to terrain).
    pub physical_marker: bool,
}

#[derive(Resource, Default, Debug)]
pub struct WorldSemanticRegistry {
    pub facts: Vec<WorldSemanticFact>,
}

pub struct WorldSemanticPlugin;

impl Plugin for WorldSemanticPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldSemanticRegistry>()
            .add_systems(OnEnter(crate::state::AppState::Running), register_world_facts)
            .add_systems(
                OnEnter(crate::state::AppState::Running),
                spawn_route_sign_props.after(TerrainWorldInitSet),
            );
    }
}

#[derive(Component)]
struct RouteSignProp;

const SPAWN_SIGN_EXCLUSION_M: f32 = 8.0;

fn spawn_route_sign_props(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    registry: Res<WorldSemanticRegistry>,
    pipeline: Res<TerrainPipelineState>,
    spawn_point: Res<TerrainSpawnPoint>,
) {
    let Some(source) = pipeline.density_source.as_ref() else {
        return;
    };

    let pole_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.48, 0.38),
        ..default()
    });
    let sign_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.68, 0.55),
        ..default()
    });

    for fact in &registry.facts {
        if !fact.physical_marker {
            continue;
        }
        let Some(base) = snap_sign_base(source, fact.position.x, fact.position.z) else {
            continue;
        };
        if near_spawn(base, spawn_point.0) {
            continue;
        }

        commands.spawn((
            RouteSignProp,
            Name::new(fact.label.clone()),
            Mesh3d(meshes.add(Cylinder::new(0.12, 2.4))),
            MeshMaterial3d(pole_mat.clone()),
            Transform::from_translation(base + Vec3::Y * 1.2),
        ));
        commands.spawn((
            RouteSignProp,
            Name::new(format!("{} (board)", fact.label)),
            Mesh3d(meshes.add(Cuboid::new(1.2, 0.6, 0.08))),
            MeshMaterial3d(sign_mat.clone()),
            Transform::from_translation(base + Vec3::new(0.0, 2.6, 0.0)),
        ));
    }
}

fn snap_sign_base(source: &RecipeDensitySource, wx: f32, wz: f32) -> Option<Vec3> {
    let floor = source
        .walkable_terrain_floor_at(
            wx,
            wz,
            source.terrain_surface_height_at(wx, wz) + 6.0,
            2.5,
        )
        .or_else(|| Some(source.terrain_surface_height_at(wx, wz)))?;
    if source.is_aabb_fully_embedded_in_terrain(wx, floor + 1.3, wz, [0.12, 1.3, 0.12]) {
        return None;
    }
    Some(Vec3::new(wx, floor, wz))
}

fn near_spawn(base: Vec3, spawn_foot: Vec3) -> bool {
    let dx = base.x - spawn_foot.x;
    let dz = base.z - spawn_foot.z;
    (dx * dx + dz * dz).sqrt() < SPAWN_SIGN_EXCLUSION_M
}

fn register_world_facts(
    mut registry: ResMut<WorldSemanticRegistry>,
    config: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
) {
    let world_id = requested_world_id(&prefs);
    let Ok(world) = config.0.effective_world(Some(&world_id)) else {
        registry.facts = default_compact_facts();
        return;
    };

    if let Some(landmarks) = config.0.effective_landmarks(world) {
        registry.facts = landmarks
            .facts
            .iter()
            .filter_map(|f| {
                Some(WorldSemanticFact {
                    tag: parse_tag(&f.tag)?,
                    position: Vec3::from_array(world.recipe_to_world(f.position)),
                    label: f.label.clone(),
                    physical_marker: false,
                })
            })
            .collect();
        for sign in &landmarks.route_signs {
            registry.facts.push(WorldSemanticFact {
                tag: WorldSemanticTag::TraversableRoute,
                position: Vec3::from_array(world.recipe_to_world(sign.position)),
                label: sign.label.clone(),
                physical_marker: true,
            });
        }
    } else {
        registry.facts = default_compact_facts();
    }
}

fn parse_tag(tag: &str) -> Option<WorldSemanticTag> {
    match tag.to_ascii_lowercase().as_str() {
        "cave_entrance" => Some(WorldSemanticTag::CaveEntrance),
        "shelter" => Some(WorldSemanticTag::Shelter),
        "fresh_water" => Some(WorldSemanticTag::FreshWater),
        "high_ground" => Some(WorldSemanticTag::HighGround),
        "dangerous_drop" => Some(WorldSemanticTag::DangerousDrop),
        "resource_deposit" => Some(WorldSemanticTag::ResourceDeposit),
        "biome_region" => Some(WorldSemanticTag::BiomeRegion),
        "traversable_route" => Some(WorldSemanticTag::TraversableRoute),
        "blocked_route" => Some(WorldSemanticTag::BlockedRoute),
        _ => None,
    }
}

fn default_compact_facts() -> Vec<WorldSemanticFact> {
    vec![
        WorldSemanticFact {
            tag: WorldSemanticTag::FreshWater,
            position: Vec3::new(-30.0, 2.0, -25.0),
            label: "Shoreline".into(),
            physical_marker: false,
        },
        WorldSemanticFact {
            tag: WorldSemanticTag::CaveEntrance,
            position: Vec3::new(24.0, 4.0, 10.0),
            label: "Demo cave entrance".into(),
            physical_marker: false,
        },
        WorldSemanticFact {
            tag: WorldSemanticTag::TraversableRoute,
            position: Vec3::new(82.0, 32.0, 196.0),
            label: "Upland pool".into(),
            physical_marker: false,
        },
        WorldSemanticFact {
            tag: WorldSemanticTag::HighGround,
            position: Vec3::new(12.0, 14.0, 8.0),
            label: "Rocky ridge".into(),
            physical_marker: false,
        },
    ]
}
