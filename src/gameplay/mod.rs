pub mod combat;
pub mod enemies;
pub mod pickups;
pub mod upgrades;
pub mod vehicle;

use bevy::prelude::*;
use combat::CombatGameplayPlugin;
use enemies::EnemyGameplayPlugin;
use pickups::PickupGameplayPlugin;
use upgrades::UpgradeGameplayPlugin;
use vehicle::VehicleGameplayPlugin;

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(VehicleGameplayPlugin)
            .add_plugins(EnemyGameplayPlugin)
            .add_plugins(PickupGameplayPlugin)
            .add_plugins(UpgradeGameplayPlugin)
            .add_plugins(CombatGameplayPlugin);
    }
}
