//! YAML-driven structure spawner (forts, shelters).

use avian3d::prelude::{Collider, CollisionLayers, RigidBody};
use bevy::prelude::*;
use crate::data::ConfigRegistryResource;
use crate::state::AppState;
use crate::world::{requested_world_id, WorldSemanticRegistry, WorldSemanticTag};
use crate::ui::WorldTweaks;
use physics_bridge::{layers_for_profile, CollisionProfileId};

#[derive(Component)]
pub struct StructurePart;

pub struct StructurePlugin;

impl Plugin for StructurePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Running), spawn_structures);
    }
}

fn spawn_structures(
    mut commands: Commands,
    registry: Res<ConfigRegistryResource>,
    world_tweaks: Res<WorldTweaks>,
    mut semantic: ResMut<WorldSemanticRegistry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let world_id = requested_world_id(&registry, &world_tweaks);
    let Ok(world) = registry.0.effective_world(Some(&world_id)) else {
        return;
    };
    let structures = registry.0.world_structures(world);
    if structures.is_empty() {
        return;
    }

    let stone = materials.add(StandardMaterial {
        base_color: Color::srgb(0.48, 0.46, 0.42),
        perceptual_roughness: 0.85,
        ..default()
    });
    let wood = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.42, 0.28),
        perceptual_roughness: 0.9,
        ..default()
    });

    for structure in structures {
        let profile = parse_collision(&structure.collision);
        let layers = CollisionLayers::from(layers_for_profile(profile));
        let yaw = structure.yaw_deg.to_radians();
        let anchor = Vec3::from_array(world.recipe_to_world(structure.anchor));

        semantic.facts.push(crate::world::WorldSemanticFact {
            tag: WorldSemanticTag::Shelter,
            position: anchor,
            label: structure.id.as_str().to_string(),
        });

        for part in &structure.parts {
            let mat = match part.material.as_deref() {
                Some("fort_wood") => wood.clone(),
                _ => stone.clone(),
            };
            let local = rotated_offset(part.offset, yaw);
            let pos = anchor + local;

            match part.kind.as_str() {
                "box" => {
                    let size = part.size.unwrap_or([1.0, 1.0, 1.0]);
                    let mesh = meshes.add(Cuboid::new(size[0], size[1], size[2]));
                    commands.spawn((
                        StructurePart,
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(mat),
                        Transform::from_translation(pos)
                            .with_rotation(Quat::from_rotation_y(yaw)),
                        RigidBody::Static,
                        Collider::cuboid(size[0] * 0.5, size[1] * 0.5, size[2] * 0.5),
                        layers,
                    ));
                }
                "cylinder" => {
                    let radius = part.radius.unwrap_or(1.0);
                    let height = part.height.unwrap_or(2.0);
                    let mesh = meshes.add(Cylinder::new(radius, height));
                    commands.spawn((
                        StructurePart,
                        Mesh3d(mesh),
                        MeshMaterial3d(mat),
                        Transform::from_translation(pos)
                            .with_rotation(Quat::from_rotation_y(yaw)),
                        RigidBody::Static,
                        Collider::cylinder(radius, height * 0.5),
                        layers,
                    ));
                }
                _ => {}
            }
        }
    }
}

fn rotated_offset(offset: [f32; 3], yaw: f32) -> Vec3 {
    let v = Vec3::new(offset[0], offset[1], offset[2]);
    Quat::from_rotation_y(yaw) * v
}

fn parse_collision(value: &str) -> CollisionProfileId {
    match value {
        "dynamic_prop" => CollisionProfileId::DynamicProp,
        "moving_platform" => CollisionProfileId::MovingPlatform,
        "static_terrain" | "terrain" => CollisionProfileId::Terrain,
        _ => CollisionProfileId::Terrain,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_data::load_registry_from_directory;
    use std::path::PathBuf;

    fn workspace_assets() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .canonicalize()
            .expect("assets")
    }

    #[test]
    fn coastal_fort_yaml_compiles_with_parts() {
        let registry = load_registry_from_directory(workspace_assets()).expect("registry");
        let fort = registry
            .structures
            .get(&shared::StableId::new("structure.coastal_fort"))
            .expect("fort");
        assert_eq!(fort.parts.len(), 6);
        assert!(fort.parts.iter().any(|p| p.tag.as_deref() == Some("gate")));
    }
}
