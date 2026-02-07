use crate::commentary_stub::CommentaryStubState;
use crate::config::{GameConfig, VehicleConfig};
use crate::gameplay::vehicle::{
    PlayerVehicle, VehicleInputState, VehicleStuntMetrics, VehicleTelemetry,
};
use crate::states::GameState;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use std::fs;
use std::path::Path;

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugRunStats>()
            .init_resource::<KeybindOverlayState>()
            .init_resource::<DebugGameplayGuards>()
            .init_resource::<VehicleTuningPanelState>()
            .add_systems(Update, spawn_debug_overlay)
            .add_systems(Update, toggle_keybind_overlay)
            .add_systems(Update, toggle_vehicle_tuning_panel)
            .add_systems(Update, sync_keybind_overlay_visibility)
            .add_systems(OnEnter(GameState::InRun), reset_run_stats)
            .add_systems(
                Update,
                (update_run_stats, update_debug_overlay_text)
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            )
            .add_systems(
                EguiPrimaryContextPass,
                vehicle_tuning_panel_ui
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            );
    }
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct DebugGameplayGuards {
    pub player_invulnerable: bool,
}

#[derive(Component)]
struct DebugOverlayText;

#[derive(Component)]
struct KeybindOverlayText;

#[derive(Component)]
pub struct EnemyDebugMarker;

#[derive(Resource, Debug, Clone)]
pub struct DebugRunStats {
    pub distance_m: f32,
    pub speed_mps: f32,
    pub grounded: bool,
    pub active_segment_id: String,
}

