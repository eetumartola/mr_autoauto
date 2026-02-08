use crate::config::{BackgroundConfig, GameConfig, SfxConfig, TerrainConfig, VehicleConfig};
use crate::gameplay::vehicle::{
    PlayerVehicle, VehicleInputState, VehicleStuntMetrics, VehicleTelemetry,
};
use crate::states::{GameState, RunSummary};
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
            .init_resource::<DebugTextOverlayState>()
            .init_resource::<DebugGameplayGuards>()
            .init_resource::<DebugCameraPanState>()
            .init_resource::<VehicleTuningPanelState>()
            .init_resource::<BackgroundTuningPanelState>()
            .init_resource::<AudioTuningPanelState>()
            .add_systems(Update, spawn_debug_overlay)
            .add_systems(Update, toggle_debug_text_overlay)
            .add_systems(Update, toggle_keybind_overlay)
            .add_systems(Update, toggle_vehicle_tuning_panel)
            .add_systems(Update, toggle_background_tuning_panel)
            .add_systems(Update, toggle_audio_tuning_panel)
            .add_systems(Update, sync_debug_overlay_visibility)
            .add_systems(Update, sync_keybind_overlay_visibility)
            .add_systems(OnEnter(GameState::InRun), reset_run_stats)
            .add_systems(
                Update,
                update_debug_camera_pan
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            )
            .add_systems(
                Update,
                (update_run_stats, update_debug_overlay_text)
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            )
            .add_systems(
                EguiPrimaryContextPass,
                (
                    vehicle_tuning_panel_ui,
                    background_tuning_panel_ui,
                    audio_tuning_panel_ui,
                )
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            );
    }
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct DebugGameplayGuards {
    pub player_invulnerable: bool,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct DebugCameraPanState {
    pub offset_x_m: f32,
}

const DEBUG_CAMERA_PAN_SPEED_MPS: f32 = 70.0;

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

#[derive(Resource, Debug, Clone)]
struct DebugTextOverlayState {
    visible: bool,
}

impl Default for DebugTextOverlayState {
    fn default() -> Self {
        Self { visible: false }
    }
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

#[derive(Debug, Clone)]
struct BackgroundTuningParams {
    parallax: f32,
    offset_x_m: f32,
    offset_y_m: f32,
    offset_z_m: f32,
    scale_x: f32,
    scale_y: f32,
    scale_z: f32,
    loop_length_m: f32,
    wave_a_amplitude: f32,
    wave_a_frequency: f32,
    wave_b_amplitude: f32,
    wave_b_frequency: f32,
    wave_c_amplitude: f32,
    wave_c_frequency: f32,
    ground_lowering_m: f32,
}

impl BackgroundTuningParams {
    fn from_config(background: &BackgroundConfig, terrain: &TerrainConfig) -> Self {
        Self {
            parallax: background.parallax,
            offset_x_m: background.offset_x_m,
            offset_y_m: background.offset_y_m,
            offset_z_m: background.offset_z_m,
            scale_x: background.scale_x,
            scale_y: background.scale_y,
            scale_z: background.scale_z,
            loop_length_m: background.loop_length_m,
            wave_a_amplitude: background
                .wave_a_amplitude
                .unwrap_or(terrain.wave_a_amplitude),
            wave_a_frequency: background
                .wave_a_frequency
                .unwrap_or(terrain.wave_a_frequency),
            wave_b_amplitude: background
                .wave_b_amplitude
                .unwrap_or(terrain.wave_b_amplitude),
            wave_b_frequency: background
                .wave_b_frequency
                .unwrap_or(terrain.wave_b_frequency),
            wave_c_amplitude: background
                .wave_c_amplitude
                .unwrap_or(terrain.wave_c_amplitude),
            wave_c_frequency: background
                .wave_c_frequency
                .unwrap_or(terrain.wave_c_frequency),
            ground_lowering_m: terrain.ground_lowering_m,
        }
    }

    fn apply_to_background(&self, background: &mut BackgroundConfig) {
        background.parallax = self.parallax;
        background.offset_x_m = self.offset_x_m;
        background.offset_y_m = self.offset_y_m;
        background.offset_z_m = self.offset_z_m;
        background.scale_x = self.scale_x;
        background.scale_y = self.scale_y;
        background.scale_z = self.scale_z;
        background.loop_length_m = self.loop_length_m;
        background.wave_a_amplitude = Some(self.wave_a_amplitude);
        background.wave_a_frequency = Some(self.wave_a_frequency);
        background.wave_b_amplitude = Some(self.wave_b_amplitude);
        background.wave_b_frequency = Some(self.wave_b_frequency);
        background.wave_c_amplitude = Some(self.wave_c_amplitude);
        background.wave_c_frequency = Some(self.wave_c_frequency);
    }

    fn apply_to_terrain(&self, terrain: &mut TerrainConfig) {
        terrain.ground_lowering_m = self.ground_lowering_m;
    }
}

#[derive(Resource, Debug, Default)]
struct BackgroundTuningPanelState {
    visible: bool,
    source_background_id: String,
    params: Option<BackgroundTuningParams>,
    status: String,
}

#[derive(Debug, Clone)]
struct AudioTuningParams {
    master_volume: f32,
    music_volume: f32,
    engine_volume: f32,
    gun_shot_volume: f32,
    gun_hit_volume: f32,
    gun_miss_volume: f32,
    missile_launch_volume: f32,
    missile_hit_volume: f32,
    explode_volume: f32,
}

impl AudioTuningParams {
    fn from_sfx(sfx: &SfxConfig) -> Self {
        Self {
            master_volume: sfx.master_volume,
            music_volume: sfx.music_volume,
            engine_volume: sfx.engine_volume,
            gun_shot_volume: sfx.gun_shot_volume,
            gun_hit_volume: sfx.gun_hit_volume,
            gun_miss_volume: sfx.gun_miss_volume,
            missile_launch_volume: sfx.missile_launch_volume,
            missile_hit_volume: sfx.missile_hit_volume,
            explode_volume: sfx.explode_volume,
        }
    }

    fn apply_to_sfx(&self, sfx: &mut SfxConfig) {
        sfx.master_volume = self.master_volume;
        sfx.music_volume = self.music_volume;
        sfx.engine_volume = self.engine_volume;
        sfx.gun_shot_volume = self.gun_shot_volume;
        sfx.gun_hit_volume = self.gun_hit_volume;
        sfx.gun_miss_volume = self.gun_miss_volume;
        sfx.missile_launch_volume = self.missile_launch_volume;
        sfx.missile_hit_volume = self.missile_hit_volume;
        sfx.explode_volume = self.explode_volume;
    }
}

#[derive(Resource, Debug, Default)]
struct AudioTuningPanelState {
    visible: bool,
    params: Option<AudioTuningParams>,
    status: String,
}

fn spawn_debug_overlay(
    mut commands: Commands,
    keybind_overlay: Res<KeybindOverlayState>,
    debug_text_overlay: Res<DebugTextOverlayState>,
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
        Text::new("debug diagnostics initializing..."),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::srgb(0.88, 0.93, 0.97)),
        BackgroundColor(Color::srgba(0.04, 0.06, 0.08, 0.72)),
        BorderColor::all(Color::srgba(0.48, 0.58, 0.66, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(12.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        if debug_text_overlay.visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
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
        if debug_text_overlay.visible && keybind_overlay.visible {
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

fn update_debug_camera_pan(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    config: Res<GameConfig>,
    mut pan_state: ResMut<DebugCameraPanState>,
) {
    if !config.game.app.debug_overlay {
        pan_state.offset_x_m = 0.0;
        return;
    }

    let mut axis = 0.0_f32;
    if keyboard.pressed(KeyCode::KeyI) {
        axis -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyP) {
        axis += 1.0;
    }

    if axis.abs() > f32::EPSILON {
        pan_state.offset_x_m += axis * DEBUG_CAMERA_PAN_SPEED_MPS * time.delta_secs();
        pan_state.offset_x_m = pan_state.offset_x_m.clamp(-3_000.0, 3_000.0);
    }
}

#[allow(clippy::too_many_arguments)]
fn update_debug_overlay_text(
    diagnostics: Res<DiagnosticsStore>,
    run_stats: Res<DebugRunStats>,
    enemy_query: Query<(), With<EnemyDebugMarker>>,
    player_query: Query<&Transform, With<PlayerVehicle>>,
    run_summary: Option<Res<RunSummary>>,
    input_state: Option<Res<VehicleInputState>>,
    stunts: Option<Res<VehicleStuntMetrics>>,
    camera_pan: Res<DebugCameraPanState>,
    debug_text_overlay: Res<DebugTextOverlayState>,
    mut overlay_query: Query<&mut Text, With<DebugOverlayText>>,
) {
    if !debug_text_overlay.visible {
        return;
    }

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

    let (score, kill_count, coin_pickup_count) = match run_summary {
        Some(summary) => (summary.score, summary.kill_count, summary.coin_pickup_count),
        None => (0, 0, 0),
    };

    let (airtime_cur, wheelie_cur, crash_count, max_speed, last_impact) = match stunts {
        Some(stunts) => (
            stunts.airtime_current_s,
            stunts.wheelie_current_s,
            stunts.crash_count,
            stunts.max_speed_mps,
            stunts.last_landing_impact_speed_mps,
        ),
        None => (0.0, 0.0, 0, 0.0, 0.0),
    };

    *text = Text::new(format!(
        "DBG FPS: {fps:>5.1}\nX: {player_x:>7.1} m | Pan: {camera_pan_offset:>6.1} m | Enemy: {enemy_count}\nInput: accel={accel} brake={brake} grounded={grounded}\nSpeed: {speed:>6.1} m/s | Score: {score} | Kills: {kills} | Coins: {coins}\nAir: {air_cur:>4.2}s | Wheelie: {wheelie_cur:>4.2}s | Crashes: {crashes}\nMax: {max_speed:>6.1} m/s | Impact: {impact:>5.1} m/s\nSegment: {segment}",
        player_x = player_x,
        speed = run_stats.speed_mps,
        score = score,
        kills = kill_count,
        coins = coin_pickup_count,
        accel = if input_accel { "yes" } else { "no" },
        brake = if input_brake { "yes" } else { "no" },
        grounded = if run_stats.grounded { "yes" } else { "no" },
        air_cur = airtime_cur,
        wheelie_cur = wheelie_cur,
        crashes = crash_count,
        max_speed = max_speed,
        impact = last_impact,
        camera_pan_offset = camera_pan.offset_x_m,
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

fn toggle_debug_text_overlay(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<DebugTextOverlayState>,
    config: Option<Res<GameConfig>>,
) {
    let Some(config) = config else {
        return;
    };

    if !config.game.app.debug_overlay {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyO) {
        state.visible = !state.visible;
        info!(
            "Debug text overlays {}.",
            if state.visible { "shown" } else { "hidden" }
        );
    }
}

fn sync_debug_overlay_visibility(
    state: Res<DebugTextOverlayState>,
    mut query: Query<&mut Visibility, With<DebugOverlayText>>,
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

fn sync_keybind_overlay_visibility(
    state: Res<KeybindOverlayState>,
    debug_text_overlay: Res<DebugTextOverlayState>,
    mut query: Query<&mut Visibility, With<KeybindOverlayText>>,
) {
    if !state.is_changed() && !debug_text_overlay.is_changed() {
        return;
    }

    let next_visibility = if debug_text_overlay.visible && state.visible {
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

fn toggle_background_tuning_panel(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut panel_state: ResMut<BackgroundTuningPanelState>,
    config: Option<Res<GameConfig>>,
    run_stats: Res<DebugRunStats>,
) {
    if !keyboard.just_pressed(KeyCode::KeyB) {
        return;
    }

    panel_state.visible = !panel_state.visible;
    if panel_state.visible {
        if let Some(config) = config {
            if let Err(error) =
                sync_background_panel_state_from_config(&mut panel_state, &config, &run_stats)
            {
                panel_state.status = error;
            }
        }
        info!("Background tuning panel shown.");
    } else {
        info!("Background tuning panel hidden.");
    }
}

fn toggle_audio_tuning_panel(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut panel_state: ResMut<AudioTuningPanelState>,
    config: Option<Res<GameConfig>>,
) {
    if !keyboard.just_pressed(KeyCode::KeyM) {
        return;
    }

    panel_state.visible = !panel_state.visible;
    if panel_state.visible {
        if let Some(config) = config {
            if let Err(error) = sync_audio_panel_state_from_config(&mut panel_state, &config) {
                panel_state.status = error;
            }
        }
        info!("Audio tuning panel shown.");
    } else {
        info!("Audio tuning panel hidden.");
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

fn background_tuning_panel_ui(
    mut egui_contexts: EguiContexts,
    mut panel_state: ResMut<BackgroundTuningPanelState>,
    mut config: ResMut<GameConfig>,
    run_stats: Res<DebugRunStats>,
) {
    if !panel_state.visible {
        return;
    }

    let active_background_id = match preferred_background_id(&config, &run_stats) {
        Some(id) => id,
        None => {
            panel_state.status =
                "Background tuning panel: no background id available from config.".to_string();
            return;
        }
    };

    if panel_state.params.is_none() || panel_state.source_background_id != active_background_id {
        if let Err(error) =
            sync_background_panel_state_from_config(&mut panel_state, &config, &run_stats)
        {
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
    let background_id = panel_state.source_background_id.clone();

    let Ok(ctx) = egui_contexts.ctx_mut() else {
        return;
    };
    egui::Window::new("Background Splat Tuning")
        .open(&mut window_open)
        .resizable(true)
        .default_width(580.0)
        .show(ctx, |ui| {
            ui.label(format!("Active background: {background_id}"));
            ui.label("Each row has a slider plus a free-form float value.");
            ui.label("Scale values are non-zero; negative scale flips that axis.");
            ui.separator();

            params_changed |=
                tuning_slider_row(ui, "parallax", &mut params.parallax, -2.0..=2.0, 0.01);
            params_changed |= tuning_slider_row(
                ui,
                "offset_x_m",
                &mut params.offset_x_m,
                -2000.0..=2000.0,
                0.05,
            );
            params_changed |= tuning_slider_row(
                ui,
                "offset_y_m",
                &mut params.offset_y_m,
                -1000.0..=1000.0,
                0.05,
            );
            params_changed |= tuning_slider_row(
                ui,
                "offset_z_m",
                &mut params.offset_z_m,
                -1000.0..=1000.0,
                0.05,
            );
            params_changed |=
                tuning_slider_row(ui, "scale_x", &mut params.scale_x, -50.0..=50.0, 0.01);
            params_changed |=
                tuning_slider_row(ui, "scale_y", &mut params.scale_y, -50.0..=50.0, 0.01);
            params_changed |=
                tuning_slider_row(ui, "scale_z", &mut params.scale_z, -50.0..=50.0, 0.01);
            params_changed |= tuning_slider_row(
                ui,
                "loop_length_m (0 = disabled)",
                &mut params.loop_length_m,
                0.0..=5000.0,
                0.1,
            );
            params_changed |= tuning_slider_row(
                ui,
                "wave_a_amplitude",
                &mut params.wave_a_amplitude,
                -20.0..=20.0,
                0.01,
            );
            params_changed |= tuning_slider_row(
                ui,
                "wave_a_frequency",
                &mut params.wave_a_frequency,
                0.0..=2.0,
                0.001,
            );
            params_changed |= tuning_slider_row(
                ui,
                "wave_b_amplitude",
                &mut params.wave_b_amplitude,
                -20.0..=20.0,
                0.01,
            );
            params_changed |= tuning_slider_row(
                ui,
                "wave_b_frequency",
                &mut params.wave_b_frequency,
                0.0..=2.0,
                0.001,
            );
            params_changed |= tuning_slider_row(
                ui,
                "wave_c_amplitude",
                &mut params.wave_c_amplitude,
                -20.0..=20.0,
                0.01,
            );
            params_changed |= tuning_slider_row(
                ui,
                "wave_c_frequency",
                &mut params.wave_c_frequency,
                0.0..=2.0,
                0.001,
            );
            params_changed |= tuning_slider_row(
                ui,
                "ground_lowering_m",
                &mut params.ground_lowering_m,
                0.0..=120.0,
                0.05,
            );

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Reload From Config").clicked() {
                    reload_clicked = true;
                }
                if ui.button("Apply To backgrounds.toml + game.toml").clicked() {
                    apply_clicked = true;
                }
            });

            if !status.is_empty() {
                ui.separator();
                ui.label(status);
            }
        });

    panel_state.visible = window_open;

    if reload_clicked {
        match sync_background_panel_state_from_config(&mut panel_state, &config, &run_stats) {
            Ok(()) => panel_state.status = "Reloaded values from current config.".to_string(),
            Err(error) => panel_state.status = error,
        }
        return;
    }

    panel_state.params = Some(params.clone());

    if params_changed {
        if let Err(error) =
            apply_background_tuning_to_runtime_config(&mut config, &background_id, &params)
        {
            panel_state.status = error;
        } else {
            panel_state.status = "Live-tuning active (in-memory config updated).".to_string();
        }
    }

    if apply_clicked {
        match persist_background_and_terrain_tuning_and_reload(&mut config, &background_id, &params)
        {
            Ok(message) => {
                panel_state.status = message;
                if let Err(error) =
                    sync_background_panel_state_from_config(&mut panel_state, &config, &run_stats)
                {
                    panel_state.status = error;
                }
            }
            Err(error) => panel_state.status = error,
        }
    }
}

fn audio_tuning_panel_ui(
    mut egui_contexts: EguiContexts,
    mut panel_state: ResMut<AudioTuningPanelState>,
    mut config: ResMut<GameConfig>,
) {
    if !panel_state.visible {
        return;
    }

    if panel_state.params.is_none() {
        if let Err(error) = sync_audio_panel_state_from_config(&mut panel_state, &config) {
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

    let Ok(ctx) = egui_contexts.ctx_mut() else {
        return;
    };
    egui::Window::new("Audio Mix Tuning")
        .open(&mut window_open)
        .resizable(true)
        .default_width(560.0)
        .show(ctx, |ui| {
            ui.label("Live-tune SFX/music levels with sliders and numeric fields.");
            ui.separator();

            params_changed |= tuning_slider_row(
                ui,
                "master_volume",
                &mut params.master_volume,
                0.0..=2.0,
                0.01,
            );
            params_changed |= tuning_slider_row(
                ui,
                "music_volume",
                &mut params.music_volume,
                0.0..=2.0,
                0.01,
            );
            params_changed |= tuning_slider_row(
                ui,
                "engine_volume",
                &mut params.engine_volume,
                0.0..=2.0,
                0.01,
            );
            params_changed |= tuning_slider_row(
                ui,
                "gun_shot_volume",
                &mut params.gun_shot_volume,
                0.0..=2.0,
                0.01,
            );
            params_changed |= tuning_slider_row(
                ui,
                "gun_hit_volume",
                &mut params.gun_hit_volume,
                0.0..=2.0,
                0.01,
            );
            params_changed |= tuning_slider_row(
                ui,
                "gun_miss_volume",
                &mut params.gun_miss_volume,
                0.0..=2.0,
                0.01,
            );
            params_changed |= tuning_slider_row(
                ui,
                "missile_launch_volume",
                &mut params.missile_launch_volume,
                0.0..=2.0,
                0.01,
            );
            params_changed |= tuning_slider_row(
                ui,
                "missile_hit_volume",
                &mut params.missile_hit_volume,
                0.0..=2.0,
                0.01,
            );
            params_changed |= tuning_slider_row(
                ui,
                "explode_volume",
                &mut params.explode_volume,
                0.0..=2.0,
                0.01,
            );

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Reload From Config").clicked() {
                    reload_clicked = true;
                }
                if ui.button("Apply To game.toml").clicked() {
                    apply_clicked = true;
                }
            });

            if !status.is_empty() {
                ui.separator();
                ui.label(status);
            }
        });

    panel_state.visible = window_open;

    if reload_clicked {
        match sync_audio_panel_state_from_config(&mut panel_state, &config) {
            Ok(()) => panel_state.status = "Reloaded values from current config.".to_string(),
            Err(error) => panel_state.status = error,
        }
        return;
    }

    panel_state.params = Some(params.clone());

    if params_changed {
        if let Err(error) = apply_audio_tuning_to_runtime_config(&mut config, &params) {
            panel_state.status = error;
        } else {
            panel_state.status = "Live-tuning active (in-memory config updated).".to_string();
        }
    }

    if apply_clicked {
        match persist_audio_tuning_and_reload(&mut config, &params) {
            Ok(message) => {
                panel_state.status = message;
                if let Err(error) = sync_audio_panel_state_from_config(&mut panel_state, &config) {
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

fn preferred_background_id(config: &GameConfig, run_stats: &DebugRunStats) -> Option<String> {
    if config
        .backgrounds_by_id
        .contains_key(run_stats.active_segment_id.as_str())
    {
        return Some(run_stats.active_segment_id.clone());
    }

    config
        .segments
        .segment_sequence
        .first()
        .map(|segment| segment.id.clone())
        .or_else(|| {
            config
                .backgrounds
                .backgrounds
                .first()
                .map(|background| background.id.clone())
        })
}

fn sync_background_panel_state_from_config(
    panel_state: &mut BackgroundTuningPanelState,
    config: &GameConfig,
    run_stats: &DebugRunStats,
) -> Result<(), String> {
    let Some(background_id) = preferred_background_id(config, run_stats) else {
        return Err("Background tuning panel: no background id available from config.".to_string());
    };
    let Some(background) = config.backgrounds_by_id.get(&background_id) else {
        return Err(format!(
            "Background tuning panel: background `{background_id}` not found in config."
        ));
    };

    panel_state.source_background_id = background_id;
    panel_state.params = Some(BackgroundTuningParams::from_config(
        background,
        &config.game.terrain,
    ));
    Ok(())
}

fn sync_audio_panel_state_from_config(
    panel_state: &mut AudioTuningPanelState,
    config: &GameConfig,
) -> Result<(), String> {
    if !config.game.sfx.master_volume.is_finite() {
        return Err("Audio tuning panel: invalid `sfx.master_volume` value.".to_string());
    }

    panel_state.params = Some(AudioTuningParams::from_sfx(&config.game.sfx));
    Ok(())
}

fn apply_audio_tuning_to_runtime_config(
    config: &mut GameConfig,
    params: &AudioTuningParams,
) -> Result<(), String> {
    for (label, value) in [
        ("master_volume", params.master_volume),
        ("music_volume", params.music_volume),
        ("engine_volume", params.engine_volume),
        ("gun_shot_volume", params.gun_shot_volume),
        ("gun_hit_volume", params.gun_hit_volume),
        ("gun_miss_volume", params.gun_miss_volume),
        ("missile_launch_volume", params.missile_launch_volume),
        ("missile_hit_volume", params.missile_hit_volume),
        ("explode_volume", params.explode_volume),
    ] {
        if !value.is_finite() {
            return Err(format!("Audio tuning panel: `{label}` must be finite."));
        }
        if value < 0.0 {
            return Err(format!("Audio tuning panel: `{label}` must be >= 0."));
        }
    }

    params.apply_to_sfx(&mut config.game.sfx);
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

fn apply_background_tuning_to_runtime_config(
    config: &mut GameConfig,
    background_id: &str,
    params: &BackgroundTuningParams,
) -> Result<(), String> {
    for (label, value) in [
        ("parallax", params.parallax),
        ("offset_x_m", params.offset_x_m),
        ("offset_y_m", params.offset_y_m),
        ("offset_z_m", params.offset_z_m),
        ("scale_x", params.scale_x),
        ("scale_y", params.scale_y),
        ("scale_z", params.scale_z),
        ("loop_length_m", params.loop_length_m),
        ("wave_a_amplitude", params.wave_a_amplitude),
        ("wave_a_frequency", params.wave_a_frequency),
        ("wave_b_amplitude", params.wave_b_amplitude),
        ("wave_b_frequency", params.wave_b_frequency),
        ("wave_c_amplitude", params.wave_c_amplitude),
        ("wave_c_frequency", params.wave_c_frequency),
        ("ground_lowering_m", params.ground_lowering_m),
    ] {
        if !value.is_finite() {
            return Err(format!(
                "Background tuning panel: `{label}` must be a finite number."
            ));
        }
    }
    if params.scale_x.abs() <= f32::EPSILON
        || params.scale_y.abs() <= f32::EPSILON
        || params.scale_z.abs() <= f32::EPSILON
    {
        return Err("Background tuning panel: scale_x/scale_y/scale_z cannot be zero.".to_string());
    }
    for (label, value) in [
        ("loop_length_m", params.loop_length_m),
        ("wave_a_frequency", params.wave_a_frequency),
        ("wave_b_frequency", params.wave_b_frequency),
        ("wave_c_frequency", params.wave_c_frequency),
        ("ground_lowering_m", params.ground_lowering_m),
    ] {
        if value < 0.0 {
            return Err(format!("Background tuning panel: `{label}` must be >= 0."));
        }
    }

    params.apply_to_terrain(&mut config.game.terrain);

    let Some(background) = config.backgrounds_by_id.get_mut(background_id) else {
        return Err(format!(
            "Background tuning panel: runtime background `{background_id}` not found in backgrounds_by_id."
        ));
    };
    params.apply_to_background(background);

    let Some(background) = config
        .backgrounds
        .backgrounds
        .iter_mut()
        .find(|background| background.id == background_id)
    else {
        return Err(format!(
            "Background tuning panel: runtime background `{background_id}` not found in backgrounds list."
        ));
    };
    params.apply_to_background(background);
    Ok(())
}

fn persist_audio_tuning_and_reload(
    config: &mut GameConfig,
    params: &AudioTuningParams,
) -> Result<String, String> {
    let game_path = Path::new("config").join("game.toml");
    let original_game_raw = fs::read_to_string(&game_path)
        .map_err(|error| format!("Failed reading `{}`: {error}", game_path.display()))?;
    let mut game_root: toml::Value = toml::from_str(&original_game_raw)
        .map_err(|error| format!("Failed parsing `{}`: {error}", game_path.display()))?;

    write_sfx_params_to_game_toml_value(&mut game_root, params)?;

    let updated_game_raw = toml::to_string_pretty(&game_root)
        .map_err(|error| format!("Failed serializing game TOML: {error}"))?;
    fs::write(&game_path, updated_game_raw)
        .map_err(|error| format!("Failed writing `{}`: {error}", game_path.display()))?;

    match GameConfig::load_from_dir(Path::new("config")) {
        Ok(new_config) => {
            *config = new_config;
            Ok(format!(
                "Applied audio tuning and saved to {}.",
                game_path.to_string_lossy()
            ))
        }
        Err(error) => {
            let _ = fs::write(&game_path, original_game_raw);
            if let Ok(restored) = GameConfig::load_from_dir(Path::new("config")) {
                *config = restored;
            }
            Err(format!(
                "Apply failed validation: {error}. Reverted `{}`.",
                game_path.display()
            ))
        }
    }
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

fn persist_background_and_terrain_tuning_and_reload(
    config: &mut GameConfig,
    background_id: &str,
    params: &BackgroundTuningParams,
) -> Result<String, String> {
    let backgrounds_path = Path::new("config").join("backgrounds.toml");
    let game_path = Path::new("config").join("game.toml");

    let original_backgrounds_raw = fs::read_to_string(&backgrounds_path)
        .map_err(|error| format!("Failed reading `{}`: {error}", backgrounds_path.display()))?;
    let original_game_raw = fs::read_to_string(&game_path)
        .map_err(|error| format!("Failed reading `{}`: {error}", game_path.display()))?;

    let mut backgrounds_root: toml::Value = toml::from_str(&original_backgrounds_raw)
        .map_err(|error| format!("Failed parsing `{}`: {error}", backgrounds_path.display()))?;
    let mut game_root: toml::Value = toml::from_str(&original_game_raw)
        .map_err(|error| format!("Failed parsing `{}`: {error}", game_path.display()))?;

    write_background_params_to_toml_value(&mut backgrounds_root, background_id, params)?;
    write_terrain_params_to_game_toml_value(&mut game_root, params)?;

    let updated_backgrounds_raw = toml::to_string_pretty(&backgrounds_root)
        .map_err(|error| format!("Failed serializing backgrounds TOML: {error}"))?;
    let updated_game_raw = toml::to_string_pretty(&game_root)
        .map_err(|error| format!("Failed serializing game TOML: {error}"))?;

    fs::write(&backgrounds_path, updated_backgrounds_raw)
        .map_err(|error| format!("Failed writing `{}`: {error}", backgrounds_path.display()))?;
    fs::write(&game_path, updated_game_raw)
        .map_err(|error| format!("Failed writing `{}`: {error}", game_path.display()))?;

    match GameConfig::load_from_dir(Path::new("config")) {
        Ok(new_config) => {
            *config = new_config;
            Ok(format!(
                "Applied tuning and saved to {} and {}.",
                backgrounds_path.to_string_lossy(),
                game_path.to_string_lossy()
            ))
        }
        Err(error) => {
            let _ = fs::write(&backgrounds_path, original_backgrounds_raw);
            let _ = fs::write(&game_path, original_game_raw);
            if let Ok(restored) = GameConfig::load_from_dir(Path::new("config")) {
                *config = restored;
            }
            Err(format!(
                "Apply failed validation: {error}. Reverted `{}` and `{}`.",
                backgrounds_path.display(),
                game_path.display()
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

fn write_background_params_to_toml_value(
    root: &mut toml::Value,
    background_id: &str,
    params: &BackgroundTuningParams,
) -> Result<(), String> {
    let Some(backgrounds_array) = root
        .get_mut("backgrounds")
        .and_then(toml::Value::as_array_mut)
    else {
        return Err("backgrounds.toml: missing or invalid `backgrounds` array".to_string());
    };

    let Some(background_table) = backgrounds_array.iter_mut().find_map(|background_value| {
        let table = background_value.as_table_mut()?;
        if table.get("id").and_then(toml::Value::as_str) == Some(background_id) {
            Some(table)
        } else {
            None
        }
    }) else {
        return Err(format!(
            "backgrounds.toml: could not find background with id `{background_id}`"
        ));
    };

    set_toml_float(background_table, "parallax", params.parallax)?;
    set_toml_float(background_table, "offset_x_m", params.offset_x_m)?;
    set_toml_float(background_table, "offset_y_m", params.offset_y_m)?;
    set_toml_float(background_table, "offset_z_m", params.offset_z_m)?;
    set_toml_float(background_table, "scale_x", params.scale_x)?;
    set_toml_float(background_table, "scale_y", params.scale_y)?;
    set_toml_float(background_table, "scale_z", params.scale_z)?;
    set_toml_float(background_table, "loop_length_m", params.loop_length_m)?;
    set_toml_float(
        background_table,
        "wave_a_amplitude",
        params.wave_a_amplitude,
    )?;
    set_toml_float(
        background_table,
        "wave_a_frequency",
        params.wave_a_frequency,
    )?;
    set_toml_float(
        background_table,
        "wave_b_amplitude",
        params.wave_b_amplitude,
    )?;
    set_toml_float(
        background_table,
        "wave_b_frequency",
        params.wave_b_frequency,
    )?;
    set_toml_float(
        background_table,
        "wave_c_amplitude",
        params.wave_c_amplitude,
    )?;
    set_toml_float(
        background_table,
        "wave_c_frequency",
        params.wave_c_frequency,
    )?;

    Ok(())
}

fn write_terrain_params_to_game_toml_value(
    root: &mut toml::Value,
    params: &BackgroundTuningParams,
) -> Result<(), String> {
    let Some(terrain_table) = root.get_mut("terrain").and_then(toml::Value::as_table_mut) else {
        return Err("game.toml: missing or invalid `terrain` table".to_string());
    };

    set_toml_float(terrain_table, "ground_lowering_m", params.ground_lowering_m)?;
    Ok(())
}

fn write_sfx_params_to_game_toml_value(
    root: &mut toml::Value,
    params: &AudioTuningParams,
) -> Result<(), String> {
    let Some(sfx_table) = root.get_mut("sfx").and_then(toml::Value::as_table_mut) else {
        return Err("game.toml: missing or invalid `sfx` table".to_string());
    };

    set_toml_float(sfx_table, "master_volume", params.master_volume)?;
    set_toml_float(sfx_table, "music_volume", params.music_volume)?;
    set_toml_float(sfx_table, "engine_volume", params.engine_volume)?;
    set_toml_float(sfx_table, "gun_shot_volume", params.gun_shot_volume)?;
    set_toml_float(sfx_table, "gun_hit_volume", params.gun_hit_volume)?;
    set_toml_float(sfx_table, "gun_miss_volume", params.gun_miss_volume)?;
    set_toml_float(
        sfx_table,
        "missile_launch_volume",
        params.missile_launch_volume,
    )?;
    set_toml_float(sfx_table, "missile_hit_volume", params.missile_hit_volume)?;
    set_toml_float(sfx_table, "explode_volume", params.explode_volume)?;
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
O - Toggle debug text overlays\n\
V - Toggle vehicle tuning panel\n\
B - Toggle background tuning panel\n\
M - Toggle audio tuning panel\n\
I / P - Pan camera left / right\n\
Tab - Debug warp to next segment\n\
A/D or Left/Right - Choose upgrade option (when shown)\n\
F5 - Hot-reload config\n\
D / Right - Accelerate\n\
A / Left - Brake / reverse\n\
J - Queue BigJump commentary event\n\
K - Queue Kill commentary event\n\
C - Queue Crash commentary event\n\
N - Dump loaded vehicle model scene info\n\
Esc - Pause / resume\n\
R - Open results\n\
Enter - Pause -> results\n\
Space - Results -> new run\n\
Q - Quit from results"
}
