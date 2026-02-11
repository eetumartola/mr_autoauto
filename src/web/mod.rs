use crate::config::GameConfig;
use crate::states::GameState;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

const TOUCH_BUTTON_IDLE_ALPHA: f32 = 0.10;
const TOUCH_BUTTON_ACTIVE_ALPHA: f32 = 0.26;

pub struct WebSupportPlugin;

impl Plugin for WebSupportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VirtualControlState>()
            .init_resource::<AudioUnlockState>()
            .init_resource::<WebRuntimeState>()
            .add_systems(Startup, configure_primary_window_for_web)
            .add_systems(
                Update,
                sync_web_runtime_state.run_if(resource_exists::<GameConfig>),
            )
            .add_systems(
                OnEnter(GameState::InRun),
                (spawn_touch_controls_ui, spawn_audio_unlock_ui),
            )
            .add_systems(OnExit(GameState::InRun), cleanup_web_ui)
            .add_systems(
                Update,
                (
                    update_virtual_controls_from_pointer_and_touch,
                    unlock_audio_on_first_user_gesture,
                    update_touch_controls_ui,
                    update_audio_unlock_ui,
                )
                    .chain()
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            );
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct WebRuntimeState {
    pub active: bool,
    pub show_touch_controls: bool,
    pub require_audio_tap: bool,
}

