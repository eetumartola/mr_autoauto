use crate::config::{GameConfig, RunUpgradeEffectKind, RunUpgradeOptionConfig};
use crate::gameplay::pickups::{PickupCollectedEvent, PickupKind};
use crate::gameplay::vehicle::{PlayerHealth, PlayerVehicle};
use crate::states::GameState;
use bevy::prelude::*;
use bevy::time::Virtual;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_VISIBLE_UPGRADE_CHOICES: usize = 2;

pub struct UpgradeGameplayPlugin;

impl Plugin for UpgradeGameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UpgradeProgressState>()
            .add_message::<UpgradeAppliedEvent>()
            .add_systems(
                OnEnter(GameState::InRun),
                (
                    reset_upgrade_progress_state,
                    spawn_upgrade_offer_ui,
                    resume_game_time_for_upgrades,
                ),
            )
            .add_systems(
                OnExit(GameState::InRun),
                (cleanup_upgrade_offer_ui, resume_game_time_for_upgrades),
            )
            .add_systems(
                Update,
                (
                    track_coin_progress_and_open_offer,
                    sync_upgrade_pause_time,
                    handle_upgrade_offer_input,
                    update_upgrade_offer_ui,
                )
                    .chain()
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            );
    }
}

#[derive(Message, Debug, Clone)]
#[allow(dead_code)]
pub struct UpgradeAppliedEvent {
    pub upgrade_id: String,
    pub label: String,
    pub stack: u32,
    pub effect_summary: String,
}

#[derive(Component)]
struct UpgradeOfferRoot;

#[derive(Component)]
struct UpgradeOfferHeaderText;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct UpgradeOfferCardSlot {
    slot: usize,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct UpgradeOfferCardTextSlot {
    slot: usize,
}

#[derive(Debug, Clone)]
struct UpgradeOfferChoice {
    id: String,
    label: String,
    effect: RunUpgradeEffectKind,
    value: f32,
    max_stacks: u32,
}

#[derive(Debug, Clone)]
struct PendingUpgradeOffer {
    choices: Vec<UpgradeOfferChoice>,
}

#[derive(Resource, Debug, Clone)]
struct UpgradeProgressState {
    total_coin_pickups: u32,
    next_offer_coin_threshold: u32,
    stack_counts: HashMap<String, u32>,
    pending_offer: Option<PendingUpgradeOffer>,
    wait_for_fresh_selection_input: bool,
    rng_state: u64,
}

impl Default for UpgradeProgressState {
    fn default() -> Self {
        Self {
            total_coin_pickups: 0,
            next_offer_coin_threshold: 5,
            stack_counts: HashMap::new(),
            pending_offer: None,
            wait_for_fresh_selection_input: false,
            rng_state: 0xD94A_4B53_9E13_BC87,
        }
    }
}

fn reset_upgrade_progress_state(mut state: ResMut<UpgradeProgressState>, config: Res<GameConfig>) {
    let mut next = UpgradeProgressState {
        next_offer_coin_threshold: config.game.run_upgrades.coins_per_offer.max(1),
        ..Default::default()
    };
    next.rng_state ^= unix_timestamp_seconds();
    *state = next;
}

fn resume_game_time_for_upgrades(mut time: ResMut<Time<Virtual>>) {
    time.set_relative_speed(1.0);
}

fn spawn_upgrade_offer_ui(
    mut commands: Commands,
    existing_query: Query<Entity, With<UpgradeOfferRoot>>,
) {
    if !existing_query.is_empty() {
        return;
    }

    commands
        .spawn((
            Name::new("UpgradeOfferPanel"),
            UpgradeOfferRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.04, 0.05, 0.72)),
            Visibility::Hidden,
            ZIndex(210),
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Name::new("UpgradeOfferCardsPanel"),
                    Node {
                        width: Val::Percent(94.0),
                        max_width: Val::Px(1080.0),
                        padding: UiRect::all(Val::Px(18.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(14.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.07, 0.10, 0.13, 0.95)),
                    BorderColor::all(Color::srgba(0.64, 0.73, 0.80, 0.95)),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        UpgradeOfferHeaderText,
                        Text::new(""),
                        TextFont {
                            font_size: 24.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.98, 1.00)),
                    ));
                    panel
                        .spawn((
                            Name::new("UpgradeOfferCardsRow"),
                            Node {
                                width: Val::Percent(100.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Stretch,
                                column_gap: Val::Px(12.0),
                                ..default()
                            },
                        ))
                        .with_children(|cards_row| {
                            for slot in 0..MAX_VISIBLE_UPGRADE_CHOICES {
                                cards_row
                                    .spawn((
                                        Name::new("UpgradeOfferCard"),
                                        UpgradeOfferCardSlot { slot },
                                        Node {
                                            width: Val::Percent(48.0),
                                            min_height: Val::Px(188.0),
                                            padding: UiRect::all(Val::Px(12.0)),
                                            border: UiRect::all(Val::Px(1.0)),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgba(0.13, 0.17, 0.22, 0.98)),
                                        BorderColor::all(Color::srgba(0.56, 0.66, 0.74, 0.95)),
                                        Visibility::Hidden,
                                    ))
                                    .with_children(|card| {
                                        card.spawn((
                                            UpgradeOfferCardTextSlot { slot },
                                            Text::new(""),
                                            TextFont {
                                                font_size: 18.0,
                                                ..default()
                                            },
                                            TextColor(Color::srgb(0.93, 0.96, 1.00)),
                                        ));
                                    });
                            }
                        });
                });
        });
}

