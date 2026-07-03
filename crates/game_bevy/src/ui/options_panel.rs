// crates/game_bevy/src/ui/options_panel.rs
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::data::ConfigRegistryResource;
use crate::state::AppState;

use super::tweaks::{
    AtmosphereTweaks, CameraTweaks, EcologyTweaks, LightingTweaks, MovementTweaks,
    PhysicsTweaks, RiverTweaks, TerrainTweaks, WaterPhysicsTweaks, WaterTweaks, WorldTweaks,
};

#[derive(Resource, Clone, Debug)]
pub struct OptionsPanelState {
    pub open: bool,
    pub tab: OptionsTab,
}

impl Default for OptionsPanelState {
    fn default() -> Self {
        Self {
            open: false,
            tab: OptionsTab::Atmosphere,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OptionsTab {
    #[default]
    Atmosphere,
    World,
    Movement,
    Physics,
    Water,
    Debug,
}

const STUB_FEATURES: &[(&str, &str)] = &[
    ("weather", "Weather simulation"),
    ("day_night", "Day / night cycle"),
    ("networking", "Multiplayer"),
    ("save_load", "Save / load"),
    ("archipelago", "Archipelago streaming"),
    ("dual_contouring", "Dual contouring mesher"),
    ("volumetric_clouds", "Volumetric clouds"),
    ("swimming", "Full swimming locomotion"),
    ("quests", "Quest system"),
    ("vehicles", "Vehicle physics"),
    ("ragdolls", "Ragdoll physics"),
    ("volumetric_water", "Volumetric water"),
    ("combat_facing", "Combat facing mode"),
];

pub struct OptionsPanelPlugin;

impl Plugin for OptionsPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OptionsPanelState>()
            .init_resource::<LightingTweaks>()
            .init_resource::<MovementTweaks>()
            .init_resource::<PhysicsTweaks>()
            .init_resource::<WorldTweaks>()
            .init_resource::<TerrainTweaks>()
            .init_resource::<WaterTweaks>()
            .init_resource::<RiverTweaks>()
            .init_resource::<AtmosphereTweaks>()
            .init_resource::<CameraTweaks>()
            .init_resource::<WaterPhysicsTweaks>()
            .init_resource::<EcologyTweaks>()
            .add_systems(
                OnEnter(AppState::MainMenu),
                init_options_from_registry,
            )
            .add_systems(OnEnter(AppState::Running), init_options_from_registry)
            .add_systems(
                Update,
                toggle_options_panel
                    .run_if(in_state(AppState::MainMenu).or_else(in_state(AppState::Running))),
            )
            .add_systems(
                EguiPrimaryContextPass,
                draw_options_panel.run_if(
                    in_state(AppState::MainMenu).or_else(in_state(AppState::Running)),
                ),
            );
    }
}

#[derive(Resource, Clone, Debug)]
pub struct OptionsKeyBindings {
    pub toggle: KeyCode,
}

fn init_options_from_registry(
    registry: Res<ConfigRegistryResource>,
    mut panel: ResMut<OptionsPanelState>,
    mut commands: Commands,
) {
    if let Ok(options) = registry.0.active_options() {
        panel.tab = parse_options_tab(&options.default_tab);
        commands.insert_resource(OptionsKeyBindings {
            toggle: parse_options_key(&options.toggle_key).unwrap_or(KeyCode::Escape),
        });
    }
}

fn parse_options_tab(name: &str) -> OptionsTab {
    match name {
        "world" => OptionsTab::World,
        "movement" => OptionsTab::Movement,
        "physics" => OptionsTab::Physics,
        "water" => OptionsTab::Water,
        "debug" => OptionsTab::Debug,
        _ => OptionsTab::Atmosphere,
    }
}

fn parse_options_key(name: &str) -> Option<KeyCode> {
    match name.to_ascii_lowercase().as_str() {
        "escape" => Some(KeyCode::Escape),
        "f11" => Some(KeyCode::F11),
        "f1" => Some(KeyCode::F1),
        _ => None,
    }
}

fn toggle_options_panel(
    keyboard: Res<ButtonInput<KeyCode>>,
    keys: Option<Res<OptionsKeyBindings>>,
    mut panel: ResMut<OptionsPanelState>,
) {
    let toggle = keys.map(|k| k.toggle).unwrap_or(KeyCode::Escape);
    if keyboard.just_pressed(toggle) || keyboard.just_pressed(KeyCode::F11) {
        panel.open = !panel.open;
    }
}

fn draw_options_panel(
    mut contexts: EguiContexts,
    mut panel: ResMut<OptionsPanelState>,
    mut lighting: ResMut<LightingTweaks>,
    mut movement: ResMut<MovementTweaks>,
    mut physics: ResMut<PhysicsTweaks>,
    mut world: ResMut<WorldTweaks>,
    mut terrain: ResMut<TerrainTweaks>,
    mut water: ResMut<WaterTweaks>,
    mut river: ResMut<RiverTweaks>,
    mut atmosphere: ResMut<AtmosphereTweaks>,
    mut camera: ResMut<CameraTweaks>,
    mut water_physics: ResMut<WaterPhysicsTweaks>,
    mut ecology: ResMut<EcologyTweaks>,
    registry: Res<ConfigRegistryResource>,
) {
    if !panel.open {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::Window::new("RPG Adrift — Options")
        .default_width(380.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                tab_button(ui, &mut panel.tab, OptionsTab::Atmosphere, "Atmosphere");
                tab_button(ui, &mut panel.tab, OptionsTab::World, "World");
                tab_button(ui, &mut panel.tab, OptionsTab::Movement, "Movement");
                tab_button(ui, &mut panel.tab, OptionsTab::Physics, "Physics");
                tab_button(ui, &mut panel.tab, OptionsTab::Water, "Water");
                tab_button(ui, &mut panel.tab, OptionsTab::Debug, "Debug");
            });
            ui.separator();

            match panel.tab {
                OptionsTab::Atmosphere => draw_atmosphere_tab(ui, &mut lighting, &mut atmosphere),
                OptionsTab::World => draw_world_tab(ui, &mut world, &mut terrain),
                OptionsTab::Movement => draw_movement_tab(ui, &mut movement, &registry),
                OptionsTab::Physics => draw_physics_tab(ui, &mut physics),
                OptionsTab::Water => {
                    draw_water_tab(ui, &mut water, &mut river, &mut water_physics)
                }
                OptionsTab::Debug => {
                    draw_debug_tab(ui, &mut camera, &mut atmosphere, &mut ecology)
                }
            }

            ui.separator();
            ui.collapsing("Coming soon (stubbed)", |ui| {
                for (_id, label) in STUB_FEATURES {
                    ui.add_enabled_ui(false, |ui| {
                        ui.label(format!("{label} — not yet available"));
                    });
                }
            });
        });
}

