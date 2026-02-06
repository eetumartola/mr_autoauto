use crate::commentary_stub::CommentaryStubState;
use crate::config::GameConfig;
use crate::states::GameState;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugRunStats>()
            .init_resource::<KeybindOverlayState>()
            .add_systems(Update, spawn_debug_overlay)
            .add_systems(Update, toggle_keybind_overlay)
            .add_systems(Update, sync_keybind_overlay_visibility)
            .add_systems(OnEnter(GameState::InRun), reset_run_stats)
            .add_systems(
                Update,
                (update_run_stats, update_debug_overlay_text)
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            );
    }
}

#[derive(Component)]
struct DebugOverlayText;

#[derive(Component)]
struct KeybindOverlayText;

#[derive(Component)]
pub struct EnemyDebugMarker;

#[derive(Resource, Debug, Clone)]
pub struct DebugRunStats {
    pub distance_m: f32,
    pub virtual_speed_mps: f32,
    pub active_segment_id: String,
}

impl Default for DebugRunStats {
    fn default() -> Self {
        Self {
            distance_m: 0.0,
            virtual_speed_mps: 12.0,
            active_segment_id: "n/a".to_string(),
        }
    }
}

#[derive(Resource, Debug, Clone)]
struct KeybindOverlayState {
    visible: bool,
}

impl Default for KeybindOverlayState {
    fn default() -> Self {
        Self { visible: true }
    }
}

fn spawn_debug_overlay(
    mut commands: Commands,
    keybind_overlay: Res<KeybindOverlayState>,
    config: Option<Res<GameConfig>>,
    existing_overlay: Query<Entity, With<DebugOverlayText>>,
) {
    if !existing_overlay.is_empty() {
        return;
    }

    let Some(config) = config else {
        return;
    };

    if !config.game.app.debug_overlay {
        return;
    }

    commands.spawn((
        DebugOverlayText,
        Text::new("debug overlay initializing..."),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgb(0.92, 0.95, 0.97)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(12.0),
            ..default()
        },
        ZIndex(100),
    ));

    commands.spawn((
        KeybindOverlayText,
        Text::new(keybind_overlay_text()),
        TextFont {
            font_size: 15.0,
            ..default()
        },
        TextColor(Color::srgb(0.90, 0.94, 0.97)),
        BackgroundColor(Color::srgba(0.06, 0.08, 0.10, 0.82)),
        BorderColor::all(Color::srgba(0.60, 0.68, 0.74, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(162.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        if keybind_overlay.visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        },
        ZIndex(100),
    ));
}

fn reset_run_stats(mut run_stats: ResMut<DebugRunStats>) {
    run_stats.distance_m = 0.0;
}

fn update_run_stats(
    time: Res<Time>,
    config: Res<GameConfig>,
    mut run_stats: ResMut<DebugRunStats>,
) {
    run_stats.distance_m += run_stats.virtual_speed_mps * time.delta_secs();

    let mut cursor = 0.0_f32;
    let mut active_segment = config
        .segments
        .segment_sequence
        .first()
        .map(|segment| segment.id.clone())
        .unwrap_or_else(|| "n/a".to_string());

    for segment in &config.segments.segment_sequence {
        cursor += segment.length;
        if run_stats.distance_m <= cursor {
            active_segment = segment.id.clone();
            break;
        }
    }

    run_stats.active_segment_id = active_segment;
}

fn update_debug_overlay_text(
    diagnostics: Res<DiagnosticsStore>,
    run_stats: Res<DebugRunStats>,
    enemy_query: Query<(), With<EnemyDebugMarker>>,
    commentary: Option<Res<CommentaryStubState>>,
    mut overlay_query: Query<&mut Text, With<DebugOverlayText>>,
) {
    let Ok(mut text) = overlay_query.single_mut() else {
        return;
    };

    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|value| value.smoothed())
        .unwrap_or(0.0);

    let enemy_count = enemy_query.iter().count();

    let (queue_len, last_line) = match commentary {
        Some(state) => (
            state.queue.len(),
            if state.last_line.is_empty() {
                "n/a".to_string()
            } else {
                state.last_line.clone()
            },
        ),
        None => (0, "n/a".to_string()),
    };

    *text = Text::new(format!(
        "FPS: {fps:>5.1}\nDistance: {distance:>7.1}m\nActive Segment: {segment}\nEnemy Count: {enemy_count}\nCommentary Queue: {queue_len}\nLast Commentary: {last_line}\nHotkeys: H help | F5 reload config | J big jump | K kill | C crash",
        distance = run_stats.distance_m,
        segment = run_stats.active_segment_id,
    ));
}

fn toggle_keybind_overlay(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<KeybindOverlayState>,
    config: Option<Res<GameConfig>>,
) {
    let Some(config) = config else {
        return;
    };

    if !config.game.app.debug_overlay {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyH) {
        state.visible = !state.visible;
        info!(
            "Debug keybind panel {}.",
            if state.visible { "shown" } else { "hidden" }
        );
    }
}

fn sync_keybind_overlay_visibility(
    state: Res<KeybindOverlayState>,
    mut query: Query<&mut Visibility, With<KeybindOverlayText>>,
) {
    if !state.is_changed() {
        return;
    }

    let next_visibility = if state.visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    for mut visibility in &mut query {
        *visibility = next_visibility;
    }
}

fn keybind_overlay_text() -> &'static str {
    "Keybinds\n\
H - Toggle this panel\n\
F5 - Hot-reload config\n\
A / Right - Accelerate (planned)\n\
D / Left - Brake / reverse (planned)\n\
J - Queue BigJump commentary event\n\
K - Queue Kill commentary event\n\
C - Queue Crash commentary event\n\
Esc - Pause / resume\n\
R - Open results\n\
Enter - Pause -> results\n\
Space - Results -> new run\n\
Q - Quit from results"
}
