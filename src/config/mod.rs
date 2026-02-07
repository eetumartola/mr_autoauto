#![allow(dead_code)]

use bevy::prelude::*;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_DIR: &str = "config";

pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_game_config)
            .add_systems(Update, reload_game_config_hotkey);
    }
}

fn load_game_config(mut commands: Commands) {
    let config = GameConfig::load_from_dir(Path::new(CONFIG_DIR)).unwrap_or_else(|error| {
        panic!("failed to load configuration from `{CONFIG_DIR}`: {error}");
    });

    log_config_summary("Loaded", &config);
    info!("Press F5 to hot-reload config files from `{CONFIG_DIR}`.");

    commands.insert_resource(config);
}

fn reload_game_config_hotkey(
    keyboard: Res<ButtonInput<KeyCode>>,
    game_config: Option<ResMut<GameConfig>>,
) {
    if !keyboard.just_pressed(KeyCode::F5) {
        return;
    }

    let Some(mut current_config) = game_config else {
        warn!("Config hot-reload requested, but `GameConfig` resource is not initialized yet.");
        return;
    };

    match GameConfig::load_from_dir(Path::new(CONFIG_DIR)) {
        Ok(new_config) => {
            *current_config = new_config;
            log_config_summary("Hot-reloaded", &current_config);
        }
        Err(error) => {
            error!("Config hot-reload failed; keeping previous config: {error}");
        }
    }
}

fn log_config_summary(prefix: &str, config: &GameConfig) {
    info!(
        "{prefix} config: {} segments, {} environments, {} enemies, {} weapons.",
        config.segments.segment_sequence.len(),
        config.environments_by_id.len(),
        config.enemy_types_by_id.len(),
        config.weapons_by_id.len()
    );
}

#[derive(Resource, Debug, Clone)]
pub struct GameConfig {
    pub game: GameFile,
    pub assets: AssetsFile,
    pub segments: SegmentsFile,
    pub backgrounds: BackgroundsFile,
    pub environments: EnvironmentsFile,
    pub enemy_types: EnemyTypesFile,
    pub spawners: SpawnersFile,
    pub weapons: WeaponsFile,
    pub vehicles: VehiclesFile,
    pub upgrades: UpgradesFile,
    pub commentator: CommentatorFile,
    pub backgrounds_by_id: HashMap<String, BackgroundConfig>,
    pub environments_by_id: HashMap<String, EnvironmentConfig>,
    pub enemy_types_by_id: HashMap<String, EnemyTypeConfig>,
    pub spawners_by_id: HashMap<String, SpawnerConfig>,
    pub weapons_by_id: HashMap<String, WeaponConfig>,
    pub vehicles_by_id: HashMap<String, VehicleConfig>,
    pub upgrades_by_id: HashMap<String, UpgradeConfig>,
    pub sprite_assets_by_id: HashMap<String, SpriteAssetConfig>,
    pub model_assets_by_id: HashMap<String, ModelAssetConfig>,
    pub splat_assets_by_id: HashMap<String, SplatAssetConfig>,
    pub audio_assets_by_id: HashMap<String, AudioAssetConfig>,
}

impl GameConfig {
    pub fn load_from_dir(config_dir: &Path) -> Result<Self, ConfigError> {
        let game: GameFile = read_toml(&config_dir.join("game.toml"))?;
        let assets: AssetsFile = read_toml(&config_dir.join("assets.toml"))?;
        let segments: SegmentsFile = read_toml(&config_dir.join("segments.toml"))?;
        let backgrounds: BackgroundsFile = read_toml(&config_dir.join("backgrounds.toml"))?;
        let environments: EnvironmentsFile = read_toml(&config_dir.join("environments.toml"))?;
        let enemy_types: EnemyTypesFile = read_toml(&config_dir.join("enemy_types.toml"))?;
        let spawners: SpawnersFile = read_toml(&config_dir.join("spawners.toml"))?;
        let weapons: WeaponsFile = read_toml(&config_dir.join("weapons.toml"))?;
        let vehicles: VehiclesFile = read_toml(&config_dir.join("vehicles.toml"))?;
        let upgrades: UpgradesFile = read_toml(&config_dir.join("upgrades.toml"))?;
        let commentator: CommentatorFile = read_toml(&config_dir.join("commentator.toml"))?;

        let config = Self {
            sprite_assets_by_id: to_index("assets.toml::sprites", &assets.sprites)?,
            model_assets_by_id: to_index("assets.toml::models", &assets.models)?,
            splat_assets_by_id: to_index("assets.toml::splats", &assets.splats)?,
            audio_assets_by_id: to_index("assets.toml::audio", &assets.audio)?,
            backgrounds_by_id: to_index("backgrounds.toml::backgrounds", &backgrounds.backgrounds)?,
            environments_by_id: to_index(
                "environments.toml::environments",
                &environments.environments,
            )?,
            enemy_types_by_id: to_index("enemy_types.toml::enemy_types", &enemy_types.enemy_types)?,
            spawners_by_id: to_index("spawners.toml::spawners", &spawners.spawners)?,
            weapons_by_id: to_index("weapons.toml::weapons", &weapons.weapons)?,
            vehicles_by_id: to_index("vehicles.toml::vehicles", &vehicles.vehicles)?,
            upgrades_by_id: to_index("upgrades.toml::upgrades", &upgrades.upgrades)?,
            game,
            assets,
            segments,
            backgrounds,
            environments,
            enemy_types,
            spawners,
            weapons,
            vehicles,
            upgrades,
            commentator,
        };

        config.validate_references()?;
        Ok(config)
    }