fn tab_button(ui: &mut egui::Ui, current: &mut OptionsTab, tab: OptionsTab, label: &str) {
    if ui.selectable_label(*current == tab, label).clicked() {
        *current = tab;
    }
}

fn draw_atmosphere_tab(
    ui: &mut egui::Ui,
    lighting: &mut LightingTweaks,
    atmosphere: &mut AtmosphereTweaks,
) {
    ui.heading("Fog (live)");
    ui.label("Adjust distance fog color and range in real time.");
    ui.horizontal(|ui| {
        ui.label("R");
        ui.add(egui::Slider::new(&mut lighting.fog_color[0], 0.0..=1.0));
    });
    ui.horizontal(|ui| {
        ui.label("G");
        ui.add(egui::Slider::new(&mut lighting.fog_color[1], 0.0..=1.0));
    });
    ui.horizontal(|ui| {
        ui.label("B");
        ui.add(egui::Slider::new(&mut lighting.fog_color[2], 0.0..=1.0));
    });
    ui.add(egui::Slider::new(&mut lighting.fog_start_m, 5.0..=120.0).text("start (m)"));
    ui.add(egui::Slider::new(&mut lighting.fog_end_m, 50.0..=400.0).text("end (m)"));

    ui.separator();
    ui.heading("Sun & sky");
    ui.checkbox(&mut atmosphere.use_overrides, "Override YAML");
    ui.add_enabled(
        atmosphere.use_overrides,
        egui::Slider::new(&mut atmosphere.sun_azimuth_deg, 0.0..=360.0).text("azimuth"),
    );
    ui.add_enabled(
        atmosphere.use_overrides,
        egui::Slider::new(&mut atmosphere.sun_elevation_deg, 5.0..=85.0).text("elevation"),
    );
    ui.add_enabled(
        atmosphere.use_overrides,
        egui::Slider::new(&mut atmosphere.mie_strength, 0.0..=2.0).text("Mie haze"),
    );
    ui.add_enabled(
        atmosphere.use_overrides,
        egui::Slider::new(&mut atmosphere.height_fog_density, 0.0..=0.1).text("height fog"),
    );
}

