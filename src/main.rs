mod states;

use bevy::prelude::*;
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
        .init_state::<GameState>()
        .add_plugins(GameStatePlugin)
        .run();
}