    fn validate_references(&self) -> Result<(), ConfigError> {
        if !self
            .environments_by_id
            .contains_key(&self.game.app.starting_environment)
        {
            return Err(ConfigError::Validation(format!(
                "game.toml::app.starting_environment references unknown environment id `{}`",
                self.game.app.starting_environment
            )));
        }

        if !self
            .vehicles_by_id
            .contains_key(&self.game.app.default_vehicle)
        {
            return Err(ConfigError::Validation(format!(
                "game.toml::app.default_vehicle references unknown vehicle id `{}`",
                self.game.app.default_vehicle
            )));
        }

        for (index, segment) in self.segments.segment_sequence.iter().enumerate() {
            if !self.backgrounds_by_id.contains_key(&segment.id) {
                return Err(ConfigError::Validation(format!(
                    "segments.toml::segment_sequence[{index}].id `{}` is missing in backgrounds.toml::backgrounds",
                    segment.id
                )));
            }

            if !self.environments_by_id.contains_key(&segment.environment) {
                return Err(ConfigError::Validation(format!(
                    "segments.toml::segment_sequence[{index}].environment references unknown environment id `{}`",
                    segment.environment
                )));
            }

            if let Some(spawn_set) = segment.spawn_set.as_deref() {
                if !self.spawners_by_id.contains_key(spawn_set) {
                    return Err(ConfigError::Validation(format!(
                        "segments.toml::segment_sequence[{index}].spawn_set references unknown spawner id `{spawn_set}`"
                    )));
                }
            }
        }

        for (index, background) in self.backgrounds.backgrounds.iter().enumerate() {
            if !background.parallax.is_finite() {
                return Err(ConfigError::Validation(format!(
                    "backgrounds.toml::backgrounds[{index}].parallax must be finite"
                )));
            }
            if !background.offset_x_m.is_finite()
                || !background.offset_y_m.is_finite()
                || !background.offset_z_m.is_finite()
            {
                return Err(ConfigError::Validation(format!(
                    "backgrounds.toml::backgrounds[{index}] offsets must be finite"
                )));
            }
            if !background.scale_x.is_finite()
                || !background.scale_y.is_finite()
                || !background.scale_z.is_finite()
                || background.scale_x.abs() <= f32::EPSILON
                || background.scale_y.abs() <= f32::EPSILON
                || background.scale_z.abs() <= f32::EPSILON
            {
                return Err(ConfigError::Validation(format!(
                    "backgrounds.toml::backgrounds[{index}] scales must be finite and non-zero (negative values are allowed for axis flips)"
                )));
            }
            if !background.loop_length_m.is_finite() || background.loop_length_m < 0.0 {
                return Err(ConfigError::Validation(format!(
                    "backgrounds.toml::backgrounds[{index}].loop_length_m must be >= 0"
                )));
            }
            if let Some(splat_asset_id) = background.splat_asset_id.as_deref() {
                if !self.splat_assets_by_id.contains_key(splat_asset_id) {
                    return Err(ConfigError::Validation(format!(
                        "backgrounds.toml::backgrounds[{index}].splat_asset_id references unknown splat id `{splat_asset_id}`"
                    )));
                }
            }
        }

        for (index, enemy) in self.enemy_types.enemy_types.iter().enumerate() {
            if !self.weapons_by_id.contains_key(&enemy.weapon_id) {
                return Err(ConfigError::Validation(format!(
                    "enemy_types.toml::enemy_types[{index}].weapon_id references unknown weapon id `{}`",
                    enemy.weapon_id
                )));
            }
            if enemy.health <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "enemy_types.toml::enemy_types[{index}].health must be > 0"
                )));
            }
            if enemy.speed <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "enemy_types.toml::enemy_types[{index}].speed must be > 0"
                )));
            }
            if enemy.hitbox_radius <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "enemy_types.toml::enemy_types[{index}].hitbox_radius must be > 0"
                )));
            }
            if !matches!(
                enemy.behavior.as_str(),
                "walker" | "flier" | "turret" | "charger" | "bomber"
            ) {
                return Err(ConfigError::Validation(format!(
                    "enemy_types.toml::enemy_types[{index}].behavior `{}` is unsupported (expected walker/flier/turret/charger/bomber)",
                    enemy.behavior
                )));
            }
            if enemy.behavior == "flier"
                && (enemy.hover_amplitude <= 0.0 || enemy.hover_frequency <= 0.0)
            {
                return Err(ConfigError::Validation(format!(
                    "enemy_types.toml::enemy_types[{index}] flier behavior requires hover_amplitude > 0 and hover_frequency > 0"
                )));
            }
            if enemy.behavior == "charger" && enemy.charge_speed_multiplier <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "enemy_types.toml::enemy_types[{index}] charger behavior requires charge_speed_multiplier > 0"
                )));
            }
        }

        for (index, spawner) in self.spawners.spawners.iter().enumerate() {
            for (enemy_index, enemy_id) in spawner.spawn_enemy_ids.iter().enumerate() {
                if !self.enemy_types_by_id.contains_key(enemy_id) {
                    return Err(ConfigError::Validation(format!(
                        "spawners.toml::spawners[{index}].spawn_enemy_ids[{enemy_index}] references unknown enemy id `{enemy_id}`"
                    )));
                }
            }
        }

        for (index, weapon) in self.weapons.weapons.iter().enumerate() {
            if !matches!(weapon.projectile_type.as_str(), "bullet" | "missile") {
                return Err(ConfigError::Validation(format!(
                    "weapons.toml::weapons[{index}].projectile_type `{}` is unsupported (expected bullet/missile)",
                    weapon.projectile_type
                )));
            }
            if weapon.bullet_speed <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "weapons.toml::weapons[{index}].bullet_speed must be > 0"
                )));
            }
            if weapon.fire_rate <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "weapons.toml::weapons[{index}].fire_rate must be > 0"
                )));
            }
            if weapon.spread_degrees < 0.0 {
                return Err(ConfigError::Validation(format!(
                    "weapons.toml::weapons[{index}].spread_degrees must be >= 0"
                )));
            }
            if weapon.damage <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "weapons.toml::weapons[{index}].damage must be > 0"
                )));
            }
            if weapon.burst_count == 0 {
                return Err(ConfigError::Validation(format!(
                    "weapons.toml::weapons[{index}].burst_count must be >= 1"
                )));
            }
            if weapon.burst_interval_seconds < 0.0 {
                return Err(ConfigError::Validation(format!(
                    "weapons.toml::weapons[{index}].burst_interval_seconds must be >= 0"
                )));
            }
            if weapon.projectile_drag < 0.0 {
                return Err(ConfigError::Validation(format!(
                    "weapons.toml::weapons[{index}].projectile_drag must be >= 0"
                )));
            }
            if weapon.projectile_lifetime_seconds <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "weapons.toml::weapons[{index}].projectile_lifetime_seconds must be > 0"
                )));
            }
            if weapon.missile_gravity_scale < 0.0 {
                return Err(ConfigError::Validation(format!(
                    "weapons.toml::weapons[{index}].missile_gravity_scale must be >= 0"
                )));
            }
            if weapon.homing_turn_rate_degrees < 0.0 {
                return Err(ConfigError::Validation(format!(
                    "weapons.toml::weapons[{index}].homing_turn_rate_degrees must be >= 0"
                )));
            }
        }

        for (index, vehicle) in self.vehicles.vehicles.iter().enumerate() {
            if !self.weapons_by_id.contains_key(&vehicle.default_weapon_id) {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].default_weapon_id references unknown weapon id `{}`",
                    vehicle.default_weapon_id
                )));
            }
            if let Some(secondary_weapon_id) = vehicle.secondary_weapon_id.as_deref() {
                let Some(secondary_weapon) = self.weapons_by_id.get(secondary_weapon_id) else {
                    return Err(ConfigError::Validation(format!(
                        "vehicles.toml::vehicles[{index}].secondary_weapon_id references unknown weapon id `{secondary_weapon_id}`"
                    )));
                };
                if secondary_weapon.projectile_type != "missile" {
                    return Err(ConfigError::Validation(format!(
                        "vehicles.toml::vehicles[{index}].secondary_weapon_id must point to a missile weapon"
                    )));
                }
                if vehicle.missile_fire_interval_seconds <= 0.0 {
                    return Err(ConfigError::Validation(format!(
                        "vehicles.toml::vehicles[{index}].missile_fire_interval_seconds must be > 0 when secondary_weapon_id is set"
                    )));
                }
            }
            if vehicle.max_forward_speed <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].max_forward_speed must be > 0"
                )));
            }
            if vehicle.max_reverse_speed <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].max_reverse_speed must be > 0"
                )));
            }
            if vehicle.max_fall_speed <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].max_fall_speed must be > 0"
                )));
            }
            if vehicle.air_max_rotation_speed <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].air_max_rotation_speed must be > 0"
                )));
            }
            if vehicle.linear_speed_scale <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].linear_speed_scale must be > 0"
                )));
            }
            if vehicle.ground_coast_damping < 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].ground_coast_damping must be >= 0"
                )));
            }
            if vehicle.air_base_damping < 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].air_base_damping must be >= 0"
                )));
            }
            if vehicle.air_env_drag_factor < 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].air_env_drag_factor must be >= 0"
                )));
            }
            if vehicle.linear_inertia <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].linear_inertia must be > 0"
                )));
            }
            if vehicle.rotational_inertia <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].rotational_inertia must be > 0"
                )));
            }
            if vehicle.gravity_scale <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].gravity_scale must be > 0"
                )));
            }
            if vehicle.suspension_rest_length_m <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].suspension_rest_length_m must be > 0"
                )));
            }
            if vehicle.suspension_stiffness <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].suspension_stiffness must be > 0"
                )));
            }
            if vehicle.suspension_damping < 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].suspension_damping must be >= 0"
                )));
            }
            if vehicle.suspension_max_compression_m <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].suspension_max_compression_m must be > 0"
                )));
            }
            if vehicle.suspension_max_extension_m < 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].suspension_max_extension_m must be >= 0"
                )));
            }
            if vehicle.tire_longitudinal_grip <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].tire_longitudinal_grip must be > 0"
                )));
            }
            if !(0.0..=1.0).contains(&vehicle.tire_slip_grip_floor) {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].tire_slip_grip_floor must be in [0, 1]"
                )));
            }
            if !(0.0..=1.0).contains(&vehicle.front_drive_ratio) {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].front_drive_ratio must be in [0, 1]"
                )));
            }
            if vehicle.rear_drive_traction_assist_distance_m < 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].rear_drive_traction_assist_distance_m must be >= 0"
                )));
            }
            if !(0.0..=1.0).contains(&vehicle.rear_drive_traction_assist_min_factor) {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].rear_drive_traction_assist_min_factor must be in [0, 1]"
                )));
            }
            if vehicle.turret_range_m <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].turret_range_m must be > 0"
                )));
            }
            if !(0.0 < vehicle.turret_cone_degrees && vehicle.turret_cone_degrees <= 180.0) {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].turret_cone_degrees must be in (0, 180]"
                )));
            }
            if !matches!(
                vehicle.turret_target_priority.as_str(),
                "nearest" | "strongest"
            ) {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].turret_target_priority `{}` is unsupported (expected nearest/strongest)",
                    vehicle.turret_target_priority
                )));
            }
            if vehicle.camera_look_ahead_max <= vehicle.camera_look_ahead_min {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}] camera look-ahead range is invalid (max must be > min)"
                )));
            }
        }

        if !self.game.terrain.wave_a_frequency.is_finite()
            || !self.game.terrain.wave_b_frequency.is_finite()
            || !self.game.terrain.wave_c_frequency.is_finite()
            || self.game.terrain.wave_a_frequency < 0.0
            || self.game.terrain.wave_b_frequency < 0.0
            || self.game.terrain.wave_c_frequency < 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::terrain wave frequencies must be >= 0".to_string(),
            ));
        }
        if !self.game.scoring.points_per_meter.is_finite()
            || self.game.scoring.points_per_meter < 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::scoring.points_per_meter must be >= 0".to_string(),
            ));
        }
        if !self.game.scoring.airtime_points_per_second.is_finite()
            || self.game.scoring.airtime_points_per_second < 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::scoring.airtime_points_per_second must be >= 0".to_string(),
            ));
        }
        if !self.game.scoring.wheelie_points_per_second.is_finite()
            || self.game.scoring.wheelie_points_per_second < 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::scoring.wheelie_points_per_second must be >= 0".to_string(),
            ));
        }
        if !self.game.pickups.despawn_seconds.is_finite()
            || self.game.pickups.despawn_seconds <= 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups.despawn_seconds must be > 0".to_string(),
            ));
        }
        if !self.game.pickups.despawn_behind_player_m.is_finite()
            || self.game.pickups.despawn_behind_player_m < 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups.despawn_behind_player_m must be >= 0".to_string(),
            ));
        }
        if !self.game.pickups.gravity_mps2.is_finite() || self.game.pickups.gravity_mps2 < 0.0 {
            return Err(ConfigError::Validation(
                "game.toml::pickups.gravity_mps2 must be >= 0".to_string(),
            ));
        }
        if !self.game.pickups.bounce_damping.is_finite()
            || !(0.0..=1.0).contains(&self.game.pickups.bounce_damping)
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups.bounce_damping must be in [0, 1]".to_string(),
            ));
        }
        if !self.game.pickups.ground_stop_speed_mps.is_finite()
            || self.game.pickups.ground_stop_speed_mps < 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups.ground_stop_speed_mps must be >= 0".to_string(),
            ));
        }
        if !self.game.pickups.ground_slide_damping.is_finite()
            || !(0.0..=1.0).contains(&self.game.pickups.ground_slide_damping)
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups.ground_slide_damping must be in [0, 1]".to_string(),
            ));
        }
        if !self.game.pickups.collection_radius_m.is_finite()
            || self.game.pickups.collection_radius_m <= 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups.collection_radius_m must be > 0".to_string(),
            ));
        }
        if !self.game.pickups.drop_horizontal_spread_mps.is_finite()
            || self.game.pickups.drop_horizontal_spread_mps < 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups.drop_horizontal_spread_mps must be >= 0".to_string(),
            ));
        }
        if !self.game.pickups.drop_vertical_speed_min_mps.is_finite()
            || !self.game.pickups.drop_vertical_speed_max_mps.is_finite()
            || self.game.pickups.drop_vertical_speed_min_mps < 0.0
            || self.game.pickups.drop_vertical_speed_max_mps
                < self.game.pickups.drop_vertical_speed_min_mps
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups drop vertical speed range is invalid".to_string(),
            ));
        }
        if !self.game.pickups.health_drop_chance.is_finite()
            || !(0.0..=1.0).contains(&self.game.pickups.health_drop_chance)
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups.health_drop_chance must be in [0, 1]".to_string(),
            ));
        }
        if !self.game.pickups.health_drop_heal_amount.is_finite()
            || self.game.pickups.health_drop_heal_amount < 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups.health_drop_heal_amount must be >= 0".to_string(),
            ));
        }
        if !self.game.pickups.coin_score_scale.is_finite()
            || self.game.pickups.coin_score_scale < 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups.coin_score_scale must be >= 0".to_string(),
            ));
        }
        if !self.game.pickups.coin_radius_m.is_finite() || self.game.pickups.coin_radius_m <= 0.0 {
            return Err(ConfigError::Validation(
                "game.toml::pickups.coin_radius_m must be > 0".to_string(),
            ));
        }
        if !self.game.pickups.health_box_size_m.is_finite()
            || self.game.pickups.health_box_size_m <= 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups.health_box_size_m must be > 0".to_string(),
            ));
        }
        if !self.game.pickups.coin_pickup_radius_m.is_finite()
            || self.game.pickups.coin_pickup_radius_m <= 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups.coin_pickup_radius_m must be > 0".to_string(),
            ));
        }
        if !self.game.pickups.health_pickup_radius_m.is_finite()
            || self.game.pickups.health_pickup_radius_m <= 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups.health_pickup_radius_m must be > 0".to_string(),
            ));
        }
        if !self.game.pickups.coin_spin_speed_min_rad_s.is_finite()
            || !self.game.pickups.coin_spin_speed_max_rad_s.is_finite()
            || self.game.pickups.coin_spin_speed_min_rad_s
                > self.game.pickups.coin_spin_speed_max_rad_s
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups coin spin speed range is invalid".to_string(),
            ));
        }
        if !self.game.pickups.health_spin_speed_min_rad_s.is_finite()
            || !self.game.pickups.health_spin_speed_max_rad_s.is_finite()
            || self.game.pickups.health_spin_speed_min_rad_s
                > self.game.pickups.health_spin_speed_max_rad_s
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups health spin speed range is invalid".to_string(),
            ));
        }
        if !self.game.pickups.coin_jitter_x_m.is_finite()
            || !self.game.pickups.coin_jitter_y_m.is_finite()
            || !self.game.pickups.health_jitter_x_m.is_finite()
            || !self.game.pickups.health_jitter_y_m.is_finite()
            || self.game.pickups.coin_jitter_x_m < 0.0
            || self.game.pickups.coin_jitter_y_m < 0.0
            || self.game.pickups.health_jitter_x_m < 0.0
            || self.game.pickups.health_jitter_y_m < 0.0
        {
            return Err(ConfigError::Validation(
                "game.toml::pickups jitter values must be >= 0".to_string(),
            ));
        }
        if self.game.run_upgrades.coins_per_offer == 0 {
            return Err(ConfigError::Validation(
                "game.toml::run_upgrades.coins_per_offer must be >= 1".to_string(),
            ));
        }
        if self.game.run_upgrades.choices_per_offer == 0 {
            return Err(ConfigError::Validation(
                "game.toml::run_upgrades.choices_per_offer must be >= 1".to_string(),
            ));
        }
        if self.game.run_upgrades.options.is_empty() {
            return Err(ConfigError::Validation(
                "game.toml::run_upgrades.options must include at least one option".to_string(),
            ));
        }
        let mut seen_upgrade_ids = std::collections::HashSet::new();
        for (index, option) in self.game.run_upgrades.options.iter().enumerate() {
            if option.id.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "game.toml::run_upgrades.options[{index}].id cannot be empty"
                )));
            }
            if !seen_upgrade_ids.insert(option.id.clone()) {
                return Err(ConfigError::Validation(format!(
                    "game.toml::run_upgrades.options contains duplicate id `{}`",
                    option.id
                )));
            }
            if option.label.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "game.toml::run_upgrades.options[{index}].label cannot be empty"
                )));
            }
            if !option.value.is_finite() || option.value <= 0.0 {
                return Err(ConfigError::Validation(format!(
                    "game.toml::run_upgrades.options[{index}].value must be > 0"
                )));
            }
            if option.max_stacks == 0 {
                return Err(ConfigError::Validation(format!(
                    "game.toml::run_upgrades.options[{index}].max_stacks must be >= 1"
                )));
            }
        }
        if self.commentator.commentary.min_seconds_between_lines < 0.0 {
            return Err(ConfigError::Validation(
                "commentator.toml::commentary.min_seconds_between_lines must be >= 0".to_string(),
            ));
        }
        if self.commentator.commentary.max_events_per_batch == 0 {
            return Err(ConfigError::Validation(
                "commentator.toml::commentary.max_events_per_batch must be >= 1".to_string(),
            ));
        }
        if !self
            .commentator
            .commentary
            .api_retry_backoff_seconds
            .is_finite()
            || self.commentator.commentary.api_retry_backoff_seconds < 0.0
        {
            return Err(ConfigError::Validation(
                "commentator.toml::commentary.api_retry_backoff_seconds must be >= 0".to_string(),
            ));
        }
        if !self
            .commentator
            .commentary
            .api_stale_request_timeout_seconds
            .is_finite()
            || self
                .commentator
                .commentary
                .api_stale_request_timeout_seconds
                <= 0.0
        {
            return Err(ConfigError::Validation(
                "commentator.toml::commentary.api_stale_request_timeout_seconds must be > 0"
                    .to_string(),
            ));
        }
        if !self.commentator.commentary.narration_volume.is_finite()
            || self.commentator.commentary.narration_volume < 0.0
        {
            return Err(ConfigError::Validation(
                "commentator.toml::commentary.narration_volume must be >= 0".to_string(),
            ));
        }
        if self.commentator.commentators.len() < 2 {
            return Err(ConfigError::Validation(
                "commentator.toml must define at least two `[[commentators]]` profiles".to_string(),
            ));
        }
        let mut commentator_ids = std::collections::HashSet::new();
        for (index, commentator) in self.commentator.commentators.iter().enumerate() {
            if commentator.id.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "commentator.toml::commentators[{index}].id cannot be empty"
                )));
            }
            if commentator.name.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "commentator.toml::commentators[{index}].name cannot be empty"
                )));
            }
            if commentator.character_id.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "commentator.toml::commentators[{index}].character_id cannot be empty"
                )));
            }
            if !commentator_ids.insert(commentator.id.clone()) {
                return Err(ConfigError::Validation(format!(
                    "commentator.toml::commentators contains duplicate id `{}`",
                    commentator.id
                )));
            }
            if commentator.style_instruction.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "commentator.toml::commentators[{index}].style_instruction cannot be empty"
                )));
            }
            if !matches!(
                commentator.style_length.as_str(),
                "short" | "medium" | "long"
            ) {
                return Err(ConfigError::Validation(format!(
                    "commentator.toml::commentators[{index}].style_length `{}` is unsupported (expected short/medium/long)",
                    commentator.style_length
                )));
            }
            if commentator.emotions.is_empty() {
                return Err(ConfigError::Validation(format!(
                    "commentator.toml::commentators[{index}].emotions must contain at least one emotion"
                )));
            }
            for (emotion_index, emotion) in commentator.emotions.iter().enumerate() {
                if emotion.trim().is_empty() {
                    return Err(ConfigError::Validation(format!(
                        "commentator.toml::commentators[{index}].emotions[{emotion_index}] cannot be empty"
                    )));
                }
            }
        }

        for (index, sprite) in self.assets.sprites.iter().enumerate() {
            if sprite.path.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "assets.toml::sprites[{index}].path cannot be empty"
                )));
            }
        }

        for (index, model) in self.assets.models.iter().enumerate() {
            if model.scene_path.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "assets.toml::models[{index}].scene_path cannot be empty"
                )));
            }
            if model.root_node.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "assets.toml::models[{index}].root_node cannot be empty"
                )));
            }
            if model.wheel_nodes.is_empty() {
                return Err(ConfigError::Validation(format!(
                    "assets.toml::models[{index}].wheel_nodes must contain at least one node name"
                )));
            }
        }

        for (index, splat) in self.assets.splats.iter().enumerate() {
            if splat.path.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "assets.toml::splats[{index}].path cannot be empty"
                )));
            }
        }

        for (index, audio) in self.assets.audio.iter().enumerate() {
            if audio.path.trim().is_empty() {
                return Err(ConfigError::Validation(format!(
                    "assets.toml::audio[{index}].path cannot be empty"
                )));
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
    Validation(String),
}

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "failed to read `{}`: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(f, "failed to parse `{}`: {source}", path.display())
            }
            Self::Validation(message) => write!(f, "{message}"),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Validation(_) => None,
        }
    }
}

