use bevy::prelude::*;
use tracing::info;

use crate::player::Player;
use crate::state::AppState;

#[derive(Component)]
pub struct Interactable {
    pub radius_m: f32,
    pub activated: bool,
}

#[derive(Component)]
pub struct InteractionPrompt;

#[derive(Component)]
pub struct CaveBeaconLight;

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Running), spawn_cave_interactable)
            .add_systems(
                Update,
                (update_interaction, toggle_prompt_visibility).run_if(in_state(AppState::Running)),
            );
    }
}

fn spawn_cave_interactable(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Interactable {
            radius_m: 3.0,
            activated: false,
        },
        CaveBeaconLight,
        Mesh3d(meshes.add(Sphere::new(0.6))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.85, 1.0),
            emissive: LinearRgba::from(Color::srgb(0.2, 0.6, 0.9)),
            ..default()
        })),
        Transform::from_xyz(26.0, -1.0, 12.0),
        PointLight {
            intensity: 0.0,
            color: Color::srgb(0.4, 0.9, 1.0),
            ..default()
        },
    ));

    commands.spawn((
        InteractionPrompt,
        Text::new("Press E to activate beacon"),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(80.0),
            left: Val::Percent(50.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

fn update_interaction(
    keyboard: Res<ButtonInput<KeyCode>>,
    player: Query<&Transform, With<Player>>,
    mut interactables: Query<(&Transform, &mut Interactable, &mut PointLight), With<CaveBeaconLight>>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    for (tf, mut interactable, mut light) in &mut interactables {
        let dist = player_tf.translation.distance(tf.translation);
        if dist <= interactable.radius_m && keyboard.just_pressed(KeyCode::KeyE) {
            interactable.activated = !interactable.activated;
            if interactable.activated {
                info!("Cave beacon activated — path illuminated.");
            }
        }
        light.intensity = if interactable.activated { 800000.0 } else { 0.0 };
    }
}

fn toggle_prompt_visibility(
    player: Query<&Transform, With<Player>>,
    interactables: Query<(&Transform, &Interactable), With<CaveBeaconLight>>,
    mut prompt: Query<&mut Visibility, With<InteractionPrompt>>,
) {
    let Ok(player_tf) = player.single() else {
        return;
    };
    let Ok(mut vis) = prompt.single_mut() else {
        return;
    };
    let in_range = interactables.iter().any(|(tf, interactable)| {
        !interactable.activated
            && player_tf.translation.distance(tf.translation) <= interactable.radius_m
    });
    *vis = if in_range {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}
