use crate::assets::AssetRegistry;
use crate::config::{GameConfig, SfxConfig};
use crate::gameplay::combat::{
    EnemyKilledEvent, PlayerProjectileAudioKind, PlayerProjectileImpactEvent,
    PlayerProjectileImpactTarget, PlayerWeaponFiredEvent,
};
use crate::gameplay::vehicle::{VehicleInputState, VehicleTelemetry};
use crate::states::GameState;
use bevy::audio::{
    AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume,
};
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const AUDIO_ID_ENGINE_LOOP: &str = "sfx_engine_loop";
const AUDIO_ID_EXPLODE: &str = "sfx_explode";
const AUDIO_ID_GUN_HIT: &str = "sfx_gun_hit";
const AUDIO_ID_GUN_MISS: &str = "sfx_gun_miss";
const AUDIO_ID_GUN_SHOT: &str = "sfx_gun_shot";
const AUDIO_ID_MUSIC_LOOP: &str = "music_background_loop";
const AUDIO_ID_MISSILE_HIT: &str = "sfx_missile_hit";
const AUDIO_ID_MISSILE_LAUNCH: &str = "sfx_missile_launch";

const ENGINE_JITTER_REFRESH_MIN_S: f32 = 0.14;
const ENGINE_JITTER_REFRESH_MAX_S: f32 = 0.36;
const MUSIC_FADE_IN_SECONDS: f32 = 2.4;

pub struct GameplaySfxPlugin;

impl Plugin for GameplaySfxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SfxRngState>()
            .init_resource::<SfxMissingAssetWarnings>()
            .init_resource::<SfxRuntimeAudioCache>()
            .add_systems(
                OnEnter(GameState::InRun),
                (
                    reset_sfx_rng_state,
                    clear_sfx_warnings,
                    clear_sfx_audio_cache,
                ),
            )
            .add_systems(
                Update,
                (ensure_background_music_audio, update_background_music_audio)
                    .chain()
                    .run_if(resource_exists::<GameConfig>),
            )
            .add_systems(OnExit(GameState::InRun), cleanup_sfx_entities)
            .add_systems(
                Update,
                (
                    ensure_engine_loop_audio,
                    update_engine_loop_audio,
                    play_gameplay_sfx,
                )
                    .chain()
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            );
    }
}

#[derive(Component)]
struct EngineLoopAudio;

#[derive(Component)]
struct BackgroundMusicAudio;

#[derive(Component, Debug, Clone, Copy)]
struct BackgroundMusicRuntime {
    fade_elapsed_s: f32,
}

impl Default for BackgroundMusicRuntime {
    fn default() -> Self {
        Self {
            fade_elapsed_s: 0.0,
        }
    }
}

#[derive(Component, Debug, Clone, Copy)]
struct EngineLoopRuntime {
    smoothed_load: f32,
    pitch_jitter_current: f32,
    pitch_jitter_target: f32,
    pitch_jitter_refresh_s: f32,
}

impl Default for EngineLoopRuntime {
    fn default() -> Self {
        Self {
            smoothed_load: 0.0,
            pitch_jitter_current: 0.0,
            pitch_jitter_target: 0.0,
            pitch_jitter_refresh_s: 0.0,
        }
    }
}

#[derive(Component)]
struct GameplaySfxTransient;

#[derive(Resource, Debug, Clone, Copy)]
struct SfxRngState {
    seed: u64,
}

impl Default for SfxRngState {
    fn default() -> Self {
        Self {
            seed: 0x7C5E_A48B_D113_90F2,
        }
    }
}

#[derive(Resource, Debug, Default)]
struct SfxMissingAssetWarnings {
    missing_ids: HashSet<String>,
}

#[derive(Resource, Debug, Default)]
struct SfxRuntimeAudioCache {
    handles_by_id: HashMap<String, Handle<AudioSource>>,
}

fn reset_sfx_rng_state(mut rng: ResMut<SfxRngState>) {
    rng.seed ^= unix_timestamp_seconds();
}