fn read_toml<T: DeserializeOwned>(path: &Path) -> Result<T, ConfigError> {
    let raw = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&raw).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })
}

fn to_index<T>(label: &str, rows: &[T]) -> Result<HashMap<String, T>, ConfigError>
where
    T: HasId + Clone,
{
    let mut map = HashMap::new();

    for row in rows {
        let id = row.id();
        if id.trim().is_empty() {
            return Err(ConfigError::Validation(format!(
                "{label} contains an empty id"
            )));
        }

        if map.insert(id.to_string(), row.clone()).is_some() {
            return Err(ConfigError::Validation(format!(
                "{label} contains duplicate id `{id}`"
            )));
        }
    }

    Ok(map)
}

trait HasId {
    fn id(&self) -> &str;
}

#[derive(Debug, Clone, Deserialize)]
pub struct GameFile {
    pub app: AppConfig,
    pub terrain: TerrainConfig,
    #[serde(default)]
    pub scoring: ScoringConfig,
    #[serde(default)]
    pub pickups: PickupConfig,
    #[serde(default)]
    pub run_upgrades: RunUpgradeConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub fixed_timestep_hz: f32,
    pub starting_environment: String,
    pub default_vehicle: String,
    pub debug_overlay: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TerrainConfig {
    pub base_height: f32,
    pub ramp_slope: f32,
    pub wave_a_amplitude: f32,
    pub wave_a_frequency: f32,
    pub wave_b_amplitude: f32,
    pub wave_b_frequency: f32,
    #[serde(default = "default_terrain_wave_c_amplitude")]
    pub wave_c_amplitude: f32,
    #[serde(default = "default_terrain_wave_c_frequency")]
    pub wave_c_frequency: f32,
}

fn default_terrain_wave_c_amplitude() -> f32 {
    0.0
}

fn default_terrain_wave_c_frequency() -> f32 {
    0.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScoringConfig {
    #[serde(default = "default_points_per_meter")]
    pub points_per_meter: f32,
    #[serde(default = "default_airtime_points_per_second")]
    pub airtime_points_per_second: f32,
    #[serde(default = "default_wheelie_points_per_second")]
    pub wheelie_points_per_second: f32,
    #[serde(default = "default_flip_points")]
    pub flip_points: u32,
    #[serde(default = "default_no_damage_bonus")]
    pub no_damage_bonus: u32,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            points_per_meter: default_points_per_meter(),
            airtime_points_per_second: default_airtime_points_per_second(),
            wheelie_points_per_second: default_wheelie_points_per_second(),
            flip_points: default_flip_points(),
            no_damage_bonus: default_no_damage_bonus(),
        }
    }
}