fn cleanup_upgrade_offer_ui(
    mut commands: Commands,
    root_query: Query<Entity, With<UpgradeOfferRoot>>,
) {
    for entity in &root_query {
        commands.entity(entity).try_despawn();
    }
}

fn track_coin_progress_and_open_offer(
    config: Res<GameConfig>,
    mut state: ResMut<UpgradeProgressState>,
    mut pickup_events: MessageReader<PickupCollectedEvent>,
) {
    for event in pickup_events.read() {
        if event.kind == PickupKind::Coin {
            state.total_coin_pickups = state.total_coin_pickups.saturating_add(1);
        }
    }

    let run_upgrade_cfg = &config.game.run_upgrades;
    let coins_per_offer = run_upgrade_cfg.coins_per_offer.max(1);

    while state.total_coin_pickups >= state.next_offer_coin_threshold {
        state.next_offer_coin_threshold = state
            .next_offer_coin_threshold
            .saturating_add(coins_per_offer);

        if state.pending_offer.is_some() {
            continue;
        }

        let stack_counts = state.stack_counts.clone();
        let Some(offer) = roll_upgrade_offer(run_upgrade_cfg, &stack_counts, &mut state.rng_state)
        else {
            continue;
        };

        info!(
            "Upgrade offer ready at {} coin pickups with {} options.",
            state.total_coin_pickups,
            offer.choices.len()
        );
        state.pending_offer = Some(offer);
        state.wait_for_fresh_selection_input = true;
        break;
    }
}

fn sync_upgrade_pause_time(state: Res<UpgradeProgressState>, mut time: ResMut<Time<Virtual>>) {
    let target_speed = if state.pending_offer.is_some() {
        0.0
    } else {
        1.0
    };
    if (time.relative_speed() - target_speed).abs() <= f32::EPSILON {
        return;
    }

    time.set_relative_speed(target_speed);
}

#[allow(clippy::too_many_arguments)]
fn handle_upgrade_offer_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<GameConfig>,
    mut state: ResMut<UpgradeProgressState>,
    mut player_query: Query<&mut PlayerHealth, With<PlayerVehicle>>,
    mut applied_events: MessageWriter<UpgradeAppliedEvent>,
) {
    let Some(offer) = state.pending_offer.clone() else {
        state.wait_for_fresh_selection_input = false;
        return;
    };

    if state.wait_for_fresh_selection_input {
        if selection_keys_held(&keyboard) {
            return;
        }
        state.wait_for_fresh_selection_input = false;
        return;
    }

    let Some(selected_index) = selected_upgrade_index_from_input(&keyboard, offer.choices.len())
    else {
        return;
    };
    let Some(choice) = offer.choices.get(selected_index).cloned() else {
        return;
    };

    let current_stacks = state.stack_counts.get(&choice.id).copied().unwrap_or(0);
    if current_stacks >= choice.max_stacks {
        return;
    }

    match apply_upgrade_choice(&choice, &mut config, &mut player_query) {
        Ok(effect_summary) => {
            let stack = current_stacks.saturating_add(1);
            state.stack_counts.insert(choice.id.clone(), stack);
            state.pending_offer = None;
            state.wait_for_fresh_selection_input = false;
            info!(
                "Applied upgrade `{}` (stack {stack}/{}): {}",
                choice.label, choice.max_stacks, effect_summary
            );
            applied_events.write(UpgradeAppliedEvent {
                upgrade_id: choice.id,
                label: choice.label,
                stack,
                effect_summary,
            });
        }
        Err(error) => {
            warn!("Failed to apply upgrade choice: {error}");
        }
    }
}

