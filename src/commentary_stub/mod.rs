use crate::config::{CommentatorProfile, GameConfig};
use crate::gameplay::combat::EnemyKilledEvent;
use crate::gameplay::enemies::{PlayerDamageEvent, PlayerDamageSource, PlayerEnemyCrashEvent};
use crate::gameplay::vehicle::{
    PlayerHealth, PlayerVehicle, VehicleStuntEvent, VehicleStuntMetrics, VehicleTelemetry,
};
use crate::states::GameState;
use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const API_BASE_URL_ENV: &str = "NEOCORTEX_API_BASE_URL";
const API_KEY_ENV: &str = "NEOCORTEX_API_KEY";
const DEFAULT_API_BASE_URL: &str = "https://neocortex.link";
const DEFAULT_AUDIO_FORMAT: &str = "wav";
const CURL_CONNECT_TIMEOUT_SECONDS: u32 = 4;
const CURL_REQUEST_TIMEOUT_SECONDS: u32 = 14;
const COMMENTARY_STREAK_WINDOW_SECONDS: f64 = 4.5;
const COMMENTARY_SUBTITLE_DURATION_SECONDS: f64 = 5.0;
const COMMENTARY_MAX_QUEUE_SIZE: usize = 96;
const COMMENTARY_RETAINED_RECENT_EVENTS: usize = 12;
const SUBTITLE_PANEL_BOTTOM_PX: f32 = 36.0;
const COMMENTARY_HEAVY_DAMAGE_THRESHOLD_HP: f32 = 8.0;

pub struct CommentaryStubPlugin;

impl Plugin for CommentaryStubPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommentaryStubState>()
            .add_systems(
                OnEnter(GameState::InRun),
                (reset_commentary_stub, spawn_commentary_subtitle_overlay),
            )
            .add_systems(
                OnExit(GameState::InRun),
                (
                    cleanup_commentary_subtitle_overlay,
                    cleanup_commentary_narration_playback,
                ),
            )
            .add_systems(
                Update,
                (
                    collect_commentary_events,
                    poll_neocortex_api_result,
                    play_pending_commentary_audio,
                    process_commentary_queue,
                    sync_commentary_subtitle_overlay,
                )
                    .chain()
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            );
    }
}

#[derive(Component)]
struct CommentarySubtitleRoot;

#[derive(Component)]
struct CommentarySubtitleText;

#[derive(Component)]
struct CommentaryNarrationPlayback;

#[derive(Debug, Clone)]
pub enum GameEvent {
    JumpBig {
        duration_s: f32,
    },
    JumpHuge {
        duration_s: f32,
    },
    WheelieLong {
        duration_s: f32,
    },
    Flip {
        total_flips: u32,
    },
    Kill {
        enemy_type_id: String,
    },
    Crash {
        impact_speed_mps: f32,
    },
    SpeedTier {
        tier: u8,
        speed_mps: f32,
    },
    NearDeath {
        health_fraction: f32,
    },
    HeavyDamage {
        amount: f32,
    },
    HitByBomb {
        damage: f32,
    },
    CrashIntoEnemy {
        speed_mps: f32,
        enemy_type_id: String,
    },
    Streak {
        count: u32,
    },
    Manual {
        label: String,
    },
}

impl GameEvent {
    fn label(&self) -> &'static str {
        match self {
            Self::JumpBig { .. } => "JumpBig",
            Self::JumpHuge { .. } => "JumpHuge",
            Self::WheelieLong { .. } => "WheelieLong",
            Self::Flip { .. } => "Flip",
            Self::Kill { .. } => "Kill",
            Self::Crash { .. } => "Crash",
            Self::SpeedTier { .. } => "SpeedTier",
            Self::NearDeath { .. } => "NearDeath",
            Self::HeavyDamage { .. } => "HeavyDamage",
            Self::HitByBomb { .. } => "HitByBomb",
            Self::CrashIntoEnemy { .. } => "CrashIntoEnemy",
            Self::Streak { .. } => "Streak",
            Self::Manual { .. } => "Manual",
        }
    }
}

#[derive(Debug)]
struct InFlightApiRequest {
    speaker_id: String,
    speaker_name: String,
    subtitle_color: [f32; 3],
    fallback_line: String,
    started_at_seconds: f64,
    handle: JoinHandle<Result<NeocortexJobResult, String>>,
}

#[derive(Debug)]
struct NeocortexJobArgs {
    api_base_url: String,
    api_key: String,
    character_id: String,
    prompt: String,
    voice_emotion: String,
    session_id: Option<String>,
    audio_format: String,
    output_path: PathBuf,
    max_retries: u32,
    retry_backoff_seconds: f32,
}

#[derive(Debug)]
struct NeocortexJobResult {
    response_line: String,
    session_id: Option<String>,
    audio_path: String,
    chat_status: u16,
    audio_status: u16,
}

