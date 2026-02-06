#![allow(dead_code)]

use crate::config::{
    AudioAssetConfig, GameConfig, ModelAssetConfig, SplatAssetConfig, SpriteAssetConfig,
};
use bevy::prelude::*;
use std::collections::HashMap;
use std::path::Path;

#[cfg(feature = "gaussian_splats")]
use bevy_gaussian_splatting::GaussianScene;

const ASSET_ROOT_DIR: &str = "assets";

pub struct AssetRegistryPlugin;

impl Plugin for AssetRegistryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            sync_asset_registry.run_if(resource_exists::<GameConfig>),
        );
    }
}

fn sync_asset_registry(
    mut commands: Commands,
    config: Res<GameConfig>,
    asset_server: Res<AssetServer>,
    registry: Option<ResMut<AssetRegistry>>,
) {
    if registry.is_some() && !config.is_changed() {
        return;
    }

    let new_registry =
        AssetRegistry::from_config(&config, &asset_server, Path::new(ASSET_ROOT_DIR));

    match registry {
        Some(mut existing_registry) => {
            *existing_registry = new_registry;
            log_asset_registry_summary("Updated", &existing_registry);
        }
        None => {
            log_asset_registry_summary("Initialized", &new_registry);
            commands.insert_resource(new_registry);
        }
    }
}

fn log_asset_registry_summary(prefix: &str, registry: &AssetRegistry) {
    info!(
        "{prefix} asset registry: sprites {}/{}, models {}/{}, splats {}/{}, audio {}/{}.",
        registry.available_sprite_count(),
        registry.sprites.len(),
        registry.available_model_count(),
        registry.models.len(),
        registry.available_splat_count(),
        registry.splats.len(),
        registry.available_audio_count(),
        registry.audio.len(),
    );
}

#[derive(Resource, Debug, Clone, Default)]
pub struct AssetRegistry {
    pub sprites: HashMap<String, SpriteAssetEntry>,
    pub models: HashMap<String, ModelAssetEntry>,
    pub splats: HashMap<String, SplatAssetEntry>,
    pub audio: HashMap<String, AudioAssetEntry>,
}

impl AssetRegistry {
    pub fn from_config(config: &GameConfig, asset_server: &AssetServer, asset_root: &Path) -> Self {
        let sprites = config
            .assets
            .sprites
            .iter()
            .map(|entry| {
                let sprite = SpriteAssetEntry::from_config(entry, asset_server, asset_root);
                (entry.id.clone(), sprite)
            })
            .collect();

        let models = config
            .assets
            .models
            .iter()
            .map(|entry| {
                let model = ModelAssetEntry::from_config(entry, asset_server, asset_root);
                (entry.id.clone(), model)
            })
            .collect();

        let splats = config
            .assets
            .splats
            .iter()
            .map(|entry| {
                let splat = SplatAssetEntry::from_config(entry, asset_server, asset_root);
                (entry.id.clone(), splat)
            })
            .collect();

        let audio = config
            .assets
            .audio
            .iter()
            .map(|entry| {
                let sound = AudioAssetEntry::from_config(entry, asset_server, asset_root);
                (entry.id.clone(), sound)
            })
            .collect();

        Self {
            sprites,
            models,
            splats,
            audio,
        }
    }

    fn available_sprite_count(&self) -> usize {
        self.sprites
            .values()
            .filter(|entry| entry.exists_on_disk)
            .count()
    }

    fn available_model_count(&self) -> usize {
        self.models
            .values()
            .filter(|entry| entry.exists_on_disk)
            .count()
    }

    fn available_splat_count(&self) -> usize {
        self.splats
            .values()
            .filter(|entry| entry.exists_on_disk)
            .count()
    }

    fn available_audio_count(&self) -> usize {
        self.audio
            .values()
            .filter(|entry| entry.exists_on_disk)
            .count()
    }
}

#[derive(Debug, Clone)]
pub struct SpriteAssetEntry {
    pub path: String,
    pub exists_on_disk: bool,
    pub handle: Option<Handle<Image>>,
}

impl SpriteAssetEntry {
    fn from_config(
        config: &SpriteAssetConfig,
        asset_server: &AssetServer,
        asset_root: &Path,
    ) -> Self {
        let exists_on_disk = asset_exists(asset_root, &config.path);
        let handle = exists_on_disk.then(|| asset_server.load(config.path.clone()));

        Self {
            path: config.path.clone(),
            exists_on_disk,
            handle,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelAssetEntry {
    pub scene_path: String,
    pub exists_on_disk: bool,
    pub handle: Option<Handle<Scene>>,
    pub hierarchy: ModelHierarchy,
}

impl ModelAssetEntry {
    fn from_config(
        config: &ModelAssetConfig,
        asset_server: &AssetServer,
        asset_root: &Path,
    ) -> Self {
        let exists_on_disk = asset_exists(asset_root, &config.scene_path);
        let handle = exists_on_disk.then(|| asset_server.load(config.scene_path.clone()));

        Self {
            scene_path: config.scene_path.clone(),
            exists_on_disk,
            handle,
            hierarchy: ModelHierarchy {
                root_node: config.root_node.clone(),
                wheel_nodes: config.wheel_nodes.clone(),
                turret_node: config.turret_node.clone(),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelHierarchy {
    pub root_node: String,
    pub wheel_nodes: Vec<String>,
    pub turret_node: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SplatAssetEntry {
    pub path: String,
    pub exists_on_disk: bool,
    #[cfg(feature = "gaussian_splats")]
    pub handle: Option<Handle<GaussianScene>>,
}

impl SplatAssetEntry {
    fn from_config(
        config: &SplatAssetConfig,
        _asset_server: &AssetServer,
        asset_root: &Path,
    ) -> Self {
        let exists_on_disk = asset_exists(asset_root, &config.path);
        #[cfg(feature = "gaussian_splats")]
        let handle = exists_on_disk.then(|| _asset_server.load(config.path.clone()));

        Self {
            path: config.path.clone(),
            exists_on_disk,
            #[cfg(feature = "gaussian_splats")]
            handle,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AudioAssetEntry {
    pub path: String,
    pub exists_on_disk: bool,
    pub handle: Option<Handle<AudioSource>>,
}

impl AudioAssetEntry {
    fn from_config(
        config: &AudioAssetConfig,
        asset_server: &AssetServer,
        asset_root: &Path,
    ) -> Self {
        let exists_on_disk = asset_exists(asset_root, &config.path);
        let handle = exists_on_disk.then(|| asset_server.load(config.path.clone()));

        Self {
            path: config.path.clone(),
            exists_on_disk,
            handle,
        }
    }
}

fn asset_exists(asset_root: &Path, path: &str) -> bool {
    let file_path = path.split('#').next().unwrap_or(path);
    asset_root.join(file_path).exists()
}
