// crates/game_bevy/src/ui/hud.rs
use bevy::prelude::*;
use physics_bridge::GroundedState;

use crate::environment::{biomes::classify_biome, BiomeCatalog};
use crate::player::{Player, PlayerMovementState};
use crate::state::AppState;
use crate::terrain::TerrainPipelineState;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Running), spawn_hud)
            .add_systems(Update, update_hud.run_if(in_state(AppState::Running)));
    }
}

#[derive(Component)]
struct HudRoot;

#[derive(Component)]
struct HudSpeedText;

#[derive(Component)]
struct HudGroundedText;

#[derive(Component)]
struct HudBiomeText;

#[derive(Component)]
struct HudWaterText;

fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(12.0),
                left: Val::Px(12.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            Visibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((HudSpeedText, Text::new("Speed: 0.0 m/s")));
            parent.spawn((HudGroundedText, Text::new("Grounded: no")));
            parent.spawn((HudBiomeText, Text::new("Biome: -")));
            parent.spawn((HudWaterText, Text::new("")));
        });
}

fn update_hud(
    pipeline: Res<TerrainPipelineState>,
    biomes: Res<BiomeCatalog>,
    player: Query<(&Transform, &PlayerMovementState, &GroundedState), With<Player>>,
    mut speed: Query<&mut Text, (With<HudSpeedText>, Without<HudGroundedText>, Without<HudBiomeText>)>,
    mut grounded: Query<
        &mut Text,
        (With<HudGroundedText>, Without<HudSpeedText>, Without<HudBiomeText>),
    >,
    mut biome_text: Query<
        &mut Text,
        (With<HudBiomeText>, Without<HudSpeedText>, Without<HudGroundedText>, Without<HudWaterText>),
    >,
    mut water_text: Query<
        &mut Text,
        (With<HudWaterText>, Without<HudSpeedText>, Without<HudGroundedText>, Without<HudBiomeText>),
    >,
    mut last_biome: Local<Option<(i32, i32, i32, String)>>,
) {
    let Ok((transform, movement, grounded_state)) = player.single() else {
        return;
    };
    let speed_mps = movement.planar_velocity.length();
    if let Ok(mut text) = speed.single_mut() {
        **text = format!("Speed: {speed_mps:.1} m/s");
    }
    if let Ok(mut text) = grounded.single_mut() {
        **text = format!(
            "Grounded: {}",
            if grounded_state.grounded { "yes" } else { "no" }
        );
    }
    if let (Some(source), Ok(mut text)) = (pipeline.density_source.as_ref(), biome_text.single_mut())
    {
        let p = transform.translation;
        let cell = (
            p.x.floor() as i32,
            p.y.floor() as i32,
            p.z.floor() as i32,
        );
        let label = if last_biome
            .as_ref()
            .is_some_and(|(x, y, z, _)| (*x, *y, *z) == cell)
        {
            last_biome.as_ref().unwrap().3.clone()
        } else {
            let density = source.density_at(p.x, p.y, p.z);
            let biome = classify_biome(biomes.as_ref(), source.as_ref(), p.x, p.y, p.z, density);
            let label = format!("Biome: {biome:?}");
            *last_biome = Some((cell.0, cell.1, cell.2, label.clone()));
            label
        };
        **text = label;
    }
    if let Ok(mut text) = water_text.single_mut() {
        **text = if movement.in_shallow_water {
            "Shallow water".to_string()
        } else {
            String::new()
        };
    }
}