fn draw_world_tab(ui: &mut egui::Ui, world: &mut WorldTweaks, terrain: &mut TerrainTweaks) {
    ui.label("World profile is selected on the Setup screen (main menu).");

    ui.separator();
    ui.heading("Chunk residency");
    ui.add(egui::Slider::new(&mut world.density_radius, 2..=12).text("density radius"));
    ui.add(egui::Slider::new(&mut world.render_radius, 2..=10).text("render radius"));
    ui.add(egui::Slider::new(&mut world.physics_radius, 2..=8).text("physics radius"));
    ui.checkbox(&mut world.show_residency_rings, "Show residency rings (Ctrl+F7)");

    ui.separator();
    ui.heading("Terrain fields");
    ui.checkbox(&mut terrain.use_overrides, "Override field amplitudes");
    ui.add_enabled(
        terrain.use_overrides,
        egui::Slider::new(&mut terrain.ridge_amplitude, 0.0..=2.0).text("ridge"),
    );
    ui.add_enabled(
        terrain.use_overrides,
        egui::Slider::new(&mut terrain.valley_depth, 0.0..=2.0).text("valley"),
    );
    ui.checkbox(&mut terrain.show_masks, "Show terrain masks");
}

fn draw_movement_tab(
    ui: &mut egui::Ui,
    movement: &mut MovementTweaks,
    registry: &ConfigRegistryResource,
) {
    if let Ok(player) = registry.0.active_player() {
        if !movement.use_overrides {
            movement.walk_speed = player.walk_speed_mps;
            movement.run_speed = player.run_speed_mps;
            movement.acceleration = player.acceleration_mps2;
            movement.deceleration = player.deceleration_mps2;
            movement.max_slope_deg = player.maximum_walkable_slope_deg;
            movement.jump_buffer_s = player.jump_buffer_s;
            movement.coyote_time_s = player.coyote_time_s;
        }
    }

    ui.checkbox(&mut movement.use_overrides, "Override movement");

    ui.add_enabled(
        movement.use_overrides,
        egui::Slider::new(&mut movement.walk_speed, 1.0..=8.0).text("walk speed"),
    );
    ui.add_enabled(
        movement.use_overrides,
        egui::Slider::new(&mut movement.run_speed, 2.0..=12.0).text("run speed"),
    );
    ui.add_enabled(
        movement.use_overrides,
        egui::Slider::new(&mut movement.acceleration, 5.0..=50.0).text("acceleration"),
    );
    ui.add_enabled(
        movement.use_overrides,
        egui::Slider::new(&mut movement.deceleration, 5.0..=60.0).text("deceleration"),
    );
    ui.add_enabled(
        movement.use_overrides,
        egui::Slider::new(&mut movement.jump_buffer_s, 0.05..=0.2).text("jump buffer (s)"),
    );
    ui.add_enabled(
        movement.use_overrides,
        egui::Slider::new(&mut movement.coyote_time_s, 0.05..=0.15).text("coyote time (s)"),
    );
    ui.add_enabled(
        movement.use_overrides,
        egui::Slider::new(&mut movement.max_slope_deg, 30.0..=60.0).text("max slope (deg)"),
    );
}

fn draw_physics_tab(ui: &mut egui::Ui, physics: &mut PhysicsTweaks) {
    ui.checkbox(&mut physics.use_overrides, "Override physics");
    ui.add_enabled(
        physics.use_overrides,
        egui::Slider::new(&mut physics.gravity, 5.0..=30.0).text("gravity"),
    );
    ui.add_enabled(
        physics.use_overrides,
        egui::Slider::new(&mut physics.prop_friction, 0.0..=1.5).text("prop friction"),
    );
    ui.add_enabled(
        physics.use_overrides,
        egui::Slider::new(&mut physics.platform_speed, 0.5..=6.0).text("platform speed"),
    );
    ui.separator();
    ui.add_enabled_ui(false, |ui| {
        ui.label("Buoyancy — coming in Water Physics phase");
        ui.label("Break thresholds — deferred");
    });
}

