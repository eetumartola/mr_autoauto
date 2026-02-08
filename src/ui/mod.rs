use crate::config::GameConfig;
use crate::gameplay::upgrades::UpgradeAppliedEvent;
use crate::gameplay::vehicle::{
    PlayerHealth, PlayerVehicle, VehicleStuntMetrics, VehicleTelemetry,
};
use crate::states::{GameState, RunSummary};
use bevy::prelude::*;
use std::collections::HashMap;

const HUD_PANEL_Z_INDEX: i32 = 190;
const HUD_PANEL_BG: Color = Color::srgba(0.06, 0.09, 0.12, 0.86);
const HUD_PANEL_BORDER: Color = Color::srgba(0.58, 0.68, 0.76, 0.92);
const HUD_TEXT_PRIMARY: Color = Color::srgb(0.94, 0.97, 1.0);
const HUD_TEXT_MUTED: Color = Color::srgb(0.76, 0.83, 0.9);
const HUD_HEALTH_BAR_WIDTH_PX: f32 = 260.0;

pub struct GameHudPlugin;

impl Plugin for GameHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudUpgradeState>()
            .add_systems(
                OnEnter(GameState::InRun),
                (reset_hud_upgrade_state, spawn_game_hud),
            )
            .add_systems(OnExit(GameState::InRun), cleanup_game_hud)
            .add_systems(
                Update,
                (track_upgrade_applies_for_hud, update_game_hud)
                    .chain()
                    .run_if(in_state(GameState::InRun)),
            );
    }
}

#[derive(Component)]
struct GameHudRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum HudTextKind {
    Score,
    Health,
    CoreStats,
    StuntStats,
    Segment,
    Upgrades,
}

#[derive(Component)]
struct HudHealthFill;

#[derive(Resource, Debug, Clone, Default)]
struct HudUpgradeState {
    by_id: HashMap<String, HudUpgradeEntry>,
}

#[derive(Debug, Clone)]
struct HudUpgradeEntry {
    label: String,
    stack: u32,
    effect_summary: String,
}

fn reset_hud_upgrade_state(mut state: ResMut<HudUpgradeState>) {
    state.by_id.clear();
}

