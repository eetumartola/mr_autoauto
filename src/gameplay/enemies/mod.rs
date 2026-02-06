use crate::config::{EnemyTypeConfig, GameConfig};
use crate::debug::EnemyDebugMarker;
use crate::gameplay::vehicle::PlayerVehicle;
use crate::states::GameState;
use bevy::prelude::*;
use std::f32::consts::TAU;

const ENEMY_SPAWN_START_AHEAD_M: f32 = 32.0;
const ENEMY_SPAWN_SPACING_M: f32 = 16.0;
const ENEMY_DESPAWN_BEHIND_M: f32 = 48.0;
const ENEMY_DESPAWN_AHEAD_M: f32 = 220.0;
const GROUND_FOLLOW_SNAP_RATE: f32 = 14.0;
const ENEMY_HP_BAR_OFFSET_Y_M: f32 = 1.2;
const ENEMY_HP_BAR_BG_WIDTH_M: f32 = 2.0;
const ENEMY_HP_BAR_BG_HEIGHT_M: f32 = 0.26;
const ENEMY_HP_BAR_FILL_HEIGHT_M: f32 = 0.16;
const ENEMY_HP_BAR_Z_M: f32 = 0.9;
const ENEMY_HIT_FLASH_DURATION_S: f32 = 0.12;

pub struct EnemyGameplayPlugin;

impl Plugin for EnemyGameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnemyBootstrapState>()
            .add_systems(OnEnter(GameState::InRun), reset_enemy_bootstrap)
            .add_systems(
                Update,
                (
                    spawn_bootstrap_enemies,
                    update_enemy_behaviors,
                    update_enemy_hit_flash_effects,
                    update_enemy_health_bars,
                    despawn_far_enemies,
                    rearm_bootstrap_when_empty,
                )
                    .chain()
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            );
    }
}

#[derive(Component)]
pub struct Enemy;

#[derive(Component, Debug, Clone)]
pub struct EnemyHitbox {
    pub radius_m: f32,
}

#[derive(Component, Debug, Clone)]
pub struct EnemyTypeId(pub String);