fn update_upgrade_offer_ui(
    state: Res<UpgradeProgressState>,
    mut root_query: Query<&mut Visibility, With<UpgradeOfferRoot>>,
    mut header_query: Query<&mut Text, With<UpgradeOfferHeaderText>>,
    mut card_query: Query<(&UpgradeOfferCardSlot, &mut Visibility), Without<UpgradeOfferRoot>>,
    mut card_text_query: Query<
        (&UpgradeOfferCardTextSlot, &mut Text),
        Without<UpgradeOfferHeaderText>,
    >,
) {
    let Ok(mut root_visibility) = root_query.single_mut() else {
        return;
    };
    let Ok(mut header) = header_query.single_mut() else {
        return;
    };

    let Some(offer) = state.pending_offer.as_ref() else {
        *root_visibility = Visibility::Hidden;
        return;
    };

    *root_visibility = Visibility::Inherited;
    *header = Text::new(format!(
        "Choose Upgrade (game paused)\nPress A/D or Left/Right    Coins: {}    Next Offer: {}",
        state.total_coin_pickups, state.next_offer_coin_threshold
    ));

    for (slot, mut visibility) in &mut card_query {
        *visibility = if slot.slot < offer.choices.len() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    for (slot, mut text) in &mut card_text_query {
        if let Some(choice) = offer.choices.get(slot.slot) {
            *text = Text::new(build_card_text(slot.slot, choice, &state.stack_counts));
        } else {
            *text = Text::new("");
        }
    }
}

fn build_card_text(
    slot: usize,
    choice: &UpgradeOfferChoice,
    stack_counts: &HashMap<String, u32>,
) -> String {
    let current_stacks = stack_counts.get(&choice.id).copied().unwrap_or(0);
    let slot_hint = if slot == 0 { "LEFT" } else { "RIGHT" };
    format!(
        "{slot_hint}\n\n{}\n{}\n\nStacks: {}/{}",
        choice.label,
        describe_effect(choice.effect, choice.value),
        current_stacks,
        choice.max_stacks
    )
}

fn roll_upgrade_offer(
    config: &crate::config::RunUpgradeConfig,
    stack_counts: &HashMap<String, u32>,
    seed: &mut u64,
) -> Option<PendingUpgradeOffer> {
    let mut eligible_options: Vec<&RunUpgradeOptionConfig> = config
        .options
        .iter()
        .filter(|option| stack_counts.get(&option.id).copied().unwrap_or(0) < option.max_stacks)
        .collect();

    let choice_count = config
        .choices_per_offer
        .min(MAX_VISIBLE_UPGRADE_CHOICES)
        .min(eligible_options.len());
    if choice_count == 0 {
        return None;
    }

    let mut choices = Vec::with_capacity(choice_count);
    while choices.len() < choice_count {
        let index = random_index(seed, eligible_options.len());
        let option = eligible_options.swap_remove(index);
        choices.push(UpgradeOfferChoice {
            id: option.id.clone(),
            label: option.label.clone(),
            effect: option.effect,
            value: option.value,
            max_stacks: option.max_stacks,
        });
    }

    Some(PendingUpgradeOffer { choices })
}

fn random_index(seed: &mut u64, len: usize) -> usize {
    if len <= 1 {
        return 0;
    }

    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 32) as usize) % len
}