fn clear_sfx_warnings(mut warnings: ResMut<SfxMissingAssetWarnings>) {
    warnings.missing_ids.clear();
}

fn clear_sfx_audio_cache(mut cache: ResMut<SfxRuntimeAudioCache>) {
    cache.handles_by_id.clear();
}

#[allow(clippy::too_many_arguments)]
fn ensure_background_music_audio(
    mut commands: Commands,
    config: Res<GameConfig>,
    registry: Option<Res<AssetRegistry>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut runtime_audio_cache: ResMut<SfxRuntimeAudioCache>,
    state: Option<Res<State<GameState>>>,
    existing_query: Query<Entity, With<BackgroundMusicAudio>>,
    mut warnings: ResMut<SfxMissingAssetWarnings>,
) {
    let Some(state) = state else {
        return;
    };

    if matches!(state.get(), &GameState::Boot) {
        return;
    }

    if !config.game.sfx.enabled {
        for entity in &existing_query {
            commands.entity(entity).try_despawn();
        }
        return;
    }

    if !existing_query.is_empty() {
        return;
    }

    let Some(registry) = registry else {
        return;
    };
    let Some(handle) = resolve_runtime_audio_handle(
        AUDIO_ID_MUSIC_LOOP,
        registry.as_ref(),
        &mut audio_sources,
        &mut runtime_audio_cache,
        &mut warnings,
    ) else {
        return;
    };

    commands.spawn((
        Name::new("SfxBackgroundMusicLoop"),
        BackgroundMusicAudio,
        BackgroundMusicRuntime::default(),
        AudioPlayer::new(handle),
        PlaybackSettings::LOOP
            .with_volume(Volume::Linear(0.0))
            .with_speed(1.0),
    ));
}

fn update_background_music_audio(
    time: Res<Time>,
    config: Res<GameConfig>,
    mut music_query: Query<
        (&mut AudioSink, &mut BackgroundMusicRuntime),
        With<BackgroundMusicAudio>,
    >,
) {
    if music_query.is_empty() {
        return;
    }

    let target_volume = (config.game.sfx.master_volume * config.game.sfx.music_volume).max(0.0);
    let dt = time.delta_secs().max(0.0);
    for (mut sink, mut runtime) in &mut music_query {
        runtime.fade_elapsed_s = (runtime.fade_elapsed_s + dt).min(MUSIC_FADE_IN_SECONDS);
        let fade_alpha = if MUSIC_FADE_IN_SECONDS <= f32::EPSILON {
            1.0
        } else {
            (runtime.fade_elapsed_s / MUSIC_FADE_IN_SECONDS).clamp(0.0, 1.0)
        };
        sink.set_volume(Volume::Linear((target_volume * fade_alpha).max(0.0)));
    }
}

#[allow(clippy::type_complexity)]
fn cleanup_sfx_entities(
    mut commands: Commands,
    sfx_query: Query<Entity, Or<(With<EngineLoopAudio>, With<GameplaySfxTransient>)>>,
) {
    for entity in &sfx_query {
        commands.entity(entity).try_despawn();
    }
}

fn ensure_engine_loop_audio(
    mut commands: Commands,
    config: Res<GameConfig>,
    registry: Option<Res<AssetRegistry>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut runtime_audio_cache: ResMut<SfxRuntimeAudioCache>,
    mut warnings: ResMut<SfxMissingAssetWarnings>,
    existing_query: Query<Entity, With<EngineLoopAudio>>,
) {
    if !config.game.sfx.enabled {
        for entity in &existing_query {
            commands.entity(entity).try_despawn();
        }
        return;
    }

    if !existing_query.is_empty() {
        return;
    }

    let Some(registry) = registry else {
        return;
    };
    let Some(handle) = resolve_runtime_audio_handle(
        AUDIO_ID_ENGINE_LOOP,
        registry.as_ref(),
        &mut audio_sources,
        &mut runtime_audio_cache,
        &mut warnings,
    ) else {
        return;
    };

    let initial_speed = config.game.sfx.engine_base_speed.max(0.05);
    let initial_volume = (config.game.sfx.master_volume
        * config.game.sfx.engine_volume
        * config.game.sfx.engine_idle_gain)
        .max(0.0);

    commands.spawn((
        Name::new("SfxEngineLoop"),
        EngineLoopAudio,
        EngineLoopRuntime::default(),
        AudioPlayer::new(handle),
        PlaybackSettings::LOOP
            .with_volume(Volume::Linear(initial_volume))
            .with_speed(initial_speed),
    ));
}

