use crate::config::GameConfig;
use crate::gameplay::combat::EnemyKilledEvent;
use crate::gameplay::vehicle::{
    PlayerHealth, PlayerVehicle, VehicleStuntMetrics, VehicleTelemetry,
};
use bevy::app::AppExit;
use bevy::asset::LoadState;
use bevy::prelude::*;

const LOADING_LOGO_PATH: &str = "sprites/autoauto_logo.jpg";
const MIN_LOADING_SCREEN_SECONDS: f64 = 0.75;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    Boot,
    Loading,
    InRun,
    Pause,
    Results,
}

pub struct GameStatePlugin;

impl Plugin for GameStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RunSummary>()
            .add_systems(Startup, setup_camera)
            .add_systems(OnEnter(GameState::Boot), enter_boot)
            .add_systems(Update, boot_to_loading.run_if(in_state(GameState::Boot)))
            .add_systems(OnEnter(GameState::Loading), enter_loading)
            .add_systems(OnExit(GameState::Loading), cleanup_loading_screen)
            .add_systems(
                Update,
                loading_to_in_run.run_if(in_state(GameState::Loading)),
            )
            .add_systems(OnEnter(GameState::InRun), enter_in_run)
            .add_systems(
                Update,
                (
                    update_run_summary_progress,
                    apply_kill_score_events,
                    apply_stunt_score_sources,
                    finalize_run_summary_score,
                    trigger_results_on_player_death,
                    in_run_controls,
                )
                    .chain()
                    .run_if(in_state(GameState::InRun)),
            )
            .add_systems(OnEnter(GameState::Pause), enter_pause)
            .add_systems(Update, pause_controls.run_if(in_state(GameState::Pause)))
            .add_systems(OnEnter(GameState::Results), enter_results)
            .add_systems(OnExit(GameState::Results), cleanup_results_screen)
            .add_systems(
                Update,
                results_controls.run_if(in_state(GameState::Results)),
            );
    }
}

#[derive(Component)]
struct LoadingScreenLogo;

#[derive(Component)]
struct ResultsScreenRoot;

#[derive(Resource, Debug, Clone)]
struct LoadingScreenState {
    entered_at_s: f64,
    logo_handle: Handle<Image>,
}

#[derive(Resource, Debug, Clone, Default)]
struct RunSummary {
    score: u32,
    distance_m: f32,
    distance_score: u32,
    kill_score: u32,
    stunt_score: u32,
    airtime_score: u32,
    wheelie_score: u32,
    flip_score: u32,
    no_damage_bonus_score: u32,
    kill_count: u32,
    total_airtime_s: f32,
    total_wheelie_s: f32,
    flip_count: u32,
    big_jump_count: u32,
    huge_jump_count: u32,
    long_wheelie_count: u32,
    took_damage: bool,
    was_game_over: bool,
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn enter_boot() {
    info!("Entered state: Boot");
}

fn boot_to_loading(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::Loading);
}

fn enter_loading(mut commands: Commands, asset_server: Res<AssetServer>, time: Res<Time>) {
    info!("Entered state: Loading");
    let logo_handle = asset_server.load(LOADING_LOGO_PATH);

    commands.insert_resource(LoadingScreenState {
        entered_at_s: time.elapsed_secs_f64(),
        logo_handle: logo_handle.clone(),
    });

    commands.spawn((
        Name::new("LoadingLogo"),
        LoadingScreenLogo,
        Sprite::from_image(logo_handle),
        Transform::from_xyz(0.0, 0.0, 100.0),
    ));
}

fn cleanup_loading_screen(
    mut commands: Commands,
    loading_logo_query: Query<Entity, With<LoadingScreenLogo>>,
) {
    for entity in &loading_logo_query {
        commands.entity(entity).try_despawn();
    }
    commands.remove_resource::<LoadingScreenState>();
}

fn loading_to_in_run(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    loading_state: Option<Res<LoadingScreenState>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Some(loading_state) = loading_state else {
        return;
    };

    let has_min_time =
        time.elapsed_secs_f64() - loading_state.entered_at_s >= MIN_LOADING_SCREEN_SECONDS;
    if !has_min_time {
        return;
    }

    let logo_loaded = asset_server.is_loaded_with_dependencies(loading_state.logo_handle.id());
    let logo_failed = matches!(
        asset_server.load_state(loading_state.logo_handle.id()),
        LoadState::Failed(_)
    );
    if !logo_loaded && !logo_failed {
        return;
    }

    if logo_failed {
        warn!("Loading logo failed to load, continuing to run state.");
    }

    next_state.set(GameState::InRun);
}