#[derive(Resource, Debug)]
pub struct CommentaryStubState {
    pub queue: VecDeque<GameEvent>,
    pub last_line: String,
    pub last_speaker: String,
    pub pending_speaker_id: String,
    pub pending_chat_emotion: String,
    pub pending_voice_emotion: String,
    pub pending_summary_preview: String,
    pub pending_prompt_preview: String,
    pub recent_events: VecDeque<String>,
    pub api_status: String,
    pub last_audio_path: String,
    pending_audio_path: Option<String>,
    last_emit_time_seconds: f64,
    next_commentator_index: usize,
    rng_state: u64,
    last_crash_count: u32,
    last_speed_tier: u8,
    near_death_active: bool,
    kill_streak_count: u32,
    last_kill_time_seconds: f64,
    last_speaker_id: String,
    subtitle_line: String,
    subtitle_color: [f32; 3],
    subtitle_expires_at_seconds: f64,
    last_line_by_commentator: HashMap<String, String>,
    session_id_by_commentator: HashMap<String, String>,
    inflight_request: Option<InFlightApiRequest>,
}

impl Default for CommentaryStubState {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            last_line: String::new(),
            last_speaker: String::new(),
            pending_speaker_id: String::new(),
            pending_chat_emotion: String::new(),
            pending_voice_emotion: String::new(),
            pending_summary_preview: String::new(),
            pending_prompt_preview: String::new(),
            recent_events: VecDeque::new(),
            api_status: "idle".to_string(),
            last_audio_path: String::new(),
            pending_audio_path: None,
            last_emit_time_seconds: 0.0,
            next_commentator_index: 0,
            rng_state: 0x4D52_4155_544F_4155,
            last_crash_count: 0,
            last_speed_tier: 0,
            near_death_active: false,
            kill_streak_count: 0,
            last_kill_time_seconds: 0.0,
            last_speaker_id: String::new(),
            subtitle_line: String::new(),
            subtitle_color: [0.9, 0.9, 0.9],
            subtitle_expires_at_seconds: 0.0,
            last_line_by_commentator: HashMap::new(),
            session_id_by_commentator: HashMap::new(),
            inflight_request: None,
        }
    }
}

#[derive(Default)]
struct SummaryAggregation {
    kills: u32,
    biggest_jump_s: f32,
    has_huge_jump: bool,
    wheelie_longest_s: f32,
    latest_flip_total: u32,
    crashes: u32,
    latest_crash_impact_mps: f32,
    highest_speed_tier: u8,
    highest_speed_mps: f32,
    near_death_fraction: Option<f32>,
    heaviest_damage_hp: f32,
    bomb_hit_count: u32,
    total_bomb_damage_hp: f32,
    crash_enemy_count: u32,
    fastest_enemy_crash_mps: f32,
    streak_count: u32,
    manual_labels: Vec<String>,
}
fn reset_commentary_stub(mut state: ResMut<CommentaryStubState>) {
    *state = CommentaryStubState::default();
    state.rng_state ^= unix_timestamp_seconds();
}

fn spawn_commentary_subtitle_overlay(mut commands: Commands) {
    commands
        .spawn((
            Name::new("CommentarySubtitleRoot"),
            CommentarySubtitleRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(SUBTITLE_PANEL_BOTTOM_PX),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
            ZIndex(220),
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("CommentarySubtitleText"),
                CommentarySubtitleText,
                Text::new(""),
                TextFont {
                    font_size: 26.0,
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.95, 0.95)),
                BackgroundColor(Color::srgba(0.04, 0.05, 0.06, 0.72)),
                Node {
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                    ..default()
                },
            ));
        });
}

fn cleanup_commentary_subtitle_overlay(
    mut commands: Commands,
    root_query: Query<Entity, With<CommentarySubtitleRoot>>,
) {
    for entity in &root_query {
        commands.entity(entity).try_despawn();
    }
}