#[derive(Component, Debug, Clone, Copy)]
pub struct EnemyHealth {
    pub current: f32,
    pub max: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct EnemyHitFlash {
    pub remaining_s: f32,
}

#[derive(Component, Debug, Clone, Copy)]
struct EnemyBaseColor(pub Color);

#[derive(Component, Debug, Clone, Copy)]
struct EnemyHpBarBackground {
    owner: Entity,
}

#[derive(Component, Debug, Clone, Copy)]
struct EnemyHpBarFill {
    owner: Entity,
    max_width_m: f32,
}

#[derive(Component, Debug, Clone, Copy)]
struct EnemyMotion {
    base_speed_mps: f32,
}

#[derive(Component, Debug, Clone, Copy)]
struct EnemyBehavior {
    kind: EnemyBehaviorKind,
    base_altitude_m: f32,
    hover_amplitude_m: f32,
    hover_frequency_hz: f32,
    charge_speed_multiplier: f32,
    phase_offset_rad: f32,
    elapsed_s: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnemyBehaviorKind {
    Walker,
    Flier,
    Turret,
    Charger,
}

#[derive(Resource, Debug, Default)]
struct EnemyBootstrapState {
    seeded: bool,
    wave_counter: u32,
}

fn reset_enemy_bootstrap(mut bootstrap: ResMut<EnemyBootstrapState>) {
    bootstrap.seeded = false;
}

fn spawn_bootstrap_enemies(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut bootstrap: ResMut<EnemyBootstrapState>,
    player_query: Query<&Transform, With<PlayerVehicle>>,
) {
    if bootstrap.seeded {
        return;
    }

    let Ok(player_transform) = player_query.single() else {
        return;
    };

    if config.enemy_types.enemy_types.is_empty() {
        return;
    }

    for (index, enemy_cfg) in config.enemy_types.enemy_types.iter().enumerate() {
        let spawn_x = player_transform.translation.x
            + ENEMY_SPAWN_START_AHEAD_M
            + (index as f32 * ENEMY_SPAWN_SPACING_M);
        spawn_enemy_instance(
            &mut commands,
            &config,
            enemy_cfg,
            spawn_x,
            bootstrap.wave_counter + index as u32,
        );
    }

    bootstrap.wave_counter = bootstrap
        .wave_counter
        .saturating_add(config.enemy_types.enemy_types.len() as u32);
    bootstrap.seeded = true;
}

fn spawn_enemy_instance(
    commands: &mut Commands,
    config: &GameConfig,
    enemy_cfg: &EnemyTypeConfig,
    spawn_x: f32,
    sequence: u32,
) {
    let behavior_kind = behavior_kind_from_config(enemy_cfg.behavior.as_str());
    let body_size = body_size_for_behavior(behavior_kind, enemy_cfg.hitbox_radius);
    let body_color = color_for_behavior(behavior_kind);
    let ground_y = terrain_height_at_x(config, spawn_x) + enemy_cfg.hitbox_radius.max(0.15);
    let phase_offset = (sequence as f32 * 0.37).rem_euclid(1.0) * TAU;

    let base_altitude = match behavior_kind {
        EnemyBehaviorKind::Flier => ground_y + enemy_cfg.hover_amplitude.max(0.5) + 1.6,
        _ => ground_y,
    };

    let start_y = match behavior_kind {
        EnemyBehaviorKind::Flier => {
            base_altitude + phase_offset.sin() * enemy_cfg.hover_amplitude.max(0.5)
        }
        _ => ground_y,
    };

    let enemy_entity = commands
        .spawn((
            Name::new(format!("Enemy/{}", enemy_cfg.id)),
            Enemy,
            EnemyDebugMarker,
            EnemyTypeId(enemy_cfg.id.clone()),
            EnemyBaseColor(body_color),
            EnemyHealth {
                current: enemy_cfg.health,
                max: enemy_cfg.health,
            },
            EnemyHitbox {
                radius_m: enemy_cfg.hitbox_radius,
            },
            EnemyMotion {
                base_speed_mps: enemy_cfg.speed,
            },
            EnemyBehavior {
                kind: behavior_kind,
                base_altitude_m: base_altitude,
                hover_amplitude_m: enemy_cfg.hover_amplitude.max(0.5),
                hover_frequency_hz: enemy_cfg.hover_frequency.max(0.4),
                charge_speed_multiplier: enemy_cfg.charge_speed_multiplier.max(1.2),
                phase_offset_rad: phase_offset,
                elapsed_s: 0.0,
            },
            Sprite::from_color(body_color, body_size),
            Transform::from_xyz(spawn_x, start_y, 8.0),
        ))
        .id();

    commands.entity(enemy_entity).with_children(|parent| {
        parent.spawn((
            Name::new("EnemyHpBarBackground"),
            EnemyHpBarBackground {
                owner: enemy_entity,
            },
            Sprite::from_color(
                Color::srgba(0.06, 0.08, 0.10, 0.85),
                Vec2::new(ENEMY_HP_BAR_BG_WIDTH_M, ENEMY_HP_BAR_BG_HEIGHT_M),
            ),
            Transform::from_xyz(0.0, ENEMY_HP_BAR_OFFSET_Y_M, ENEMY_HP_BAR_Z_M),
            Visibility::Hidden,
        ));

        parent.spawn((
            Name::new("EnemyHpBarFill"),
            EnemyHpBarFill {
                owner: enemy_entity,
                max_width_m: ENEMY_HP_BAR_BG_WIDTH_M - 0.04,
            },
            Sprite::from_color(
                Color::srgba(0.12, 0.86, 0.22, 0.92),
                Vec2::new(ENEMY_HP_BAR_BG_WIDTH_M - 0.04, ENEMY_HP_BAR_FILL_HEIGHT_M),
            ),
            Transform::from_xyz(0.0, ENEMY_HP_BAR_OFFSET_Y_M, ENEMY_HP_BAR_Z_M + 0.01),
            Visibility::Hidden,
        ));
    });
}

#[allow(clippy::type_complexity)]
fn update_enemy_behaviors(
    time: Res<Time>,
    config: Res<GameConfig>,
    player_query: Query<&Transform, (With<PlayerVehicle>, Without<Enemy>)>,
    mut enemy_query: Query<
        (
            &mut Transform,
            &mut EnemyBehavior,
            &EnemyMotion,
            &EnemyHitbox,
            &EnemyTypeId,
        ),
        (With<Enemy>, Without<PlayerVehicle>),
    >,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_x = player_transform.translation.x;
    let dt = time.delta_secs();

    for (mut transform, mut behavior, motion, hitbox, enemy_type_id) in &mut enemy_query {
        let ground_offset = hitbox.radius_m.max(0.15);
        behavior.elapsed_s += dt;

        match behavior.kind {
            EnemyBehaviorKind::Walker => {
                transform.translation.x -= motion.base_speed_mps * dt;
                let ground_y =
                    terrain_height_at_x(&config, transform.translation.x) + ground_offset;
                transform.translation.y = transform
                    .translation
                    .y
                    .lerp(ground_y, (GROUND_FOLLOW_SNAP_RATE * dt).clamp(0.0, 1.0));
            }
            EnemyBehaviorKind::Flier => {
                transform.translation.x -= motion.base_speed_mps * 0.82 * dt;
                let hover = (behavior.elapsed_s * behavior.hover_frequency_hz * TAU
                    + behavior.phase_offset_rad)
                    .sin()
                    * behavior.hover_amplitude_m;
                let target_y = behavior.base_altitude_m + hover;
                transform.translation.y = transform
                    .translation
                    .y
                    .lerp(target_y, (7.0 * dt).clamp(0.0, 1.0));
            }
            EnemyBehaviorKind::Turret => {
                transform.translation.x -= motion.base_speed_mps * 0.06 * dt;
                let ground_y =
                    terrain_height_at_x(&config, transform.translation.x) + ground_offset;
                transform.translation.y = transform
                    .translation
                    .y
                    .lerp(ground_y, (GROUND_FOLLOW_SNAP_RATE * dt).clamp(0.0, 1.0));
            }
            EnemyBehaviorKind::Charger => {
                let distance_to_player = transform.translation.x - player_x;
                let charge_multiplier = if distance_to_player <= 20.0 {
                    behavior.charge_speed_multiplier
                } else {
                    0.55
                };
                transform.translation.x -= motion.base_speed_mps * charge_multiplier * dt;
                let ground_y =
                    terrain_height_at_x(&config, transform.translation.x) + ground_offset;
                transform.translation.y = transform
                    .translation
                    .y
                    .lerp(ground_y, (GROUND_FOLLOW_SNAP_RATE * dt).clamp(0.0, 1.0));
            }
        }

        if enemy_type_id.0.is_empty() {
            warn!("Encountered enemy with empty type id.");
        }
    }
}

fn despawn_far_enemies(
    mut commands: Commands,
    player_query: Query<&Transform, With<PlayerVehicle>>,
    enemy_query: Query<(Entity, &Transform), With<Enemy>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let min_x = player_transform.translation.x - ENEMY_DESPAWN_BEHIND_M;
    let max_x = player_transform.translation.x + ENEMY_DESPAWN_AHEAD_M;

    for (entity, transform) in &enemy_query {
        if transform.translation.x < min_x || transform.translation.x > max_x {
            commands.entity(entity).despawn();
        }
    }
}

fn rearm_bootstrap_when_empty(
    mut bootstrap: ResMut<EnemyBootstrapState>,
    enemy_query: Query<Entity, With<Enemy>>,
) {
    if bootstrap.seeded && enemy_query.is_empty() {
        bootstrap.seeded = false;
    }
}

fn update_enemy_hit_flash_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut enemy_query: Query<
        (
            Entity,
            &EnemyBaseColor,
            &mut Sprite,
            Option<&mut EnemyHitFlash>,
        ),
        With<Enemy>,
    >,
) {
    let dt = time.delta_secs();

    for (entity, base_color, mut sprite, hit_flash) in &mut enemy_query {
        if let Some(mut flash) = hit_flash {
            flash.remaining_s -= dt;
            sprite.color = Color::srgba(1.0, 1.0, 1.0, 1.0);

            if flash.remaining_s <= 0.0 {
                commands.entity(entity).remove::<EnemyHitFlash>();
                sprite.color = base_color.0;
            }
        } else {
            sprite.color = base_color.0;
        }
    }
}

fn update_enemy_health_bars(
    enemy_health_query: Query<&EnemyHealth, With<Enemy>>,
    mut hp_bg_query: Query<(&EnemyHpBarBackground, &mut Visibility)>,
    mut hp_fill_query: Query<
        (
            &EnemyHpBarFill,
            &mut Transform,
            &mut Sprite,
            &mut Visibility,
        ),
        Without<EnemyHpBarBackground>,
    >,
) {
    for (bar_bg, mut visibility) in &mut hp_bg_query {
        let Ok(enemy_health) = enemy_health_query.get(bar_bg.owner) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let health_fraction = (enemy_health.current / enemy_health.max).clamp(0.0, 1.0);
        *visibility = if health_fraction < 0.999 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }

    for (bar_fill, mut transform, mut sprite, mut visibility) in &mut hp_fill_query {
        let Ok(enemy_health) = enemy_health_query.get(bar_fill.owner) else {
            *visibility = Visibility::Hidden;
            continue;
        };

        let health_fraction = (enemy_health.current / enemy_health.max).clamp(0.0, 1.0);
        if health_fraction < 0.999 {
            *visibility = Visibility::Inherited;
            transform.scale.x = health_fraction.max(0.001);
            transform.translation.x = -((1.0 - health_fraction) * bar_fill.max_width_m * 0.5);

            let red = 0.92 - (0.78 * health_fraction);
            let green = 0.18 + (0.66 * health_fraction);
            sprite.color = Color::srgba(red, green, 0.18, 0.94);
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

fn body_size_for_behavior(kind: EnemyBehaviorKind, hitbox_radius: f32) -> Vec2 {
    let r = hitbox_radius.max(0.15);
    match kind {
        EnemyBehaviorKind::Walker => Vec2::new(r * 2.3, r * 1.9),
        EnemyBehaviorKind::Flier => Vec2::new(r * 2.0, r * 1.6),
        EnemyBehaviorKind::Turret => Vec2::new(r * 2.5, r * 2.2),
        EnemyBehaviorKind::Charger => Vec2::new(r * 2.7, r * 1.9),
    }
}

fn color_for_behavior(kind: EnemyBehaviorKind) -> Color {
    match kind {
        EnemyBehaviorKind::Walker => Color::srgb(0.86, 0.57, 0.36),
        EnemyBehaviorKind::Flier => Color::srgb(0.54, 0.74, 0.92),
        EnemyBehaviorKind::Turret => Color::srgb(0.81, 0.54, 0.84),
        EnemyBehaviorKind::Charger => Color::srgb(0.90, 0.41, 0.41),
    }
}

pub fn enemy_hit_flash_duration_seconds() -> f32 {
    ENEMY_HIT_FLASH_DURATION_S
}

fn behavior_kind_from_config(raw: &str) -> EnemyBehaviorKind {
    match raw {
        "flier" => EnemyBehaviorKind::Flier,
        "turret" => EnemyBehaviorKind::Turret,
        "charger" => EnemyBehaviorKind::Charger,
        _ => EnemyBehaviorKind::Walker,
    }
}

fn terrain_height_at_x(config: &GameConfig, x: f32) -> f32 {
    let terrain = &config.game.terrain;
    terrain.base_height
        + (x * terrain.ramp_slope)
        + (x * terrain.wave_a_frequency).sin() * terrain.wave_a_amplitude
        + (x * terrain.wave_b_frequency).sin() * terrain.wave_b_amplitude
}