impl Default for DebugRunStats {
    fn default() -> Self {
        Self {
            distance_m: 0.0,
            speed_mps: 0.0,
            grounded: true,
            active_segment_id: "n/a".to_string(),
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
struct KeybindOverlayState {
    visible: bool,
}

#[derive(Debug, Clone)]
struct VehicleTuningParams {
    health: f32,
    acceleration: f32,
    brake_strength: f32,
    air_pitch_torque: f32,
    air_max_rotation_speed: f32,
    max_forward_speed: f32,
    max_reverse_speed: f32,
    max_fall_speed: f32,
    linear_speed_scale: f32,
    ground_coast_damping: f32,
    air_base_damping: f32,
    air_env_drag_factor: f32,
    linear_inertia: f32,
    rotational_inertia: f32,
    gravity_scale: f32,
    suspension_rest_length_m: f32,
    suspension_stiffness: f32,
    suspension_damping: f32,
    suspension_max_compression_m: f32,
    suspension_max_extension_m: f32,
    tire_longitudinal_grip: f32,
    tire_slip_grip_floor: f32,
    front_drive_ratio: f32,
    rear_drive_traction_assist_distance_m: f32,
    rear_drive_traction_assist_min_factor: f32,
    turret_range_m: f32,
    turret_cone_degrees: f32,
    missile_fire_interval_seconds: f32,
    camera_look_ahead_factor: f32,
    camera_look_ahead_min: f32,
    camera_look_ahead_max: f32,
}

impl VehicleTuningParams {
    fn from_vehicle(vehicle: &VehicleConfig) -> Self {
        Self {
            health: vehicle.health,
            acceleration: vehicle.acceleration,
            brake_strength: vehicle.brake_strength,
            air_pitch_torque: vehicle.air_pitch_torque,
            air_max_rotation_speed: vehicle.air_max_rotation_speed,
            max_forward_speed: vehicle.max_forward_speed,
            max_reverse_speed: vehicle.max_reverse_speed,
            max_fall_speed: vehicle.max_fall_speed,
            linear_speed_scale: vehicle.linear_speed_scale,
            ground_coast_damping: vehicle.ground_coast_damping,
            air_base_damping: vehicle.air_base_damping,
            air_env_drag_factor: vehicle.air_env_drag_factor,
            linear_inertia: vehicle.linear_inertia,
            rotational_inertia: vehicle.rotational_inertia,
            gravity_scale: vehicle.gravity_scale,
            suspension_rest_length_m: vehicle.suspension_rest_length_m,
            suspension_stiffness: vehicle.suspension_stiffness,
            suspension_damping: vehicle.suspension_damping,
            suspension_max_compression_m: vehicle.suspension_max_compression_m,
            suspension_max_extension_m: vehicle.suspension_max_extension_m,
            tire_longitudinal_grip: vehicle.tire_longitudinal_grip,
            tire_slip_grip_floor: vehicle.tire_slip_grip_floor,
            front_drive_ratio: vehicle.front_drive_ratio,
            rear_drive_traction_assist_distance_m: vehicle.rear_drive_traction_assist_distance_m,
            rear_drive_traction_assist_min_factor: vehicle.rear_drive_traction_assist_min_factor,
            turret_range_m: vehicle.turret_range_m,
            turret_cone_degrees: vehicle.turret_cone_degrees,
            missile_fire_interval_seconds: vehicle.missile_fire_interval_seconds,
            camera_look_ahead_factor: vehicle.camera_look_ahead_factor,
            camera_look_ahead_min: vehicle.camera_look_ahead_min,
            camera_look_ahead_max: vehicle.camera_look_ahead_max,
        }
    }

    fn apply_to_vehicle(&self, vehicle: &mut VehicleConfig) {
        vehicle.health = self.health;
        vehicle.acceleration = self.acceleration;
        vehicle.brake_strength = self.brake_strength;
        vehicle.air_pitch_torque = self.air_pitch_torque;
        vehicle.air_max_rotation_speed = self.air_max_rotation_speed;
        vehicle.max_forward_speed = self.max_forward_speed;
        vehicle.max_reverse_speed = self.max_reverse_speed;
        vehicle.max_fall_speed = self.max_fall_speed;
        vehicle.linear_speed_scale = self.linear_speed_scale;
        vehicle.ground_coast_damping = self.ground_coast_damping;
        vehicle.air_base_damping = self.air_base_damping;
        vehicle.air_env_drag_factor = self.air_env_drag_factor;
        vehicle.linear_inertia = self.linear_inertia;
        vehicle.rotational_inertia = self.rotational_inertia;
        vehicle.gravity_scale = self.gravity_scale;
        vehicle.suspension_rest_length_m = self.suspension_rest_length_m;
        vehicle.suspension_stiffness = self.suspension_stiffness;
        vehicle.suspension_damping = self.suspension_damping;
        vehicle.suspension_max_compression_m = self.suspension_max_compression_m;
        vehicle.suspension_max_extension_m = self.suspension_max_extension_m;
        vehicle.tire_longitudinal_grip = self.tire_longitudinal_grip;
        vehicle.tire_slip_grip_floor = self.tire_slip_grip_floor;
        vehicle.front_drive_ratio = self.front_drive_ratio;
        vehicle.rear_drive_traction_assist_distance_m = self.rear_drive_traction_assist_distance_m;
        vehicle.rear_drive_traction_assist_min_factor = self.rear_drive_traction_assist_min_factor;
        vehicle.turret_range_m = self.turret_range_m;
        vehicle.turret_cone_degrees = self.turret_cone_degrees;
        vehicle.missile_fire_interval_seconds = self.missile_fire_interval_seconds;
        vehicle.camera_look_ahead_factor = self.camera_look_ahead_factor;
        vehicle.camera_look_ahead_min = self.camera_look_ahead_min;
        vehicle.camera_look_ahead_max = self.camera_look_ahead_max;
    }
}

#[derive(Resource, Debug, Default)]
struct VehicleTuningPanelState {
    visible: bool,
    source_vehicle_id: String,
    params: Option<VehicleTuningParams>,
    status: String,
}

fn spawn_debug_overlay(
    mut commands: Commands,
    keybind_overlay: Res<KeybindOverlayState>,
    config: Option<Res<GameConfig>>,
    existing_overlay: Query<Entity, With<DebugOverlayText>>,
) {
    if !existing_overlay.is_empty() {
        return;
    }

    let Some(config) = config else {
        return;
    };

    if !config.game.app.debug_overlay {
        return;
    }

    commands.spawn((
        DebugOverlayText,
        Text::new("debug overlay initializing..."),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.92, 0.95, 0.97)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(12.0),
            ..default()
        },
        ZIndex(100),
    ));

    commands.spawn((
        KeybindOverlayText,
        Text::new(keybind_overlay_text()),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgb(0.90, 0.94, 0.97)),
        BackgroundColor(Color::srgba(0.06, 0.08, 0.10, 0.82)),
        BorderColor::all(Color::srgba(0.60, 0.68, 0.74, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(162.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        if keybind_overlay.visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        },
        ZIndex(100),
    ));
}

fn reset_run_stats(mut run_stats: ResMut<DebugRunStats>) {
    run_stats.distance_m = 0.0;
    run_stats.speed_mps = 0.0;
    run_stats.grounded = true;
}

fn update_run_stats(
    time: Res<Time>,
    config: Res<GameConfig>,
    vehicle: Option<Res<VehicleTelemetry>>,
    mut run_stats: ResMut<DebugRunStats>,
) {
    match vehicle {
        Some(vehicle) => {
            run_stats.distance_m = vehicle.distance_m;
            run_stats.speed_mps = vehicle.speed_mps;
            run_stats.grounded = vehicle.grounded;
        }
        None => {
            run_stats.distance_m += run_stats.speed_mps * time.delta_secs();
        }
    }

    let mut cursor = 0.0_f32;
    let mut active_segment = "n/a".to_string();

    for segment in &config.segments.segment_sequence {
        cursor += segment.length;
        active_segment = segment.id.clone();
        if run_stats.distance_m <= cursor {
            break;
        }
    }

    run_stats.active_segment_id = active_segment;
}

#[allow(clippy::too_many_arguments)]
fn update_debug_overlay_text(
    diagnostics: Res<DiagnosticsStore>,
    run_stats: Res<DebugRunStats>,
    enemy_query: Query<(), With<EnemyDebugMarker>>,
    player_query: Query<&Transform, With<PlayerVehicle>>,
    commentary: Option<Res<CommentaryStubState>>,
    input_state: Option<Res<VehicleInputState>>,
    stunts: Option<Res<VehicleStuntMetrics>>,
    mut overlay_query: Query<&mut Text, With<DebugOverlayText>>,
) {
    let Ok(mut text) = overlay_query.single_mut() else {
        return;
    };

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|value| value.smoothed())
        .unwrap_or(0.0);

    let enemy_count = enemy_query.iter().count();
    let player_x = player_query
        .single()
        .map(|transform| transform.translation.x)
        .unwrap_or(run_stats.distance_m);
    let (input_accel, input_brake) = input_state
        .map(|state| (state.accelerate, state.brake))
        .unwrap_or((false, false));

    let (queue_len, last_line) = match commentary {
        Some(state) => (
            state.queue.len(),
            if state.last_line.is_empty() {
                "n/a".to_string()
            } else {
                state.last_line.clone()
            },
        ),
        None => (0, "n/a".to_string()),
    };

    let (airtime_cur, airtime_best, wheelie_best, flip_count, crash_count, max_speed, last_impact) =
        match stunts {
            Some(stunts) => (
                stunts.airtime_current_s,
                stunts.airtime_best_s,
                stunts.wheelie_best_s,
                stunts.flip_count,
                stunts.crash_count,
                stunts.max_speed_mps,
                stunts.last_landing_impact_speed_mps,
            ),
            None => (0.0, 0.0, 0.0, 0, 0, 0.0, 0.0),
        };

    *text = Text::new(format!(
        "FPS: {fps:>5.1}\nDistance: {distance:>7.1}m | X: {player_x:>7.1}m\nSpeed: {speed:>6.1} m/s\nInput: accel={accel} brake={brake}\nGrounded: {grounded}\nAirtime: {air_cur:>4.2}s (best {air_best:>4.2})\nWheelie Best: {wheelie_best:>4.2}s | Flips: {flips} | Crashes: {crashes}\nMax Speed: {max_speed:>6.1} m/s | Last Impact: {impact:>5.1} m/s\nActive Segment: {segment}\nEnemy Count: {enemy_count}\nCommentary Queue: {queue_len}\nLast Commentary: {last_line}\nHotkeys: H help | V vehicle tune | F5 reload config | J big jump | K kill | C crash",
        distance = run_stats.distance_m,
        player_x = player_x,
        speed = run_stats.speed_mps,
        accel = if input_accel { "yes" } else { "no" },
        brake = if input_brake { "yes" } else { "no" },
        grounded = if run_stats.grounded { "yes" } else { "no" },
        air_cur = airtime_cur,
        air_best = airtime_best,
        wheelie_best = wheelie_best,
        flips = flip_count,
        crashes = crash_count,
        max_speed = max_speed,
        impact = last_impact,
        segment = run_stats.active_segment_id,
    ));
}

fn toggle_keybind_overlay(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<KeybindOverlayState>,
    config: Option<Res<GameConfig>>,
) {
    let Some(config) = config else {
        return;
    };

    if !config.game.app.debug_overlay {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyH) {
        state.visible = !state.visible;
        info!(
            "Debug keybind panel {}.",
            if state.visible { "shown" } else { "hidden" }
        );
    }
}

fn sync_keybind_overlay_visibility(
    state: Res<KeybindOverlayState>,
    mut query: Query<&mut Visibility, With<KeybindOverlayText>>,
) {
    if !state.is_changed() {
        return;
    }

    let next_visibility = if state.visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    for mut visibility in &mut query {
        *visibility = next_visibility;
    }
}

fn toggle_vehicle_tuning_panel(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut debug_guards: ResMut<DebugGameplayGuards>,
    mut panel_state: ResMut<VehicleTuningPanelState>,
    config: Option<Res<GameConfig>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyV) {
        return;
    }

