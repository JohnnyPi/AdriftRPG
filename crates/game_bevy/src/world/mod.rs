// crates/game_bevy/src/world/mod.rs
mod preview;
mod profile;

pub use preview::{
    MapPreviewState, cancel_map_preview_build, hash_prefs, poll_map_preview_build,
    start_map_preview_build,
};
pub use profile::{WorldProfilePlugin, effective_world_from_prefs, requested_world_id};

use bevy::prelude::*;

use crate::data::ConfigRegistryResource;
use crate::data::UserSetupPrefs;

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

impl WorldSemanticRegistry {
    /// Query entry point for quest/AI systems filtering by tag.
    #[allow(dead_code)]
    pub fn facts_with_tag(
        &self,
        tag: WorldSemanticTag,
    ) -> impl Iterator<Item = &WorldSemanticFact> {
        self.facts.iter().filter(move |f| f.tag == tag)
    }

    pub fn nearest_fact(&self, world_pos: Vec3, max_radius_m: f32) -> Option<&WorldSemanticFact> {
        let max_radius_m = max_radius_m.max(0.0);
        self.nearest_fact_any(world_pos)
            .filter(|(_, dist)| *dist <= max_radius_m)
            .map(|(fact, _)| fact)
    }

    pub fn nearest_fact_any(&self, world_pos: Vec3) -> Option<(&WorldSemanticFact, f32)> {
        let probe = Vec2::new(world_pos.x, world_pos.z);
        self.facts
            .iter()
            .map(|fact| {
                let fact_xz = Vec2::new(fact.position.x, fact.position.z);
                (fact, probe.distance(fact_xz))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }
}

pub fn semantic_tag_color(tag: WorldSemanticTag) -> Color {
    match tag {
        WorldSemanticTag::CaveEntrance => Color::srgb(0.55, 0.35, 0.85),
        WorldSemanticTag::Shelter => Color::srgb(0.85, 0.65, 0.25),
        WorldSemanticTag::FreshWater => Color::srgb(0.25, 0.55, 0.95),
        WorldSemanticTag::HighGround => Color::srgb(0.45, 0.85, 0.45),
        WorldSemanticTag::DangerousDrop => Color::srgb(0.95, 0.25, 0.25),
        WorldSemanticTag::ResourceDeposit => Color::srgb(0.75, 0.55, 0.20),
        WorldSemanticTag::BiomeRegion => Color::srgb(0.35, 0.75, 0.65),
        WorldSemanticTag::TraversableRoute => Color::srgb(0.30, 0.90, 0.35),
        WorldSemanticTag::BlockedRoute => Color::srgb(0.90, 0.30, 0.55),
    }
}

pub struct WorldSemanticPlugin;

impl Plugin for WorldSemanticPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldSemanticRegistry>().add_systems(
            OnEnter(crate::state::AppState::Running),
            register_world_facts,
        );
    }
}

fn register_world_facts(
    mut registry: ResMut<WorldSemanticRegistry>,
    config: Res<ConfigRegistryResource>,
    prefs: Res<UserSetupPrefs>,
) {
    let world_id = requested_world_id(&prefs);
    let Ok(world) = config.0.effective_world(Some(&world_id)) else {
        registry.facts.clear();
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
    } else {
        registry.facts.clear();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_fact_within_radius() {
        let registry = WorldSemanticRegistry {
            facts: vec![
                WorldSemanticFact {
                    tag: WorldSemanticTag::Shelter,
                    position: Vec3::new(10.0, 0.0, 10.0),
                    label: "A".into(),
                },
                WorldSemanticFact {
                    tag: WorldSemanticTag::FreshWater,
                    position: Vec3::new(20.0, 0.0, 20.0),
                    label: "B".into(),
                },
            ],
        };
        assert_eq!(
            registry
                .nearest_fact(Vec3::new(11.0, 0.0, 11.0), 5.0)
                .unwrap()
                .label,
            "A"
        );
        assert!(
            registry
                .nearest_fact(Vec3::new(11.0, 0.0, 11.0), 1.0)
                .is_none()
        );
    }

    #[test]
    fn facts_with_tag_filters() {
        let registry = WorldSemanticRegistry {
            facts: vec![
                WorldSemanticFact {
                    tag: WorldSemanticTag::Shelter,
                    position: Vec3::ZERO,
                    label: "shelter".into(),
                },
                WorldSemanticFact {
                    tag: WorldSemanticTag::FreshWater,
                    position: Vec3::ONE,
                    label: "water".into(),
                },
            ],
        };
        assert_eq!(
            registry.facts_with_tag(WorldSemanticTag::Shelter).count(),
            1
        );
    }
}