fn spawn_game_hud(mut commands: Commands, existing_hud: Query<Entity, With<GameHudRoot>>) {
    if !existing_hud.is_empty() {
        return;
    }

    commands
        .spawn((
            Name::new("GameHudRoot"),
            GameHudRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                right: Val::Px(12.0),
                top: Val::Px(10.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            ZIndex(HUD_PANEL_Z_INDEX),
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("GameHudMainPanel"),
                Node {
                    width: Val::Px(470.0),
                    max_width: Val::Percent(68.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(12.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(HUD_PANEL_BG),
                BorderColor::all(HUD_PANEL_BORDER),
            ))
            .with_children(|panel| {
                panel.spawn((
                    HudTextKind::Score,
                    Text::new("SCORE 0"),
                    TextFont {
                        font_size: 30.0,
                        ..default()
                    },
                    TextColor(HUD_TEXT_PRIMARY),
                ));
                panel.spawn((
                    HudTextKind::Health,
                    Text::new("HP 0 / 0"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(HUD_TEXT_PRIMARY),
                ));
                panel
                    .spawn((
                        Name::new("HudHealthBar"),
                        Node {
                            width: Val::Px(HUD_HEALTH_BAR_WIDTH_PX),
                            height: Val::Px(14.0),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.02, 0.03, 0.04, 0.84)),
                        BorderColor::all(Color::srgba(0.56, 0.64, 0.70, 0.9)),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            HudHealthFill,
                            Node {
                                width: Val::Px(HUD_HEALTH_BAR_WIDTH_PX),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgb(0.38, 0.90, 0.34)),
                        ));
                    });
                panel.spawn((
                    HudTextKind::CoreStats,
                    Text::new("Distance 0.0m | Speed 0.0 m/s | Kills 0 | Coins 0"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(HUD_TEXT_PRIMARY),
                ));
                panel.spawn((
                    HudTextKind::StuntStats,
                    Text::new("Airtime 0.00s | Wheelie 0.00s | Flips 0"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(HUD_TEXT_MUTED),
                ));
                panel.spawn((
                    HudTextKind::Segment,
                    Text::new("Segment: n/a"),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(HUD_TEXT_MUTED),
                ));
            });

            root.spawn((
                Name::new("GameHudUpgradePanel"),
                Node {
                    width: Val::Px(380.0),
                    max_width: Val::Percent(45.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.08, 0.1, 0.8)),
                BorderColor::all(Color::srgba(0.45, 0.56, 0.64, 0.85)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("UPGRADES"),
                    TextFont {
                        font_size: 18.0,
                        ..default()
                    },
                    TextColor(HUD_TEXT_PRIMARY),
                ));
                panel.spawn((
                    HudTextKind::Upgrades,
                    Text::new("No upgrades yet.\nCollect coins to trigger offers."),
                    TextFont {
                        font_size: 15.0,
                        ..default()
                    },
                    TextColor(HUD_TEXT_MUTED),
                ));
            });
        });
}

fn cleanup_game_hud(mut commands: Commands, hud_query: Query<Entity, With<GameHudRoot>>) {
    for entity in &hud_query {
        commands.entity(entity).try_despawn();
    }
}

fn track_upgrade_applies_for_hud(
    mut events: MessageReader<UpgradeAppliedEvent>,
    mut state: ResMut<HudUpgradeState>,
) {
    for event in events.read() {
        state.by_id.insert(
            event.upgrade_id.clone(),
            HudUpgradeEntry {
                label: event.label.clone(),
                stack: event.stack,
                effect_summary: event.effect_summary.clone(),
            },
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn update_game_hud(
    config: Option<Res<GameConfig>>,
    telemetry: Option<Res<VehicleTelemetry>>,
    stunts: Option<Res<VehicleStuntMetrics>>,
    run_summary: Option<Res<RunSummary>>,
    player_query: Query<&PlayerHealth, With<PlayerVehicle>>,
    upgrades: Res<HudUpgradeState>,
    mut text_query: Query<(&HudTextKind, &mut Text)>,
    mut health_fill_query: Query<(&mut Node, &mut BackgroundColor), With<HudHealthFill>>,
) {
    let (distance_m, speed_mps) = telemetry
        .map(|telemetry| (telemetry.distance_m.max(0.0), telemetry.speed_mps))
        .unwrap_or((0.0, 0.0));

    let (score, kills, coins) = run_summary
        .map(|summary| (summary.score, summary.kill_count, summary.coin_pickup_count))
        .unwrap_or((0, 0, 0));

    let (airtime_total_s, wheelie_total_s, flip_count, airtime_best_s, wheelie_best_s) = stunts
        .map(|metrics| {
            (
                metrics.airtime_total_s.max(0.0),
                metrics.wheelie_total_s.max(0.0),
                metrics.flip_count,
                metrics.airtime_best_s.max(0.0),
                metrics.wheelie_best_s.max(0.0),
            )
        })
        .unwrap_or((0.0, 0.0, 0, 0.0, 0.0));

    let (hp_current, hp_max) = player_query
        .single()
        .map(|health| (health.current.max(0.0), health.max.max(1.0)))
        .unwrap_or((0.0, 1.0));
    let hp_fraction = (hp_current / hp_max).clamp(0.0, 1.0);

    if let Ok((mut bar_node, mut bar_color)) = health_fill_query.single_mut() {
        bar_node.width = Val::Px(HUD_HEALTH_BAR_WIDTH_PX * hp_fraction);
        let red = (1.0 - hp_fraction).clamp(0.0, 1.0);
        let green = (0.25 + hp_fraction * 0.75).clamp(0.0, 1.0);
        *bar_color = BackgroundColor(Color::srgb(red, green, 0.2));
    }

    let active_segment = config
        .as_ref()
        .map(|cfg| resolve_active_segment_id(distance_m, cfg))
        .unwrap_or_else(|| "n/a".to_string());
    let next_upgrade_remaining = config
        .as_ref()
        .map(|cfg| {
            let coins_per_offer = cfg.game.run_upgrades.coins_per_offer.max(1);
            let next_threshold = ((coins / coins_per_offer) + 1).saturating_mul(coins_per_offer);
            next_threshold.saturating_sub(coins)
        })
        .unwrap_or(0);

    let upgrades_text = build_upgrade_summary(&upgrades.by_id);

    for (kind, mut text) in &mut text_query {
        match kind {
            HudTextKind::Score => {
                *text = Text::new(format!("SCORE {score}"));
            }
            HudTextKind::Health => {
                *text = Text::new(format!("HP {hp_current:.0} / {hp_max:.0}"));
            }
            HudTextKind::CoreStats => {
                *text = Text::new(format!(
                    "Distance {distance_m:.1} m | Speed {speed_mps:.1} m/s | Kills {kills} | Coins {coins} | Next upgrade in {next_upgrade_remaining}"
                ));
            }
            HudTextKind::StuntStats => {
                *text = Text::new(format!(
                    "Airtime {airtime_total_s:.2}s (best {airtime_best_s:.2}) | Wheelie {wheelie_total_s:.2}s (best {wheelie_best_s:.2}) | Flips {flip_count}"
                ));
            }
            HudTextKind::Segment => {
                *text = Text::new(format!("Segment: {active_segment}"));
            }
            HudTextKind::Upgrades => {
                *text = Text::new(upgrades_text.clone());
            }
        }
    }
}

fn build_upgrade_summary(upgrades_by_id: &HashMap<String, HudUpgradeEntry>) -> String {
    if upgrades_by_id.is_empty() {
        return "No upgrades yet.\nCollect coins to trigger offers.".to_string();
    }

    let mut lines = Vec::<String>::new();
    let mut entries = upgrades_by_id
        .iter()
        .map(|(id, entry)| {
            (
                id.clone(),
                entry.label.clone(),
                entry.stack,
                entry.effect_summary.clone(),
            )
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    for (_id, label, stack, summary) in entries {
        lines.push(format!("{label} x{stack} ({summary})"));
    }
    lines.join("\n")
}

fn resolve_active_segment_id(distance_m: f32, config: &GameConfig) -> String {
    config
        .active_segment_id_for_distance(distance_m)
        .unwrap_or("n/a")
        .to_string()
}