fn update_engine_loop_audio(
    time: Res<Time>,
    config: Res<GameConfig>,
    input: Res<VehicleInputState>,
    telemetry: Res<VehicleTelemetry>,
    mut rng: ResMut<SfxRngState>,
    mut engine_query: Query<(&mut AudioSink, &mut EngineLoopRuntime), With<EngineLoopAudio>>,
) {
    if !config.game.sfx.enabled {
        return;
    }

    let dt = time.delta_secs().max(0.000_1);
    let vehicle_max_speed = config
        .vehicles_by_id
        .get(&config.game.app.default_vehicle)
        .map(|vehicle| vehicle.max_forward_speed.max(1.0))
        .unwrap_or(120.0);
    let speed_norm = (telemetry.speed_mps.abs() / vehicle_max_speed).clamp(0.0, 1.0);
    let throttle = if input.accelerate { 1.0 } else { 0.0 };

    for (mut sink, mut runtime) in &mut engine_query {
        let target_load = (throttle * 0.75 + speed_norm * 0.55).clamp(0.0, 1.0);
        runtime.smoothed_load = runtime
            .smoothed_load
            .lerp(target_load, (dt * 6.0).clamp(0.0, 1.0));

        runtime.pitch_jitter_refresh_s -= dt;
        if runtime.pitch_jitter_refresh_s <= 0.0 {
            runtime.pitch_jitter_refresh_s = lerp(
                ENGINE_JITTER_REFRESH_MIN_S,
                ENGINE_JITTER_REFRESH_MAX_S,
                next_unit_random(&mut rng.seed),
            );
            runtime.pitch_jitter_target =
                next_signed_unit_random(&mut rng.seed) * config.game.sfx.engine_pitch_jitter;
        }
        runtime.pitch_jitter_current = runtime
            .pitch_jitter_current
            .lerp(runtime.pitch_jitter_target, (dt * 5.0).clamp(0.0, 1.0));

        let playback_speed = (config.game.sfx.engine_base_speed
            + (runtime.smoothed_load * config.game.sfx.engine_accel_speed_boost)
            + (speed_norm * config.game.sfx.engine_velocity_speed_boost))
            * (1.0 + runtime.pitch_jitter_current);
        let engine_gain = config.game.sfx.engine_idle_gain
            + (runtime.smoothed_load * config.game.sfx.engine_load_gain);
        let volume = config.game.sfx.master_volume * config.game.sfx.engine_volume * engine_gain;

        sink.set_speed(playback_speed.max(0.05));
        sink.set_volume(Volume::Linear(volume.max(0.0)));
    }
}

