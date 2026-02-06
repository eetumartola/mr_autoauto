pub mod enemies;
pub mod vehicle;

use bevy::prelude::*;
use enemies::EnemyGameplayPlugin;
use vehicle::VehicleGameplayPlugin;

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(VehicleGameplayPlugin)
            .add_plugins(EnemyGameplayPlugin);
    }
}
