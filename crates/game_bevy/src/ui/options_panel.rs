// crates/game_bevy/src/ui/options_panel.rs
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use crate::data::ConfigRegistryResource;
use crate::data::{UserSetupPrefs, save_user_prefs};
use crate::environment::SimulationTime;
use crate::environment::celestial::CelestialState;
use crate::environment::lighting_state::{
    EnvironmentLightingState, environment_intensity_with_clouds,
};
use crate::state::AppState;

use super::tweaks::{
    AtmosphereTweaks, CameraTweaks, EcologyTweaks, LightingTweaks, MovementTweaks, PhysicsTweaks,
    RiverTweaks, TerrainMaterialTweaks, TerrainTweaks, WaterPhysicsTweaks, WaterTweaks,
    WorldTweaks,
};
use super::world_select::draw_world_profile_combo;

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
            .init_resource::<TerrainMaterialTweaks>()
            .init_resource::<WaterTweaks>()
            .init_resource::<RiverTweaks>()
            .init_resource::<AtmosphereTweaks>()
            .init_resource::<CameraTweaks>()
            .init_resource::<WaterPhysicsTweaks>()
            .init_resource::<EcologyTweaks>()
            .add_systems(OnEnter(AppState::MainMenu), init_options_from_registry)
            .add_systems(OnEnter(AppState::Running), init_options_from_registry)
            .add_systems(
                Update,
                toggle_options_panel
                    .run_if(in_state(AppState::MainMenu).or_else(in_state(AppState::Running))),
            )
            .add_systems(
                EguiPrimaryContextPass,
                draw_options_panel
                    .run_if(in_state(AppState::MainMenu).or_else(in_state(AppState::Running))),
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
    mut movement: ResMut<MovementTweaks>,
    mut physics: ResMut<PhysicsTweaks>,
    mut water: ResMut<WaterTweaks>,
    mut camera: ResMut<CameraTweaks>,
    mut lighting: ResMut<LightingTweaks>,
) {
    if let Ok(options) = registry.0.active_options() {
        panel.tab = parse_options_tab(&options.default_tab);
        commands.insert_resource(OptionsKeyBindings {
            toggle: parse_options_key(&options.toggle_key).unwrap_or(KeyCode::Escape),
        });
    }
    if let Ok(player) = registry.0.active_player() {
        movement.apply_authored_player(player);
    }
    if let Ok(physics_def) = registry.0.active_physics() {
        physics.apply_authored_physics(physics_def);
    }
    if let Ok(water_def) = registry.0.active_water() {
        water.apply_authored_water(water_def);
    }
    if let Ok(camera_def) = registry.0.active_camera() {
        camera.apply_authored_camera(camera_def);
    }
    if let Some(fog) = registry.0.active_fog() {
        lighting.apply_authored_defaults(fog);
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

#[derive(SystemParam)]
struct OptionsPanelDrawParams<'w> {
    registry: Res<'w, ConfigRegistryResource>,
    prefs: ResMut<'w, UserSetupPrefs>,
    panel: ResMut<'w, OptionsPanelState>,
    lighting: ResMut<'w, LightingTweaks>,
    movement: ResMut<'w, MovementTweaks>,
    physics: ResMut<'w, PhysicsTweaks>,
    world: ResMut<'w, WorldTweaks>,
    terrain: ResMut<'w, TerrainTweaks>,
    water: ResMut<'w, WaterTweaks>,
    river: ResMut<'w, RiverTweaks>,
    atmosphere: ResMut<'w, AtmosphereTweaks>,
    camera: ResMut<'w, CameraTweaks>,
    water_physics: ResMut<'w, WaterPhysicsTweaks>,
    ecology: ResMut<'w, EcologyTweaks>,
    sim_time: ResMut<'w, SimulationTime>,
    lighting_state: ResMut<'w, EnvironmentLightingState>,
    celestial: Option<Res<'w, CelestialState>>,
}

fn draw_options_panel(mut contexts: EguiContexts, mut params: OptionsPanelDrawParams) {
    if !params.panel.open {
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
                tab_button(
                    ui,
                    &mut params.panel.tab,
                    OptionsTab::Atmosphere,
                    "Atmosphere",
                );
                tab_button(ui, &mut params.panel.tab, OptionsTab::World, "World");
                tab_button(ui, &mut params.panel.tab, OptionsTab::Movement, "Movement");
                tab_button(ui, &mut params.panel.tab, OptionsTab::Physics, "Physics");
                tab_button(ui, &mut params.panel.tab, OptionsTab::Water, "Water");
                tab_button(ui, &mut params.panel.tab, OptionsTab::Debug, "Debug");
            });
            ui.separator();

            match params.panel.tab {
                OptionsTab::Atmosphere => draw_atmosphere_tab(
                    ui,
                    &mut params.lighting,
                    &mut params.atmosphere,
                    &mut params.lighting_state,
                ),
                OptionsTab::World => draw_world_tab(
                    ui,
                    &params.registry,
                    &mut params.prefs,
                    &mut params.world,
                    &mut params.terrain,
                ),
                OptionsTab::Movement => draw_movement_tab(ui, &mut params.movement),
                OptionsTab::Physics => draw_physics_tab(ui, &mut params.physics),
                OptionsTab::Water => draw_water_tab(
                    ui,
                    &mut params.water,
                    &mut params.river,
                    &mut params.water_physics,
                ),
                OptionsTab::Debug => draw_debug_tab(
                    ui,
                    &mut params.camera,
                    &mut params.ecology,
                    &mut params.sim_time,
                    &mut params.lighting_state,
                    params.celestial.as_deref(),
                ),
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
    lighting_state: &mut EnvironmentLightingState,
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
    ui.add(egui::Slider::new(&mut lighting.fog_end_m, 100.0..=800.0).text("end (m)"));

    ui.separator();
    ui.heading("Sun");
    ui.checkbox(
        &mut lighting_state.override_sun_angles,
        "Override YAML sun angles",
    );
    ui.add_enabled(
        lighting_state.override_sun_angles,
        egui::Slider::new(&mut lighting_state.override_sun_azimuth_deg, 0.0..=360.0)
            .text("azimuth"),
    );
    ui.add_enabled(
        lighting_state.override_sun_angles,
        egui::Slider::new(&mut lighting_state.override_sun_elevation_deg, -10.0..=85.0)
            .text("elevation"),
    );
    ui.add(egui::Slider::new(&mut atmosphere.height_fog_density, 0.0..=0.1).text("height fog"));
}

fn draw_world_tab(
    ui: &mut egui::Ui,
    registry: &ConfigRegistryResource,
    prefs: &mut UserSetupPrefs,
    world: &mut WorldTweaks,
    terrain: &mut TerrainTweaks,
) {
    ui.heading("World profile");
    if draw_world_profile_combo(ui, &registry.0, &mut prefs.world_id) {
        let _ = save_user_prefs(prefs);
    }
    if let Ok(profile) = registry.0.world_by_id(&prefs.world_stable_id()) {
        if let Some(worldgen) = profile.worldgen.as_ref() {
            ui.label(format!(
                "Terrain source: Milestone A worldgen ({})",
                worldgen.as_str()
            ));
        } else if profile.island_gen.is_some() {
            ui.label("Terrain source: runtime procedural island_gen atlas");
        }
    }

    ui.separator();
    ui.heading("Chunk residency");
    ui.add(egui::Slider::new(&mut world.density_radius, 2..=12).text("density radius"));
    ui.add(egui::Slider::new(&mut world.render_radius, 2..=10).text("render radius"));
    ui.add(egui::Slider::new(&mut world.physics_radius, 2..=8).text("physics radius"));
    ui.checkbox(
        &mut world.show_residency_rings,
        "Show residency rings (Ctrl+F7)",
    );
    ui.checkbox(
        &mut world.show_semantic_landmarks,
        "Show semantic landmarks",
    );

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

fn draw_movement_tab(ui: &mut egui::Ui, movement: &mut MovementTweaks) {
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
    ui.label("Prop friction applies to physics demo crates when overrides are enabled.");
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
    ui.add(egui::Slider::new(&mut water_physics.buoyancy_strength, 0.0..=3.0).text("buoyancy"));
    ui.add(
        egui::Slider::new(&mut water_physics.flow_multiplier, 0.0..=3.0).text("flow multiplier"),
    );
    ui.add(egui::Slider::new(&mut water_physics.shallow_depth_m, 0.5..=3.0).text("shallow depth"));
    ui.add(
        egui::Slider::new(&mut water_physics.swim_up_speed_mps, 0.5..=6.0).text("swim up speed"),
    );
    ui.add(
        egui::Slider::new(&mut water_physics.shallow_speed_scale, 0.1..=1.0)
            .text("shallow speed scale"),
    );
    ui.add(
        egui::Slider::new(&mut water_physics.submerged_sink_cap_mps, 0.5..=4.0)
            .text("submerged sink cap"),
    );
    ui.checkbox(
        &mut water_physics.buoyancy_surface_only,
        "buoyancy surface only (wading band)",
    );
}

fn draw_debug_tab(
    ui: &mut egui::Ui,
    camera: &mut CameraTweaks,
    ecology: &mut EcologyTweaks,
    sim_time: &mut SimulationTime,
    lighting_state: &mut EnvironmentLightingState,
    celestial: Option<&CelestialState>,
) {
    ui.heading("Day / night cycle");
    ui.checkbox(&mut sim_time.auto_advance, "Auto-advance time");
    ui.add(
        egui::Slider::new(&mut sim_time.day_length_minutes, 1.0..=120.0)
            .text("real minutes per full day"),
    );
    ui.add(egui::Slider::new(&mut sim_time.time_scale, 0.0..=8.0).text("time scale"));
    if !sim_time.auto_advance {
        ui.add(egui::Slider::new(&mut sim_time.time_of_day_hours, 0.0..=24.0).text("manual hours"));
    }

    ui.heading("Time of day");
    ui.checkbox(
        &mut lighting_state.drive_sun_from_time_of_day,
        "Drive sun & sky from clock",
    );
    ui.add_enabled(
        lighting_state.drive_sun_from_time_of_day && !sim_time.auto_advance,
        egui::Slider::new(&mut sim_time.time_of_day_hours, 0.0..=24.0)
            .text("hours (0/24=night, 6=dawn, 12=noon, 18=dusk)"),
    );
    let (sun_azimuth, sun_elevation) = if let Some(celestial) = celestial {
        (celestial.sun_azimuth_deg, celestial.sun_elevation_deg)
    } else if lighting_state.drive_sun_from_time_of_day {
        super::tweaks::sun_angles_from_time_of_day(sim_time.time_of_day_hours)
    } else if lighting_state.override_sun_angles {
        (
            lighting_state.override_sun_azimuth_deg,
            lighting_state.override_sun_elevation_deg,
        )
    } else {
        (
            lighting_state.authored_sun_azimuth_deg,
            lighting_state.authored_sun_elevation_deg,
        )
    };
    ui.label(format!(
        "Sun azimuth {:.0}°, elevation {:.0}°",
        sun_azimuth, sun_elevation
    ));

    if let Some(celestial) = celestial {
        let env_map_intensity = environment_intensity_with_clouds(
            celestial.environment_intensity,
            celestial.cloud_cover,
        );
        ui.label(format!(
            "Target EV100 {:.2} · env-map {:.2}",
            celestial.exposure_ev100, env_map_intensity
        ));
        ui.label(format!(
            "Applied EV100 {:.2}",
            lighting_state.current_exposure
        ));
    }

    ui.add_enabled(
        lighting_state.drive_sun_from_time_of_day,
        egui::Slider::new(&mut lighting_state.exposure_ev_min, 7.0..=12.0).text("exposure EV min"),
    );
    ui.add_enabled(
        lighting_state.drive_sun_from_time_of_day,
        egui::Slider::new(&mut lighting_state.exposure_ev_max, 12.0..=16.0).text("exposure EV max"),
    );
    ui.add_enabled(
        lighting_state.drive_sun_from_time_of_day,
        egui::Slider::new(&mut lighting_state.exposure_bias, -1.0..=1.0).text("exposure bias"),
    );
    ui.add_enabled(
        lighting_state.drive_sun_from_time_of_day,
        egui::Slider::new(&mut lighting_state.environment_intensity_scale, 0.5..=1.5)
            .text("env-map scale"),
    );
    ui.label("Scrub the clock to preview day/night lighting balance.");

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
