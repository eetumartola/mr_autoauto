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

        for (index, enemy) in self.enemy_types.enemy_types.iter().enumerate() {
            if !self.weapons_by_id.contains_key(&enemy.weapon_id) {
                return Err(ConfigError::Validation(format!(
                    "enemy_types.toml::enemy_types[{index}].weapon_id references unknown weapon id `{}`",
                    enemy.weapon_id
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

        for (index, vehicle) in self.vehicles.vehicles.iter().enumerate() {
            if !self.weapons_by_id.contains_key(&vehicle.default_weapon_id) {
                return Err(ConfigError::Validation(format!(
                    "vehicles.toml::vehicles[{index}].default_weapon_id references unknown weapon id `{}`",
                    vehicle.default_weapon_id
                )));
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub fixed_timestep_hz: f32,
    pub starting_environment: String,
    pub default_vehicle: String,
    pub debug_overlay: bool,
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
    pub health: f32,
    pub speed: f32,
    pub contact_damage: f32,
    pub weapon_id: String,
    pub hitbox_radius: f32,
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
    pub default_weapon_id: String,
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
    pub fallback: FallbackLines,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommentaryConfig {
    pub min_seconds_between_lines: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommentaryThresholds {
    pub airtime_big_jump: f32,
    pub wheelie_long: f32,
    pub flip_count: u32,
    pub speed_tier_1: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FallbackLines {
    pub lines: Vec<String>,
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
                    health: 10.0,
                    speed: 1.0,
                    contact_damage: 2.0,
                    weapon_id: "enemy_weapon".to_string(),
                    hitbox_radius: 0.5,
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
                    },
                    WeaponConfig {
                        id: "player_weapon".to_string(),
                        projectile_type: "bullet".to_string(),
                        bullet_speed: 12.0,
                        fire_rate: 2.0,
                        spread_degrees: 0.0,
                        damage: 3.0,
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
                },
                thresholds: CommentaryThresholds {
                    airtime_big_jump: 1.0,
                    wheelie_long: 0.8,
                    flip_count: 1,
                    speed_tier_1: 12.0,
                },
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
                    health: 10.0,
                    speed: 1.0,
                    contact_damage: 2.0,
                    weapon_id: "enemy_weapon".to_string(),
                    hitbox_radius: 0.5,
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
