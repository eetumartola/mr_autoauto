use bevy::app::AppExit;
use bevy::prelude::*;

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

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn enter_boot() {
    info!("Entered state: Boot");
}

fn boot_to_loading(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::Loading);
}

fn enter_loading() {
    info!("Entered state: Loading");
}

fn loading_to_in_run(mut next_state: ResMut<NextState<GameState>>) {
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