    panel_state.visible = !panel_state.visible;
    debug_guards.player_invulnerable = panel_state.visible;
    if panel_state.visible {
        if let Some(config) = config {
            if let Err(error) = sync_panel_state_from_config(&mut panel_state, &config) {
                panel_state.status = error;
            }
        }
        info!("Vehicle tuning panel shown.");
    } else {
        info!("Vehicle tuning panel hidden.");
    }
}

fn vehicle_tuning_panel_ui(
    mut egui_contexts: EguiContexts,
    mut debug_guards: ResMut<DebugGameplayGuards>,
    mut panel_state: ResMut<VehicleTuningPanelState>,
    mut config: ResMut<GameConfig>,
) {
    if !panel_state.visible {
        return;
    }

    if panel_state.params.is_none()
        || panel_state.source_vehicle_id != config.game.app.default_vehicle
    {
        if let Err(error) = sync_panel_state_from_config(&mut panel_state, &config) {
            panel_state.status = error;
            return;
        }
    }

    let Some(mut params) = panel_state.params.clone() else {
        return;
    };

    let mut window_open = panel_state.visible;
    let mut params_changed = false;
    let mut reload_clicked = false;
    let mut apply_clicked = false;
    let status = panel_state.status.clone();
    let vehicle_id = panel_state.source_vehicle_id.clone();

    let Ok(ctx) = egui_contexts.ctx_mut() else {
        return;
    };
    egui::Window::new("Vehicle + Physics Tuning")
        .open(&mut window_open)
        .resizable(true)
        .default_width(680.0)
        .show(ctx, |ui| {
            ui.label(format!("Active vehicle: {vehicle_id}"));
            ui.label("Each row has a slider plus a free-form float value.");
            ui.separator();

            ui.collapsing("Core Dynamics", |ui| {
                params_changed |=
                    tuning_slider_row(ui, "health", &mut params.health, 1.0..=500.0, 0.5);
                params_changed |= tuning_slider_row(
                    ui,
                    "acceleration",
                    &mut params.acceleration,
                    0.0..=600.0,
                    0.5,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "brake_strength (reverse)",
                    &mut params.brake_strength,
                    0.0..=600.0,
                    0.5,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "air_pitch_torque",
                    &mut params.air_pitch_torque,
                    0.0..=400.0,
                    0.5,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "air_max_rotation_speed",
                    &mut params.air_max_rotation_speed,
                    0.1..=40.0,
                    0.05,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "linear_inertia",
                    &mut params.linear_inertia,
                    0.01..=40.0,
                    0.05,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "rotational_inertia",
                    &mut params.rotational_inertia,
                    0.01..=40.0,
                    0.05,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "gravity_scale",
                    &mut params.gravity_scale,
                    0.01..=5.0,
                    0.01,
                );
            });

            ui.collapsing("Speed Limits + Damping", |ui| {
                params_changed |= tuning_slider_row(
                    ui,
                    "max_forward_speed",
                    &mut params.max_forward_speed,
                    0.1..=200.0,
                    0.1,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "max_reverse_speed",
                    &mut params.max_reverse_speed,
                    0.1..=200.0,
                    0.1,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "max_fall_speed",
                    &mut params.max_fall_speed,
                    0.1..=200.0,
                    0.1,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "linear_speed_scale",
                    &mut params.linear_speed_scale,
                    0.01..=20.0,
                    0.02,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "ground_coast_damping",
                    &mut params.ground_coast_damping,
                    0.0..=10.0,
                    0.01,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "air_base_damping",
                    &mut params.air_base_damping,
                    0.0..=10.0,
                    0.01,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "air_env_drag_factor",
                    &mut params.air_env_drag_factor,
                    0.0..=10.0,
                    0.01,
                );
            });

            ui.collapsing("Suspension + Tire Contact", |ui| {
                params_changed |= tuning_slider_row(
                    ui,
                    "suspension_rest_length_m",
                    &mut params.suspension_rest_length_m,
                    0.05..=5.0,
                    0.01,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "suspension_stiffness",
                    &mut params.suspension_stiffness,
                    0.01..=1000.0,
                    0.5,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "suspension_damping",
                    &mut params.suspension_damping,
                    0.0..=1000.0,
                    0.5,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "suspension_max_compression_m",
                    &mut params.suspension_max_compression_m,
                    0.01..=5.0,
                    0.01,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "suspension_max_extension_m",
                    &mut params.suspension_max_extension_m,
                    0.0..=5.0,
                    0.01,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "tire_longitudinal_grip",
                    &mut params.tire_longitudinal_grip,
                    0.01..=5.0,
                    0.01,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "tire_slip_grip_floor",
                    &mut params.tire_slip_grip_floor,
                    0.0..=1.0,
                    0.01,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "front_drive_ratio",
                    &mut params.front_drive_ratio,
                    0.0..=1.0,
                    0.01,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "rear_drive_traction_assist_distance_m",
                    &mut params.rear_drive_traction_assist_distance_m,
                    0.0..=5.0,
                    0.01,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "rear_drive_traction_assist_min_factor",
                    &mut params.rear_drive_traction_assist_min_factor,
                    0.0..=1.0,
                    0.01,
                );
            });

            ui.collapsing("Turret + Camera", |ui| {
                params_changed |= tuning_slider_row(
                    ui,
                    "turret_range_m",
                    &mut params.turret_range_m,
                    0.1..=500.0,
                    0.1,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "turret_cone_degrees",
                    &mut params.turret_cone_degrees,
                    1.0..=180.0,
                    0.1,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "missile_fire_interval_seconds",
                    &mut params.missile_fire_interval_seconds,
                    0.01..=20.0,
                    0.01,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "camera_look_ahead_factor",
                    &mut params.camera_look_ahead_factor,
                    -20.0..=20.0,
                    0.05,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "camera_look_ahead_min",
                    &mut params.camera_look_ahead_min,
                    -1000.0..=1000.0,
                    0.5,
                );
                params_changed |= tuning_slider_row(
                    ui,
                    "camera_look_ahead_max",
                    &mut params.camera_look_ahead_max,
                    -1000.0..=1000.0,
                    0.5,
                );
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Reload From Config").clicked() {
                    reload_clicked = true;
                }
                if ui.button("Apply To vehicles.toml").clicked() {
                    apply_clicked = true;
                }
            });

            if !status.is_empty() {
                ui.separator();
                ui.label(status);
            }
        });

    panel_state.visible = window_open;
    debug_guards.player_invulnerable = panel_state.visible;

    if reload_clicked {
        match sync_panel_state_from_config(&mut panel_state, &config) {
            Ok(()) => panel_state.status = "Reloaded values from current config.".to_string(),
            Err(error) => panel_state.status = error,
        }
        return;
    }

    panel_state.params = Some(params.clone());

    if params_changed {
        if let Err(error) = apply_vehicle_tuning_to_runtime_config(
            &mut config,
            &panel_state.source_vehicle_id,
            &params,
        ) {
            panel_state.status = error;
        } else {
            panel_state.status = "Live-tuning active (in-memory config updated).".to_string();
        }
    }

    if apply_clicked {
        match persist_vehicle_tuning_and_reload(
            &mut config,
            &panel_state.source_vehicle_id,
            &params,
        ) {
            Ok(message) => {
                panel_state.status = message;
                if let Err(error) = sync_panel_state_from_config(&mut panel_state, &config) {
                    panel_state.status = error;
                }
            }
            Err(error) => panel_state.status = error,
        }
    }
}

