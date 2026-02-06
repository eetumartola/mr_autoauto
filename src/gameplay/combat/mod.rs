use crate::config::GameConfig;
use crate::gameplay::enemies::{
    enemy_hit_flash_duration_seconds, Enemy, EnemyHealth, EnemyHitFlash, EnemyHitbox, EnemyTypeId,
};
use crate::gameplay::vehicle::PlayerVehicle;
use crate::states::GameState;
use bevy::prelude::*;
use std::collections::HashSet;
use std::f32::consts::{PI, TAU};

const TURRET_MOUNT_OFFSET_LOCAL: Vec3 = Vec3::new(0.35, 1.05, 2.5);
const TARGET_LASER_THICKNESS_M: f32 = 0.12;
const TARGET_CONE_LINE_THICKNESS_M: f32 = 0.08;
const TARGET_LINE_OPACITY: f32 = 0.30;
const TARGET_LASER_Z: f32 = 0.8;
const TARGET_CONE_Z: f32 = 0.7;
const PROJECTILE_Z: f32 = 2.1;
const BULLET_LENGTH_M: f32 = 0.52;
const BULLET_THICKNESS_M: f32 = 0.12;
const MISSILE_LENGTH_M: f32 = 0.82;
const MISSILE_THICKNESS_M: f32 = 0.18;
const MAX_BURST_SHOTS_PER_FRAME: u32 = 12;
const MIN_BURST_INTERVAL_S: f64 = 1.0 / 240.0;
const BULLET_TRAIL_SEGMENT_COUNT: usize = 7;
const MISSILE_TRAIL_SEGMENT_COUNT: usize = 8;
const BULLET_TRAIL_SEGMENT_LENGTH_M: f32 = 0.18;
const MISSILE_TRAIL_SEGMENT_LENGTH_M: f32 = 0.24;
const IMPACT_FX_SIZE_M: Vec2 = Vec2::new(0.52, 0.52);
const IMPACT_FX_LIFETIME_S: f32 = 0.15;
const EXPLOSION_FX_SIZE_M: Vec2 = Vec2::new(1.45, 1.45);
const EXPLOSION_FX_LIFETIME_S: f32 = 0.28;
const FX_Z: f32 = 4.4;

#[derive(Message, Debug, Clone)]
pub struct EnemyKilledEvent {
    pub enemy_type_id: String,
}

pub struct CombatGameplayPlugin;

impl Plugin for CombatGameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TurretTargetingState>()
            .init_resource::<TurretFireState>()
            .add_message::<EnemyKilledEvent>()
            .add_systems(
                OnEnter(GameState::InRun),
                (reset_turret_targeting_state, reset_turret_fire_state),
            )
            .add_systems(
                Update,
                (
                    spawn_turret_visuals,
                    update_turret_targeting_state,
                    fire_turret_projectiles,
                    sync_turret_targeting_visuals,
                    simulate_player_projectiles,
                    resolve_player_projectile_enemy_hits,
                    update_fade_out_fx,
                )
                    .chain()
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            );
    }
}

#[derive(Component)]
struct TurretVisualAnchor;

#[derive(Component)]
struct TurretTargetLaserVisual;

#[derive(Component, Debug, Clone, Copy)]
struct TurretConeBoundaryVisual {
    side_sign: f32,
}

