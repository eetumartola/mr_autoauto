mod config;
mod states;

use bevy::prelude::*;
use config::ConfigPlugin;
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
        .add_plugins(ConfigPlugin)
        .init_state::<GameState>()
        .add_plugins(GameStatePlugin)
        .run();
}
