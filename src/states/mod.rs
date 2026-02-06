use crate::config::GameConfig;
use crate::gameplay::combat::EnemyKilledEvent;
use crate::gameplay::vehicle::{PlayerHealth, PlayerVehicle, VehicleTelemetry};
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
                    update_run_summary_score,
                    apply_kill_score_events,
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
    kill_score: u32,
    kill_count: u32,
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
        commands.entity(entity).despawn();
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
    run_summary.kill_score = 0;
    run_summary.kill_count = 0;
    run_summary.was_game_over = false;
    info!("Entered state: InRun");
}

fn update_run_summary_score(
    telemetry: Option<Res<VehicleTelemetry>>,
    mut run_summary: ResMut<RunSummary>,
) {
    let Some(telemetry) = telemetry else {
        return;
    };

    run_summary.distance_m = telemetry.distance_m.max(0.0);
    run_summary.score = run_summary.distance_m.round() as u32 + run_summary.kill_score;
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
        run_summary.score = run_summary.distance_m.round() as u32 + run_summary.kill_score;
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
    let summary_text = format!(
        "{title}\nScore: {}\nDistance: {:.1} m\nKills: {} (+{})\n\nSpace - New Run\nQ - Quit",
        run_summary.score, run_summary.distance_m, run_summary.kill_count, run_summary.kill_score
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
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.82)),
            ZIndex(300),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(summary_text),
                TextFont {
                    font_size: 46.0,
                    ..default()
                },
                TextColor(Color::srgb(0.93, 0.96, 0.99)),
                Node {
                    padding: UiRect::all(Val::Px(16.0)),
                    ..default()
                },
            ));
        });

    info!("Entered state: Results");
}

fn cleanup_results_screen(
    mut commands: Commands,
    results_screen_query: Query<Entity, With<ResultsScreenRoot>>,
) {
    for entity in &results_screen_query {
        commands.entity(entity).despawn();
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