fn enter_in_run(mut run_summary: ResMut<RunSummary>) {
    run_summary.score = 0;
    run_summary.distance_m = 0.0;
    run_summary.distance_score = 0;
    run_summary.kill_score = 0;
    run_summary.stunt_score = 0;
    run_summary.airtime_score = 0;
    run_summary.wheelie_score = 0;
    run_summary.flip_score = 0;
    run_summary.no_damage_bonus_score = 0;
    run_summary.kill_count = 0;
    run_summary.total_airtime_s = 0.0;
    run_summary.total_wheelie_s = 0.0;
    run_summary.flip_count = 0;
    run_summary.big_jump_count = 0;
    run_summary.huge_jump_count = 0;
    run_summary.long_wheelie_count = 0;
    run_summary.took_damage = false;
    run_summary.was_game_over = false;
    info!("Entered state: InRun");
}

fn update_run_summary_progress(
    telemetry: Option<Res<VehicleTelemetry>>,
    player_query: Query<&PlayerHealth, With<PlayerVehicle>>,
    mut run_summary: ResMut<RunSummary>,
) {
    let Some(telemetry) = telemetry else {
        return;
    };

    run_summary.distance_m = telemetry.distance_m.max(0.0);
    let Ok(player_health) = player_query.single() else {
        return;
    };
    if player_health.current < player_health.max {
        run_summary.took_damage = true;
    }
}

fn apply_stunt_score_sources(
    metrics: Option<Res<VehicleStuntMetrics>>,
    config: Option<Res<GameConfig>>,
    mut run_summary: ResMut<RunSummary>,
) {
    let Some(metrics) = metrics else {
        return;
    };
    let Some(config) = config else {
        return;
    };

    run_summary.total_airtime_s = metrics.airtime_total_s.max(0.0);
    run_summary.total_wheelie_s = metrics.wheelie_total_s.max(0.0);
    run_summary.flip_count = metrics.flip_count;
    run_summary.big_jump_count = metrics.big_jump_count;
    run_summary.huge_jump_count = metrics.huge_jump_count;
    run_summary.long_wheelie_count = metrics.long_wheelie_count;

    run_summary.airtime_score = score_points_from_duration(
        run_summary.total_airtime_s,
        config.game.scoring.airtime_points_per_second,
    );
    run_summary.wheelie_score = score_points_from_duration(
        run_summary.total_wheelie_s,
        config.game.scoring.wheelie_points_per_second,
    );
    run_summary.flip_score = run_summary
        .flip_count
        .saturating_mul(config.game.scoring.flip_points);
    run_summary.stunt_score = run_summary
        .airtime_score
        .saturating_add(run_summary.wheelie_score)
        .saturating_add(run_summary.flip_score);
}

fn finalize_run_summary_score(
    config: Option<Res<GameConfig>>,
    mut run_summary: ResMut<RunSummary>,
) {
    let Some(config) = config else {
        return;
    };

    run_summary.distance_score =
        score_points_from_duration(run_summary.distance_m, config.game.scoring.points_per_meter);
    run_summary.no_damage_bonus_score = if run_summary.took_damage {
        0
    } else {
        config.game.scoring.no_damage_bonus
    };
    run_summary.score = run_summary
        .distance_score
        .saturating_add(run_summary.kill_score)
        .saturating_add(run_summary.stunt_score)
        .saturating_add(run_summary.no_damage_bonus_score);
}

fn score_points_from_duration(duration: f32, points_per_unit: f32) -> u32 {
    if !duration.is_finite() || !points_per_unit.is_finite() {
        return 0;
    }
    (duration.max(0.0) * points_per_unit.max(0.0)).floor() as u32
}

fn apply_kill_score_events(
    mut kill_events: MessageReader<EnemyKilledEvent>,
    config: Option<Res<GameConfig>>,
    mut run_summary: ResMut<RunSummary>,
) {
    let Some(config) = config else {
        return;
    };

    let mut total_added = 0_u32;
    for event in kill_events.read() {
        let kill_points = config
            .enemy_types_by_id
            .get(&event.enemy_type_id)
            .map(|enemy_cfg| enemy_cfg.kill_score)
            .unwrap_or(0);

        total_added = total_added.saturating_add(kill_points);
        run_summary.kill_count = run_summary.kill_count.saturating_add(1);
    }

    if total_added > 0 {
        run_summary.kill_score = run_summary.kill_score.saturating_add(total_added);
    }
}