fn cleanup_commentary_narration_playback(
    mut commands: Commands,
    playback_query: Query<Entity, With<CommentaryNarrationPlayback>>,
) {
    for entity in &playback_query {
        commands.entity(entity).try_despawn();
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_commentary_events(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    config: Res<GameConfig>,
    telemetry: Option<Res<VehicleTelemetry>>,
    stunt_metrics: Option<Res<VehicleStuntMetrics>>,
    player_query: Query<&PlayerHealth, With<PlayerVehicle>>,
    mut stunt_events: MessageReader<VehicleStuntEvent>,
    mut kill_events: MessageReader<EnemyKilledEvent>,
    mut player_damage_events: MessageReader<PlayerDamageEvent>,
    mut player_enemy_crash_events: MessageReader<PlayerEnemyCrashEvent>,
    mut state: ResMut<CommentaryStubState>,
) {
    let now = time.elapsed_secs_f64();

    if now - state.last_kill_time_seconds > COMMENTARY_STREAK_WINDOW_SECONDS {
        state.kill_streak_count = 0;
    }

    if keyboard.just_pressed(KeyCode::KeyJ) {
        push_event(
            &mut state,
            GameEvent::Manual {
                label: "debug big jump trigger".to_string(),
            },
        );
    }
    if keyboard.just_pressed(KeyCode::KeyK) {
        push_event(
            &mut state,
            GameEvent::Manual {
                label: "debug kill trigger".to_string(),
            },
        );
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        push_event(
            &mut state,
            GameEvent::Manual {
                label: "debug crash trigger".to_string(),
            },
        );
    }

    for event in stunt_events.read() {
        match event {
            VehicleStuntEvent::AirtimeBig { duration_s } => {
                push_event(
                    &mut state,
                    GameEvent::JumpBig {
                        duration_s: *duration_s,
                    },
                );
            }
            VehicleStuntEvent::AirtimeHuge { duration_s } => {
                push_event(
                    &mut state,
                    GameEvent::JumpHuge {
                        duration_s: *duration_s,
                    },
                );
            }
            VehicleStuntEvent::WheelieLong { duration_s } => {
                push_event(
                    &mut state,
                    GameEvent::WheelieLong {
                        duration_s: *duration_s,
                    },
                );
            }
            VehicleStuntEvent::Flip { total_flips } => {
                push_event(
                    &mut state,
                    GameEvent::Flip {
                        total_flips: *total_flips,
                    },
                );
            }
        }
    }

    for event in kill_events.read() {
        push_event(
            &mut state,
            GameEvent::Kill {
                enemy_type_id: event.enemy_type_id.clone(),
            },
        );
        state.last_kill_time_seconds = now;
        state.kill_streak_count = state.kill_streak_count.saturating_add(1);
        if state.kill_streak_count >= 2 {
            let streak_count = state.kill_streak_count;
            push_event(
                &mut state,
                GameEvent::Streak {
                    count: streak_count,
                },
            );
        }
    }

    for event in player_damage_events.read() {
        if event.amount >= COMMENTARY_HEAVY_DAMAGE_THRESHOLD_HP {
            push_event(
                &mut state,
                GameEvent::HeavyDamage {
                    amount: event.amount,
                },
            );
        }
        if event.source == PlayerDamageSource::ProjectileBomb {
            push_event(
                &mut state,
                GameEvent::HitByBomb {
                    damage: event.amount,
                },
            );
        }
    }

    for event in player_enemy_crash_events.read() {
        push_event(
            &mut state,
            GameEvent::CrashIntoEnemy {
                speed_mps: event.player_speed_mps,
                enemy_type_id: event.enemy_type_id.clone(),
            },
        );
    }

    if let Some(telemetry) = telemetry {
        let thresholds = &config.commentator.thresholds;
        let new_tier = if telemetry.speed_mps
            >= thresholds.speed_tier_2.max(thresholds.speed_tier_1)
            && thresholds.speed_tier_2 > 0.0
        {
            2
        } else if telemetry.speed_mps >= thresholds.speed_tier_1.max(0.01) {
            1
        } else {
            0
        };
        if new_tier > state.last_speed_tier {
            state.last_speed_tier = new_tier;
            push_event(
                &mut state,
                GameEvent::SpeedTier {
                    tier: new_tier,
                    speed_mps: telemetry.speed_mps,
                },
            );
        }
    }

    if let Some(stunts) = stunt_metrics {
        if stunts.crash_count > state.last_crash_count {
            let new_crashes = stunts.crash_count - state.last_crash_count;
            state.last_crash_count = stunts.crash_count;
            for _ in 0..new_crashes {
                push_event(
                    &mut state,
                    GameEvent::Crash {
                        impact_speed_mps: stunts.last_landing_impact_speed_mps,
                    },
                );
            }
        }
    }

    if let Ok(player_health) = player_query.single() {
        let health_fraction = if player_health.max > 0.0 {
            (player_health.current / player_health.max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let near_death_threshold = config
            .commentator
            .thresholds
            .near_death_health_fraction
            .clamp(0.01, 1.0);
        if health_fraction <= near_death_threshold && !state.near_death_active {
            state.near_death_active = true;
            push_event(&mut state, GameEvent::NearDeath { health_fraction });
        } else if health_fraction > (near_death_threshold + 0.05).min(1.0) {
            state.near_death_active = false;
        }
    }
}
fn poll_neocortex_api_result(
    time: Res<Time>,
    config: Res<GameConfig>,
    mut state: ResMut<CommentaryStubState>,
) {
    let Some(inflight) = state.inflight_request.as_ref() else {
        return;
    };
    let now = time.elapsed_secs_f64();
    if !inflight.handle.is_finished() {
        let stale_timeout_seconds = config
            .commentator
            .commentary
            .api_stale_request_timeout_seconds
            .max(0.25) as f64;
        if now - inflight.started_at_seconds <= stale_timeout_seconds {
            return;
        }
        let Some(stale_request) = state.inflight_request.take() else {
            return;
        };
        state.api_status =
            format!("stale request timed out after {stale_timeout_seconds:.1}s, using fallback");
        state.last_audio_path.clear();
        state.pending_audio_path = None;
        finalize_commentary_line(
            &mut state,
            &stale_request.speaker_id,
            &stale_request.speaker_name,
            stale_request.subtitle_color,
            stale_request.fallback_line,
            now,
        );
        return;
    }

    let Some(inflight) = state.inflight_request.take() else {
        return;
    };

    match inflight.handle.join() {
        Ok(Ok(result)) => {
            if let Some(session_id) = result.session_id {
                state
                    .session_id_by_commentator
                    .insert(inflight.speaker_id.clone(), session_id);
            }
            state.api_status = format!(
                "ok (chat {}, audio {})",
                result.chat_status, result.audio_status
            );
            state.last_audio_path = result.audio_path.clone();
            state.pending_audio_path = Some(result.audio_path);
            let emitted_line = sanitize_subtitle_line(&result.response_line);
            finalize_commentary_line(
                &mut state,
                &inflight.speaker_id,
                &inflight.speaker_name,
                inflight.subtitle_color,
                if emitted_line.is_empty() {
                    inflight.fallback_line
                } else {
                    emitted_line
                },
                now,
            );
        }
        Ok(Err(error_message)) => {
            state.api_status = format!("error: {}", truncate(&error_message, 140));
            state.last_audio_path.clear();
            state.pending_audio_path = None;
            finalize_commentary_line(
                &mut state,
                &inflight.speaker_id,
                &inflight.speaker_name,
                inflight.subtitle_color,
                inflight.fallback_line,
                now,
            );
        }
        Err(join_error) => {
            state.api_status = format!("error: API worker panicked ({join_error:?})");
            state.last_audio_path.clear();
            state.pending_audio_path = None;
            finalize_commentary_line(
                &mut state,
                &inflight.speaker_id,
                &inflight.speaker_name,
                inflight.subtitle_color,
                inflight.fallback_line,
                now,
            );
        }
    }
}

fn play_pending_commentary_audio(
    config: Res<GameConfig>,
    mut commands: Commands,
    mut state: ResMut<CommentaryStubState>,
    mut audio_sources: ResMut<Assets<AudioSource>>,
    existing_playback_query: Query<Entity, With<CommentaryNarrationPlayback>>,
) {
    let Some(audio_path) = state.pending_audio_path.take() else {
        return;
    };

    let audio_bytes = match fs::read(&audio_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            state.api_status = format!(
                "audio playback skipped (failed reading `{}`: {})",
                audio_path,
                truncate(&error.to_string(), 120)
            );
            return;
        }
    };

    if audio_bytes.is_empty() {
        state.api_status = "audio playback skipped (empty file)".to_string();
        return;
    }

    let audio_bytes = match prepare_narration_audio_bytes(audio_bytes) {
        Ok(bytes) => bytes,
        Err(error) => {
            state.api_status = format!("audio playback skipped ({})", truncate(&error, 140));
            warn!(
                "Skipped narration playback for `{}`: {}",
                audio_path,
                truncate(&error, 220)
            );
            return;
        }
    };

    for entity in &existing_playback_query {
        commands.entity(entity).try_despawn();
    }

    let volume = config
        .commentator
        .commentary
        .narration_volume
        .clamp(0.0, 2.0);
    let handle = audio_sources.add(AudioSource {
        bytes: audio_bytes.into(),
    });

    commands.spawn((
        Name::new("CommentaryNarrationPlayback"),
        CommentaryNarrationPlayback,
        AudioPlayer::new(handle),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
    ));
}

fn prepare_narration_audio_bytes(mut bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if bytes.len() < 12 {
        return Err("audio buffer is too small".to_string());
    }

    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        normalize_wav_unknown_sizes(&mut bytes)?;
        return Ok(bytes);
    }

    if bytes.starts_with(b"OggS")
        || bytes.starts_with(b"ID3")
        || (bytes[0] == 0xFF && (bytes[1] & 0b1110_0000) == 0b1110_0000)
    {
        return Ok(bytes);
    }

    Err(format!(
        "unrecognized audio header {:02X?}",
        &bytes[..bytes.len().min(8)]
    ))
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

fn process_commentary_queue(
    time: Res<Time>,
    config: Res<GameConfig>,
    mut state: ResMut<CommentaryStubState>,
) {
    if state.inflight_request.is_some() {
        return;
    }

    let Some(profile) = select_next_commentator_profile(&config, state.next_commentator_index)
    else {
        return;
    };
    state.pending_speaker_id = profile.id.clone();

    let max_events = config.commentator.commentary.max_events_per_batch.max(1);
    let preview_events: Vec<GameEvent> = state.queue.iter().take(max_events).cloned().collect();
    if preview_events.is_empty() {
        state.pending_summary_preview.clear();
        state.pending_prompt_preview.clear();
        state.pending_chat_emotion.clear();
        state.pending_voice_emotion.clear();
        return;
    }

    let preview_summary = build_summary_text(&preview_events);
    state.pending_summary_preview = preview_summary.clone();

    let preview_chat_emotion = profile
        .emotions
        .first()
        .map(|value| normalize_chat_emotion(value))
        .unwrap_or_else(|| "neutral".to_string());
    let other_last_line = previous_commentator_line(&config, &state, &profile.id);
    state.pending_prompt_preview = build_prompt_preview(
        &profile,
        &preview_summary,
        &other_last_line,
        &preview_chat_emotion,
    );

    let cooldown_s = config
        .commentator
        .commentary
        .min_seconds_between_lines
        .max(0.0) as f64;
    let now = time.elapsed_secs_f64();
    if now - state.last_emit_time_seconds < cooldown_s {
        return;
    }

    let to_emit_count = preview_events.len();
    let mut emitted_events = Vec::with_capacity(to_emit_count);
    for _ in 0..to_emit_count {
        if let Some(event) = state.queue.pop_front() {
            emitted_events.push(event);
        }
    }
    if emitted_events.is_empty() {
        return;
    }

    let summary = build_summary_text(&emitted_events);
    let fallback_line = if summary.is_empty() {
        fallback_line(&config, &mut state).unwrap_or_else(|| "player event.".to_string())
    } else {
        summary.clone()
    };

    let (chat_emotion, voice_emotion) = select_commentator_emotion(&profile, &mut state);
    state.pending_chat_emotion = chat_emotion.clone();
    state.pending_voice_emotion = voice_emotion.clone();
    state.pending_summary_preview = summary.clone();

    let prompt = build_prompt_preview(&profile, &summary, &other_last_line, &chat_emotion);
    state.pending_prompt_preview = prompt.clone();

    let speaker_name = if profile.name.trim().is_empty() {
        profile.id.clone()
    } else {
        profile.name.clone()
    };

    record_recent_event_labels(&mut state, &emitted_events);
    state.next_commentator_index =
        (state.next_commentator_index + 1) % config.commentator.commentators.len().max(1);

    let Some(api_key) = std::env::var(API_KEY_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        state.api_status = format!("{API_KEY_ENV} not set, using fallback");
        state.last_audio_path.clear();
        state.pending_audio_path = None;
        finalize_commentary_line(
            &mut state,
            &profile.id,
            &speaker_name,
            profile.subtitle_color,
            fallback_line,
            now,
        );
        return;
    };

    let character_id = profile.character_id.trim().to_string();
    if character_id.is_empty() {
        state.api_status = "character_id missing, using fallback".to_string();
        state.last_audio_path.clear();
        state.pending_audio_path = None;
        finalize_commentary_line(
            &mut state,
            &profile.id,
            &speaker_name,
            profile.subtitle_color,
            fallback_line,
            now,
        );
        return;
    }

    let api_base_url =
        std::env::var(API_BASE_URL_ENV).unwrap_or_else(|_| DEFAULT_API_BASE_URL.to_string());
    let session_id = state.session_id_by_commentator.get(&profile.id).cloned();
    let output_path = next_audio_output_path(DEFAULT_AUDIO_FORMAT, &profile.id);

    let args = NeocortexJobArgs {
        api_base_url,
        api_key,
        character_id,
        prompt,
        voice_emotion,
        session_id,
        audio_format: DEFAULT_AUDIO_FORMAT.to_string(),
        output_path,
        max_retries: config.commentator.commentary.api_max_retries,
        retry_backoff_seconds: config.commentator.commentary.api_retry_backoff_seconds,
    };

    let worker = std::thread::spawn(move || run_neocortex_chat_to_voice(args));
    state.inflight_request = Some(InFlightApiRequest {
        speaker_id: profile.id.clone(),
        speaker_name,
        subtitle_color: profile.subtitle_color,
        fallback_line,
        started_at_seconds: now,
        handle: worker,
    });
    state.api_status = "requesting chat+audio...".to_string();
}
fn sync_commentary_subtitle_overlay(
    time: Res<Time>,
    state: Res<CommentaryStubState>,
    mut root_query: Query<&mut Visibility, With<CommentarySubtitleRoot>>,
    mut text_query: Query<(&mut Text, &mut TextColor), With<CommentarySubtitleText>>,
) {
    let Ok(mut root_visibility) = root_query.single_mut() else {
        return;
    };
    let Ok((mut subtitle_text, mut subtitle_color)) = text_query.single_mut() else {
        return;
    };

    if state.subtitle_line.is_empty() || time.elapsed_secs_f64() > state.subtitle_expires_at_seconds
    {
        *root_visibility = Visibility::Hidden;
        return;
    }

    *root_visibility = Visibility::Inherited;
    *subtitle_text = Text::new(format!("{}: {}", state.last_speaker, state.subtitle_line));
    *subtitle_color = TextColor(Color::srgb(
        state.subtitle_color[0].clamp(0.0, 1.0),
        state.subtitle_color[1].clamp(0.0, 1.0),
        state.subtitle_color[2].clamp(0.0, 1.0),
    ));
}

fn finalize_commentary_line(
    state: &mut CommentaryStubState,
    speaker_id: &str,
    speaker_name: &str,
    subtitle_color: [f32; 3],
    line: String,
    now_seconds: f64,
) {
    let line = sanitize_subtitle_line(&line);
    state.last_speaker_id = speaker_id.to_string();
    state.last_speaker = format_speaker_label(speaker_name);
    state.last_line = line.clone();
    state
        .last_line_by_commentator
        .insert(speaker_id.to_string(), line.clone());
    state.last_emit_time_seconds = now_seconds;
    state.subtitle_line = line;
    state.subtitle_color = subtitle_color;
    state.subtitle_expires_at_seconds = now_seconds + COMMENTARY_SUBTITLE_DURATION_SECONDS;
    state.pending_speaker_id.clear();
    state.pending_chat_emotion.clear();
    state.pending_voice_emotion.clear();
    state.pending_summary_preview.clear();
    state.pending_prompt_preview.clear();
}

fn format_speaker_label(raw_name: &str) -> String {
    let trimmed = raw_name.trim();
    if trimmed.is_empty() {
        return "Commentator".to_string();
    }

    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return "Commentator".to_string();
    };
    let first_up = first.to_uppercase().collect::<String>();
    let rest = chars.as_str().to_ascii_lowercase();
    format!("{first_up}{rest}")
}

fn push_event(state: &mut CommentaryStubState, event: GameEvent) {
    if state.queue.len() >= COMMENTARY_MAX_QUEUE_SIZE {
        state.queue.pop_front();
    }
    state.queue.push_back(event);
}

fn record_recent_event_labels(state: &mut CommentaryStubState, events: &[GameEvent]) {
    for event in events {
        state.recent_events.push_back(event.label().to_string());
        if state.recent_events.len() > COMMENTARY_RETAINED_RECENT_EVENTS {
            state.recent_events.pop_front();
        }
    }
}

fn select_next_commentator_profile(
    config: &GameConfig,
    next_index: usize,
) -> Option<CommentatorProfile> {
    if config.commentator.commentators.is_empty() {
        return None;
    }
    let index = next_index % config.commentator.commentators.len();
    config.commentator.commentators.get(index).cloned()
}

fn select_commentator_emotion(
    profile: &CommentatorProfile,
    state: &mut CommentaryStubState,
) -> (String, String) {
    let fallback = "Neutral".to_string();
    let emotion_pool = if profile.emotions.is_empty() {
        vec![fallback]
    } else {
        profile.emotions.clone()
    };
    let index = next_rng_u32(&mut state.rng_state) as usize % emotion_pool.len();
    let raw = emotion_pool[index].trim();
    (normalize_chat_emotion(raw), normalize_voice_emotion(raw))
}

fn normalize_chat_emotion(value: &str) -> String {
    let cleaned = value.trim();
    if cleaned.is_empty() {
        "neutral".to_string()
    } else {
        cleaned.to_ascii_lowercase()
    }
}

fn normalize_voice_emotion(value: &str) -> String {
    let cleaned = value.trim();
    if cleaned.is_empty() {
        "NEUTRAL".to_string()
    } else {
        cleaned.to_ascii_uppercase()
    }
}

fn previous_commentator_line(
    config: &GameConfig,
    state: &CommentaryStubState,
    current_speaker_id: &str,
) -> String {
    if config.commentator.commentators.len() < 2 {
        return "n/a".to_string();
    }
    let previous = config
        .commentator
        .commentators
        .iter()
        .find(|profile| profile.id != current_speaker_id)
        .and_then(|profile| state.last_line_by_commentator.get(&profile.id))
        .cloned();
    previous.unwrap_or_else(|| "n/a".to_string())
}
fn build_summary_text(events: &[GameEvent]) -> String {
    if events.is_empty() {
        return String::new();
    }
    let mut agg = SummaryAggregation::default();

    for event in events {
        match event {
            GameEvent::JumpBig { duration_s } => {
                agg.biggest_jump_s = agg.biggest_jump_s.max(*duration_s);
            }
            GameEvent::JumpHuge { duration_s } => {
                agg.biggest_jump_s = agg.biggest_jump_s.max(*duration_s);
                agg.has_huge_jump = true;
            }
            GameEvent::WheelieLong { duration_s } => {
                agg.wheelie_longest_s = agg.wheelie_longest_s.max(*duration_s);
            }
            GameEvent::Flip { total_flips } => {
                agg.latest_flip_total = agg.latest_flip_total.max(*total_flips);
            }
            GameEvent::Kill { enemy_type_id } => {
                let _ = enemy_type_id;
                agg.kills = agg.kills.saturating_add(1);
            }
            GameEvent::Crash { impact_speed_mps } => {
                agg.crashes = agg.crashes.saturating_add(1);
                agg.latest_crash_impact_mps = agg.latest_crash_impact_mps.max(*impact_speed_mps);
            }
            GameEvent::SpeedTier { tier, speed_mps } => {
                agg.highest_speed_tier = agg.highest_speed_tier.max(*tier);
                agg.highest_speed_mps = agg.highest_speed_mps.max(*speed_mps);
            }
            GameEvent::NearDeath { health_fraction } => {
                agg.near_death_fraction = Some(
                    agg.near_death_fraction
                        .map_or(*health_fraction, |current| current.min(*health_fraction)),
                );
            }
            GameEvent::HeavyDamage { amount } => {
                agg.heaviest_damage_hp = agg.heaviest_damage_hp.max(*amount);
            }
            GameEvent::HitByBomb { damage } => {
                agg.bomb_hit_count = agg.bomb_hit_count.saturating_add(1);
                agg.total_bomb_damage_hp += damage.max(0.0);
            }
            GameEvent::CrashIntoEnemy {
                speed_mps,
                enemy_type_id,
            } => {
                let _ = enemy_type_id;
                agg.crash_enemy_count = agg.crash_enemy_count.saturating_add(1);
                agg.fastest_enemy_crash_mps = agg.fastest_enemy_crash_mps.max(*speed_mps);
            }
            GameEvent::Streak { count } => {
                agg.streak_count = agg.streak_count.max(*count);
            }
            GameEvent::Manual { label } => {
                agg.manual_labels.push(label.clone());
            }
        }
    }

    let mut parts = Vec::new();
    if agg.has_huge_jump {
        parts.push(format!(
            "player made a huge jump ({:.2}s airtime)",
            agg.biggest_jump_s.max(0.0)
        ));
    } else if agg.biggest_jump_s > 0.0 {
        parts.push(format!(
            "player made a big jump ({:.2}s airtime)",
            agg.biggest_jump_s.max(0.0)
        ));
    }
    if agg.wheelie_longest_s > 0.0 {
        parts.push(format!(
            "player kept a long wheelie ({:.2}s)",
            agg.wheelie_longest_s.max(0.0)
        ));
    }
    if agg.latest_flip_total > 0 {
        parts.push(format!("player has {} total flips", agg.latest_flip_total));
    }
    if agg.kills > 0 {
        if agg.kills == 1 {
            parts.push("player destroyed an enemy".to_string());
        } else {
            parts.push(format!("player destroyed {} enemies", agg.kills));
        }
    }
    if agg.crashes > 0 {
        parts.push(format!(
            "player crashed (impact {:.1} m/s)",
            agg.latest_crash_impact_mps.max(0.0)
        ));
    }
    if agg.highest_speed_tier > 0 {
        parts.push(format!(
            "player reached speed tier {} at {:.1} m/s",
            agg.highest_speed_tier,
            agg.highest_speed_mps.max(0.0)
        ));
    }
    if let Some(health_fraction) = agg.near_death_fraction {
        parts.push(format!(
            "player is close to death ({:.0}% hp)",
            (health_fraction.clamp(0.0, 1.0) * 100.0)
        ));
    }
    if agg.heaviest_damage_hp > 0.0 {
        parts.push(format!(
            "player took heavy damage ({:.1} hp)",
            agg.heaviest_damage_hp
        ));
    }
    if agg.bomb_hit_count > 0 {
        parts.push(format!(
            "player was hit by a bomb ({} hit, {:.1} hp)",
            agg.bomb_hit_count, agg.total_bomb_damage_hp
        ));
    }
    if agg.crash_enemy_count > 0 {
        parts.push(format!(
            "player crashed into an enemy at {:.1} m/s",
            agg.fastest_enemy_crash_mps
        ));
    }
    if agg.streak_count >= 2 {
        parts.push(format!("player kill streak is {}", agg.streak_count));
    }
    if !agg.manual_labels.is_empty() {
        parts.push(format!("debug events: {}", agg.manual_labels.join(", ")));
    }

    if parts.is_empty() {
        return "player event".to_string();
    }

    format!("{}.", parts.join("; "))
}

fn build_prompt_preview(
    profile: &CommentatorProfile,
    summary: &str,
    other_commentator_last_line: &str,
    chat_emotion: &str,
) -> String {
    format!(
        "INSTRUCTION: {}\n\
EMOTION: {}\n\
OTHER COMMENTATOR LAST LINE:\n\
{}\n\
WHAT HAPPENED (dry game facts):\n\
{}",
        profile.style_instruction, chat_emotion, other_commentator_last_line, summary
    )
}

fn fallback_line(config: &GameConfig, state: &mut CommentaryStubState) -> Option<String> {
    if config.commentator.fallback.lines.is_empty() {
        return None;
    }
    let index =
        next_rng_u32(&mut state.rng_state) as usize % config.commentator.fallback.lines.len();
    Some(config.commentator.fallback.lines[index].clone())
}

fn next_rng_u32(state: &mut u64) -> u32 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 32) & 0xFFFF_FFFF) as u32
}

