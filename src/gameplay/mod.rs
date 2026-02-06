pub mod vehicle;

use bevy::prelude::*;
use vehicle::VehicleGameplayPlugin;

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(VehicleGameplayPlugin);
    }
}