fn apply_upgrade_choice(
    choice: &UpgradeOfferChoice,
    config: &mut GameConfig,
    player_query: &mut Query<&mut PlayerHealth, With<PlayerVehicle>>,
) -> Result<String, String> {
    match choice.effect {
        RunUpgradeEffectKind::HealthFlat => {
            let health_delta = choice.value.max(0.0);
            let Ok(mut health) = player_query.single_mut() else {
                return Err("player entity not found for health upgrade".to_string());
            };
            health.max += health_delta;
            health.current = (health.current + health_delta).min(health.max);
            Ok(format!("max health +{health_delta:.1}"))
        }
        RunUpgradeEffectKind::WeaponFireRatePercent => {
            let scale = 1.0 + choice.value.max(0.0);
            let default_vehicle_id = config.game.app.default_vehicle.clone();
            let default_weapon_id = config
                .vehicles_by_id
                .get(&default_vehicle_id)
                .ok_or_else(|| {
                    format!("default vehicle `{default_vehicle_id}` not found for upgrade")
                })?
                .default_weapon_id
                .clone();

            let new_fire_rate = {
                let weapon = config
                    .weapons_by_id
                    .get_mut(&default_weapon_id)
                    .ok_or_else(|| {
                        format!("default weapon `{default_weapon_id}` not found for upgrade")
                    })?;
                weapon.fire_rate = (weapon.fire_rate * scale).max(0.05);
                weapon.fire_rate
            };

            if let Some(weapon) = config
                .weapons
                .weapons
                .iter_mut()
                .find(|weapon| weapon.id == default_weapon_id)
            {
                weapon.fire_rate = new_fire_rate;
            }

            Ok(format!(
                "{default_weapon_id} fire rate -> {new_fire_rate:.3} shots/s"
            ))
        }
        RunUpgradeEffectKind::MissileFireRatePercent => {
            let scale = 1.0 + choice.value.max(0.0);
            let default_vehicle_id = config.game.app.default_vehicle.clone();

            let new_interval_seconds = {
                let vehicle = config
                    .vehicles_by_id
                    .get_mut(&default_vehicle_id)
                    .ok_or_else(|| {
                        format!("default vehicle `{default_vehicle_id}` not found for upgrade")
                    })?;
                vehicle.missile_fire_interval_seconds =
                    (vehicle.missile_fire_interval_seconds / scale).max(0.05);
                vehicle.missile_fire_interval_seconds
            };

            if let Some(vehicle) = config
                .vehicles
                .vehicles
                .iter_mut()
                .find(|vehicle| vehicle.id == default_vehicle_id)
            {
                vehicle.missile_fire_interval_seconds = new_interval_seconds;
            }

            Ok(format!(
                "missile fire interval -> {new_interval_seconds:.3}s"
            ))
        }
        RunUpgradeEffectKind::VehiclePowerPercent => {
            let scale = 1.0 + choice.value.max(0.0);
            let default_vehicle_id = config.game.app.default_vehicle.clone();

            let new_acceleration = {
                let vehicle = config
                    .vehicles_by_id
                    .get_mut(&default_vehicle_id)
                    .ok_or_else(|| {
                        format!("default vehicle `{default_vehicle_id}` not found for upgrade")
                    })?;
                vehicle.acceleration = (vehicle.acceleration * scale).max(0.01);
                vehicle.acceleration
            };

            if let Some(vehicle) = config
                .vehicles
                .vehicles
                .iter_mut()
                .find(|vehicle| vehicle.id == default_vehicle_id)
            {
                vehicle.acceleration = new_acceleration;
            }

            Ok(format!("acceleration -> {new_acceleration:.3}"))
        }
        RunUpgradeEffectKind::TurretConeDegreesFlat => {
            let cone_delta = choice.value.max(0.0);
            let default_vehicle_id = config.game.app.default_vehicle.clone();

            let new_cone_degrees = {
                let vehicle = config
                    .vehicles_by_id
                    .get_mut(&default_vehicle_id)
                    .ok_or_else(|| {
                        format!("default vehicle `{default_vehicle_id}` not found for upgrade")
                    })?;
                vehicle.turret_cone_degrees =
                    (vehicle.turret_cone_degrees + cone_delta).clamp(1.0, 180.0);
                vehicle.turret_cone_degrees
            };

            if let Some(vehicle) = config
                .vehicles
                .vehicles
                .iter_mut()
                .find(|vehicle| vehicle.id == default_vehicle_id)
            {
                vehicle.turret_cone_degrees = new_cone_degrees;
            }

            Ok(format!("turret cone -> {new_cone_degrees:.1} deg"))
        }
        RunUpgradeEffectKind::MissileTurnRatePercent => {
            let scale = 1.0 + choice.value.max(0.0);
            let default_vehicle_id = config.game.app.default_vehicle.clone();
            let secondary_weapon_id = config
                .vehicles_by_id
                .get(&default_vehicle_id)
                .ok_or_else(|| {
                    format!("default vehicle `{default_vehicle_id}` not found for upgrade")
                })?
                .secondary_weapon_id
                .clone()
                .ok_or_else(|| {
                    "default vehicle has no secondary weapon for missile turn upgrade".to_string()
                })?;

            let new_turn_rate_degrees = {
                let weapon = config
                    .weapons_by_id
                    .get_mut(&secondary_weapon_id)
                    .ok_or_else(|| {
                        format!("secondary weapon `{secondary_weapon_id}` not found for upgrade")
                    })?;
                weapon.homing_turn_rate_degrees =
                    (weapon.homing_turn_rate_degrees * scale).max(0.0);
                weapon.homing_turn_rate_degrees
            };

            if let Some(weapon) = config
                .weapons
                .weapons
                .iter_mut()
                .find(|weapon| weapon.id == secondary_weapon_id)
            {
                weapon.homing_turn_rate_degrees = new_turn_rate_degrees;
            }

            Ok(format!(
                "{secondary_weapon_id} turn rate -> {new_turn_rate_degrees:.3} deg/s"
            ))
        }
        RunUpgradeEffectKind::TurretRangePercent => {
            let scale = 1.0 + choice.value.max(0.0);
            let default_vehicle_id = config.game.app.default_vehicle.clone();

            let new_range_m = {
                let vehicle = config
                    .vehicles_by_id
                    .get_mut(&default_vehicle_id)
                    .ok_or_else(|| {
                        format!("default vehicle `{default_vehicle_id}` not found for upgrade")
                    })?;
                vehicle.turret_range_m = (vehicle.turret_range_m * scale).max(0.1);
                vehicle.turret_range_m
            };

            if let Some(vehicle) = config
                .vehicles
                .vehicles
                .iter_mut()
                .find(|vehicle| vehicle.id == default_vehicle_id)
            {
                vehicle.turret_range_m = new_range_m;
            }

            Ok(format!("turret range -> {new_range_m:.3} m"))
        }
    }
}

