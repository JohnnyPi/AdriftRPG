mod profile;

pub use profile::{requested_world_id, WorldProfilePlugin};

use bevy::prelude::*;

use crate::data::ConfigRegistryResource;
use crate::ui::WorldTweaks;

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
    pub tag: WorldSemanticTag,
    pub position: Vec3,
    pub label: String,
}

#[derive(Resource, Default, Debug)]
pub struct WorldSemanticRegistry {
    pub facts: Vec<WorldSemanticFact>,
}

pub struct WorldSemanticPlugin;

impl Plugin for WorldSemanticPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldSemanticRegistry>()
            .add_systems(
                OnEnter(crate::state::AppState::Running),
                (register_world_facts, spawn_landmark_props).chain(),
            );
    }
}

#[derive(Component)]
struct LandmarkSign;

fn spawn_landmark_props(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    registry: Res<WorldSemanticRegistry>,
) {
    let pole_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.48, 0.38),
        ..default()
    });
    let sign_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.72, 0.68, 0.55),
        ..default()
    });
    for fact in &registry.facts {
        if fact.tag == WorldSemanticTag::DangerousDrop {
            continue;
        }
        commands.spawn((
            LandmarkSign,
            Name::new(fact.label.clone()),
            Mesh3d(meshes.add(Cylinder::new(0.12, 2.4))),
            MeshMaterial3d(pole_mat.clone()),
            Transform::from_translation(fact.position + Vec3::Y * 1.2),
        ));
        commands.spawn((
            LandmarkSign,
            Name::new(fact.label.clone()),
            Mesh3d(meshes.add(Cuboid::new(1.2, 0.6, 0.08))),
            MeshMaterial3d(sign_mat.clone()),
            Transform::from_translation(fact.position + Vec3::new(0.0, 2.6, 0.0)),
        ));
    }
}

fn register_world_facts(
    mut registry: ResMut<WorldSemanticRegistry>,
    config: Res<ConfigRegistryResource>,
    world_tweaks: Res<WorldTweaks>,
) {
    let world_id = requested_world_id(&config, &world_tweaks);
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
                })
            })
            .collect();
        for sign in &landmarks.route_signs {
            registry.facts.push(WorldSemanticFact {
                tag: WorldSemanticTag::TraversableRoute,
                position: Vec3::from_array(world.recipe_to_world(sign.position)),
                label: sign.label.clone(),
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
        },
        WorldSemanticFact {
            tag: WorldSemanticTag::CaveEntrance,
            position: Vec3::new(24.0, 4.0, 10.0),
            label: "Demo cave entrance".into(),
        },
        WorldSemanticFact {
            tag: WorldSemanticTag::TraversableRoute,
            position: Vec3::new(82.0, 32.0, 196.0),
            label: "Upland pool".into(),
        },
        WorldSemanticFact {
            tag: WorldSemanticTag::HighGround,
            position: Vec3::new(12.0, 14.0, 8.0),
            label: "Rocky ridge".into(),
        },
    ]
}