#[allow(clippy::too_many_arguments)]
fn play_gameplay_sfx(
    mut commands: Commands,
    config: Res<GameConfig>,
    registry: Option<Res<AssetRegistry>>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    mut runtime_audio_cache: ResMut<SfxRuntimeAudioCache>,
    mut rng: ResMut<SfxRngState>,
    mut warnings: ResMut<SfxMissingAssetWarnings>,
    mut fired_events: MessageReader<PlayerWeaponFiredEvent>,
    mut impact_events: MessageReader<PlayerProjectileImpactEvent>,
    mut killed_events: MessageReader<EnemyKilledEvent>,
) {
    if !config.game.sfx.enabled {
        let _ = fired_events.read().count();
        let _ = impact_events.read().count();
        let _ = killed_events.read().count();
        return;
    }

    let Some(registry) = registry else {
        let _ = fired_events.read().count();
        let _ = impact_events.read().count();
        let _ = killed_events.read().count();
        return;
    };

    let sfx = &config.game.sfx;

    for event in fired_events.read() {
        let _shot_position = event.world_position;
        match event.kind {
            PlayerProjectileAudioKind::Bullet => play_sfx_by_id(
                &mut commands,
                registry.as_ref(),
                &mut audio_sources,
                &mut runtime_audio_cache,
                sfx,
                AUDIO_ID_GUN_SHOT,
                sfx.gun_shot_volume,
                &mut rng.seed,
                &mut warnings,
            ),
            PlayerProjectileAudioKind::Missile => play_sfx_by_id(
                &mut commands,
                registry.as_ref(),
                &mut audio_sources,
                &mut runtime_audio_cache,
                sfx,
                AUDIO_ID_MISSILE_LAUNCH,
                sfx.missile_launch_volume,
                &mut rng.seed,
                &mut warnings,
            ),
        }
    }

    for event in impact_events.read() {
        let _impact_position = event.world_position;
        match (event.kind, event.target) {
            (PlayerProjectileAudioKind::Bullet, PlayerProjectileImpactTarget::Enemy) => {
                play_sfx_by_id(
                    &mut commands,
                    registry.as_ref(),
                    &mut audio_sources,
                    &mut runtime_audio_cache,
                    sfx,
                    AUDIO_ID_GUN_HIT,
                    sfx.gun_hit_volume,
                    &mut rng.seed,
                    &mut warnings,
                );
            }
            (PlayerProjectileAudioKind::Bullet, PlayerProjectileImpactTarget::Ground) => {
                play_sfx_by_id(
                    &mut commands,
                    registry.as_ref(),
                    &mut audio_sources,
                    &mut runtime_audio_cache,
                    sfx,
                    AUDIO_ID_GUN_MISS,
                    sfx.gun_miss_volume,
                    &mut rng.seed,
                    &mut warnings,
                );
            }
            (PlayerProjectileAudioKind::Missile, _) => {
                play_sfx_by_id(
                    &mut commands,
                    registry.as_ref(),
                    &mut audio_sources,
                    &mut runtime_audio_cache,
                    sfx,
                    AUDIO_ID_MISSILE_HIT,
                    sfx.missile_hit_volume,
                    &mut rng.seed,
                    &mut warnings,
                );
            }
        }
    }

    for _ in killed_events.read() {
        play_sfx_by_id(
            &mut commands,
            registry.as_ref(),
            &mut audio_sources,
            &mut runtime_audio_cache,
            sfx,
            AUDIO_ID_EXPLODE,
            sfx.explode_volume,
            &mut rng.seed,
            &mut warnings,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn play_sfx_by_id(
    commands: &mut Commands,
    registry: &AssetRegistry,
    audio_sources: &mut Assets<AudioSource>,
    runtime_audio_cache: &mut SfxRuntimeAudioCache,
    sfx: &SfxConfig,
    audio_id: &str,
    relative_volume: f32,
    seed: &mut u64,
    warnings: &mut SfxMissingAssetWarnings,
) {
    let Some(handle) = resolve_runtime_audio_handle(
        audio_id,
        registry,
        audio_sources,
        runtime_audio_cache,
        warnings,
    ) else {
        return;
    };

    let volume = (sfx.master_volume * relative_volume).max(0.0);
    if volume <= f32::EPSILON {
        return;
    }

    let pitch = lerp(
        sfx.pitch_random_min,
        sfx.pitch_random_max,
        next_unit_random(seed),
    )
    .max(0.01);

    commands.spawn((
        Name::new("GameplaySfxShot"),
        GameplaySfxTransient,
        AudioPlayer::<AudioSource>::new(handle),
        PlaybackSettings::DESPAWN
            .with_volume(Volume::Linear(volume))
            .with_speed(pitch),
    ));
}

fn resolve_runtime_audio_handle(
    audio_id: &str,
    registry: &AssetRegistry,
    audio_sources: &mut Assets<AudioSource>,
    runtime_audio_cache: &mut SfxRuntimeAudioCache,
    warnings: &mut SfxMissingAssetWarnings,
) -> Option<Handle<AudioSource>> {
    if let Some(handle) = runtime_audio_cache.handles_by_id.get(audio_id) {
        return Some(handle.clone());
    }

    let Some(entry) = registry.audio.get(audio_id) else {
        if warnings.missing_ids.insert(audio_id.to_string()) {
            warn!("SFX audio asset `{}` is not present in registry.", audio_id);
        }
        return None;
    };
    if !entry.exists_on_disk {
        if warnings.missing_ids.insert(audio_id.to_string()) {
            warn!(
                "SFX audio asset `{}` path `{}` does not exist on disk.",
                audio_id, entry.path
            );
        }
        return None;
    }

    let audio_file_path = resolve_asset_file_path(&entry.path);
    let mut bytes = match fs::read(&audio_file_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            if warnings.missing_ids.insert(audio_id.to_string()) {
                warn!(
                    "Failed reading SFX asset `{}` from `{}`: {}",
                    audio_id,
                    audio_file_path.to_string_lossy(),
                    error
                );
            }
            return None;
        }
    };

    if let Err(error) = sanitize_runtime_audio_bytes(&mut bytes) {
        if warnings.missing_ids.insert(audio_id.to_string()) {
            warn!(
                "Skipping SFX asset `{}` from `{}`: {}",
                audio_id,
                audio_file_path.to_string_lossy(),
                error
            );
        }
        return None;
    }

    let handle = audio_sources.add(AudioSource {
        bytes: bytes.into(),
    });
    runtime_audio_cache
        .handles_by_id
        .insert(audio_id.to_string(), handle.clone());
    Some(handle)
}

fn resolve_asset_file_path(asset_path: &str) -> PathBuf {
    let relative_file_path = asset_path.split('#').next().unwrap_or(asset_path);
    Path::new("assets").join(relative_file_path)
}

fn sanitize_runtime_audio_bytes(bytes: &mut [u8]) -> Result<(), String> {
    if bytes.len() < 12 {
        return Err("audio buffer is too small".to_string());
    }

    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        normalize_wav_unknown_sizes(bytes)?;
        normalize_wav_linear_pcm_header_fields(bytes)?;
        return Ok(());
    }

    Err("non-WAV payload not supported in SFX runtime loader".to_string())
}

fn normalize_wav_unknown_sizes(bytes: &mut [u8]) -> Result<(), String> {
    if bytes.len() < 44 {
        return Err("wav payload too small".to_string());
    }
    if !(bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE")) {
        return Ok(());
    }

    let riff_size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if riff_size == u32::MAX {
        let fixed_riff_size = (bytes.len().saturating_sub(8)).min(u32::MAX as usize) as u32;
        bytes[4..8].copy_from_slice(&fixed_riff_size.to_le_bytes());
    }

    let mut cursor = 12usize;
    let mut found_data = false;
    while cursor + 8 <= bytes.len() {
        let chunk_id = &bytes[cursor..cursor + 4];
        let chunk_size = u32::from_le_bytes([
            bytes[cursor + 4],
            bytes[cursor + 5],
            bytes[cursor + 6],
            bytes[cursor + 7],
        ]);
        let chunk_data_start = cursor + 8;
        if chunk_data_start > bytes.len() {
            break;
        }
        let chunk_size_usize = chunk_size as usize;
        let available = bytes.len() - chunk_data_start;

        if chunk_id == b"data" {
            found_data = true;
            if chunk_size == u32::MAX || chunk_size_usize > available {
                let fixed_data_size = available.min(u32::MAX as usize) as u32;
                bytes[cursor + 4..cursor + 8].copy_from_slice(&fixed_data_size.to_le_bytes());
            }
            break;
        }

        if chunk_size_usize > available {
            return Err("wav chunk size exceeds available data".to_string());
        }

        let padding = chunk_size_usize & 1;
        cursor = chunk_data_start + chunk_size_usize + padding;
    }

    if !found_data {
        return Err("wav payload missing data chunk".to_string());
    }
    Ok(())
}

fn normalize_wav_linear_pcm_header_fields(bytes: &mut [u8]) -> Result<(), String> {
    if bytes.len() < 44 {
        return Err("wav payload too small".to_string());
    }
    if !(bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE")) {
        return Ok(());
    }

    let mut cursor = 12usize;
    let mut found_fmt = false;
    while cursor + 8 <= bytes.len() {
        let chunk_id = &bytes[cursor..cursor + 4];
        let chunk_size = u32::from_le_bytes([
            bytes[cursor + 4],
            bytes[cursor + 5],
            bytes[cursor + 6],
            bytes[cursor + 7],
        ]) as usize;
        let chunk_data_start = cursor + 8;
        let chunk_data_end = chunk_data_start.saturating_add(chunk_size);
        if chunk_data_start > bytes.len() || chunk_data_end > bytes.len() {
            return Err("wav chunk size exceeds available data".to_string());
        }

        if chunk_id == b"fmt " {
            found_fmt = true;
            if chunk_size < 16 {
                return Err("wav fmt chunk too small".to_string());
            }

            let audio_format =
                u16::from_le_bytes([bytes[chunk_data_start], bytes[chunk_data_start + 1]]);
            if audio_format != 1 {
                return Ok(());
            }

            let channels =
                u16::from_le_bytes([bytes[chunk_data_start + 2], bytes[chunk_data_start + 3]]);
            let sample_rate = u32::from_le_bytes([
                bytes[chunk_data_start + 4],
                bytes[chunk_data_start + 5],
                bytes[chunk_data_start + 6],
                bytes[chunk_data_start + 7],
            ]);
            let byte_rate = u32::from_le_bytes([
                bytes[chunk_data_start + 8],
                bytes[chunk_data_start + 9],
                bytes[chunk_data_start + 10],
                bytes[chunk_data_start + 11],
            ]);
            let block_align =
                u16::from_le_bytes([bytes[chunk_data_start + 12], bytes[chunk_data_start + 13]]);
            let bits_per_sample =
                u16::from_le_bytes([bytes[chunk_data_start + 14], bytes[chunk_data_start + 15]]);

            if channels == 0 || bits_per_sample == 0 {
                return Err("wav fmt chunk contains invalid channels/bits".to_string());
            }

            let expected_block_align_u32 = (channels as u32)
                .checked_mul(bits_per_sample as u32)
                .ok_or_else(|| "wav fmt block align overflow".to_string())?
                / 8;
            if expected_block_align_u32 == 0 || expected_block_align_u32 > u16::MAX as u32 {
                return Err("wav fmt block align is out of range".to_string());
            }
            let expected_block_align = expected_block_align_u32 as u16;
            if block_align != expected_block_align {
                bytes[chunk_data_start + 12..chunk_data_start + 14]
                    .copy_from_slice(&expected_block_align.to_le_bytes());
            }

            let expected_byte_rate = sample_rate
                .checked_mul(expected_block_align as u32)
                .ok_or_else(|| "wav fmt byte rate overflow".to_string())?;
            if byte_rate != expected_byte_rate {
                bytes[chunk_data_start + 8..chunk_data_start + 12]
                    .copy_from_slice(&expected_byte_rate.to_le_bytes());
            }
            break;
        }

        cursor = chunk_data_end + (chunk_size & 1);
    }

    if !found_fmt {
        return Err("wav payload missing fmt chunk".to_string());
    }
    Ok(())
}

fn next_unit_random(seed: &mut u64) -> f32 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 32) as u32) as f32 / u32::MAX as f32
}

fn next_signed_unit_random(seed: &mut u64) -> f32 {
    (next_unit_random(seed) * 2.0) - 1.0
}

fn unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + ((b - a) * t.clamp(0.0, 1.0))
}