fn next_audio_output_path(extension: &str, speaker_id: &str) -> PathBuf {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let safe_speaker = speaker_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    PathBuf::from(format!(
        "out/commentary/{timestamp_ms}-{safe_speaker}.{extension}"
    ))
}
fn run_neocortex_chat_to_voice(args: NeocortexJobArgs) -> Result<NeocortexJobResult, String> {
    let total_attempts = args.max_retries.saturating_add(1).max(1);
    let mut delay_seconds = args.retry_backoff_seconds.max(0.0) as f64;
    let mut last_error = String::new();

    for attempt_index in 0..total_attempts {
        match run_neocortex_chat_to_voice_once(&args) {
            Ok(result) => return Ok(result),
            Err(error_message) => {
                last_error = error_message;
                if attempt_index + 1 >= total_attempts {
                    break;
                }
                warn!(
                    "Neocortex request attempt {}/{} failed: {}",
                    attempt_index + 1,
                    total_attempts,
                    truncate(&last_error, 180)
                );
                if delay_seconds > 0.0 {
                    std::thread::sleep(Duration::from_secs_f64(delay_seconds));
                }
                if delay_seconds > 0.0 {
                    delay_seconds = (delay_seconds * 2.0).min(8.0);
                }
            }
        }
    }

    Err(format!(
        "all {} attempt(s) failed: {}",
        total_attempts,
        truncate(&last_error, 220)
    ))
}