fn trigger_results_on_player_death(
    player_query: Query<&PlayerHealth, With<PlayerVehicle>>,
    mut run_summary: ResMut<RunSummary>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    let Ok(player_health) = player_query.single() else {
        return;
    };

    if player_health.current <= 0.0 {
        if !run_summary.was_game_over {
            info!(
                "Player health depleted; entering results with score {}.",
                run_summary.score
            );
        }
        run_summary.took_damage = true;
        run_summary.was_game_over = true;
        next_state.set(GameState::Results);
    }
}

fn in_run_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut run_summary: ResMut<RunSummary>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::Pause);
    }

    if keyboard.just_pressed(KeyCode::KeyR) && !run_summary.was_game_over {
        run_summary.was_game_over = false;
        next_state.set(GameState::Results);
    }
}

fn enter_pause() {
    info!("Entered state: Pause");
}

fn pause_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::InRun);
    }

    if keyboard.just_pressed(KeyCode::Enter) {
        next_state.set(GameState::Results);
    }
}

fn enter_results(mut commands: Commands, run_summary: Res<RunSummary>) {
    let title = if run_summary.was_game_over {
        "GAME OVER"
    } else {
        "RESULTS"
    };
    let no_damage_line = if run_summary.no_damage_bonus_score > 0 {
        format!("No Damage Bonus: +{}", run_summary.no_damage_bonus_score)
    } else {
        "No Damage Bonus: +0".to_string()
    };
    let summary_text = format!(
        "Score: {score}\n\
Distance: {distance:.1} m (+{distance_score})\n\
Kills: {kill_count} (+{kill_score})\n\
Stunts: +{stunt_score} (airtime +{airtime_score}, wheelie +{wheelie_score}, flips +{flip_score})\n\
Airtime Total: {airtime_total:.2}s | Wheelie Total: {wheelie_total:.2}s | Flips: {flip_count}\n\
Big/Huge Jumps: {big_jumps}/{huge_jumps} | Long Wheelies: {long_wheelies}\n\
{no_damage_line}\n\n\
Space - New Run\n\
Q - Quit",
        score = run_summary.score,
        distance = run_summary.distance_m,
        distance_score = run_summary.distance_score,
        kill_count = run_summary.kill_count,
        kill_score = run_summary.kill_score,
        stunt_score = run_summary.stunt_score,
        airtime_score = run_summary.airtime_score,
        wheelie_score = run_summary.wheelie_score,
        flip_score = run_summary.flip_score,
        airtime_total = run_summary.total_airtime_s,
        wheelie_total = run_summary.total_wheelie_s,
        flip_count = run_summary.flip_count,
        big_jumps = run_summary.big_jump_count,
        huge_jumps = run_summary.huge_jump_count,
        long_wheelies = run_summary.long_wheelie_count,
        no_damage_line = no_damage_line,
    );

    commands
        .spawn((
            Name::new("ResultsOverlay"),
            ResultsScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.02, 0.03, 0.94)),
            ZIndex(300),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: Val::Percent(74.0),
                        max_width: Val::Px(980.0),
                        min_width: Val::Px(520.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(10.0),
                        padding: UiRect::all(Val::Px(16.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.08, 0.10, 0.13, 0.96)),
                    BorderColor::all(Color::srgba(0.56, 0.62, 0.68, 0.92)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Text::new(title),
                        TextFont {
                            font_size: 52.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.94, 0.97, 1.00)),
                    ));
                    panel.spawn((
                        Text::new(summary_text),
                        TextFont {
                            font_size: 22.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.90, 0.94, 0.98)),
                    ));
                });
        });

    info!("Entered state: Results");
}

fn cleanup_results_screen(
    mut commands: Commands,
    results_screen_query: Query<Entity, With<ResultsScreenRoot>>,
) {
    for entity in &results_screen_query {
        commands.entity(entity).try_despawn();
    }
}

fn results_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        next_state.set(GameState::Boot);
    }

    if keyboard.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
    }
}
