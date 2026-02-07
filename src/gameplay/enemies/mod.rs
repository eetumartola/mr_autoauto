use crate::config::{EnemyTypeConfig, GameConfig, WeaponConfig};
use crate::debug::EnemyDebugMarker;
use crate::gameplay::vehicle::{PlayerHealth, PlayerVehicle};
use crate::states::GameState;
use bevy::prelude::*;
use std::collections::HashMap;
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
const ENEMY_ATTACK_RANGE_M: f32 = 38.0;
const ENEMY_BOMBER_DROP_RANGE_M: f32 = 8.5;
const ENEMY_FLIER_ARC_ANGLE_RAD: f32 = 0.28;
const ENEMY_CHARGER_SPREAD_HALF_ANGLE_RAD: f32 = 0.16;
const ENEMY_BOMBER_ALTITUDE_SCALE: f32 = 1.2;
const ENEMY_BOMBER_ALTITUDE_DEFAULT_M: f32 = 8.0;
const ENEMY_PROJECTILE_Z_M: f32 = 2.0;
const ENEMY_BULLET_LENGTH_M: f32 = 0.42;
const ENEMY_BULLET_THICKNESS_M: f32 = 0.10;
const ENEMY_MISSILE_LENGTH_M: f32 = 0.72;
const ENEMY_MISSILE_THICKNESS_M: f32 = 0.16;
const ENEMY_BOMB_LENGTH_M: f32 = 0.62;
const ENEMY_BOMB_THICKNESS_M: f32 = 0.62;
const ENEMY_PROJECTILE_ARC_GRAVITY_SCALE: f32 = 0.6;
const PLAYER_CONTACT_HIT_RADIUS_M: f32 = 1.45;
const PLAYER_PROJECTILE_HIT_RADIUS_M: f32 = 1.25;
const MIN_ENEMY_FIRE_RATE_HZ: f32 = 0.05;
const MIN_ENEMY_FIRE_COOLDOWN_S: f32 = 0.12;
const ENEMY_EXTERNAL_VELOCITY_DAMPING: f32 = 3.8;
const PLAYER_COLLISION_RADIUS_M: f32 = 1.4;
const PLAYER_COLLISION_MASS: f32 = 5.0;
const ENEMY_COLLISION_IMPULSE_GAIN: f32 = 4.4;
const ENEMY_PLAYER_COLLISION_IMPULSE_GAIN: f32 = 5.2;
const ENEMY_COLLISION_SEPARATION_BIAS_M: f32 = 0.02;
const ENEMY_MASS_PER_RADIUS_SQUARED: f32 = 18.0;
const ENEMY_MIN_MASS: f32 = 2.4;

pub struct EnemyGameplayPlugin;