fn run_neocortex_chat_to_voice_once(args: &NeocortexJobArgs) -> Result<NeocortexJobResult, String> {
    let chat_url = format!("{}/api/v2/chat", args.api_base_url.trim_end_matches('/'));
    let audio_url = format!(
        "{}/api/v2/audio/generate",
        args.api_base_url.trim_end_matches('/')
    );

    let chat_payload = ChatRequestPayload {
        session_id: args.session_id.as_deref(),
        character_id: &args.character_id,
        message: &args.prompt,
    };
    debug!(
        "Neocortex chat prompt (character_id={}, session_id={}):\n{}",
        args.character_id,
        args.session_id.as_deref().unwrap_or("new"),
        args.prompt
    );
    let chat_payload_raw = serde_json::to_string(&chat_payload)
        .map_err(|e| format!("chat payload encode failed: {e}"))?;
    let (chat_status, chat_body) = run_curl_json_post(&chat_url, &chat_payload_raw, &args.api_key)?;
    if !(200..300).contains(&chat_status) {
        return Err(format!(
            "chat http {}: {}",
            chat_status,
            truncate(&chat_body, 260)
        ));
    }

    let chat_response: ChatResponsePayload = serde_json::from_str(&chat_body).map_err(|e| {
        format!(
            "chat response decode failed: {e}; body={}",
            truncate(&chat_body, 260)
        )
    })?;
    debug!(
        "Neocortex chat response (character_id={}, session_id={}):\n{}",
        args.character_id,
        chat_response.session_id.as_deref().unwrap_or("new"),
        chat_response.response
    );
    let response_line = sanitize_subtitle_line(&chat_response.response);
    if response_line.is_empty() {
        return Err("chat returned empty response line".to_string());
    }

    if let Some(parent) = args.output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create output dir {}: {e}", parent.display()))?;
    }

    let voice_payload = GenerateSpeechRequestPayload {
        character_id: &args.character_id,
        message: &response_line,
        emotion: &args.voice_emotion,
        format: &args.audio_format,
    };
    let voice_payload_raw = serde_json::to_string(&voice_payload)
        .map_err(|e| format!("audio payload encode failed: {e}"))?;
    let audio_status = run_curl_download_post(
        &audio_url,
        &voice_payload_raw,
        &args.api_key,
        &args.output_path,
    )?;
    if !(200..300).contains(&audio_status) {
        let body_preview = fs::read_to_string(&args.output_path).unwrap_or_default();
        let _ = fs::remove_file(&args.output_path);
        return Err(format!(
            "audio http {}: {}",
            audio_status,
            truncate(&body_preview, 260)
        ));
    }

    Ok(NeocortexJobResult {
        response_line,
        session_id: chat_response.session_id,
        audio_path: args.output_path.display().to_string(),
        chat_status,
        audio_status,
    })
}

