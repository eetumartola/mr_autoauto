mod assets;
mod commentary_stub;
mod config;
mod debug;
mod gameplay;
mod states;

use assets::AssetRegistryPlugin;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::prelude::*;
use commentary_stub::CommentaryStubPlugin;
use config::ConfigPlugin;
use debug::DebugOverlayPlugin;
use gameplay::GameplayPlugin;
use states::{GameState, GameStatePlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Mr. Autoauto".to_string(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(ConfigPlugin)
        .add_plugins(AssetRegistryPlugin)
        .add_plugins(DebugOverlayPlugin)
        .add_plugins(CommentaryStubPlugin)
        .add_plugins(GameplayPlugin)
        .init_state::<GameState>()
        .add_plugins(GameStatePlugin)
        .run();
}