#[derive(Component, Debug, Clone, Copy)]
struct PlayerProjectile {
    kind: PlayerProjectileKind,
    damage: f32,
    velocity_mps: Vec2,
    drag: f32,
    remaining_lifetime_s: f32,
    homing_turn_rate_rad_s: f32,
    gravity_scale: f32,
    target_entity: Option<Entity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerProjectileKind {
    Bullet,
    Missile,
}

#[derive(Component, Debug, Clone, Copy)]
struct FadeOutFx {
    remaining_s: f32,
    total_s: f32,
    initial_alpha: f32,
}

#[derive(Resource, Debug, Clone)]
pub struct TurretTargetingState {
    pub target_entity: Option<Entity>,
    pub aim_point_world: Vec2,
    pub aim_direction_local: Vec2,
    pub aim_distance_m: f32,
    pub range_m: f32,
    pub cone_half_angle_rad: f32,
}

impl Default for TurretTargetingState {
    fn default() -> Self {
        Self {
            target_entity: None,
            aim_point_world: Vec2::ZERO,
            aim_direction_local: Vec2::X,
            aim_distance_m: 0.0,
            range_m: 28.0,
            cone_half_angle_rad: 30.0_f32.to_radians(),
        }
    }
}

#[derive(Resource, Debug, Clone)]
struct TurretFireState {
    next_fire_time_s: f64,
    next_missile_fire_time_s: f64,
    burst_shots_remaining: u32,
    next_burst_shot_time_s: f64,
    rng_state: u64,
}

impl Default for TurretFireState {
    fn default() -> Self {
        Self {
            next_fire_time_s: 0.0,
            next_missile_fire_time_s: 0.0,
            burst_shots_remaining: 0,
            next_burst_shot_time_s: 0.0,
            rng_state: 0xA77C_C1B5_D7E3_42FD,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurretTargetPriority {
    Nearest,
    Strongest,
}

#[derive(Debug, Clone, Copy)]
struct TargetCandidate {
    entity: Entity,
    distance_m: f32,
    strength: f32,
    aim_direction_local: Vec2,
    aim_point_world: Vec2,
}

fn reset_turret_targeting_state(mut targeting: ResMut<TurretTargetingState>) {
    *targeting = TurretTargetingState::default();
}

fn reset_turret_fire_state(mut fire_state: ResMut<TurretFireState>) {
    *fire_state = TurretFireState::default();
}

fn spawn_turret_visuals(
    mut commands: Commands,
    player_query: Query<Entity, With<PlayerVehicle>>,
    existing_anchor_query: Query<Entity, With<TurretVisualAnchor>>,
) {
    if !existing_anchor_query.is_empty() {
        return;
    }

    let Ok(player_entity) = player_query.single() else {
        return;
    };

    let anchor_entity = commands
        .spawn((
            Name::new("TurretVisualAnchor"),
            TurretVisualAnchor,
            Transform::from_translation(TURRET_MOUNT_OFFSET_LOCAL),
            GlobalTransform::default(),
            Visibility::Inherited,
        ))
        .id();

    commands.entity(player_entity).add_child(anchor_entity);
    commands.entity(anchor_entity).with_children(|parent| {
        parent.spawn((
            Name::new("TurretTargetLaserVisual"),
            TurretTargetLaserVisual,
            Sprite::from_color(
                Color::srgba(0.23, 0.72, 0.96, TARGET_LINE_OPACITY),
                Vec2::new(1.0, TARGET_LASER_THICKNESS_M),
            ),
            Transform::from_xyz(0.0, 0.0, TARGET_LASER_Z),
        ));

        for side_sign in [-1.0_f32, 1.0_f32] {
            parent.spawn((
                Name::new("TurretConeBoundaryVisual"),
                TurretConeBoundaryVisual { side_sign },
                Sprite::from_color(
                    Color::srgba(0.24, 0.87, 0.38, TARGET_LINE_OPACITY),
                    Vec2::new(1.0, TARGET_CONE_LINE_THICKNESS_M),
                ),
                Transform::from_xyz(0.0, 0.0, TARGET_CONE_Z),
            ));
        }
    });
}

fn update_turret_targeting_state(
    config: Res<GameConfig>,
    player_query: Query<&Transform, With<PlayerVehicle>>,
    enemy_query: Query<(Entity, &Transform, &EnemyHealth), With<Enemy>>,
    mut targeting: ResMut<TurretTargetingState>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let Some(vehicle_config) = config.vehicles_by_id.get(&config.game.app.default_vehicle) else {
        return;
    };

    let target_priority = parse_target_priority(&vehicle_config.turret_target_priority);
    let range_m = vehicle_config.turret_range_m.max(0.1);
    let cone_half_angle_rad =
        (vehicle_config.turret_cone_degrees.to_radians() * 0.5).clamp(0.001, PI);

    let player_rotation_rad = player_transform.rotation.to_euler(EulerRot::XYZ).2;
    let player_rotation = Mat2::from_angle(player_rotation_rad);
    let player_inverse_rotation = Mat2::from_angle(-player_rotation_rad);
    let origin_world = player_transform.translation.truncate()
        + player_rotation * TURRET_MOUNT_OFFSET_LOCAL.truncate();

    let mut best_candidate: Option<TargetCandidate> = None;

    for (entity, enemy_transform, enemy_health) in &enemy_query {
        let to_enemy_world = enemy_transform.translation.truncate() - origin_world;
        let distance_m = to_enemy_world.length();

        if distance_m <= 0.001 || distance_m > range_m {
            continue;
        }

        let to_enemy_local = player_inverse_rotation * to_enemy_world;
        let angle_to_enemy = to_enemy_local.y.atan2(to_enemy_local.x).abs();
        if angle_to_enemy > cone_half_angle_rad {
            continue;
        }

        let enemy_strength = enemy_health.current.max(0.0);

        let candidate = TargetCandidate {
            entity,
            distance_m,
            strength: enemy_strength,
            aim_direction_local: to_enemy_local.normalize_or_zero(),
            aim_point_world: enemy_transform.translation.truncate(),
        };

        if should_replace_target(candidate, best_candidate, target_priority) {
            best_candidate = Some(candidate);
        }
    }

    match best_candidate {
        Some(candidate) => {
            targeting.target_entity = Some(candidate.entity);
            targeting.aim_direction_local = candidate.aim_direction_local;
            targeting.aim_distance_m = candidate.distance_m;
            targeting.aim_point_world = candidate.aim_point_world;
        }
        None => {
            targeting.target_entity = None;
            targeting.aim_direction_local = Vec2::X;
            targeting.aim_distance_m = range_m;
            targeting.aim_point_world =
                origin_world + (player_rotation * (targeting.aim_direction_local * range_m));
        }
    }

    targeting.range_m = range_m;
    targeting.cone_half_angle_rad = cone_half_angle_rad;
}

fn fire_turret_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    targeting: Res<TurretTargetingState>,
    player_query: Query<&Transform, With<PlayerVehicle>>,
    mut fire_state: ResMut<TurretFireState>,
) {
    if targeting.target_entity.is_none() {
        fire_state.burst_shots_remaining = 0;
        return;
    }

    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let Some(vehicle_config) = config.vehicles_by_id.get(&config.game.app.default_vehicle) else {
        return;
    };

    let Some(primary_weapon) = config.weapons_by_id.get(&vehicle_config.default_weapon_id) else {
        return;
    };

    let base_direction_local = targeting.aim_direction_local.normalize_or_zero();
    if base_direction_local.length_squared() <= f32::EPSILON {
        return;
    }

    let now = time.elapsed_secs_f64();
    let trigger_interval_s = (1.0 / primary_weapon.fire_rate.max(0.001)) as f64;
    let burst_interval_s = (primary_weapon.burst_interval_seconds as f64).max(MIN_BURST_INTERVAL_S);
    let shots_per_burst = primary_weapon.burst_count.max(1);

    if fire_state.burst_shots_remaining == 0 && now >= fire_state.next_fire_time_s {
        fire_state.burst_shots_remaining = shots_per_burst;
        fire_state.next_burst_shot_time_s = now;
        fire_state.next_fire_time_s = now + trigger_interval_s;
    }

    let player_rotation_rad = player_transform.rotation.to_euler(EulerRot::XYZ).2;
    let player_rotation = Mat2::from_angle(player_rotation_rad);
    let turret_origin_world = player_transform.translation.truncate()
        + player_rotation * TURRET_MOUNT_OFFSET_LOCAL.truncate();

    let spread_half_angle_rad = primary_weapon.spread_degrees.to_radians() * 0.5;
    let mut shots_spawned = 0_u32;

    while fire_state.burst_shots_remaining > 0
        && now >= fire_state.next_burst_shot_time_s
        && shots_spawned < MAX_BURST_SHOTS_PER_FRAME
    {
        let spread_angle_rad =
            next_signed_unit_random(&mut fire_state.rng_state) * spread_half_angle_rad;
        let shot_direction_local =
            (Mat2::from_angle(spread_angle_rad) * base_direction_local).normalize_or_zero();
        let shot_direction_world = (player_rotation * shot_direction_local).normalize_or_zero();
        if shot_direction_world.length_squared() <= f32::EPSILON {
            break;
        }

        spawn_player_projectile(
            &mut commands,
            primary_weapon,
            shot_direction_world,
            turret_origin_world,
            player_rotation,
            targeting.target_entity,
        );

        fire_state.burst_shots_remaining -= 1;
        fire_state.next_burst_shot_time_s += burst_interval_s;
        shots_spawned += 1;
    }

    if let Some(secondary_weapon_id) = vehicle_config.secondary_weapon_id.as_deref() {
        if now >= fire_state.next_missile_fire_time_s {
            if let Some(secondary_weapon) = config.weapons_by_id.get(secondary_weapon_id) {
                let missile_direction_local =
                    Vec2::from_angle(targeting.cone_half_angle_rad).normalize_or_zero();
                let missile_direction_world =
                    (player_rotation * missile_direction_local).normalize_or_zero();

                if missile_direction_world.length_squared() > f32::EPSILON {
                    spawn_player_projectile(
                        &mut commands,
                        secondary_weapon,
                        missile_direction_world,
                        turret_origin_world,
                        player_rotation,
                        targeting.target_entity,
                    );
                    fire_state.next_missile_fire_time_s =
                        now + vehicle_config.missile_fire_interval_seconds.max(0.001) as f64;
                }
            }
        }
    }
}

fn sync_turret_targeting_visuals(
    targeting: Res<TurretTargetingState>,
    mut laser_query: Query<
        &mut Transform,
        (
            With<TurretTargetLaserVisual>,
            Without<TurretConeBoundaryVisual>,
        ),
    >,
    mut cone_query: Query<
        (&TurretConeBoundaryVisual, &mut Transform),
        Without<TurretTargetLaserVisual>,
    >,
) {
    let laser_length = targeting.aim_distance_m.max(0.001);
    let aim_direction = targeting.aim_direction_local.normalize_or_zero();
    let aim_angle = aim_direction.y.atan2(aim_direction.x);

    let Ok(mut laser_transform) = laser_query.single_mut() else {
        return;
    };

    laser_transform.translation = (aim_direction * (laser_length * 0.5)).extend(TARGET_LASER_Z);
    laser_transform.rotation = Quat::from_rotation_z(aim_angle);
    laser_transform.scale = Vec3::new(laser_length, 1.0, 1.0);

    let cone_length = targeting.range_m.max(0.001);
    for (boundary, mut transform) in &mut cone_query {
        let boundary_angle = boundary.side_sign * targeting.cone_half_angle_rad;
        let boundary_direction = Vec2::from_angle(boundary_angle);
        transform.translation = (boundary_direction * (cone_length * 0.5)).extend(TARGET_CONE_Z);
        transform.rotation = Quat::from_rotation_z(boundary_angle);
        transform.scale = Vec3::new(cone_length, 1.0, 1.0);
    }
}

fn simulate_player_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    enemy_query: Query<&Transform, (With<Enemy>, Without<PlayerProjectile>)>,
    mut projectile_query: Query<(Entity, &mut Transform, &mut PlayerProjectile), Without<Enemy>>,
) {
    let Some(environment) = config
        .environments_by_id
        .get(&config.game.app.starting_environment)
    else {
        return;
    };

    let dt = time.delta_secs();
    for (entity, mut transform, mut projectile) in &mut projectile_query {
        if projectile.kind == PlayerProjectileKind::Missile {
            projectile.velocity_mps.y -= environment.gravity * projectile.gravity_scale * dt;

            if projectile.homing_turn_rate_rad_s > 0.0 {
                if let Some(target_entity) = projectile.target_entity {
                    if let Ok(target_transform) = enemy_query.get(target_entity) {
                        let to_target = target_transform.translation.truncate()
                            - transform.translation.truncate();
                        let desired_direction = to_target.normalize_or_zero();
                        if desired_direction.length_squared() > f32::EPSILON {
                            let current_speed = projectile.velocity_mps.length().max(0.001);
                            let current_direction = projectile.velocity_mps.normalize_or_zero();
                            let current_angle = current_direction.y.atan2(current_direction.x);
                            let desired_angle = desired_direction.y.atan2(desired_direction.x);
                            let max_step = projectile.homing_turn_rate_rad_s * dt;
                            let clamped_delta =
                                shortest_angle_delta_rad(desired_angle, current_angle)
                                    .clamp(-max_step, max_step);
                            let next_direction = Vec2::from_angle(current_angle + clamped_delta);
                            projectile.velocity_mps = next_direction * current_speed;
                        }
                    }
                }
            }
        }

        let drag_damping = f32::exp(-(projectile.drag.max(0.0) * dt));
        projectile.velocity_mps *= drag_damping;
        transform.translation += (projectile.velocity_mps * dt).extend(0.0);

        if projectile.velocity_mps.length_squared() > f32::EPSILON {
            let angle = projectile.velocity_mps.y.atan2(projectile.velocity_mps.x);
            transform.rotation = Quat::from_rotation_z(angle);
        }

        let ground_y = terrain_height_at_x(&config, transform.translation.x);
        if transform.translation.y <= ground_y {
            let impact_position = Vec2::new(transform.translation.x, ground_y);
            spawn_impact_fx(&mut commands, impact_position, projectile.kind);
            if projectile.kind == PlayerProjectileKind::Missile {
                spawn_explosion_fx(&mut commands, impact_position);
            }
            projectile.remaining_lifetime_s = -1.0;
            commands.entity(entity).despawn();
            continue;
        }

        projectile.remaining_lifetime_s -= dt;

        if projectile.remaining_lifetime_s <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn resolve_player_projectile_enemy_hits(
    mut commands: Commands,
    projectile_query: Query<(Entity, &Transform, &PlayerProjectile)>,
    mut enemy_query: Query<
        (
            Entity,
            &Transform,
            &EnemyHitbox,
            &EnemyTypeId,
            &mut EnemyHealth,
        ),
        With<Enemy>,
    >,
    mut killed_message_writer: MessageWriter<EnemyKilledEvent>,
) {
    let projectile_snapshots: Vec<(Entity, Vec2, f32, PlayerProjectileKind)> = projectile_query
        .iter()
        .filter(|(_, _, projectile)| projectile.remaining_lifetime_s > 0.0)
        .map(|(entity, transform, projectile)| {
            (
                entity,
                transform.translation.truncate(),
                projectile.damage,
                projectile.kind,
            )
        })
        .collect();

    if projectile_snapshots.is_empty() {
        return;
    }

    let mut consumed_projectiles = HashSet::new();
    let mut dead_enemies = Vec::new();
    let mut flashed_enemies = HashSet::new();
    let mut impact_fx_positions = Vec::new();
    let mut explosion_fx_positions = Vec::new();

    for (enemy_entity, enemy_transform, hitbox, enemy_type_id, mut health) in &mut enemy_query {
        let enemy_position = enemy_transform.translation.truncate();
        if health.current <= 0.0 {
            dead_enemies.push((enemy_entity, enemy_type_id.0.clone()));
            explosion_fx_positions.push(enemy_position);
            continue;
        }

        for (projectile_entity, projectile_position, damage, projectile_kind) in
            &projectile_snapshots
        {
            if consumed_projectiles.contains(projectile_entity) {
                continue;
            }

            let distance_sq = enemy_position.distance_squared(*projectile_position);
            let hit_radius_sq = hitbox.radius_m * hitbox.radius_m;
            if distance_sq > hit_radius_sq {
                continue;
            }

            health.current -= *damage;
            consumed_projectiles.insert(*projectile_entity);
            impact_fx_positions.push((*projectile_position, *projectile_kind));

            if health.current <= 0.0 {
                dead_enemies.push((enemy_entity, enemy_type_id.0.clone()));
                explosion_fx_positions.push(enemy_position);
                break;
            } else {
                flashed_enemies.insert(enemy_entity);
            }
        }
    }

    for projectile_entity in consumed_projectiles {
        commands.entity(projectile_entity).despawn();
    }

    for enemy_entity in flashed_enemies {
        commands.entity(enemy_entity).insert(EnemyHitFlash {
            remaining_s: enemy_hit_flash_duration_seconds(),
        });
    }

    for (position, projectile_kind) in impact_fx_positions {
        spawn_impact_fx(&mut commands, position, projectile_kind);
    }

    for position in explosion_fx_positions {
        spawn_explosion_fx(&mut commands, position);
    }

    for (enemy_entity, enemy_type_id) in dead_enemies {
        killed_message_writer.write(EnemyKilledEvent { enemy_type_id });
        commands.entity(enemy_entity).despawn();
    }
}

fn update_fade_out_fx(
    mut commands: Commands,
    time: Res<Time>,
    mut fx_query: Query<(Entity, &mut FadeOutFx, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    for (entity, mut fx, mut sprite) in &mut fx_query {
        fx.remaining_s -= dt;
        let t = (fx.remaining_s / fx.total_s).clamp(0.0, 1.0);
        let mut color = sprite.color;
        color.set_alpha(fx.initial_alpha * t);
        sprite.color = color;

        if fx.remaining_s <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn should_replace_target(
    candidate: TargetCandidate,
    current: Option<TargetCandidate>,
    priority: TurretTargetPriority,
) -> bool {
    let Some(current) = current else {
        return true;
    };

    match priority {
        TurretTargetPriority::Nearest => {
            candidate.distance_m < current.distance_m
                || ((candidate.distance_m - current.distance_m).abs() < 0.001
                    && candidate.strength > current.strength)
        }
        TurretTargetPriority::Strongest => {
            candidate.strength > current.strength
                || ((candidate.strength - current.strength).abs() < 0.001
                    && candidate.distance_m < current.distance_m)
        }
    }
}

fn parse_target_priority(raw: &str) -> TurretTargetPriority {
    match raw {
        "strongest" => TurretTargetPriority::Strongest,
        _ => TurretTargetPriority::Nearest,
    }
}

fn parse_projectile_kind(raw: &str) -> PlayerProjectileKind {
    match raw {
        "missile" => PlayerProjectileKind::Missile,
        _ => PlayerProjectileKind::Bullet,
    }
}

fn next_signed_unit_random(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);

    let value = ((*seed >> 32) as u32) as f32 / u32::MAX as f32;
    (value * 2.0) - 1.0
}

fn shortest_angle_delta_rad(target: f32, current: f32) -> f32 {
    (target - current + PI).rem_euclid(TAU) - PI
}

fn spawn_player_projectile(
    commands: &mut Commands,
    weapon: &crate::config::WeaponConfig,
    shot_direction_world: Vec2,
    turret_origin_world: Vec2,
    player_rotation: Mat2,
    target_entity: Option<Entity>,
) {
    let projectile_kind = parse_projectile_kind(&weapon.projectile_type);
    let (projectile_length, projectile_thickness, projectile_color) = match projectile_kind {
        PlayerProjectileKind::Bullet => (
            BULLET_LENGTH_M,
            BULLET_THICKNESS_M,
            Color::srgba(0.96, 0.92, 0.70, 0.92),
        ),
        PlayerProjectileKind::Missile => (
            MISSILE_LENGTH_M,
            MISSILE_THICKNESS_M,
            Color::srgba(0.95, 0.58, 0.20, 0.95),
        ),
    };

    let muzzle_offset_local = Vec2::new(weapon.muzzle_offset_x, weapon.muzzle_offset_y);
    let muzzle_world = turret_origin_world + (player_rotation * muzzle_offset_local);
    let projectile_center = muzzle_world + (shot_direction_world * (projectile_length * 0.5));
    let shot_angle_world = shot_direction_world.y.atan2(shot_direction_world.x);

    let projectile_entity = commands
        .spawn((
            Name::new("PlayerProjectile"),
            PlayerProjectile {
                kind: projectile_kind,
                damage: weapon.damage,
                velocity_mps: shot_direction_world * weapon.bullet_speed,
                drag: weapon.projectile_drag,
                remaining_lifetime_s: weapon.projectile_lifetime_seconds,
                homing_turn_rate_rad_s: weapon.homing_turn_rate_degrees.to_radians(),
                gravity_scale: weapon.missile_gravity_scale,
                target_entity,
            },
            Sprite::from_color(
                projectile_color,
                Vec2::new(projectile_length, projectile_thickness),
            ),
            Transform::from_xyz(projectile_center.x, projectile_center.y, PROJECTILE_Z)
                .with_rotation(Quat::from_rotation_z(shot_angle_world)),
        ))
        .id();

    commands.entity(projectile_entity).with_children(|parent| {
        let (segment_count, segment_length_m) = match projectile_kind {
            PlayerProjectileKind::Bullet => {
                (BULLET_TRAIL_SEGMENT_COUNT, BULLET_TRAIL_SEGMENT_LENGTH_M)
            }
            PlayerProjectileKind::Missile => {
                (MISSILE_TRAIL_SEGMENT_COUNT, MISSILE_TRAIL_SEGMENT_LENGTH_M)
            }
        };
        let segment_thickness_m = projectile_thickness * 0.72;
        let base = projectile_color.to_srgba();
        let head_alpha = base.alpha * 0.78;

        for index in 0..segment_count {
            let fade = 1.0 - (index as f32 / segment_count as f32);
            let alpha = (head_alpha * fade.powf(1.25)).max(0.04);
            let segment_color = Color::srgba(base.red, base.green, base.blue, alpha);
            let center_x =
                -((projectile_length + segment_length_m) * 0.5) - (index as f32 * segment_length_m);

            parent.spawn((
                Name::new("ProjectileTrailSegment"),
                Sprite::from_color(
                    segment_color,
                    Vec2::new(segment_length_m + 0.01, segment_thickness_m),
                ),
                Transform::from_xyz(center_x, 0.0, -0.01 - (index as f32 * 0.001)),
            ));
        }
    });
}

fn spawn_impact_fx(commands: &mut Commands, world_position: Vec2, kind: PlayerProjectileKind) {
    let color = match kind {
        PlayerProjectileKind::Bullet => Color::srgba(1.0, 0.96, 0.82, 0.88),
        PlayerProjectileKind::Missile => Color::srgba(1.0, 0.74, 0.34, 0.9),
    };
    spawn_fade_out_fx(
        commands,
        "ProjectileImpactFx",
        world_position,
        IMPACT_FX_SIZE_M,
        color,
        IMPACT_FX_LIFETIME_S,
    );
}

fn spawn_explosion_fx(commands: &mut Commands, world_position: Vec2) {
    spawn_fade_out_fx(
        commands,
        "EnemyExplosionFx",
        world_position,
        EXPLOSION_FX_SIZE_M,
        Color::srgba(1.0, 0.58, 0.22, 0.94),
        EXPLOSION_FX_LIFETIME_S,
    );
}

fn spawn_fade_out_fx(
    commands: &mut Commands,
    name: &'static str,
    world_position: Vec2,
    size: Vec2,
    color: Color,
    lifetime_s: f32,
) {
    let initial_alpha = color.to_srgba().alpha;
    commands.spawn((
        Name::new(name),
        FadeOutFx {
            remaining_s: lifetime_s,
            total_s: lifetime_s.max(0.001),
            initial_alpha,
        },
        Sprite::from_color(color, size),
        Transform::from_xyz(world_position.x, world_position.y, FX_Z),
    ));
}

fn terrain_height_at_x(config: &GameConfig, x: f32) -> f32 {
    let terrain = &config.game.terrain;
    terrain.base_height
        + (x * terrain.ramp_slope)
        + (x * terrain.wave_a_frequency).sin() * terrain.wave_a_amplitude
        + (x * terrain.wave_b_frequency).sin() * terrain.wave_b_amplitude
}
