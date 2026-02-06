use crate::config::GameConfig;
use crate::states::GameState;
use bevy::prelude::*;
use std::collections::VecDeque;

pub struct CommentaryStubPlugin;

impl Plugin for CommentaryStubPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommentaryStubState>()
            .add_systems(OnEnter(GameState::InRun), reset_commentary_stub)
            .add_systems(
                Update,
                (emit_stub_events_from_input, process_commentary_queue)
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            );
    }
}

#[derive(Debug, Clone)]
pub enum StubGameEvent {
    BigJump,
    Kill,
    Crash,
}

impl StubGameEvent {
    fn label(&self) -> &'static str {
        match self {
            Self::BigJump => "BigJump",
            Self::Kill => "Kill",
            Self::Crash => "Crash",
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct CommentaryStubState {
    pub queue: VecDeque<StubGameEvent>,
    pub last_line: String,
    pub recent_events: VecDeque<String>,
    last_emit_time_seconds: f64,
    fallback_cursor: usize,
}

impl Default for CommentaryStubState {
    fn default() -> Self {
        Self {
            queue: VecDeque::new(),
            last_line: String::new(),
            recent_events: VecDeque::new(),
            last_emit_time_seconds: 0.0,
            fallback_cursor: 0,
        }
    }
}

fn reset_commentary_stub(mut state: ResMut<CommentaryStubState>) {
    state.queue.clear();
    state.last_line.clear();
    state.recent_events.clear();
    state.last_emit_time_seconds = 0.0;
    state.fallback_cursor = 0;
}

fn emit_stub_events_from_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<CommentaryStubState>,
) {
    if keyboard.just_pressed(KeyCode::KeyJ) {
        push_event(&mut state, StubGameEvent::BigJump);
    }
    if keyboard.just_pressed(KeyCode::KeyK) {
        push_event(&mut state, StubGameEvent::Kill);
    }
    if keyboard.just_pressed(KeyCode::KeyC) {
        push_event(&mut state, StubGameEvent::Crash);
    }
}

fn process_commentary_queue(
    time: Res<Time>,
    config: Res<GameConfig>,
    mut state: ResMut<CommentaryStubState>,
) {
    let Some(event) = state.queue.front().cloned() else {
        return;
    };

    let cooldown = config.commentator.commentary.min_seconds_between_lines as f64;
    let now = time.elapsed_secs_f64();
    if now - state.last_emit_time_seconds < cooldown {
        return;
    }

    let fallback_line = next_fallback_line(&mut state, &config);
    let built_line = format!("[{}] {}", event.label(), fallback_line);

    state.last_line = built_line.clone();
    state.last_emit_time_seconds = now;

    state.queue.pop_front();
    state.recent_events.push_back(event.label().to_string());
    while state.recent_events.len() > 8 {
        state.recent_events.pop_front();
    }

    info!("Commentary stub emitted: {built_line}");
}

fn next_fallback_line(state: &mut CommentaryStubState, config: &GameConfig) -> String {
    if config.commentator.fallback.lines.is_empty() {
        return "No fallback commentary line configured.".to_string();
    }

    let index = state.fallback_cursor % config.commentator.fallback.lines.len();
    state.fallback_cursor = state.fallback_cursor.wrapping_add(1);
    config.commentator.fallback.lines[index].clone()
}

fn push_event(state: &mut CommentaryStubState, event: StubGameEvent) {
    state.queue.push_back(event.clone());
    info!(
        "Commentary stub queued event `{}` (queue size: {}).",
        event.label(),
        state.queue.len()
    );
}