fn run_curl_json_post(
    url: &str,
    payload_json: &str,
    api_key: &str,
) -> Result<(u16, String), String> {
    let status_marker = "__HTTP_STATUS__:";
    let args = vec![
        "-sS".to_string(),
        "-L".to_string(),
        "--connect-timeout".to_string(),
        CURL_CONNECT_TIMEOUT_SECONDS.to_string(),
        "--max-time".to_string(),
        CURL_REQUEST_TIMEOUT_SECONDS.to_string(),
        "-X".to_string(),
        "POST".to_string(),
        url.to_string(),
        "-H".to_string(),
        "Content-Type: application/json".to_string(),
        "-H".to_string(),
        format!("x-api-key: {api_key}"),
        "--data-raw".to_string(),
        payload_json.to_string(),
        "-w".to_string(),
        format!("\\n{status_marker}%{{http_code}}"),
    ];

    let output = run_curl_capture_stdout(&args)?;
    let marker_index = output
        .rfind(status_marker)
        .ok_or_else(|| "missing HTTP status marker in curl output".to_string())?;
    let (body, status_suffix) = output.split_at(marker_index);
    let status_code = status_suffix[status_marker.len()..]
        .trim()
        .parse::<u16>()
        .map_err(|e| format!("failed to parse curl status code: {e}"))?;
    Ok((status_code, body.trim().to_string()))
}