fn tuning_slider_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    slider_range: std::ops::RangeInclusive<f32>,
    drag_speed: f32,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        changed |= ui
            .add(egui::Slider::new(value, slider_range).show_value(false))
            .changed();
        changed |= ui
            .add(egui::DragValue::new(value).speed(drag_speed as f64))
            .changed();
    });
    changed
}

fn sync_panel_state_from_config(
    panel_state: &mut VehicleTuningPanelState,
    config: &GameConfig,
) -> Result<(), String> {
    let vehicle_id = config.game.app.default_vehicle.clone();
    let Some(vehicle) = config.vehicles_by_id.get(&vehicle_id) else {
        return Err(format!(
            "Vehicle tuning panel: default vehicle `{vehicle_id}` not found in config."
        ));
    };

    panel_state.source_vehicle_id = vehicle_id;
    panel_state.params = Some(VehicleTuningParams::from_vehicle(vehicle));
    Ok(())
}

fn apply_vehicle_tuning_to_runtime_config(
    config: &mut GameConfig,
    vehicle_id: &str,
    params: &VehicleTuningParams,
) -> Result<(), String> {
    let Some(vehicle) = config.vehicles_by_id.get_mut(vehicle_id) else {
        return Err(format!(
            "Vehicle tuning panel: runtime vehicle `{vehicle_id}` not found in vehicles_by_id."
        ));
    };
    params.apply_to_vehicle(vehicle);

    let Some(vehicle) = config
        .vehicles
        .vehicles
        .iter_mut()
        .find(|v| v.id == vehicle_id)
    else {
        return Err(format!(
            "Vehicle tuning panel: runtime vehicle `{vehicle_id}` not found in vehicles list."
        ));
    };
    params.apply_to_vehicle(vehicle);
    Ok(())
}