fn selected_upgrade_index_from_input(
    keyboard: &ButtonInput<KeyCode>,
    choice_count: usize,
) -> Option<usize> {
    if choice_count == 0 {
        return None;
    }

    if keyboard.just_pressed(KeyCode::KeyA) || keyboard.just_pressed(KeyCode::ArrowLeft) {
        return Some(0);
    }
    if choice_count >= 2
        && (keyboard.just_pressed(KeyCode::KeyD) || keyboard.just_pressed(KeyCode::ArrowRight))
    {
        return Some(1);
    }
    None
}

fn selection_keys_held(keyboard: &ButtonInput<KeyCode>) -> bool {
    keyboard.pressed(KeyCode::KeyA)
        || keyboard.pressed(KeyCode::ArrowLeft)
        || keyboard.pressed(KeyCode::KeyD)
        || keyboard.pressed(KeyCode::ArrowRight)
}

fn describe_effect(effect: RunUpgradeEffectKind, value: f32) -> String {
    match effect {
        RunUpgradeEffectKind::HealthFlat => format!("+{value:.1} hp"),
        RunUpgradeEffectKind::WeaponFireRatePercent => {
            format!("gun fire rate +{:.0}%", value.max(0.0) * 100.0)
        }
        RunUpgradeEffectKind::MissileFireRatePercent => {
            format!("missile fire rate +{:.0}%", value.max(0.0) * 100.0)
        }
        RunUpgradeEffectKind::VehiclePowerPercent => {
            format!("car power +{:.0}%", value.max(0.0) * 100.0)
        }
        RunUpgradeEffectKind::TurretConeDegreesFlat => {
            format!("targeting cone +{:.1} deg", value.max(0.0))
        }
        RunUpgradeEffectKind::MissileTurnRatePercent => {
            format!("missile turn speed +{:.0}%", value.max(0.0) * 100.0)
        }
        RunUpgradeEffectKind::TurretRangePercent => {
            format!("targeting range +{:.0}%", value.max(0.0) * 100.0)
        }
    }
}

fn unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