impl Plugin for EnemyGameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnemyBootstrapState>()
            .add_systems(OnEnter(GameState::InRun), reset_enemy_bootstrap)
            .add_systems(OnExit(GameState::InRun), cleanup_enemy_run_entities)
            .add_systems(
                Update,
                (
                    spawn_bootstrap_enemies,
                    update_enemy_behaviors,
                    resolve_enemy_body_collisions,
                    fire_enemy_projectiles,
                    simulate_enemy_projectiles,
                    resolve_enemy_projectile_hits_player,
                    apply_enemy_contact_damage_to_player,
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

#[derive(Component, Debug, Clone, Copy)]
struct EnemyDynamics {
    external_velocity_mps: Vec2,
    mass_kg: f32,
}

#[derive(Component, Debug, Clone, Copy)]
struct EnemyAttackState {
    cooldown_s: f32,
    rng_state: u64,
}

#[derive(Component, Debug, Clone, Copy)]
struct EnemyProjectile {
    kind: EnemyProjectileKind,
    damage: f32,
    hit_radius_m: f32,
    velocity_mps: Vec2,
    drag: f32,
    gravity_scale: f32,
    remaining_lifetime_s: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnemyProjectileKind {
    Bullet,
    Missile,
    Bomb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnemyBehaviorKind {
    Walker,
    Flier,
    Turret,
    Charger,
    Bomber,
}

#[derive(Resource, Debug, Default)]
struct EnemyBootstrapState {
    seeded: bool,
    wave_counter: u32,
}

fn reset_enemy_bootstrap(mut bootstrap: ResMut<EnemyBootstrapState>) {
    bootstrap.seeded = false;
}

#[allow(clippy::type_complexity)]
fn cleanup_enemy_run_entities(
    mut commands: Commands,
    cleanup_query: Query<
        Entity,
        Or<(
            With<Enemy>,
            With<EnemyProjectile>,
            With<EnemyHpBarBackground>,
            With<EnemyHpBarFill>,
        )>,
    >,
) {
    for entity in &cleanup_query {
        commands.entity(entity).despawn();
    }
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
        EnemyBehaviorKind::Bomber => {
            ground_y
                + (enemy_cfg
                    .hover_amplitude
                    .max(ENEMY_BOMBER_ALTITUDE_DEFAULT_M)
                    * ENEMY_BOMBER_ALTITUDE_SCALE)
        }
        _ => ground_y,
    };

    let start_y = match behavior_kind {
        EnemyBehaviorKind::Flier => {
            base_altitude + phase_offset.sin() * enemy_cfg.hover_amplitude.max(0.5)
        }
        EnemyBehaviorKind::Bomber => base_altitude,
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
            EnemyDynamics {
                external_velocity_mps: Vec2::ZERO,
                mass_kg: enemy_mass_from_hitbox(enemy_cfg.hitbox_radius),
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
            EnemyAttackState {
                cooldown_s: 0.35 + ((sequence as f32 * 0.17).rem_euclid(0.8)),
                rng_state: 0xD8E5_3A1C_9F2B_4D11 ^ sequence as u64 ^ (enemy_cfg.id.len() as u64),
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
            &mut EnemyDynamics,
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

    for (mut transform, mut behavior, mut dynamics, motion, hitbox, enemy_type_id) in
        &mut enemy_query
    {
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
            EnemyBehaviorKind::Bomber => {
                transform.translation.x -= motion.base_speed_mps * 0.95 * dt;
                transform.translation.y = transform
                    .translation
                    .y
                    .lerp(behavior.base_altitude_m, (4.0 * dt).clamp(0.0, 1.0));
            }
        }

        transform.translation += (dynamics.external_velocity_mps * dt).extend(0.0);
        dynamics.external_velocity_mps *= f32::exp(-ENEMY_EXTERNAL_VELOCITY_DAMPING * dt);

        if enemy_type_id.0.is_empty() {
            warn!("Encountered enemy with empty type id.");
        }
    }
}

#[allow(clippy::type_complexity)]
fn resolve_enemy_body_collisions(
    mut player_query: Query<&mut Transform, (With<PlayerVehicle>, Without<Enemy>)>,
    mut enemy_queries: ParamSet<(
        Query<(Entity, &Transform, &EnemyHitbox, &EnemyDynamics), With<Enemy>>,
        Query<(Entity, &mut Transform, &mut EnemyDynamics), With<Enemy>>,
    )>,
) {
    #[derive(Debug, Clone, Copy)]
    struct EnemyBodySnapshot {
        entity: Entity,
        position: Vec2,
        radius_m: f32,
        mass_kg: f32,
    }

    let Ok(mut player_transform) = player_query.single_mut() else {
        return;
    };
    let player_position = player_transform.translation.truncate();

    let snapshots: Vec<EnemyBodySnapshot> = enemy_queries
        .p0()
        .iter()
        .map(|(entity, transform, hitbox, dynamics)| EnemyBodySnapshot {
            entity,
            position: transform.translation.truncate(),
            radius_m: hitbox.radius_m.max(0.05),
            mass_kg: dynamics.mass_kg.max(ENEMY_MIN_MASS),
        })
        .collect();

    if snapshots.is_empty() {
        return;
    }

    let mut player_position_offset = Vec2::ZERO;
    let mut enemy_position_offsets: HashMap<Entity, Vec2> = HashMap::new();
    let mut enemy_velocity_impulses: HashMap<Entity, Vec2> = HashMap::new();

    for enemy in &snapshots {
        let to_enemy = enemy.position - player_position;
        let distance = to_enemy.length();
        let combined_radius = PLAYER_COLLISION_RADIUS_M + enemy.radius_m;
        if distance >= combined_radius {
            continue;
        }

        let normal = if distance > 0.0001 {
            to_enemy / distance
        } else {
            Vec2::X
        };
        let penetration = (combined_radius - distance + ENEMY_COLLISION_SEPARATION_BIAS_M).max(0.0);
        if penetration <= 0.0 {
            continue;
        }

        let inv_player_mass = 1.0 / PLAYER_COLLISION_MASS.max(0.01);
        let inv_enemy_mass = 1.0 / enemy.mass_kg.max(0.01);
        let inv_total = inv_player_mass + inv_enemy_mass;
        if inv_total <= f32::EPSILON {
            continue;
        }

        let player_share = inv_player_mass / inv_total;
        let enemy_share = inv_enemy_mass / inv_total;
        let correction = normal * penetration;

        player_position_offset -= correction * player_share;
        *enemy_position_offsets
            .entry(enemy.entity)
            .or_insert(Vec2::ZERO) += correction * enemy_share;
        *enemy_velocity_impulses
            .entry(enemy.entity)
            .or_insert(Vec2::ZERO) += normal * (penetration * ENEMY_PLAYER_COLLISION_IMPULSE_GAIN);
    }

    for i in 0..snapshots.len() {
        for j in (i + 1)..snapshots.len() {
            let a = snapshots[i];
            let b = snapshots[j];
            let delta = b.position - a.position;
            let distance = delta.length();
            let combined_radius = a.radius_m + b.radius_m;
            if distance >= combined_radius {
                continue;
            }

            let normal = if distance > 0.0001 {
                delta / distance
            } else {
                Vec2::X
            };
            let penetration =
                (combined_radius - distance + ENEMY_COLLISION_SEPARATION_BIAS_M).max(0.0);
            if penetration <= 0.0 {
                continue;
            }

            let inv_mass_a = 1.0 / a.mass_kg.max(0.01);
            let inv_mass_b = 1.0 / b.mass_kg.max(0.01);
            let inv_total = inv_mass_a + inv_mass_b;
            if inv_total <= f32::EPSILON {
                continue;
            }

            let a_share = inv_mass_a / inv_total;
            let b_share = inv_mass_b / inv_total;
            let correction = normal * penetration;

            *enemy_position_offsets.entry(a.entity).or_insert(Vec2::ZERO) -= correction * a_share;
            *enemy_position_offsets.entry(b.entity).or_insert(Vec2::ZERO) += correction * b_share;

            let impulse = normal * (penetration * ENEMY_COLLISION_IMPULSE_GAIN);
            *enemy_velocity_impulses
                .entry(a.entity)
                .or_insert(Vec2::ZERO) -= impulse * a_share;
            *enemy_velocity_impulses
                .entry(b.entity)
                .or_insert(Vec2::ZERO) += impulse * b_share;
        }
    }

    player_transform.translation += player_position_offset.extend(0.0);

    for (entity, mut transform, mut dynamics) in &mut enemy_queries.p1() {
        if let Some(offset) = enemy_position_offsets.get(&entity) {
            transform.translation += offset.extend(0.0);
        }
        if let Some(impulse) = enemy_velocity_impulses.get(&entity) {
            let mass = dynamics.mass_kg.max(0.01);
            dynamics.external_velocity_mps += *impulse / mass;
        }
    }
}

#[allow(clippy::type_complexity)]
fn fire_enemy_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    player_query: Query<&Transform, With<PlayerVehicle>>,
    mut enemy_query: Query<
        (
            &Transform,
            &EnemyBehavior,
            &EnemyTypeId,
            &EnemyHitbox,
            &mut EnemyAttackState,
        ),
        With<Enemy>,
    >,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_position = player_transform.translation.truncate();
    let dt = time.delta_secs();

    for (enemy_transform, behavior, enemy_type_id, hitbox, mut attack_state) in &mut enemy_query {
        attack_state.cooldown_s -= dt;
        if attack_state.cooldown_s > 0.0 {
            continue;
        }

        let Some(enemy_type) = config.enemy_types_by_id.get(&enemy_type_id.0) else {
            continue;
        };
        let Some(weapon) = config.weapons_by_id.get(&enemy_type.weapon_id) else {
            continue;
        };

        let enemy_position = enemy_transform.translation.truncate();
        let to_player = player_position - enemy_position;
        let distance_to_player_m = to_player.length();
        let fire_cooldown_s =
            (1.0 / weapon.fire_rate.max(MIN_ENEMY_FIRE_RATE_HZ)).max(MIN_ENEMY_FIRE_COOLDOWN_S);

        if behavior.kind == EnemyBehaviorKind::Bomber {
            let x_distance = (enemy_position.x - player_position.x).abs();
            let has_drop_window = x_distance <= ENEMY_BOMBER_DROP_RANGE_M;
            let player_is_below = player_position.y < (enemy_position.y - 0.5);
            if !has_drop_window || !player_is_below {
                attack_state.cooldown_s = fire_cooldown_s;
                continue;
            }

            let bomb_spawn_world = enemy_position + Vec2::new(0.0, -(hitbox.radius_m + 0.2));
            spawn_enemy_projectile(
                &mut commands,
                weapon,
                behavior.kind,
                bomb_spawn_world,
                Vec2::NEG_Y,
            );
            attack_state.cooldown_s = fire_cooldown_s;
            continue;
        }

        if distance_to_player_m <= 0.001 || distance_to_player_m > ENEMY_ATTACK_RANGE_M {
            attack_state.cooldown_s = fire_cooldown_s;
            continue;
        }

        let aim_direction = to_player.normalize_or_zero();
        let muzzle_forward = hitbox.radius_m + weapon.muzzle_offset_x.max(0.0);
        let muzzle_world = enemy_position
            + (aim_direction * muzzle_forward)
            + Vec2::new(0.0, weapon.muzzle_offset_y);

        let (pattern_offsets_rad, pattern_count) = shot_pattern_for_behavior(behavior.kind);
        let spread_half_angle_rad = weapon.spread_degrees.to_radians() * 0.5;
        let burst_count = weapon.burst_count.max(1);

        for _ in 0..burst_count {
            for pattern_offset in pattern_offsets_rad.iter().take(pattern_count) {
                let behavior_offset = if behavior.kind == EnemyBehaviorKind::Flier {
                    pattern_offset * aim_direction.x.signum().clamp(-1.0, 1.0)
                } else {
                    *pattern_offset
                };
                let random_spread =
                    next_signed_unit_random(&mut attack_state.rng_state) * spread_half_angle_rad;
                let shot_angle =
                    aim_direction.y.atan2(aim_direction.x) + behavior_offset + random_spread;
                let shot_direction_world = Vec2::from_angle(shot_angle).normalize_or_zero();

                if shot_direction_world.length_squared() <= f32::EPSILON {
                    continue;
                }

                spawn_enemy_projectile(
                    &mut commands,
                    weapon,
                    behavior.kind,
                    muzzle_world,
                    shot_direction_world,
                );
            }
        }

        let burst_extension_s =
            weapon.burst_interval_seconds.max(0.0) * burst_count.saturating_sub(1) as f32;
        attack_state.cooldown_s = fire_cooldown_s + burst_extension_s;
    }
}

fn simulate_enemy_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    mut projectile_query: Query<(Entity, &mut Transform, &mut EnemyProjectile)>,
) {
    let Some(environment) = config
        .environments_by_id
        .get(&config.game.app.starting_environment)
    else {
        return;
    };

    let dt = time.delta_secs();
    for (entity, mut transform, mut projectile) in &mut projectile_query {
        if projectile.gravity_scale > 0.0 {
            projectile.velocity_mps.y -= environment.gravity * projectile.gravity_scale * dt;
        }

        let drag_damping = f32::exp(-(projectile.drag.max(0.0) * dt));
        projectile.velocity_mps *= drag_damping;
        transform.translation += (projectile.velocity_mps * dt).extend(0.0);

        if projectile.kind == EnemyProjectileKind::Bomb {
            transform.rotate_z(3.4 * dt);
        } else if projectile.velocity_mps.length_squared() > f32::EPSILON {
            let angle = projectile.velocity_mps.y.atan2(projectile.velocity_mps.x);
            transform.rotation = Quat::from_rotation_z(angle);
        }

        let ground_y = terrain_height_at_x(&config, transform.translation.x);
        if transform.translation.y <= ground_y {
            commands.entity(entity).despawn();
            continue;
        }

        projectile.remaining_lifetime_s -= dt;
        if projectile.remaining_lifetime_s <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn resolve_enemy_projectile_hits_player(
    mut commands: Commands,
    projectile_query: Query<(Entity, &Transform, &EnemyProjectile)>,
    mut player_query: Query<(&Transform, &mut PlayerHealth), With<PlayerVehicle>>,
) {
    let Ok((player_transform, mut player_health)) = player_query.single_mut() else {
        return;
    };
    let player_position = player_transform.translation.truncate();

    let mut total_damage = 0.0;
    let mut consumed_projectiles = Vec::new();
    for (projectile_entity, transform, projectile) in &projectile_query {
        let projectile_position = transform.translation.truncate();
        let combined_hit_radius = PLAYER_PROJECTILE_HIT_RADIUS_M + projectile.hit_radius_m;
        if projectile_position.distance_squared(player_position)
            <= (combined_hit_radius * combined_hit_radius)
        {
            total_damage += projectile.damage.max(0.0);
            consumed_projectiles.push(projectile_entity);
        }
    }

    if total_damage > 0.0 {
        player_health.current = (player_health.current - total_damage).max(0.0);
    }

    for projectile_entity in consumed_projectiles {
        commands.entity(projectile_entity).despawn();
    }
}

fn apply_enemy_contact_damage_to_player(
    time: Res<Time>,
    config: Res<GameConfig>,
    mut player_query: Query<(&Transform, &mut PlayerHealth), With<PlayerVehicle>>,
    enemy_query: Query<(&Transform, &EnemyHitbox, &EnemyTypeId), With<Enemy>>,
) {
    let Ok((player_transform, mut player_health)) = player_query.single_mut() else {
        return;
    };
    let player_position = player_transform.translation.truncate();
    let dt = time.delta_secs();

    let mut total_contact_damage = 0.0;
    for (enemy_transform, enemy_hitbox, enemy_type_id) in &enemy_query {
        let Some(enemy_type) = config.enemy_types_by_id.get(&enemy_type_id.0) else {
            continue;
        };

        let combined_radius = PLAYER_CONTACT_HIT_RADIUS_M + enemy_hitbox.radius_m;
        let distance_sq = enemy_transform
            .translation
            .truncate()
            .distance_squared(player_position);
        if distance_sq <= combined_radius * combined_radius {
            total_contact_damage += enemy_type.contact_damage.max(0.0) * dt;
        }
    }

    if total_contact_damage > 0.0 {
        player_health.current = (player_health.current - total_contact_damage).max(0.0);
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

fn shot_pattern_for_behavior(kind: EnemyBehaviorKind) -> ([f32; 3], usize) {
    match kind {
        EnemyBehaviorKind::Walker | EnemyBehaviorKind::Turret | EnemyBehaviorKind::Bomber => {
            ([0.0, 0.0, 0.0], 1)
        }
        EnemyBehaviorKind::Flier => ([ENEMY_FLIER_ARC_ANGLE_RAD, 0.0, 0.0], 1),
        EnemyBehaviorKind::Charger => (
            [
                -ENEMY_CHARGER_SPREAD_HALF_ANGLE_RAD,
                0.0,
                ENEMY_CHARGER_SPREAD_HALF_ANGLE_RAD,
            ],
            3,
        ),
    }
}

fn spawn_enemy_projectile(
    commands: &mut Commands,
    weapon: &WeaponConfig,
    behavior_kind: EnemyBehaviorKind,
    muzzle_world: Vec2,
    shot_direction_world: Vec2,
) {
    let projectile_kind = if behavior_kind == EnemyBehaviorKind::Bomber {
        EnemyProjectileKind::Bomb
    } else {
        match weapon.projectile_type.as_str() {
            "missile" => EnemyProjectileKind::Missile,
            _ => EnemyProjectileKind::Bullet,
        }
    };
    let (length_m, thickness_m, hit_radius_m, color) = match projectile_kind {
        EnemyProjectileKind::Bullet => (
            ENEMY_BULLET_LENGTH_M,
            ENEMY_BULLET_THICKNESS_M,
            ENEMY_BULLET_THICKNESS_M * 0.5,
            Color::srgba(1.0, 0.64, 0.26, 0.88),
        ),
        EnemyProjectileKind::Missile => (
            ENEMY_MISSILE_LENGTH_M,
            ENEMY_MISSILE_THICKNESS_M,
            ENEMY_MISSILE_THICKNESS_M * 0.55,
            Color::srgba(1.0, 0.48, 0.22, 0.92),
        ),
        EnemyProjectileKind::Bomb => (
            ENEMY_BOMB_LENGTH_M,
            ENEMY_BOMB_THICKNESS_M,
            ENEMY_BOMB_THICKNESS_M * 0.48,
            Color::srgba(0.98, 0.62, 0.18, 0.96),
        ),
    };

    let gravity_scale = match projectile_kind {
        EnemyProjectileKind::Missile => weapon.missile_gravity_scale.max(0.0),
        EnemyProjectileKind::Bullet => {
            if behavior_kind == EnemyBehaviorKind::Flier {
                ENEMY_PROJECTILE_ARC_GRAVITY_SCALE
            } else {
                0.0
            }
        }
        EnemyProjectileKind::Bomb => 1.0,
    };

    let initial_velocity = match projectile_kind {
        EnemyProjectileKind::Bomb => Vec2::ZERO,
        _ => shot_direction_world * weapon.bullet_speed.max(0.1),
    };

    let projectile_center = muzzle_world + (shot_direction_world * (length_m * 0.5));
    let shot_angle = shot_direction_world.y.atan2(shot_direction_world.x);

    commands.spawn((
        Name::new("EnemyProjectile"),
        EnemyProjectile {
            kind: projectile_kind,
            damage: weapon.damage.max(0.0),
            hit_radius_m,
            velocity_mps: initial_velocity,
            drag: weapon.projectile_drag.max(0.0),
            gravity_scale,
            remaining_lifetime_s: weapon.projectile_lifetime_seconds.max(0.05),
        },
        Sprite::from_color(color, Vec2::new(length_m, thickness_m)),
        Transform::from_xyz(
            projectile_center.x,
            projectile_center.y,
            ENEMY_PROJECTILE_Z_M,
        )
        .with_rotation(Quat::from_rotation_z(shot_angle)),
    ));
}

fn enemy_mass_from_hitbox(hitbox_radius_m: f32) -> f32 {
    let radius = hitbox_radius_m.max(0.1);
    ((radius * radius) * ENEMY_MASS_PER_RADIUS_SQUARED).max(ENEMY_MIN_MASS)
}

fn next_signed_unit_random(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
    let value = ((*seed >> 32) as u32) as f32 / u32::MAX as f32;
    (value * 2.0) - 1.0
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
        EnemyBehaviorKind::Bomber => Vec2::new(r * 3.1, r * 1.6),
    }
}

fn color_for_behavior(kind: EnemyBehaviorKind) -> Color {
    match kind {
        EnemyBehaviorKind::Walker => Color::srgb(0.86, 0.57, 0.36),
        EnemyBehaviorKind::Flier => Color::srgb(0.54, 0.74, 0.92),
        EnemyBehaviorKind::Turret => Color::srgb(0.81, 0.54, 0.84),
        EnemyBehaviorKind::Charger => Color::srgb(0.90, 0.41, 0.41),
        EnemyBehaviorKind::Bomber => Color::srgb(0.73, 0.78, 0.85),
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
        "bomber" => EnemyBehaviorKind::Bomber,
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
