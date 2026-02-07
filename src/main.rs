mod assets;
mod commentary_stub;
mod config;
mod debug;
mod gameplay;
mod states;

use assets::AssetRegistryPlugin;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use bevy_egui::EguiPlugin;
#[cfg(feature = "gaussian_splats")]
use bevy_gaussian_splatting::GaussianSplattingPlugin;
use bevy_rapier2d::prelude::*;
use commentary_stub::CommentaryStubPlugin;
use config::ConfigPlugin;
use debug::DebugOverlayPlugin;
use gameplay::GameplayPlugin;
use states::{GameState, GameStatePlugin};

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Mr. Autoauto".to_string(),
            resolution: (1280, 720).into(),
            ..default()
        }),
        ..default()
    }))
    .add_plugins(EguiPlugin::default())
    .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(1.0))
    .add_plugins(FrameTimeDiagnosticsPlugin::default())
    .add_plugins(ConfigPlugin)
    .add_plugins(AssetRegistryPlugin)
    .add_plugins(DebugOverlayPlugin)
    .add_plugins(CommentaryStubPlugin)
    .add_plugins(GameplayPlugin)
    .init_state::<GameState>()
    .add_plugins(GameStatePlugin);

    #[cfg(feature = "gaussian_splats")]
    app.add_plugins(GaussianSplattingPlugin);

    app.run();
}