fn persist_vehicle_tuning_and_reload(
    config: &mut GameConfig,
    vehicle_id: &str,
    params: &VehicleTuningParams,
) -> Result<String, String> {
    let path = Path::new("config").join("vehicles.toml");
    let original_raw = fs::read_to_string(&path)
        .map_err(|error| format!("Failed reading `{}`: {error}", path.display()))?;
    let mut root: toml::Value = toml::from_str(&original_raw)
        .map_err(|error| format!("Failed parsing `{}`: {error}", path.display()))?;

    write_params_to_toml_value(&mut root, vehicle_id, params)?;

    let updated_raw = toml::to_string_pretty(&root)
        .map_err(|error| format!("Failed serializing vehicles TOML: {error}"))?;
    fs::write(&path, updated_raw)
        .map_err(|error| format!("Failed writing `{}`: {error}", path.display()))?;

    match GameConfig::load_from_dir(Path::new("config")) {
        Ok(new_config) => {
            *config = new_config;
            Ok(format!(
                "Applied tuning and saved to {}.",
                path.to_string_lossy()
            ))
        }
        Err(error) => {
            let _ = fs::write(&path, original_raw);
            if let Ok(restored) = GameConfig::load_from_dir(Path::new("config")) {
                *config = restored;
            }
            Err(format!(
                "Apply failed validation: {error}. Reverted `{}`.",
                path.display()
            ))
        }
    }
}