fn draw_water_tab(
    ui: &mut egui::Ui,
    water: &mut WaterTweaks,
    river: &mut RiverTweaks,
    water_physics: &mut WaterPhysicsTweaks,
) {
    ui.checkbox(&mut water.use_overrides, "Override water bodies");
    ui.add_enabled(
        water.use_overrides,
        egui::Slider::new(&mut water.sea_level_m, -2.0..=2.0).text("sea level"),
    );

    ui.separator();
    ui.heading("River");
    ui.label("River shape comes from island_gen hydrology (Setup screen).");
    ui.checkbox(&mut river.show_spline, "Show river spline (Ctrl+F4)");
    ui.checkbox(&mut river.show_flow_arrows, "Show flow arrows");

    ui.separator();
    ui.heading("Water physics");
    ui.add(
        egui::Slider::new(&mut water_physics.buoyancy_strength, 0.0..=3.0).text("buoyancy"),
    );
    ui.add(
        egui::Slider::new(&mut water_physics.flow_multiplier, 0.0..=3.0).text("flow multiplier"),
    );
    ui.add(
        egui::Slider::new(&mut water_physics.shallow_depth_m, 0.5..=3.0).text("shallow depth"),
    );
}

fn draw_debug_tab(
    ui: &mut egui::Ui,
    camera: &mut CameraTweaks,
    atmosphere: &mut AtmosphereTweaks,
    ecology: &mut EcologyTweaks,
) {
    ui.heading("Time of day");
    ui.checkbox(
        &mut atmosphere.drive_sun_from_time_of_day,
        "Drive sun & sky from clock",
    );
    ui.add_enabled(
        atmosphere.drive_sun_from_time_of_day,
        egui::Slider::new(&mut atmosphere.time_of_day_hours, 0.0..=24.0)
            .text("hours (0=midnight, 12=noon)"),
    );
    if atmosphere.drive_sun_from_time_of_day {
        let (azimuth, elevation) =
            super::tweaks::sun_angles_from_time_of_day(atmosphere.time_of_day_hours);
        atmosphere.sun_azimuth_deg = azimuth;
        atmosphere.sun_elevation_deg = elevation;
    }
    ui.label(format!(
        "Sun azimuth {:.0}°, elevation {:.0}°",
        atmosphere.sun_azimuth_deg, atmosphere.sun_elevation_deg
    ));
    ui.label("Also try Atmosphere tab for Mie haze and fog while testing.");

    ui.separator();
    ui.heading("Fly camera");
    ui.checkbox(&mut camera.fly_cam, "Fly cam (no collision)");
    ui.add_enabled(
        camera.fly_cam,
        egui::Slider::new(&mut camera.fly_cam_speed_mps, 4.0..=80.0).text("speed (m/s)"),
    );
    ui.label("WASD move · Space/Ctrl up/down · Shift sprint · hold LMB to look");

    ui.separator();
    ui.heading("Orbit camera");
    ui.checkbox(&mut camera.use_overrides, "Override camera");
    ui.add_enabled(
        camera.use_overrides,
        egui::Slider::new(&mut camera.orbit_distance, 3.0..=20.0).text("orbit distance"),
    );
    ui.add_enabled(
        camera.use_overrides,
        egui::Slider::new(&mut camera.collision_inward_sharpness, 5.0..=40.0)
            .text("collision in sharpness"),
    );
    ui.add_enabled(
        camera.use_overrides,
        egui::Slider::new(&mut camera.collision_outward_sharpness, 2.0..=20.0)
            .text("collision out sharpness"),
    );

    ui.separator();
    ui.heading("Ecology debug");
    ui.add(
        egui::Slider::new(&mut ecology.vegetation_density, 0.0..=2.0).text("vegetation density"),
    );
    ui.checkbox(&mut ecology.show_wetness_heatmap, "Wetness heatmap");
    ui.add(egui::Slider::new(&mut ecology.biome_debug_mode, 0..=3).text("biome debug mode"));
    ui.label("Ctrl+F6 — cycle VS3 island field gizmo overlay");
}