impl Default for WebRuntimeState {
    fn default() -> Self {
        Self {
            active: cfg!(target_arch = "wasm32"),
            show_touch_controls: true,
            require_audio_tap: true,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct AudioUnlockState {
    pub unlocked: bool,
}

impl Default for AudioUnlockState {
    fn default() -> Self {
        Self {
            unlocked: !cfg!(target_arch = "wasm32"),
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct VirtualControlState {
    pub accelerate: bool,
    pub brake: bool,
    pub accelerate_just_pressed: bool,
    pub brake_just_pressed: bool,
}

#[derive(Component)]
struct WebTouchControlsRoot;

#[derive(Component)]
struct WebAudioUnlockRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum TouchControlLane {
    Brake,
    Accelerate,
}

#[derive(Component)]
struct TouchControlButton {
    lane: TouchControlLane,
}

#[cfg(target_arch = "wasm32")]
fn configure_primary_window_for_web(mut window_query: Query<&mut Window, With<PrimaryWindow>>) {
    let Ok(mut window) = window_query.single_mut() else {
        return;
    };
    window.fit_canvas_to_parent = true;
    window.prevent_default_event_handling = true;
}

#[cfg(not(target_arch = "wasm32"))]
fn configure_primary_window_for_web() {}

fn sync_web_runtime_state(
    config: Res<GameConfig>,
    mut runtime_state: ResMut<WebRuntimeState>,
    mut audio_unlock_state: ResMut<AudioUnlockState>,
) {
    runtime_state.active = config.is_web_mode_active();
    runtime_state.show_touch_controls = config.game.web.show_touch_controls;
    runtime_state.require_audio_tap = config.game.web.require_audio_tap;

    if !runtime_state.active || !runtime_state.require_audio_tap {
        audio_unlock_state.unlocked = true;
    }
}

fn spawn_touch_controls_ui(
    mut commands: Commands,
    existing_query: Query<Entity, With<WebTouchControlsRoot>>,
) {
    if !existing_query.is_empty() {
        return;
    }

    commands
        .spawn((
            Name::new("WebTouchControlsRoot"),
            WebTouchControlsRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                padding: UiRect::all(Val::Px(14.0)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexEnd,
                ..default()
            },
            Visibility::Hidden,
            ZIndex(260),
        ))
        .with_children(|parent| {
            for (lane, label) in [
                (TouchControlLane::Brake, "BRAKE"),
                (TouchControlLane::Accelerate, "ACCEL"),
            ] {
                parent
                    .spawn((
                        Name::new(format!("WebTouchButton{lane:?}")),
                        TouchControlButton { lane },
                        Node {
                            width: Val::Percent(48.5),
                            min_height: Val::Px(112.0),
                            border: UiRect::all(Val::Px(1.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.08, 0.12, 0.16, TOUCH_BUTTON_IDLE_ALPHA)),
                        BorderColor::all(Color::srgba(0.72, 0.80, 0.86, 0.38)),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new(label),
                            TextFont {
                                font_size: 24.0,
                                ..default()
                            },
                            TextColor(Color::srgba(0.92, 0.96, 0.99, 0.78)),
                        ));
                    });
            }
        });
}

fn spawn_audio_unlock_ui(
    mut commands: Commands,
    existing_query: Query<Entity, With<WebAudioUnlockRoot>>,
) {
    if !existing_query.is_empty() {
        return;
    }

    commands
        .spawn((
            Name::new("WebAudioUnlockRoot"),
            WebAudioUnlockRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(20.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            Visibility::Hidden,
            ZIndex(265),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Tap, click, or press any key to enable audio"),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.96, 0.99)),
                BackgroundColor(Color::srgba(0.05, 0.07, 0.10, 0.84)),
                Node {
                    padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BorderColor::all(Color::srgba(0.64, 0.74, 0.80, 0.90)),
            ));
        });
}

#[allow(clippy::type_complexity)]
fn cleanup_web_ui(
    mut commands: Commands,
    cleanup_query: Query<Entity, Or<(With<WebTouchControlsRoot>, With<WebAudioUnlockRoot>)>>,
) {
    for entity in &cleanup_query {
        commands.entity(entity).try_despawn();
    }
}

fn update_virtual_controls_from_pointer_and_touch(
    runtime_state: Res<WebRuntimeState>,
    touches: Res<Touches>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut controls: ResMut<VirtualControlState>,
    mut window_query: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    let prev_accelerate = controls.accelerate;
    let prev_brake = controls.brake;

    controls.accelerate = false;
    controls.brake = false;
    controls.accelerate_just_pressed = false;
    controls.brake_just_pressed = false;

    if !runtime_state.active || !runtime_state.show_touch_controls {
        return;
    }

    let Ok((window, mut cursor_options)) = window_query.single_mut() else {
        return;
    };

    let width = window.width().max(1.0);
    let split_x = width * 0.5;

    for touch in touches.iter() {
        if touch.position().x >= split_x {
            controls.accelerate = true;
        } else {
            controls.brake = true;
        }
    }

    if mouse_buttons.pressed(MouseButton::Left) {
        if let Some(cursor) = window.cursor_position() {
            if cursor.x >= split_x {
                controls.accelerate = true;
            } else {
                controls.brake = true;
            }
        }
    }

    controls.accelerate_just_pressed = controls.accelerate && !prev_accelerate;
    controls.brake_just_pressed = controls.brake && !prev_brake;

    if controls.accelerate || controls.brake {
        cursor_options.grab_mode = CursorGrabMode::Confined;
    } else {
        cursor_options.grab_mode = CursorGrabMode::None;
    }
}

fn unlock_audio_on_first_user_gesture(
    runtime_state: Res<WebRuntimeState>,
    touches: Res<Touches>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut audio_unlock_state: ResMut<AudioUnlockState>,
) {
    if !runtime_state.active || !runtime_state.require_audio_tap || audio_unlock_state.unlocked {
        return;
    }

    let touched = touches.iter_just_pressed().next().is_some();
    let clicked = mouse_buttons.just_pressed(MouseButton::Left);
    let keyed = keyboard.get_just_pressed().next().is_some();
    if touched || clicked || keyed {
        audio_unlock_state.unlocked = true;
        info!("Audio unlocked by first user gesture (web mode).");
    }
}

fn update_touch_controls_ui(
    runtime_state: Res<WebRuntimeState>,
    controls: Res<VirtualControlState>,
    mut root_query: Query<&mut Visibility, With<WebTouchControlsRoot>>,
    mut button_query: Query<(&TouchControlButton, &mut BackgroundColor)>,
) {
    let Ok(mut root_visibility) = root_query.single_mut() else {
        return;
    };

    if !runtime_state.active || !runtime_state.show_touch_controls {
        *root_visibility = Visibility::Hidden;
        return;
    }
    *root_visibility = Visibility::Inherited;

    for (button, mut background) in &mut button_query {
        let pressed = match button.lane {
            TouchControlLane::Brake => controls.brake,
            TouchControlLane::Accelerate => controls.accelerate,
        };
        let alpha = if pressed {
            TOUCH_BUTTON_ACTIVE_ALPHA
        } else {
            TOUCH_BUTTON_IDLE_ALPHA
        };
        *background = BackgroundColor(Color::srgba(0.10, 0.20, 0.28, alpha));
    }
}

fn update_audio_unlock_ui(
    runtime_state: Res<WebRuntimeState>,
    audio_unlock_state: Res<AudioUnlockState>,
    mut prompt_query: Query<&mut Visibility, With<WebAudioUnlockRoot>>,
) {
    let Ok(mut visibility) = prompt_query.single_mut() else {
        return;
    };

    let show_prompt =
        runtime_state.active && runtime_state.require_audio_tap && !audio_unlock_state.unlocked;
    *visibility = if show_prompt {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
}

pub fn audio_playback_allowed(
    config: &GameConfig,
    unlock_state: Option<&AudioUnlockState>,
) -> bool {
    if !config.is_web_mode_active() || !config.game.web.require_audio_tap {
        return true;
    }
    unlock_state.map(|state| state.unlocked).unwrap_or(false)
}

pub fn max_player_projectiles_for_platform(config: &GameConfig) -> usize {
    if config.is_web_mode_active() {
        config.game.web.max_player_projectiles.max(1)
    } else {
        usize::MAX
    }
}

pub fn max_enemy_projectiles_for_platform(config: &GameConfig) -> usize {
    if config.is_web_mode_active() {
        config.game.web.max_enemy_projectiles.max(1)
    } else {
        usize::MAX
    }
}

pub fn should_reduce_fx_for_platform(config: &GameConfig) -> bool {
    config.is_web_mode_active() && config.game.web.reduce_fx
}

pub fn should_disable_splats_for_platform(config: &GameConfig) -> bool {
    config.is_web_mode_active() && config.game.web.disable_splats
}