fn write_params_to_toml_value(
    root: &mut toml::Value,
    vehicle_id: &str,
    params: &VehicleTuningParams,
) -> Result<(), String> {
    let Some(vehicles_array) = root.get_mut("vehicles").and_then(toml::Value::as_array_mut) else {
        return Err("vehicles.toml: missing or invalid `vehicles` array".to_string());
    };

    let Some(vehicle_table) = vehicles_array.iter_mut().find_map(|vehicle_value| {
        let table = vehicle_value.as_table_mut()?;
        if table.get("id").and_then(toml::Value::as_str) == Some(vehicle_id) {
            Some(table)
        } else {
            None
        }
    }) else {
        return Err(format!(
            "vehicles.toml: could not find vehicle with id `{vehicle_id}`"
        ));
    };

    set_toml_float(vehicle_table, "health", params.health)?;
    set_toml_float(vehicle_table, "acceleration", params.acceleration)?;
    set_toml_float(vehicle_table, "brake_strength", params.brake_strength)?;
    set_toml_float(vehicle_table, "air_pitch_torque", params.air_pitch_torque)?;
    set_toml_float(
        vehicle_table,
        "air_max_rotation_speed",
        params.air_max_rotation_speed,
    )?;
    set_toml_float(vehicle_table, "max_forward_speed", params.max_forward_speed)?;
    set_toml_float(vehicle_table, "max_reverse_speed", params.max_reverse_speed)?;
    set_toml_float(vehicle_table, "max_fall_speed", params.max_fall_speed)?;
    set_toml_float(
        vehicle_table,
        "linear_speed_scale",
        params.linear_speed_scale,
    )?;
    set_toml_float(
        vehicle_table,
        "ground_coast_damping",
        params.ground_coast_damping,
    )?;
    set_toml_float(vehicle_table, "air_base_damping", params.air_base_damping)?;
    set_toml_float(
        vehicle_table,
        "air_env_drag_factor",
        params.air_env_drag_factor,
    )?;
    set_toml_float(vehicle_table, "linear_inertia", params.linear_inertia)?;
    set_toml_float(
        vehicle_table,
        "rotational_inertia",
        params.rotational_inertia,
    )?;
    set_toml_float(vehicle_table, "gravity_scale", params.gravity_scale)?;
    set_toml_float(
        vehicle_table,
        "suspension_rest_length_m",
        params.suspension_rest_length_m,
    )?;
    set_toml_float(
        vehicle_table,
        "suspension_stiffness",
        params.suspension_stiffness,
    )?;
    set_toml_float(
        vehicle_table,
        "suspension_damping",
        params.suspension_damping,
    )?;
    set_toml_float(
        vehicle_table,
        "suspension_max_compression_m",
        params.suspension_max_compression_m,
    )?;
    set_toml_float(
        vehicle_table,
        "suspension_max_extension_m",
        params.suspension_max_extension_m,
    )?;
    set_toml_float(
        vehicle_table,
        "tire_longitudinal_grip",
        params.tire_longitudinal_grip,
    )?;
    set_toml_float(
        vehicle_table,
        "tire_slip_grip_floor",
        params.tire_slip_grip_floor,
    )?;
    set_toml_float(vehicle_table, "front_drive_ratio", params.front_drive_ratio)?;
    set_toml_float(
        vehicle_table,
        "rear_drive_traction_assist_distance_m",
        params.rear_drive_traction_assist_distance_m,
    )?;
    set_toml_float(
        vehicle_table,
        "rear_drive_traction_assist_min_factor",
        params.rear_drive_traction_assist_min_factor,
    )?;
    set_toml_float(vehicle_table, "turret_range_m", params.turret_range_m)?;
    set_toml_float(
        vehicle_table,
        "turret_cone_degrees",
        params.turret_cone_degrees,
    )?;
    set_toml_float(
        vehicle_table,
        "missile_fire_interval_seconds",
        params.missile_fire_interval_seconds,
    )?;
    set_toml_float(
        vehicle_table,
        "camera_look_ahead_factor",
        params.camera_look_ahead_factor,
    )?;
    set_toml_float(
        vehicle_table,
        "camera_look_ahead_min",
        params.camera_look_ahead_min,
    )?;
    set_toml_float(
        vehicle_table,
        "camera_look_ahead_max",
        params.camera_look_ahead_max,
    )?;

    Ok(())
}

fn set_toml_float(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: f32,
) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("`{key}` is not a finite number"));
    }

    table.insert(key.to_string(), toml::Value::Float(value as f64));
    Ok(())
}

fn keybind_overlay_text() -> &'static str {
    "Keybinds\n\
H - Toggle this panel\n\
V - Toggle vehicle tuning panel\n\
F5 - Hot-reload config\n\
D / Right - Accelerate\n\
A / Left - Brake / reverse\n\
J - Queue BigJump commentary event\n\
K - Queue Kill commentary event\n\
C - Queue Crash commentary event\n\
Esc - Pause / resume\n\
R - Open results\n\
Enter - Pause -> results\n\
Space - Results -> new run\n\
Q - Quit from results"
}
