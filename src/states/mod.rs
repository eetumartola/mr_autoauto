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
        app.add_systems(Startup, setup_camera)
            .add_systems(OnEnter(GameState::Boot), enter_boot)
            .add_systems(Update, boot_to_loading.run_if(in_state(GameState::Boot)))
            .add_systems(OnEnter(GameState::Loading), enter_loading)
            .add_systems(OnExit(GameState::Loading), cleanup_loading_screen)
            .add_systems(
                Update,
                loading_to_in_run.run_if(in_state(GameState::Loading)),
            )
            .add_systems(OnEnter(GameState::InRun), enter_in_run)
            .add_systems(Update, in_run_controls.run_if(in_state(GameState::InRun)))
            .add_systems(OnEnter(GameState::Pause), enter_pause)
            .add_systems(Update, pause_controls.run_if(in_state(GameState::Pause)))
            .add_systems(OnEnter(GameState::Results), enter_results)
            .add_systems(
                Update,
                results_controls.run_if(in_state(GameState::Results)),
            );
    }
}

#[derive(Component)]
struct LoadingScreenLogo;

#[derive(Resource, Debug, Clone)]
struct LoadingScreenState {
    entered_at_s: f64,
    logo_handle: Handle<Image>,
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

fn enter_in_run() {
    info!("Entered state: InRun");
}

fn in_run_controls(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::Pause);
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
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

fn enter_results() {
    info!("Entered state: Results");
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