fn run_curl_download_post(
    url: &str,
    payload_json: &str,
    api_key: &str,
    output_path: &Path,
) -> Result<u16, String> {
    let output_file = output_path.display().to_string();
    let args = vec![
        "-sS".to_string(),
        "-L".to_string(),
        "--connect-timeout".to_string(),
        CURL_CONNECT_TIMEOUT_SECONDS.to_string(),
        "--max-time".to_string(),
        CURL_REQUEST_TIMEOUT_SECONDS.to_string(),
        "-X".to_string(),
        "POST".to_string(),
        url.to_string(),
        "-H".to_string(),
        "Content-Type: application/json".to_string(),
        "-H".to_string(),
        format!("x-api-key: {api_key}"),
        "--data-raw".to_string(),
        payload_json.to_string(),
        "--output".to_string(),
        output_file,
        "-w".to_string(),
        "%{http_code}".to_string(),
    ];

    let status_output = run_curl_capture_stdout(&args)?;
    status_output
        .trim()
        .parse::<u16>()
        .map_err(|e| format!("failed to parse audio status code: {e}"))
}

fn run_curl_capture_stdout(args: &[String]) -> Result<String, String> {
    let run = |binary: &str| Command::new(binary).args(args).output();
    let output = match run("curl.exe") {
        Ok(output) => output,
        Err(error) => {
            if error.kind() == std::io::ErrorKind::NotFound {
                run("curl").map_err(|fallback_error| {
                    format!(
                        "failed to execute curl (curl.exe + curl): {error}; fallback error: {fallback_error}"
                    )
                })?
            } else {
                return Err(format!("failed to execute curl.exe: {error}"));
            }
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("curl command failed: {}", truncate(&stderr, 240)));
    }

    String::from_utf8(output.stdout)
        .map_err(|error| format!("curl output was not valid UTF-8: {error}"))
}

fn sanitize_subtitle_line(input: &str) -> String {
    input
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

fn truncate(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut truncated = String::new();
    for (index, c) in input.chars().enumerate() {
        if index >= max_chars.saturating_sub(1) {
            break;
        }
        truncated.push(c);
    }
    truncated.push_str("...");
    truncated
}

fn unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequestPayload<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<&'a str>,
    character_id: &'a str,
    message: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatResponsePayload {
    session_id: Option<String>,
    response: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerateSpeechRequestPayload<'a> {
    character_id: &'a str,
    message: &'a str,
    emotion: &'a str,
    format: &'a str,
}