fn default_points_per_meter() -> f32 {
    1.0
}

fn default_airtime_points_per_second() -> f32 {
    14.0
}

fn default_wheelie_points_per_second() -> f32 {
    18.0
}

fn default_flip_points() -> u32 {
    120
}

fn default_no_damage_bonus() -> u32 {
    600
}

#[derive(Debug, Clone, Deserialize)]
pub struct PickupConfig {
    #[serde(default = "default_pickup_despawn_seconds")]
    pub despawn_seconds: f32,
    #[serde(default = "default_pickup_despawn_behind_player_m")]
    pub despawn_behind_player_m: f32,
    #[serde(default = "default_pickup_gravity_mps2")]
    pub gravity_mps2: f32,
    #[serde(default = "default_pickup_bounce_damping")]
    pub bounce_damping: f32,
    #[serde(default = "default_pickup_ground_stop_speed_mps")]
    pub ground_stop_speed_mps: f32,
    #[serde(default = "default_pickup_ground_slide_damping")]
    pub ground_slide_damping: f32,
    #[serde(default = "default_pickup_collection_radius_m")]
    pub collection_radius_m: f32,
    #[serde(default = "default_pickup_drop_horizontal_spread_mps")]
    pub drop_horizontal_spread_mps: f32,
    #[serde(default = "default_pickup_drop_vertical_speed_min_mps")]
    pub drop_vertical_speed_min_mps: f32,
    #[serde(default = "default_pickup_drop_vertical_speed_max_mps")]
    pub drop_vertical_speed_max_mps: f32,
    #[serde(default = "default_pickup_health_drop_chance")]
    pub health_drop_chance: f32,
    #[serde(default = "default_pickup_health_drop_heal_amount")]
    pub health_drop_heal_amount: f32,
    #[serde(default = "default_pickup_coin_score_min")]
    pub coin_score_min: u32,
    #[serde(default = "default_pickup_coin_score_scale")]
    pub coin_score_scale: f32,
    #[serde(default = "default_pickup_coin_radius_m")]
    pub coin_radius_m: f32,
    #[serde(default = "default_pickup_health_box_size_m")]
    pub health_box_size_m: f32,
    #[serde(default = "default_pickup_coin_pickup_radius_m")]
    pub coin_pickup_radius_m: f32,
    #[serde(default = "default_pickup_health_pickup_radius_m")]
    pub health_pickup_radius_m: f32,
    #[serde(default = "default_pickup_coin_spin_speed_min_rad_s")]
    pub coin_spin_speed_min_rad_s: f32,
    #[serde(default = "default_pickup_coin_spin_speed_max_rad_s")]
    pub coin_spin_speed_max_rad_s: f32,
    #[serde(default = "default_pickup_health_spin_speed_min_rad_s")]
    pub health_spin_speed_min_rad_s: f32,
    #[serde(default = "default_pickup_health_spin_speed_max_rad_s")]
    pub health_spin_speed_max_rad_s: f32,
    #[serde(default = "default_pickup_coin_jitter_x_m")]
    pub coin_jitter_x_m: f32,
    #[serde(default = "default_pickup_coin_jitter_y_m")]
    pub coin_jitter_y_m: f32,
    #[serde(default = "default_pickup_health_jitter_x_m")]
    pub health_jitter_x_m: f32,
    #[serde(default = "default_pickup_health_jitter_y_m")]
    pub health_jitter_y_m: f32,
}

impl Default for PickupConfig {
    fn default() -> Self {
        Self {
            despawn_seconds: default_pickup_despawn_seconds(),
            despawn_behind_player_m: default_pickup_despawn_behind_player_m(),
            gravity_mps2: default_pickup_gravity_mps2(),
            bounce_damping: default_pickup_bounce_damping(),
            ground_stop_speed_mps: default_pickup_ground_stop_speed_mps(),
            ground_slide_damping: default_pickup_ground_slide_damping(),
            collection_radius_m: default_pickup_collection_radius_m(),
            drop_horizontal_spread_mps: default_pickup_drop_horizontal_spread_mps(),
            drop_vertical_speed_min_mps: default_pickup_drop_vertical_speed_min_mps(),
            drop_vertical_speed_max_mps: default_pickup_drop_vertical_speed_max_mps(),
            health_drop_chance: default_pickup_health_drop_chance(),
            health_drop_heal_amount: default_pickup_health_drop_heal_amount(),
            coin_score_min: default_pickup_coin_score_min(),
            coin_score_scale: default_pickup_coin_score_scale(),
            coin_radius_m: default_pickup_coin_radius_m(),
            health_box_size_m: default_pickup_health_box_size_m(),
            coin_pickup_radius_m: default_pickup_coin_pickup_radius_m(),
            health_pickup_radius_m: default_pickup_health_pickup_radius_m(),
            coin_spin_speed_min_rad_s: default_pickup_coin_spin_speed_min_rad_s(),
            coin_spin_speed_max_rad_s: default_pickup_coin_spin_speed_max_rad_s(),
            health_spin_speed_min_rad_s: default_pickup_health_spin_speed_min_rad_s(),
            health_spin_speed_max_rad_s: default_pickup_health_spin_speed_max_rad_s(),
            coin_jitter_x_m: default_pickup_coin_jitter_x_m(),
            coin_jitter_y_m: default_pickup_coin_jitter_y_m(),
            health_jitter_x_m: default_pickup_health_jitter_x_m(),
            health_jitter_y_m: default_pickup_health_jitter_y_m(),
        }
    }
}

fn default_pickup_despawn_seconds() -> f32 {
    18.0
}

fn default_pickup_despawn_behind_player_m() -> f32 {
    96.0
}

fn default_pickup_gravity_mps2() -> f32 {
    22.0
}

fn default_pickup_bounce_damping() -> f32 {
    0.28
}

fn default_pickup_ground_stop_speed_mps() -> f32 {
    0.85
}

fn default_pickup_ground_slide_damping() -> f32 {
    0.94
}

fn default_pickup_collection_radius_m() -> f32 {
    1.45
}

fn default_pickup_drop_horizontal_spread_mps() -> f32 {
    4.2
}

fn default_pickup_drop_vertical_speed_min_mps() -> f32 {
    3.2
}

fn default_pickup_drop_vertical_speed_max_mps() -> f32 {
    5.8
}

fn default_pickup_health_drop_chance() -> f32 {
    0.24
}

fn default_pickup_health_drop_heal_amount() -> f32 {
    22.0
}

fn default_pickup_coin_score_min() -> u32 {
    8
}

fn default_pickup_coin_score_scale() -> f32 {
    0.55
}

fn default_pickup_coin_radius_m() -> f32 {
    0.32
}

fn default_pickup_health_box_size_m() -> f32 {
    0.62
}

fn default_pickup_coin_pickup_radius_m() -> f32 {
    0.44
}

fn default_pickup_health_pickup_radius_m() -> f32 {
    0.50
}

fn default_pickup_coin_spin_speed_min_rad_s() -> f32 {
    2.8
}

fn default_pickup_coin_spin_speed_max_rad_s() -> f32 {
    5.2
}

fn default_pickup_health_spin_speed_min_rad_s() -> f32 {
    -3.2
}

fn default_pickup_health_spin_speed_max_rad_s() -> f32 {
    3.2
}

fn default_pickup_coin_jitter_x_m() -> f32 {
    0.24
}

fn default_pickup_coin_jitter_y_m() -> f32 {
    0.18
}

fn default_pickup_health_jitter_x_m() -> f32 {
    0.28
}

fn default_pickup_health_jitter_y_m() -> f32 {
    0.14
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunUpgradeConfig {
    #[serde(default = "default_run_upgrade_coins_per_offer")]
    pub coins_per_offer: u32,
    #[serde(default = "default_run_upgrade_choices_per_offer")]
    pub choices_per_offer: usize,
    #[serde(default = "default_run_upgrade_options")]
    pub options: Vec<RunUpgradeOptionConfig>,
}

impl Default for RunUpgradeConfig {
    fn default() -> Self {
        Self {
            coins_per_offer: default_run_upgrade_coins_per_offer(),
            choices_per_offer: default_run_upgrade_choices_per_offer(),
            options: default_run_upgrade_options(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunUpgradeEffectKind {
    HealthFlat,
    WeaponFireRatePercent,
    MissileFireRatePercent,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunUpgradeOptionConfig {
    pub id: String,
    pub label: String,
    pub effect: RunUpgradeEffectKind,
    pub value: f32,
    #[serde(default = "default_run_upgrade_max_stacks")]
    pub max_stacks: u32,
}

fn default_run_upgrade_coins_per_offer() -> u32 {
    5
}

fn default_run_upgrade_choices_per_offer() -> usize {
    2
}

fn default_run_upgrade_max_stacks() -> u32 {
    50
}

fn default_run_upgrade_options() -> Vec<RunUpgradeOptionConfig> {
    vec![
        RunUpgradeOptionConfig {
            id: "health_plus_10".to_string(),
            label: "Health +10".to_string(),
            effect: RunUpgradeEffectKind::HealthFlat,
            value: 10.0,
            max_stacks: 50,
        },
        RunUpgradeOptionConfig {
            id: "gun_fire_rate_plus_10_percent".to_string(),
            label: "Gun Fire Rate +10%".to_string(),
            effect: RunUpgradeEffectKind::WeaponFireRatePercent,
            value: 0.10,
            max_stacks: 50,
        },
        RunUpgradeOptionConfig {
            id: "missile_fire_rate_plus_10_percent".to_string(),
            label: "Missile Fire Rate +10%".to_string(),
            effect: RunUpgradeEffectKind::MissileFireRatePercent,
            value: 0.10,
            max_stacks: 50,
        },
    ]
}

#[derive(Debug, Clone, Deserialize)]
pub struct SegmentsFile {
    pub segment_sequence: Vec<SegmentSequenceConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SegmentSequenceConfig {
    pub id: String,
    pub length: f32,
    pub environment: String,
    pub spawn_set: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundsFile {
    pub backgrounds: Vec<BackgroundConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackgroundConfig {
    pub id: String,
    pub placeholder: String,
    pub color: [f32; 3],
    pub parallax: f32,
    #[serde(default)]
    pub splat_asset_id: Option<String>,
    #[serde(default = "default_background_offset_x_m")]
    pub offset_x_m: f32,
    #[serde(default = "default_background_offset_y_m")]
    pub offset_y_m: f32,
    #[serde(default = "default_background_offset_z_m")]
    pub offset_z_m: f32,
    #[serde(default = "default_background_scale_x")]
    pub scale_x: f32,
    #[serde(default = "default_background_scale_y")]
    pub scale_y: f32,
    #[serde(default = "default_background_scale_z")]
    pub scale_z: f32,
    #[serde(default = "default_background_loop_length_m")]
    pub loop_length_m: f32,
}

fn default_background_offset_x_m() -> f32 {
    0.0
}

fn default_background_offset_y_m() -> f32 {
    0.0
}

fn default_background_offset_z_m() -> f32 {
    0.0
}

fn default_background_scale_x() -> f32 {
    1.0
}

fn default_background_scale_y() -> f32 {
    1.0
}

fn default_background_scale_z() -> f32 {
    1.0
}

fn default_background_loop_length_m() -> f32 {
    0.0
}

impl HasId for BackgroundConfig {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnvironmentsFile {
    pub environments: Vec<EnvironmentConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnvironmentConfig {
    pub id: String,
    pub gravity: f32,
    pub drag: f32,
    pub traction: f32,
    pub air_control: f32,
    pub wheel_friction: f32,
    pub projectile_drag: f32,
}

impl HasId for EnvironmentConfig {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnemyTypesFile {
    pub enemy_types: Vec<EnemyTypeConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnemyTypeConfig {
    pub id: String,
    pub behavior: String,
    pub health: f32,
    pub speed: f32,
    pub contact_damage: f32,
    #[serde(default = "default_enemy_kill_score")]
    pub kill_score: u32,
    pub weapon_id: String,
    pub hitbox_radius: f32,
    #[serde(default)]
    pub hover_amplitude: f32,
    #[serde(default)]
    pub hover_frequency: f32,
    #[serde(default)]
    pub charge_speed_multiplier: f32,
}

fn default_enemy_kill_score() -> u32 {
    10
}

impl HasId for EnemyTypeConfig {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpawnersFile {
    pub spawners: Vec<SpawnerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpawnerConfig {
    pub id: String,
    pub mode: String,
    pub spawn_enemy_ids: Vec<String>,
    pub start_distance: f32,
    pub interval_seconds: f32,
    pub max_alive: u32,
}

impl HasId for SpawnerConfig {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeaponsFile {
    pub weapons: Vec<WeaponConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeaponConfig {
    pub id: String,
    pub projectile_type: String,
    pub bullet_speed: f32,
    pub fire_rate: f32,
    pub spread_degrees: f32,
    pub damage: f32,
    #[serde(default = "default_weapon_burst_count")]
    pub burst_count: u32,
    #[serde(default)]
    pub burst_interval_seconds: f32,
    #[serde(default = "default_weapon_muzzle_offset_x")]
    pub muzzle_offset_x: f32,
    #[serde(default)]
    pub muzzle_offset_y: f32,
    #[serde(default)]
    pub projectile_drag: f32,
    #[serde(default = "default_projectile_lifetime_seconds")]
    pub projectile_lifetime_seconds: f32,
    #[serde(default = "default_missile_gravity_scale")]
    pub missile_gravity_scale: f32,
    #[serde(default)]
    pub homing_turn_rate_degrees: f32,
}

fn default_weapon_burst_count() -> u32 {
    1
}

fn default_weapon_muzzle_offset_x() -> f32 {
    1.8
}

fn default_projectile_lifetime_seconds() -> f32 {
    2.8
}

fn default_missile_gravity_scale() -> f32 {
    1.0
}

impl HasId for WeaponConfig {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct VehiclesFile {
    pub vehicles: Vec<VehicleConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VehicleConfig {
    pub id: String,
    pub health: f32,
    pub acceleration: f32,
    pub brake_strength: f32,
    pub air_pitch_torque: f32,
    #[serde(default = "default_air_max_rotation_speed")]
    pub air_max_rotation_speed: f32,
    pub max_forward_speed: f32,
    pub max_reverse_speed: f32,
    pub max_fall_speed: f32,
    pub linear_speed_scale: f32,
    pub ground_coast_damping: f32,
    pub air_base_damping: f32,
    pub air_env_drag_factor: f32,
    pub linear_inertia: f32,
    pub rotational_inertia: f32,
    pub gravity_scale: f32,
    #[serde(default = "default_suspension_rest_length_m")]
    pub suspension_rest_length_m: f32,
    #[serde(default = "default_suspension_stiffness")]
    pub suspension_stiffness: f32,
    #[serde(default = "default_suspension_damping")]
    pub suspension_damping: f32,
    #[serde(default = "default_suspension_max_compression_m")]
    pub suspension_max_compression_m: f32,
    #[serde(default = "default_suspension_max_extension_m")]
    pub suspension_max_extension_m: f32,
    #[serde(default = "default_tire_longitudinal_grip")]
    pub tire_longitudinal_grip: f32,
    #[serde(default = "default_tire_slip_grip_floor")]
    pub tire_slip_grip_floor: f32,
    #[serde(default = "default_front_drive_ratio")]
    pub front_drive_ratio: f32,
    #[serde(default = "default_rear_drive_traction_assist_distance_m")]
    pub rear_drive_traction_assist_distance_m: f32,
    #[serde(default = "default_rear_drive_traction_assist_min_factor")]
    pub rear_drive_traction_assist_min_factor: f32,
    #[serde(default = "default_turret_range_m")]
    pub turret_range_m: f32,
    #[serde(default = "default_turret_cone_degrees")]
    pub turret_cone_degrees: f32,
    #[serde(default = "default_turret_target_priority")]
    pub turret_target_priority: String,
    #[serde(default)]
    pub secondary_weapon_id: Option<String>,
    #[serde(default = "default_missile_fire_interval_seconds")]
    pub missile_fire_interval_seconds: f32,
    pub camera_look_ahead_factor: f32,
    pub camera_look_ahead_min: f32,
    pub camera_look_ahead_max: f32,
    pub default_weapon_id: String,
}

fn default_turret_range_m() -> f32 {
    28.0
}

fn default_air_max_rotation_speed() -> f32 {
    5.5
}

fn default_suspension_rest_length_m() -> f32 {
    0.78
}

fn default_suspension_stiffness() -> f32 {
    38.0
}

fn default_suspension_damping() -> f32 {
    8.0
}

fn default_suspension_max_compression_m() -> f32 {
    0.34
}

fn default_suspension_max_extension_m() -> f32 {
    0.28
}

fn default_tire_longitudinal_grip() -> f32 {
    1.0
}

fn default_tire_slip_grip_floor() -> f32 {
    0.45
}

fn default_front_drive_ratio() -> f32 {
    0.30
}

fn default_rear_drive_traction_assist_distance_m() -> f32 {
    0.20
}

fn default_rear_drive_traction_assist_min_factor() -> f32 {
    0.55
}

fn default_turret_cone_degrees() -> f32 {
    60.0
}

fn default_turret_target_priority() -> String {
    "nearest".to_string()
}

fn default_missile_fire_interval_seconds() -> f32 {
    2.0
}

impl HasId for VehicleConfig {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpgradesFile {
    pub upgrades: Vec<UpgradeConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpgradeConfig {
    pub id: String,
    pub target: String,
    pub add: f32,
    pub max_stacks: u32,
    pub rarity: String,
    pub label: String,
}

impl HasId for UpgradeConfig {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommentatorFile {
    pub commentary: CommentaryConfig,
    pub thresholds: CommentaryThresholds,
    #[serde(default = "default_commentator_profiles")]
    pub commentators: Vec<CommentatorProfile>,
    pub fallback: FallbackLines,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommentaryConfig {
    pub min_seconds_between_lines: f32,
    #[serde(default = "default_max_events_per_batch")]
    pub max_events_per_batch: usize,
    #[serde(default = "default_commentary_api_max_retries")]
    pub api_max_retries: u32,
    #[serde(default = "default_commentary_api_retry_backoff_seconds")]
    pub api_retry_backoff_seconds: f32,
    #[serde(default = "default_commentary_api_stale_request_timeout_seconds")]
    pub api_stale_request_timeout_seconds: f32,
    #[serde(default = "default_commentary_narration_volume")]
    pub narration_volume: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommentatorProfile {
    pub id: String,
    #[serde(default = "default_commentator_name")]
    pub name: String,
    #[serde(default = "default_commentator_character_id")]
    pub character_id: String,
    #[serde(default = "default_commentator_style_instruction")]
    pub style_instruction: String,
    #[serde(default = "default_commentator_style_tone")]
    pub style_tone: String,
    #[serde(default = "default_commentator_style_length")]
    pub style_length: String,
    #[serde(default = "default_commentator_emotions")]
    pub emotions: Vec<String>,
    #[serde(default = "default_commentator_profanity_filter")]
    pub profanity_filter: bool,
    #[serde(default = "default_commentator_subtitle_color")]
    pub subtitle_color: [f32; 3],
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommentaryThresholds {
    pub airtime_big_jump: f32,
    #[serde(default)]
    pub airtime_huge_jump: f32,
    pub wheelie_long: f32,
    pub flip_count: u32,
    pub speed_tier_1: f32,
    #[serde(default)]
    pub speed_tier_2: f32,
    #[serde(default)]
    pub near_death_health_fraction: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FallbackLines {
    pub lines: Vec<String>,
}

fn default_max_events_per_batch() -> usize {
    4
}

fn default_commentary_api_max_retries() -> u32 {
    1
}

fn default_commentary_api_retry_backoff_seconds() -> f32 {
    0.75
}

fn default_commentary_api_stale_request_timeout_seconds() -> f32 {
    18.0
}

fn default_commentary_narration_volume() -> f32 {
    1.0
}

fn default_commentator_style_instruction() -> String {
    "Return exactly one short colorful commentary line with playful banter grounded in the game events.".to_string()
}

fn default_commentator_style_tone() -> String {
    "neutral".to_string()
}

fn default_commentator_name() -> String {
    "Commentator".to_string()
}

fn default_commentator_character_id() -> String {
    String::new()
}

fn default_commentator_style_length() -> String {
    "short".to_string()
}

fn default_commentator_emotions() -> Vec<String> {
    vec!["Neutral".to_string()]
}

fn default_commentator_profanity_filter() -> bool {
    true
}

fn default_commentator_subtitle_color() -> [f32; 3] {
    [0.9, 0.9, 0.9]
}

fn default_commentator_profiles() -> Vec<CommentatorProfile> {
    vec![
        CommentatorProfile {
            id: "commentator_a".to_string(),
            name: "George".to_string(),
            character_id: "cmlarc6fv0003l404isl4cdxl".to_string(),
            style_instruction:
                "Return exactly one short colorful commentary line with playful banter grounded in the game events."
                    .to_string(),
            style_tone: "analytical".to_string(),
            style_length: "short".to_string(),
            emotions: vec![
                "Neutral".to_string(),
                "Concerned".to_string(),
                "Pleased".to_string(),
                "Confident".to_string(),
            ],
            profanity_filter: true,
            subtitle_color: [0.55, 0.85, 1.00],
        },
        CommentatorProfile {
            id: "commentator_b".to_string(),
            name: "jerry".to_string(),
            character_id: "cmlcaw5810001i804mblfloyr".to_string(),
            style_instruction:
                "Return exactly one short colorful commentary line with playful banter grounded in the game events."
                    .to_string(),
            style_tone: "hyped".to_string(),
            style_length: "short".to_string(),
            emotions: vec![
                "Happy".to_string(),
                "Amazed".to_string(),
                "Curious".to_string(),
                "Impressed".to_string(),
                "Confident".to_string(),
            ],
            profanity_filter: true,
            subtitle_color: [1.00, 0.78, 0.40],
        },
    ]
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AssetsFile {
    #[serde(default)]
    pub sprites: Vec<SpriteAssetConfig>,
    #[serde(default)]
    pub models: Vec<ModelAssetConfig>,
    #[serde(default)]
    pub splats: Vec<SplatAssetConfig>,
    #[serde(default)]
    pub audio: Vec<AudioAssetConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpriteAssetConfig {
    pub id: String,
    pub path: String,
}

impl HasId for SpriteAssetConfig {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelAssetConfig {
    pub id: String,
    pub scene_path: String,
    pub root_node: String,
    pub wheel_nodes: Vec<String>,
    pub turret_node: Option<String>,
}

impl HasId for ModelAssetConfig {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SplatAssetConfig {
    pub id: String,
    pub path: String,
}

impl HasId for SplatAssetConfig {
    fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AudioAssetConfig {
    pub id: String,
    pub path: String,
}

impl HasId for AudioAssetConfig {
    fn id(&self) -> &str {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_fails_for_missing_environment_reference() {
        let config = GameConfig {
            game: GameFile {
                app: AppConfig {
                    fixed_timestep_hz: 60.0,
                    starting_environment: "missing_env".to_string(),
                    default_vehicle: "starter_car".to_string(),
                    debug_overlay: true,
                },
                terrain: TerrainConfig {
                    base_height: -170.0,
                    ramp_slope: 0.0,
                    wave_a_amplitude: 40.0,
                    wave_a_frequency: 0.015,
                    wave_b_amplitude: 20.0,
                    wave_b_frequency: 0.041,
                    wave_c_amplitude: 0.0,
                    wave_c_frequency: 0.0,
                },
                scoring: ScoringConfig::default(),
                pickups: PickupConfig::default(),
                run_upgrades: RunUpgradeConfig::default(),
            },
            assets: AssetsFile::default(),
            segments: SegmentsFile {
                segment_sequence: vec![SegmentSequenceConfig {
                    id: "segment_a".to_string(),
                    length: 100.0,
                    environment: "normal".to_string(),
                    spawn_set: Some("starter_wave".to_string()),
                }],
            },
            backgrounds: BackgroundsFile {
                backgrounds: vec![BackgroundConfig {
                    id: "segment_a".to_string(),
                    placeholder: "box".to_string(),
                    color: [0.1, 0.1, 0.1],
                    parallax: 0.4,
                    splat_asset_id: None,
                    offset_x_m: 0.0,
                    offset_y_m: 0.0,
                    offset_z_m: 0.0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                    scale_z: 1.0,
                    loop_length_m: 0.0,
                }],
            },
            environments: EnvironmentsFile {
                environments: vec![EnvironmentConfig {
                    id: "normal".to_string(),
                    gravity: 9.81,
                    drag: 0.1,
                    traction: 1.0,
                    air_control: 1.0,
                    wheel_friction: 1.0,
                    projectile_drag: 0.0,
                }],
            },
            enemy_types: EnemyTypesFile {
                enemy_types: vec![EnemyTypeConfig {
                    id: "grunt".to_string(),
                    behavior: "walker".to_string(),
                    health: 10.0,
                    speed: 1.0,
                    contact_damage: 2.0,
                    kill_score: 12,
                    weapon_id: "enemy_weapon".to_string(),
                    hitbox_radius: 0.5,
                    hover_amplitude: 0.0,
                    hover_frequency: 0.0,
                    charge_speed_multiplier: 0.0,
                }],
            },
            spawners: SpawnersFile {
                spawners: vec![SpawnerConfig {
                    id: "starter_wave".to_string(),
                    mode: "distance".to_string(),
                    spawn_enemy_ids: vec!["grunt".to_string()],
                    start_distance: 5.0,
                    interval_seconds: 2.0,
                    max_alive: 4,
                }],
            },
            weapons: WeaponsFile {
                weapons: vec![
                    WeaponConfig {
                        id: "enemy_weapon".to_string(),
                        projectile_type: "bullet".to_string(),
                        bullet_speed: 10.0,
                        fire_rate: 1.0,
                        spread_degrees: 0.0,
                        damage: 2.0,
                        burst_count: 1,
                        burst_interval_seconds: 0.0,
                        muzzle_offset_x: 1.2,
                        muzzle_offset_y: 0.0,
                        projectile_drag: 0.0,
                        projectile_lifetime_seconds: 2.8,
                        missile_gravity_scale: 1.0,
                        homing_turn_rate_degrees: 0.0,
                    },
                    WeaponConfig {
                        id: "player_weapon".to_string(),
                        projectile_type: "bullet".to_string(),
                        bullet_speed: 12.0,
                        fire_rate: 2.0,
                        spread_degrees: 0.0,
                        damage: 3.0,
                        burst_count: 1,
                        burst_interval_seconds: 0.0,
                        muzzle_offset_x: 1.8,
                        muzzle_offset_y: 0.1,
                        projectile_drag: 0.0,
                        projectile_lifetime_seconds: 2.8,
                        missile_gravity_scale: 1.0,
                        homing_turn_rate_degrees: 0.0,
                    },
                ],
            },
            vehicles: VehiclesFile {
                vehicles: vec![VehicleConfig {
                    id: "starter_car".to_string(),
                    health: 100.0,
                    acceleration: 10.0,
                    brake_strength: 5.0,
                    air_pitch_torque: 2.0,
                    air_max_rotation_speed: 5.5,
                    max_forward_speed: 300.0,
                    max_reverse_speed: 160.0,
                    max_fall_speed: 240.0,
                    linear_speed_scale: 7.0,
                    ground_coast_damping: 0.22,
                    air_base_damping: 0.10,
                    air_env_drag_factor: 0.45,
                    linear_inertia: 1.0,
                    rotational_inertia: 1.0,
                    gravity_scale: 1.0,
                    suspension_rest_length_m: 0.78,
                    suspension_stiffness: 38.0,
                    suspension_damping: 8.0,
                    suspension_max_compression_m: 0.34,
                    suspension_max_extension_m: 0.28,
                    tire_longitudinal_grip: 1.0,
                    tire_slip_grip_floor: 0.45,
                    front_drive_ratio: 0.30,
                    rear_drive_traction_assist_distance_m: 0.20,
                    rear_drive_traction_assist_min_factor: 0.55,
                    turret_range_m: 28.0,
                    turret_cone_degrees: 60.0,
                    turret_target_priority: "nearest".to_string(),
                    secondary_weapon_id: None,
                    missile_fire_interval_seconds: 2.0,
                    camera_look_ahead_factor: 1.1,
                    camera_look_ahead_min: -220.0,
                    camera_look_ahead_max: 420.0,
                    default_weapon_id: "player_weapon".to_string(),
                }],
            },
            upgrades: UpgradesFile {
                upgrades: vec![UpgradeConfig {
                    id: "u1".to_string(),
                    target: "weapon.player_weapon.damage".to_string(),
                    add: 1.0,
                    max_stacks: 2,
                    rarity: "common".to_string(),
                    label: "Damage+".to_string(),
                }],
            },
            commentator: CommentatorFile {
                commentary: CommentaryConfig {
                    min_seconds_between_lines: 4.0,
                    max_events_per_batch: 4,
                    api_max_retries: 1,
                    api_retry_backoff_seconds: 0.75,
                    api_stale_request_timeout_seconds: 18.0,
                    narration_volume: 1.0,
                },
                thresholds: CommentaryThresholds {
                    airtime_big_jump: 1.0,
                    airtime_huge_jump: 2.0,
                    wheelie_long: 0.8,
                    flip_count: 1,
                    speed_tier_1: 12.0,
                    speed_tier_2: 18.0,
                    near_death_health_fraction: 0.15,
                },
                commentators: vec![
                    CommentatorProfile {
                        id: "commentator_a".to_string(),
                        name: "George".to_string(),
                        character_id: "cmlarc6fv0003l404isl4cdxl".to_string(),
                        style_instruction:
                            "Return exactly one short colorful commentary line with playful banter grounded in the game events."
                                .to_string(),
                        style_tone: "analytical".to_string(),
                        style_length: "short".to_string(),
                        emotions: vec![
                            "Neutral".to_string(),
                            "Concerned".to_string(),
                            "Pleased".to_string(),
                            "Confident".to_string(),
                        ],
                        profanity_filter: true,
                        subtitle_color: [0.55, 0.85, 1.00],
                    },
                    CommentatorProfile {
                        id: "commentator_b".to_string(),
                        name: "jerry".to_string(),
                        character_id: "cmlcaw5810001i804mblfloyr".to_string(),
                        style_instruction:
                            "Return exactly one short colorful commentary line with playful banter grounded in the game events."
                                .to_string(),
                        style_tone: "hyped".to_string(),
                        style_length: "short".to_string(),
                        emotions: vec![
                            "Happy".to_string(),
                            "Amazed".to_string(),
                            "Curious".to_string(),
                            "Impressed".to_string(),
                            "Confident".to_string(),
                        ],
                        profanity_filter: true,
                        subtitle_color: [1.00, 0.78, 0.40],
                    },
                ],
                fallback: FallbackLines {
                    lines: vec!["Nice!".to_string()],
                },
            },
            backgrounds_by_id: HashMap::from([(
                "segment_a".to_string(),
                BackgroundConfig {
                    id: "segment_a".to_string(),
                    placeholder: "box".to_string(),
                    color: [0.1, 0.1, 0.1],
                    parallax: 0.4,
                    splat_asset_id: None,
                    offset_x_m: 0.0,
                    offset_y_m: 0.0,
                    offset_z_m: 0.0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                    scale_z: 1.0,
                    loop_length_m: 0.0,
                },
            )]),
            environments_by_id: HashMap::from([(
                "normal".to_string(),
                EnvironmentConfig {
                    id: "normal".to_string(),
                    gravity: 9.81,
                    drag: 0.1,
                    traction: 1.0,
                    air_control: 1.0,
                    wheel_friction: 1.0,
                    projectile_drag: 0.0,
                },
            )]),
            enemy_types_by_id: HashMap::from([(
                "grunt".to_string(),
                EnemyTypeConfig {
                    id: "grunt".to_string(),
                    behavior: "walker".to_string(),
                    health: 10.0,
                    speed: 1.0,
                    contact_damage: 2.0,
                    kill_score: 12,
                    weapon_id: "enemy_weapon".to_string(),
                    hitbox_radius: 0.5,
                    hover_amplitude: 0.0,
                    hover_frequency: 0.0,
                    charge_speed_multiplier: 0.0,
                },
            )]),
            spawners_by_id: HashMap::from([(
                "starter_wave".to_string(),
                SpawnerConfig {
                    id: "starter_wave".to_string(),
                    mode: "distance".to_string(),
                    spawn_enemy_ids: vec!["grunt".to_string()],
                    start_distance: 5.0,
                    interval_seconds: 2.0,
                    max_alive: 4,
                },
            )]),
            weapons_by_id: HashMap::from([
                (
                    "enemy_weapon".to_string(),
                    WeaponConfig {
                        id: "enemy_weapon".to_string(),
                        projectile_type: "bullet".to_string(),
                        bullet_speed: 10.0,
                        fire_rate: 1.0,
                        spread_degrees: 0.0,
                        damage: 2.0,
                        burst_count: 1,
                        burst_interval_seconds: 0.0,
                        muzzle_offset_x: 1.2,
                        muzzle_offset_y: 0.0,
                        projectile_drag: 0.0,
                        projectile_lifetime_seconds: 2.8,
                        missile_gravity_scale: 1.0,
                        homing_turn_rate_degrees: 0.0,
                    },
                ),
                (
                    "player_weapon".to_string(),
                    WeaponConfig {
                        id: "player_weapon".to_string(),
                        projectile_type: "bullet".to_string(),
                        bullet_speed: 12.0,
                        fire_rate: 2.0,
                        spread_degrees: 0.0,
                        damage: 3.0,
                        burst_count: 1,
                        burst_interval_seconds: 0.0,
                        muzzle_offset_x: 1.8,
                        muzzle_offset_y: 0.1,
                        projectile_drag: 0.0,
                        projectile_lifetime_seconds: 2.8,
                        missile_gravity_scale: 1.0,
                        homing_turn_rate_degrees: 0.0,
                    },
                ),
            ]),
            vehicles_by_id: HashMap::from([(
                "starter_car".to_string(),
                VehicleConfig {
                    id: "starter_car".to_string(),
                    health: 100.0,
                    acceleration: 10.0,
                    brake_strength: 5.0,
                    air_pitch_torque: 2.0,
                    air_max_rotation_speed: 5.5,
                    max_forward_speed: 300.0,
                    max_reverse_speed: 160.0,
                    max_fall_speed: 240.0,
                    linear_speed_scale: 7.0,
                    ground_coast_damping: 0.22,
                    air_base_damping: 0.10,
                    air_env_drag_factor: 0.45,
                    linear_inertia: 1.0,
                    rotational_inertia: 1.0,
                    gravity_scale: 1.0,
                    suspension_rest_length_m: 0.78,
                    suspension_stiffness: 38.0,
                    suspension_damping: 8.0,
                    suspension_max_compression_m: 0.34,
                    suspension_max_extension_m: 0.28,
                    tire_longitudinal_grip: 1.0,
                    tire_slip_grip_floor: 0.45,
                    front_drive_ratio: 0.30,
                    rear_drive_traction_assist_distance_m: 0.20,
                    rear_drive_traction_assist_min_factor: 0.55,
                    turret_range_m: 28.0,
                    turret_cone_degrees: 60.0,
                    turret_target_priority: "nearest".to_string(),
                    secondary_weapon_id: None,
                    missile_fire_interval_seconds: 2.0,
                    camera_look_ahead_factor: 1.1,
                    camera_look_ahead_min: -220.0,
                    camera_look_ahead_max: 420.0,
                    default_weapon_id: "player_weapon".to_string(),
                },
            )]),
            upgrades_by_id: HashMap::from([(
                "u1".to_string(),
                UpgradeConfig {
                    id: "u1".to_string(),
                    target: "weapon.player_weapon.damage".to_string(),
                    add: 1.0,
                    max_stacks: 2,
                    rarity: "common".to_string(),
                    label: "Damage+".to_string(),
                },
            )]),
            sprite_assets_by_id: HashMap::new(),
            model_assets_by_id: HashMap::new(),
            splat_assets_by_id: HashMap::new(),
            audio_assets_by_id: HashMap::new(),
        };

        let error = config
            .validate_references()
            .expect_err("validation should fail");
        let message = error.to_string();

        assert!(message.contains("starting_environment"));
        assert!(message.contains("missing_env"));
    }
}
