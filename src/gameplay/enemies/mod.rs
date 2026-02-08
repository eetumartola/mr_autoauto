use crate::assets::{AssetRegistry, ModelAssetEntry};
use crate::config::{EnemyTypeConfig, GameConfig, WeaponConfig};
use crate::debug::{DebugGameplayGuards, EnemyDebugMarker};
use crate::gameplay::combat::EnemyKilledEvent;
use crate::gameplay::vehicle::{PlayerHealth, PlayerVehicle};
use crate::states::GameState;
use bevy::asset::LoadState;
use bevy::mesh::VertexAttributeValues;
use bevy::prelude::*;
use bevy_rapier2d::prelude::*;
use std::collections::HashSet;
use std::f32::consts::{FRAC_PI_2, TAU};

#[cfg(feature = "gaussian_splats")]
use bevy_gaussian_splatting::PlanarGaussian3d;

const ENEMY_SPAWN_START_AHEAD_M: f32 = 41.6;
const ENEMY_SPAWN_SPACING_M: f32 = 26.0;
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
const BOSS_ATTACK_RANGE_M: f32 = 92.0;
const ENEMY_BOMBER_DROP_RANGE_M: f32 = 8.5;
const ENEMY_FLIER_ARC_ANGLE_RAD: f32 = 0.28;
const ENEMY_CHARGER_SPREAD_HALF_ANGLE_RAD: f32 = 0.16;
const ENEMY_BOMBER_ALTITUDE_SCALE: f32 = 1.5;
const ENEMY_BOMBER_ALTITUDE_DEFAULT_M: f32 = 10.0;
const ENEMY_BOMBER_CRUISE_WAVE_FREQUENCY_HZ: f32 = 0.11;
const ENEMY_BOMBER_CRUISE_WAVE_AMPLITUDE_FACTOR: f32 = 0.2;
const ENEMY_BOMBER_CRUISE_WAVE_AMPLITUDE_MAX_M: f32 = 2.2;
const ENEMY_PROJECTILE_Z_M: f32 = 2.0;
const ENEMY_BULLET_LENGTH_M: f32 = 0.42;
const ENEMY_BULLET_THICKNESS_M: f32 = 0.10;
const ENEMY_MISSILE_LENGTH_M: f32 = 0.72;
const ENEMY_MISSILE_THICKNESS_M: f32 = 0.16;
const ENEMY_BOMB_LENGTH_M: f32 = 0.62;
const ENEMY_BOMB_THICKNESS_M: f32 = 0.62;
const ENEMY_PROJECTILE_ARC_GRAVITY_SCALE: f32 = 0.6;
const ENEMY_MUZZLE_FLASH_SIZE_M: Vec2 = Vec2::new(0.34, 0.22);
const ENEMY_MUZZLE_FLASH_LIFETIME_S: f32 = 0.07;
const ENEMY_MUZZLE_FLASH_Z_M: f32 = ENEMY_PROJECTILE_Z_M + 0.16;
const PLAYER_CONTACT_HIT_RADIUS_M: f32 = 1.45;
const PLAYER_PROJECTILE_HIT_RADIUS_M: f32 = 1.25;
const PLAYER_CRASH_MIN_SPEED_MPS: f32 = 2.0;
const PLAYER_CRASH_DAMAGE_TO_ENEMY_BASE_PER_SECOND: f32 = 30.0;
const PLAYER_CRASH_DAMAGE_TO_ENEMY_PER_MPS_PER_SECOND: f32 = 4.0;
const MIN_ENEMY_FIRE_RATE_HZ: f32 = 0.05;
const MIN_ENEMY_FIRE_COOLDOWN_S: f32 = 0.12;
const ENEMY_MASS_PER_RADIUS_SQUARED: f32 = 18.0;
const ENEMY_MIN_MASS: f32 = 2.4;
const ENEMY_MODEL_LOCAL_Z_M: f32 = 0.24;
const ENEMY_GAMEPLAY_BOX_ALPHA: f32 = 0.0;
const ENEMY_DEFAULT_VELOCITY_RESPONSE_HZ: f32 = 8.5;
const ENEMY_WALKER_VELOCITY_RESPONSE_HZ: f32 = 20.0;
const ENEMY_CHARGER_VELOCITY_RESPONSE_HZ: f32 = 24.0;
const ENEMY_BOMBER_VELOCITY_RESPONSE_HZ: f32 = 10.0;
const ENEMY_WALKER_UPHILL_SPEED_BOOST: f32 = 1.35;
const ENEMY_CHARGER_UPHILL_SPEED_BOOST: f32 = 1.5;
const ENEMY_WALKER_GROUND_FOLLOW_RATE: f32 = 18.0;
const ENEMY_CHARGER_GROUND_FOLLOW_RATE: f32 = 20.0;
const SEGMENT_BOSS_TRIGGER_BEFORE_END_M: f32 = 20.0;
const SEGMENT_BOSS_ENEMY_ID: &str = "segment_boss_drone";
const SEGMENT_BOSS_ENTRY_OFFSET_M: f32 = 6.0;
const SEGMENT_BOSS_PLAYER_GATE_GAP_M: f32 = 1.4;
const SEGMENT_PORTAL_LOADING_LOGO_PATH: &str = "sprites/autoauto_logo.jpg";
const SEGMENT_PORTAL_LOADING_MIN_SECONDS: f64 = 0.35;
const BOSS_STRAFE_AMPLITUDE_M: f32 = 16.0;
const BOSS_STRAFE_FREQUENCY_HZ: f32 = 0.11;
const BOSS_HOVER_FREQUENCY_HZ: f32 = 0.16;
const BOSS_TRACK_X_GAIN: f32 = 2.8;
const BOSS_TRACK_Y_GAIN: f32 = 3.4;
const BOSS_BASE_ALTITUDE_MIN_M: f32 = 7.5;

pub struct EnemyGameplayPlugin;

impl Plugin for EnemyGameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnemyBootstrapState>()
            .init_resource::<EnemyContactTracker>()
            .init_resource::<SegmentBossEncounterState>()
            .init_resource::<SegmentPortalTransitionState>()
            .add_message::<PlayerDamageEvent>()
            .add_message::<PlayerEnemyCrashEvent>()
            .add_message::<EnemyProjectileImpactEvent>()
            .add_message::<SegmentBossSpawnedEvent>()
            .add_message::<SegmentBossDefeatedEvent>()
            .add_systems(
                OnEnter(GameState::InRun),
                (
                    reset_enemy_bootstrap,
                    reset_enemy_contact_tracker,
                    reset_segment_boss_state,
                    reset_segment_portal_transition_state,
                ),
            )
            .add_systems(OnExit(GameState::InRun), cleanup_enemy_run_entities)
            .add_systems(
                Update,
                (
                    sync_segment_boss_state,
                    debug_warp_to_next_segment_hotkey,
                    enforce_player_boss_gate,
                    trigger_segment_boss_encounter,
                    spawn_bootstrap_enemies,
                    configure_enemy_model_visuals,
                    update_enemy_behaviors,
                    fire_enemy_projectiles,
                    simulate_enemy_projectiles,
                    resolve_enemy_projectile_hits_player,
                    apply_enemy_contact_damage_to_player,
                    handle_segment_boss_defeat_transition,
                    process_segment_portal_transition,
                    update_enemy_hit_flash_effects,
                    update_enemy_fade_out_fx,
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
struct EnemyModelVisualActive;

#[derive(Component, Debug, Clone, Copy)]
struct EnemyFadeOutFx {
    remaining_s: f32,
    total_s: f32,
    initial_alpha: f32,
}

#[derive(Component, Debug, Clone)]
struct EnemyModelScene {
    owner: Entity,
    model_id: String,
    scene_path: String,
    desired_size: Vec2,
}

#[derive(Component, Debug, Clone, Copy, Default)]
struct EnemyModelRuntime {
    configured: bool,
}

#[derive(Debug, Clone)]
struct EnemyModelSceneSpawn {
    handle: Handle<Scene>,
    metadata: EnemyModelScene,
}

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
    Boss,
}

#[derive(Resource, Debug, Default)]
struct EnemyBootstrapState {
    seeded: bool,
    wave_counter: u32,
}

#[derive(Resource, Debug, Default)]
struct EnemyContactTracker {
    currently_colliding: HashSet<Entity>,
}

#[derive(Resource, Debug, Clone)]
struct SegmentBossEncounterState {
    active_segment_index: usize,
    active_segment_id: String,
    active_segment_start_x: f32,
    active_segment_end_x: f32,
    boss_trigger_x: f32,
    boss_spawned_for_segment: bool,
    boss_alive: bool,
}

impl Default for SegmentBossEncounterState {
    fn default() -> Self {
        Self {
            active_segment_index: 0,
            active_segment_id: String::new(),
            active_segment_start_x: 0.0,
            active_segment_end_x: 0.0,
            boss_trigger_x: 0.0,
            boss_spawned_for_segment: false,
            boss_alive: false,
        }
    }
}

#[derive(Component)]
struct SegmentPortalLoadingOverlay;

#[derive(Debug, Clone)]
struct PendingSegmentPortal {
    previous_segment_id: String,
    next_segment_index: usize,
    next_segment_id: String,
    next_segment_start_x: f32,
    next_segment_end_x: f32,
    target_x: f32,
    previous_clearance_y: f32,
    started_at_s: f64,
    logo_handle: Handle<Image>,
    #[cfg(feature = "gaussian_splats")]
    next_splat_handle: Option<Handle<PlanarGaussian3d>>,
}

#[derive(Resource, Debug, Clone, Default)]
struct SegmentPortalTransitionState {
    pending: Option<PendingSegmentPortal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerDamageSource {
    ProjectileBullet,
    ProjectileMissile,
    ProjectileBomb,
    Contact,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct PlayerDamageEvent {
    pub amount: f32,
    pub source: PlayerDamageSource,
    pub source_world_position: Option<Vec2>,
}

#[derive(Message, Debug, Clone)]
pub struct PlayerEnemyCrashEvent {
    pub player_speed_mps: f32,
    pub enemy_type_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyProjectileImpactKind {
    Bullet,
    Missile,
    Bomb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyProjectileImpactTarget {
    Ground,
    Player,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct EnemyProjectileImpactEvent {
    pub kind: EnemyProjectileImpactKind,
    pub target: EnemyProjectileImpactTarget,
    pub world_position: Vec2,
}

#[derive(Message, Debug, Clone)]
pub struct SegmentBossSpawnedEvent {
    pub segment_id: String,
}

#[derive(Message, Debug, Clone)]
pub struct SegmentBossDefeatedEvent {
    pub segment_id: String,
}

fn reset_enemy_bootstrap(mut bootstrap: ResMut<EnemyBootstrapState>) {
    bootstrap.seeded = false;
}

fn reset_enemy_contact_tracker(mut tracker: ResMut<EnemyContactTracker>) {
    tracker.currently_colliding.clear();
}

fn reset_segment_boss_state(mut boss_state: ResMut<SegmentBossEncounterState>) {
    *boss_state = SegmentBossEncounterState::default();
}

fn reset_segment_portal_transition_state(
    mut commands: Commands,
    mut portal_state: ResMut<SegmentPortalTransitionState>,
    overlay_query: Query<Entity, With<SegmentPortalLoadingOverlay>>,
) {
    portal_state.pending = None;
    for entity in &overlay_query {
        commands.entity(entity).try_despawn();
    }
}

fn sync_segment_boss_state(
    config: Res<GameConfig>,
    mut boss_state: ResMut<SegmentBossEncounterState>,
    player_query: Query<&Transform, With<PlayerVehicle>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_x = player_transform.translation.x.max(0.0);
    let Some(segment_bounds) = config.active_segment_bounds_for_distance(player_x) else {
        return;
    };

    let segment_changed = boss_state.active_segment_id != segment_bounds.id
        || boss_state.active_segment_index != segment_bounds.index;
    if segment_changed {
        boss_state.active_segment_index = segment_bounds.index;
        boss_state.active_segment_id = segment_bounds.id.to_string();
        boss_state.active_segment_start_x = segment_bounds.start_x;
        boss_state.active_segment_end_x = segment_bounds.end_x;
        boss_state.boss_trigger_x =
            (segment_bounds.end_x - SEGMENT_BOSS_TRIGGER_BEFORE_END_M).max(segment_bounds.start_x);
        boss_state.boss_spawned_for_segment = false;
        boss_state.boss_alive = false;
    } else if boss_state.active_segment_end_x <= boss_state.active_segment_start_x {
        boss_state.active_segment_start_x = segment_bounds.start_x;
        boss_state.active_segment_end_x = segment_bounds.end_x;
        boss_state.boss_trigger_x =
            (segment_bounds.end_x - SEGMENT_BOSS_TRIGGER_BEFORE_END_M).max(segment_bounds.start_x);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn debug_warp_to_next_segment_hotkey(
    keyboard: Res<ButtonInput<KeyCode>>,
    config: Res<GameConfig>,
    mut commands: Commands,
    mut bootstrap: ResMut<EnemyBootstrapState>,
    mut boss_state: ResMut<SegmentBossEncounterState>,
    mut portal_state: ResMut<SegmentPortalTransitionState>,
    mut player_query: Query<
        (
            &mut Transform,
            Option<&mut Velocity>,
            Option<&mut ExternalForce>,
        ),
        With<PlayerVehicle>,
    >,
    enemy_query: Query<Entity, With<Enemy>>,
    enemy_projectile_query: Query<Entity, With<EnemyProjectile>>,
    overlay_query: Query<Entity, With<SegmentPortalLoadingOverlay>>,
) {
    if !config.game.app.debug_overlay {
        return;
    }
    if !keyboard.just_pressed(KeyCode::Tab) {
        return;
    }

    let segment_count = config.segments.segment_sequence.len();
    if segment_count == 0 {
        return;
    }

    let current_segment_index = if boss_state.active_segment_id.is_empty() {
        let Ok((player_transform, _, _)) = player_query.single_mut() else {
            return;
        };
        let player_x = player_transform.translation.x.max(0.0);
        config
            .active_segment_bounds_for_distance(player_x)
            .map(|bounds| bounds.index)
            .unwrap_or(0)
    } else {
        boss_state.active_segment_index
    };
    let next_segment_index = (current_segment_index + 1) % segment_count;
    let Some(next_segment_start_x) = config.segment_start_x_for_index(next_segment_index) else {
        return;
    };
    let Some(next_segment_bounds) =
        config.active_segment_bounds_for_distance(next_segment_start_x + 0.001)
    else {
        return;
    };

    let Ok((mut player_transform, player_velocity, player_external_force)) =
        player_query.single_mut()
    else {
        return;
    };

    let previous_ground_y = terrain_height_at_x(&config, player_transform.translation.x.max(0.0));
    let previous_clearance = (player_transform.translation.y - previous_ground_y).clamp(2.4, 16.0);
    let target_x = next_segment_start_x + SEGMENT_BOSS_ENTRY_OFFSET_M;
    let target_ground_y = terrain_height_at_x(&config, target_x);
    player_transform.translation.x = target_x;
    player_transform.translation.y = target_ground_y + previous_clearance;
    player_transform.rotation = Quat::IDENTITY;

    if let Some(mut velocity) = player_velocity {
        velocity.linvel = Vec2::ZERO;
        velocity.angvel = 0.0;
    }
    if let Some(mut external_force) = player_external_force {
        external_force.force = Vec2::ZERO;
        external_force.torque = 0.0;
    }

    for enemy_entity in &enemy_query {
        commands.entity(enemy_entity).try_despawn();
    }
    for projectile_entity in &enemy_projectile_query {
        commands.entity(projectile_entity).try_despawn();
    }
    for overlay_entity in &overlay_query {
        commands.entity(overlay_entity).try_despawn();
    }

    portal_state.pending = None;
    bootstrap.seeded = false;
    boss_state.active_segment_index = next_segment_bounds.index;
    boss_state.active_segment_id = next_segment_bounds.id.to_string();
    boss_state.active_segment_start_x = next_segment_start_x;
    boss_state.active_segment_end_x = next_segment_bounds.end_x;
    boss_state.boss_trigger_x =
        (next_segment_bounds.end_x - SEGMENT_BOSS_TRIGGER_BEFORE_END_M).max(next_segment_start_x);
    boss_state.boss_spawned_for_segment = false;
    boss_state.boss_alive = false;

    info!(
        "Debug warp: advanced to segment `{}` (index {}) at x={:.1}.",
        next_segment_bounds.id, next_segment_bounds.index, target_x
    );
}

fn enforce_player_boss_gate(
    boss_state: Res<SegmentBossEncounterState>,
    portal_state: Res<SegmentPortalTransitionState>,
    mut player_query: Query<
        (
            &mut Transform,
            Option<&mut Velocity>,
            Option<&mut ExternalForce>,
        ),
        With<PlayerVehicle>,
    >,
) {
    if !boss_state.boss_alive && portal_state.pending.is_none() {
        return;
    }

    let Ok((mut player_transform, player_velocity, player_force)) = player_query.single_mut()
    else {
        return;
    };

    let gate_x = (boss_state.boss_trigger_x - SEGMENT_BOSS_PLAYER_GATE_GAP_M)
        .max(boss_state.active_segment_start_x);
    if player_transform.translation.x <= gate_x {
        return;
    }

    player_transform.translation.x = gate_x;
    if let Some(mut velocity) = player_velocity {
        velocity.linvel.x = velocity.linvel.x.min(0.0);
    }
    if let Some(mut force) = player_force {
        force.force.x = force.force.x.min(0.0);
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn trigger_segment_boss_encounter(
    mut commands: Commands,
    config: Res<GameConfig>,
    asset_registry: Option<Res<AssetRegistry>>,
    mut bootstrap: ResMut<EnemyBootstrapState>,
    mut boss_state: ResMut<SegmentBossEncounterState>,
    mut boss_spawned_writer: MessageWriter<SegmentBossSpawnedEvent>,
    player_query: Query<&Transform, With<PlayerVehicle>>,
    enemy_query: Query<Entity, With<Enemy>>,
    enemy_projectile_query: Query<Entity, With<EnemyProjectile>>,
) {
    if boss_state.boss_spawned_for_segment {
        return;
    }

    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_x = player_transform.translation.x.max(0.0);
    if player_x < boss_state.boss_trigger_x {
        return;
    }

    let Some(enemy_cfg) = config.enemy_types_by_id.get(SEGMENT_BOSS_ENEMY_ID) else {
        warn!(
            "Boss trigger reached for segment `{}`, but enemy type `{}` is missing.",
            boss_state.active_segment_id, SEGMENT_BOSS_ENEMY_ID
        );
        boss_state.boss_spawned_for_segment = true;
        boss_state.boss_alive = false;
        return;
    };

    for enemy_entity in &enemy_query {
        commands.entity(enemy_entity).try_despawn();
    }
    for projectile_entity in &enemy_projectile_query {
        commands.entity(projectile_entity).try_despawn();
    }

    let spawn_x = boss_state.boss_trigger_x;

    spawn_enemy_instance(
        &mut commands,
        &config,
        asset_registry.as_deref(),
        enemy_cfg,
        spawn_x,
        bootstrap.wave_counter,
    );
    bootstrap.wave_counter = bootstrap.wave_counter.saturating_add(1);
    bootstrap.seeded = true;
    boss_state.boss_spawned_for_segment = true;
    boss_state.boss_alive = true;
    boss_spawned_writer.write(SegmentBossSpawnedEvent {
        segment_id: boss_state.active_segment_id.clone(),
    });

    info!(
        "Segment boss spawned: segment=`{}` trigger_x={:.1} spawn_x={:.1}.",
        boss_state.active_segment_id, boss_state.boss_trigger_x, spawn_x
    );
}

#[allow(clippy::type_complexity)]
fn cleanup_enemy_run_entities(
    mut commands: Commands,
    cleanup_query: Query<
        Entity,
        Or<(
            With<Enemy>,
            With<EnemyProjectile>,
            With<EnemyFadeOutFx>,
            With<EnemyHpBarBackground>,
            With<EnemyHpBarFill>,
            With<SegmentPortalLoadingOverlay>,
        )>,
    >,
) {
    for entity in &cleanup_query {
        commands.entity(entity).try_despawn();
    }
}

fn spawn_bootstrap_enemies(
    mut commands: Commands,
    config: Res<GameConfig>,
    asset_registry: Option<Res<AssetRegistry>>,
    mut bootstrap: ResMut<EnemyBootstrapState>,
    boss_state: Res<SegmentBossEncounterState>,
    player_query: Query<&Transform, With<PlayerVehicle>>,
) {
    if bootstrap.seeded || boss_state.boss_alive || boss_state.boss_spawned_for_segment {
        return;
    }

    let Ok(player_transform) = player_query.single() else {
        return;
    };

    if config.enemy_types.enemy_types.is_empty() {
        return;
    }

    let mut spawned_count = 0_u32;
    for enemy_cfg in &config.enemy_types.enemy_types {
        if behavior_kind_from_config(enemy_cfg.behavior.as_str()) == EnemyBehaviorKind::Boss
            || enemy_cfg.id == SEGMENT_BOSS_ENEMY_ID
        {
            continue;
        }
        let spawn_x = player_transform.translation.x
            + ENEMY_SPAWN_START_AHEAD_M
            + (spawned_count as f32 * ENEMY_SPAWN_SPACING_M);
        spawn_enemy_instance(
            &mut commands,
            &config,
            asset_registry.as_deref(),
            enemy_cfg,
            spawn_x,
            bootstrap.wave_counter + spawned_count,
        );
        spawned_count = spawned_count.saturating_add(1);
    }

    if spawned_count > 0 {
        bootstrap.wave_counter = bootstrap.wave_counter.saturating_add(spawned_count);
        bootstrap.seeded = true;
    }
}

fn spawn_enemy_instance(
    commands: &mut Commands,
    config: &GameConfig,
    asset_registry: Option<&AssetRegistry>,
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
        EnemyBehaviorKind::Boss => {
            ground_y + enemy_cfg.hover_amplitude.max(BOSS_BASE_ALTITUDE_MIN_M)
        }
        _ => ground_y,
    };

    let start_y = match behavior_kind {
        EnemyBehaviorKind::Flier => {
            base_altitude + phase_offset.sin() * enemy_cfg.hover_amplitude.max(0.5)
        }
        EnemyBehaviorKind::Bomber => base_altitude,
        EnemyBehaviorKind::Boss => base_altitude,
        _ => ground_y,
    };
    let enemy_mass = enemy_mass_from_hitbox(enemy_cfg.hitbox_radius);
    let gravity_scale = match behavior_kind {
        EnemyBehaviorKind::Flier | EnemyBehaviorKind::Bomber | EnemyBehaviorKind::Boss => 0.0,
        _ => 1.0,
    };
    let gameplay_box_color = body_color.with_alpha(ENEMY_GAMEPLAY_BOX_ALPHA);

    let enemy_entity = commands
        .spawn((
            Name::new(format!("Enemy/{}", enemy_cfg.id)),
            Enemy,
            EnemyDebugMarker,
            EnemyTypeId(enemy_cfg.id.clone()),
            EnemyBaseColor(gameplay_box_color),
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
            EnemyAttackState {
                cooldown_s: 0.35 + ((sequence as f32 * 0.17).rem_euclid(0.8)),
                rng_state: 0xD8E5_3A1C_9F2B_4D11 ^ sequence as u64 ^ (enemy_cfg.id.len() as u64),
            },
            Sprite::from_color(gameplay_box_color, body_size),
            Transform::from_xyz(spawn_x, start_y, 8.0),
        ))
        .insert((
            RigidBody::Dynamic,
            Collider::ball(enemy_cfg.hitbox_radius.max(0.08)),
            ColliderMassProperties::Mass(enemy_mass),
            Friction::coefficient(1.10),
            Restitution::coefficient(0.02),
            GravityScale(gravity_scale),
            Damping {
                linear_damping: 2.4,
                angular_damping: 3.2,
            },
            Velocity::zero(),
            LockedAxes::ROTATION_LOCKED,
            Ccd::enabled(),
            Sleeping::disabled(),
        ))
        .id();

    let model_scene = asset_registry
        .and_then(|registry| resolve_enemy_model_entry(registry, &enemy_cfg.id, behavior_kind))
        .and_then(|(model_id, model_entry)| {
            model_entry
                .handle
                .as_ref()
                .map(|handle| EnemyModelSceneSpawn {
                    handle: handle.clone(),
                    metadata: EnemyModelScene {
                        owner: enemy_entity,
                        model_id,
                        scene_path: model_entry.scene_path.clone(),
                        desired_size: body_size,
                    },
                })
        });

    commands.entity(enemy_entity).with_children(|parent| {
        if let Some(model_scene) = &model_scene {
            parent.spawn((
                Name::new("EnemyModelScene"),
                model_scene.metadata.clone(),
                EnemyModelRuntime::default(),
                SceneRoot(model_scene.handle.clone()),
                Transform::from_xyz(0.0, 0.0, ENEMY_MODEL_LOCAL_Z_M),
            ));
        }

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
fn configure_enemy_model_visuals(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    mut scene_query: Query<(
        Entity,
        &EnemyModelScene,
        &mut EnemyModelRuntime,
        &mut Transform,
        &GlobalTransform,
    )>,
    children_query: Query<&Children>,
    mesh_node_query: Query<(&Mesh3d, &GlobalTransform), Without<EnemyModelScene>>,
    mut enemy_sprite_query: Query<&mut Sprite, With<Enemy>>,
) {
    for (scene_entity, model, mut runtime, mut scene_transform, scene_global) in &mut scene_query {
        if runtime.configured {
            continue;
        }

        let mut descendants = Vec::new();
        collect_descendants(scene_entity, &children_query, &mut descendants);
        if descendants.is_empty() {
            continue;
        }

        let scene_inverse = scene_global.affine().inverse();
        let mut local_min = Vec3::splat(f32::INFINITY);
        let mut local_max = Vec3::splat(f32::NEG_INFINITY);
        let mut mesh_count = 0usize;

        for descendant in descendants {
            let Ok((mesh3d, node_global)) = mesh_node_query.get(descendant) else {
                continue;
            };
            let Some(mesh) = meshes.get(&mesh3d.0) else {
                continue;
            };
            let Some((mesh_min, mesh_max)) = mesh_local_bounds(mesh) else {
                continue;
            };
            mesh_count += 1;

            for corner in aabb_corners(mesh_min, mesh_max) {
                let world_point = node_global.affine().transform_point3(corner);
                let root_local = scene_inverse.transform_point3(world_point);
                local_min = local_min.min(root_local);
                local_max = local_max.max(root_local);
            }
        }

        if mesh_count == 0 || !local_min.is_finite() || !local_max.is_finite() {
            continue;
        }

        let source_size = local_max - local_min;
        let target_size = model.desired_size.max(Vec2::splat(0.05));
        let scale_x = target_size.x / source_size.x.max(0.001);
        let scale_y = target_size.y / source_size.y.max(0.001);
        let fit_scale = scale_x.min(scale_y).clamp(0.01, 500.0);
        let model_scale_multiplier = enemy_model_scale_multiplier(model);
        let uniform_scale = (fit_scale * model_scale_multiplier).clamp(0.01, 500.0);
        let model_rotation = enemy_model_rotation(model);

        let source_center = (local_min + local_max) * 0.5;
        let rotated_center = model_rotation * source_center;
        scene_transform.rotation = model_rotation;
        scene_transform.scale = Vec3::splat(uniform_scale);
        scene_transform.translation = Vec3::new(
            -rotated_center.x * uniform_scale,
            -rotated_center.y * uniform_scale,
            ENEMY_MODEL_LOCAL_Z_M - (rotated_center.z * uniform_scale),
        );
        runtime.configured = true;

        commands.entity(model.owner).insert(EnemyModelVisualActive);
        if let Ok(mut sprite) = enemy_sprite_query.get_mut(model.owner) {
            sprite.color = sprite.color.with_alpha(0.0);
        }

        info!(
            "Enemy model setup: enemy=`{}` model=`{}` scene=`{}` fitted size=({:.3}, {:.3}) source=({:.3}, {:.3}, {:.3}) fit_scale={:.3} final_scale={:.3} rotation_y_deg={:.1}",
            model.owner.index(),
            model.model_id,
            model.scene_path,
            target_size.x,
            target_size.y,
            source_size.x,
            source_size.y,
            source_size.z,
            fit_scale,
            uniform_scale,
            scene_transform.rotation.to_euler(EulerRot::XYZ).1.to_degrees()
        );
    }
}

#[allow(clippy::type_complexity)]
fn update_enemy_behaviors(
    time: Res<Time>,
    config: Res<GameConfig>,
    boss_state: Res<SegmentBossEncounterState>,
    player_query: Query<&Transform, (With<PlayerVehicle>, Without<Enemy>)>,
    mut enemy_query: Query<
        (
            &mut Transform,
            &mut Velocity,
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

    for (mut transform, mut velocity, mut behavior, motion, hitbox, enemy_type_id) in
        &mut enemy_query
    {
        let enemy_position = transform.translation.truncate();
        let ground_offset = hitbox.radius_m.max(0.15);
        behavior.elapsed_s += dt;
        let mut desired_velocity = velocity.linvel;

        match behavior.kind {
            EnemyBehaviorKind::Walker => {
                let ground_tangent = terrain_tangent_at_x(&config, enemy_position.x);
                let ground_y = terrain_height_at_x(&config, enemy_position.x) + ground_offset;
                let climb_boost = if -ground_tangent.y > 0.0 {
                    ENEMY_WALKER_UPHILL_SPEED_BOOST
                } else {
                    1.0
                };
                let along_ground = -ground_tangent * (motion.base_speed_mps * climb_boost);
                desired_velocity.x = along_ground.x;
                desired_velocity.y = along_ground.y
                    + ((ground_y - enemy_position.y) * ENEMY_WALKER_GROUND_FOLLOW_RATE);
                transform.rotation =
                    Quat::from_rotation_z(ground_tangent.y.atan2(ground_tangent.x));
            }
            EnemyBehaviorKind::Flier => {
                desired_velocity.x = -(motion.base_speed_mps * 0.82);
                let hover = (behavior.elapsed_s * behavior.hover_frequency_hz * TAU
                    + behavior.phase_offset_rad)
                    .sin()
                    * behavior.hover_amplitude_m;
                let target_y = behavior.base_altitude_m + hover;
                desired_velocity.y = (target_y - enemy_position.y) * 7.0;
            }
            EnemyBehaviorKind::Turret => {
                desired_velocity.x = -(motion.base_speed_mps * 0.06);
                let ground_y = terrain_height_at_x(&config, enemy_position.x) + ground_offset;
                desired_velocity.y = (ground_y - enemy_position.y) * GROUND_FOLLOW_SNAP_RATE;
            }
            EnemyBehaviorKind::Charger => {
                let distance_to_player = enemy_position.x - player_x;
                let charge_multiplier = if distance_to_player <= 20.0 {
                    behavior.charge_speed_multiplier
                } else {
                    0.55
                };
                let ground_tangent = terrain_tangent_at_x(&config, enemy_position.x);
                let ground_y = terrain_height_at_x(&config, enemy_position.x) + ground_offset;
                let climb_boost = if -ground_tangent.y > 0.0 {
                    ENEMY_CHARGER_UPHILL_SPEED_BOOST
                } else {
                    1.0
                };
                let along_ground =
                    -ground_tangent * (motion.base_speed_mps * charge_multiplier * climb_boost);
                desired_velocity.x = along_ground.x;
                desired_velocity.y = along_ground.y
                    + ((ground_y - enemy_position.y) * ENEMY_CHARGER_GROUND_FOLLOW_RATE);
                transform.rotation =
                    Quat::from_rotation_z(ground_tangent.y.atan2(ground_tangent.x));
            }
            EnemyBehaviorKind::Bomber => {
                desired_velocity.x = -(motion.base_speed_mps * 0.95);
                let cruise_wave_amplitude = (behavior.hover_amplitude_m
                    * ENEMY_BOMBER_CRUISE_WAVE_AMPLITUDE_FACTOR)
                    .clamp(0.5, ENEMY_BOMBER_CRUISE_WAVE_AMPLITUDE_MAX_M);
                let cruise_wave =
                    ((behavior.elapsed_s * ENEMY_BOMBER_CRUISE_WAVE_FREQUENCY_HZ * TAU)
                        + behavior.phase_offset_rad)
                        .sin()
                        * cruise_wave_amplitude;
                let target_y = behavior.base_altitude_m + cruise_wave;
                desired_velocity.y = (target_y - enemy_position.y) * 4.0;
            }
            EnemyBehaviorKind::Boss => {
                let anchor_x = boss_state.boss_trigger_x;
                if transform.translation.x > anchor_x {
                    transform.translation.x = anchor_x;
                    velocity.linvel.x = velocity.linvel.x.min(0.0);
                }

                let strafe_left = ((behavior.elapsed_s * BOSS_STRAFE_FREQUENCY_HZ * TAU)
                    + behavior.phase_offset_rad)
                    .sin()
                    .abs()
                    * BOSS_STRAFE_AMPLITUDE_M;
                let target_x = anchor_x - strafe_left;
                desired_velocity.x = (target_x - enemy_position.x) * BOSS_TRACK_X_GAIN;

                let hover = ((behavior.elapsed_s * BOSS_HOVER_FREQUENCY_HZ * TAU)
                    + behavior.phase_offset_rad * 0.75)
                    .sin()
                    * behavior.hover_amplitude_m.max(1.5);
                let target_y = behavior.base_altitude_m + hover;
                desired_velocity.y = (target_y - enemy_position.y) * BOSS_TRACK_Y_GAIN;
            }
        }

        let response_hz = match behavior.kind {
            EnemyBehaviorKind::Walker => ENEMY_WALKER_VELOCITY_RESPONSE_HZ,
            EnemyBehaviorKind::Charger => ENEMY_CHARGER_VELOCITY_RESPONSE_HZ,
            EnemyBehaviorKind::Bomber => ENEMY_BOMBER_VELOCITY_RESPONSE_HZ,
            EnemyBehaviorKind::Boss => ENEMY_BOMBER_VELOCITY_RESPONSE_HZ,
            _ => ENEMY_DEFAULT_VELOCITY_RESPONSE_HZ,
        };
        let smooth = (response_hz * dt).clamp(0.0, 1.0);
        velocity.linvel = velocity.linvel.lerp(desired_velocity, smooth);
        velocity.linvel.y = velocity.linvel.y.clamp(-40.0, 40.0);
        velocity.linvel.x = velocity.linvel.x.clamp(-90.0, 90.0);

        if enemy_type_id.0.is_empty() {
            warn!("Encountered enemy with empty type id.");
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
        let attack_range_m = if behavior.kind == EnemyBehaviorKind::Boss {
            BOSS_ATTACK_RANGE_M
        } else {
            ENEMY_ATTACK_RANGE_M
        };
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

        if distance_to_player_m <= 0.001 || distance_to_player_m > attack_range_m {
            attack_state.cooldown_s = fire_cooldown_s;
            continue;
        }

        let aim_direction = clamp_enemy_fire_direction(to_player.normalize_or_zero());
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
                let shot_direction_world =
                    clamp_enemy_fire_direction(Vec2::from_angle(shot_angle).normalize_or_zero());

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

fn clamp_enemy_fire_direction(direction: Vec2) -> Vec2 {
    let mut dir = if direction.length_squared() > f32::EPSILON {
        direction.normalize()
    } else {
        Vec2::NEG_X
    };

    if dir.x > 0.0 {
        dir.x = 0.0;
        if dir.y < 0.15 {
            dir.y = 0.15;
        }
    }

    if dir.length_squared() > f32::EPSILON {
        dir.normalize()
    } else {
        Vec2::Y
    }
}

fn simulate_enemy_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    mut impact_writer: MessageWriter<EnemyProjectileImpactEvent>,
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
            impact_writer.write(EnemyProjectileImpactEvent {
                kind: enemy_projectile_impact_kind(projectile.kind),
                target: EnemyProjectileImpactTarget::Ground,
                world_position: Vec2::new(transform.translation.x, ground_y),
            });
            commands.entity(entity).try_despawn();
            continue;
        }

        projectile.remaining_lifetime_s -= dt;
        if projectile.remaining_lifetime_s <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

fn resolve_enemy_projectile_hits_player(
    mut commands: Commands,
    debug_guards: Option<Res<DebugGameplayGuards>>,
    mut player_damage_writer: MessageWriter<PlayerDamageEvent>,
    mut impact_writer: MessageWriter<EnemyProjectileImpactEvent>,
    projectile_query: Query<(Entity, &Transform, &EnemyProjectile)>,
    mut player_query: Query<(&Transform, &mut PlayerHealth), With<PlayerVehicle>>,
) {
    let Ok((player_transform, mut player_health)) = player_query.single_mut() else {
        return;
    };
    let player_position = player_transform.translation.truncate();

    let mut total_damage = 0.0;
    let mut bullet_damage = 0.0;
    let mut missile_damage = 0.0;
    let mut bomb_damage = 0.0;
    let mut bullet_source_sum = Vec2::ZERO;
    let mut bullet_source_weight = 0.0;
    let mut missile_source_sum = Vec2::ZERO;
    let mut missile_source_weight = 0.0;
    let mut bomb_source_sum = Vec2::ZERO;
    let mut bomb_source_weight = 0.0;
    let mut consumed_projectiles = Vec::new();
    for (projectile_entity, transform, projectile) in &projectile_query {
        let projectile_position = transform.translation.truncate();
        let combined_hit_radius = PLAYER_PROJECTILE_HIT_RADIUS_M + projectile.hit_radius_m;
        if projectile_position.distance_squared(player_position)
            <= (combined_hit_radius * combined_hit_radius)
        {
            let hit_damage = projectile.damage.max(0.0);
            total_damage += hit_damage;
            match projectile.kind {
                EnemyProjectileKind::Bullet => {
                    bullet_damage += hit_damage;
                    bullet_source_sum += projectile_position * hit_damage.max(0.001);
                    bullet_source_weight += hit_damage.max(0.001);
                }
                EnemyProjectileKind::Missile => {
                    missile_damage += hit_damage;
                    missile_source_sum += projectile_position * hit_damage.max(0.001);
                    missile_source_weight += hit_damage.max(0.001);
                }
                EnemyProjectileKind::Bomb => {
                    bomb_damage += hit_damage;
                    bomb_source_sum += projectile_position * hit_damage.max(0.001);
                    bomb_source_weight += hit_damage.max(0.001);
                }
            }
            consumed_projectiles.push((projectile_entity, projectile.kind, projectile_position));
        }
    }

    let player_invulnerable = debug_guards
        .as_ref()
        .is_some_and(|guards| guards.player_invulnerable);
    if total_damage > 0.0 && !player_invulnerable {
        player_health.current = (player_health.current - total_damage).max(0.0);
        if bullet_damage > 0.0 {
            player_damage_writer.write(PlayerDamageEvent {
                amount: bullet_damage,
                source: PlayerDamageSource::ProjectileBullet,
                source_world_position: if bullet_source_weight > f32::EPSILON {
                    Some(bullet_source_sum / bullet_source_weight)
                } else {
                    None
                },
            });
        }
        if missile_damage > 0.0 {
            player_damage_writer.write(PlayerDamageEvent {
                amount: missile_damage,
                source: PlayerDamageSource::ProjectileMissile,
                source_world_position: if missile_source_weight > f32::EPSILON {
                    Some(missile_source_sum / missile_source_weight)
                } else {
                    None
                },
            });
        }
        if bomb_damage > 0.0 {
            player_damage_writer.write(PlayerDamageEvent {
                amount: bomb_damage,
                source: PlayerDamageSource::ProjectileBomb,
                source_world_position: if bomb_source_weight > f32::EPSILON {
                    Some(bomb_source_sum / bomb_source_weight)
                } else {
                    None
                },
            });
        }
    }

    for (projectile_entity, projectile_kind, projectile_position) in consumed_projectiles {
        impact_writer.write(EnemyProjectileImpactEvent {
            kind: enemy_projectile_impact_kind(projectile_kind),
            target: EnemyProjectileImpactTarget::Player,
            world_position: projectile_position,
        });
        commands.entity(projectile_entity).try_despawn();
    }
}

fn enemy_projectile_impact_kind(kind: EnemyProjectileKind) -> EnemyProjectileImpactKind {
    match kind {
        EnemyProjectileKind::Bullet => EnemyProjectileImpactKind::Bullet,
        EnemyProjectileKind::Missile => EnemyProjectileImpactKind::Missile,
        EnemyProjectileKind::Bomb => EnemyProjectileImpactKind::Bomb,
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_enemy_contact_damage_to_player(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    debug_guards: Option<Res<DebugGameplayGuards>>,
    mut contact_tracker: ResMut<EnemyContactTracker>,
    mut killed_message_writer: MessageWriter<EnemyKilledEvent>,
    mut player_damage_writer: MessageWriter<PlayerDamageEvent>,
    mut crash_event_writer: MessageWriter<PlayerEnemyCrashEvent>,
    mut player_query: Query<(&Transform, &Velocity, &mut PlayerHealth), With<PlayerVehicle>>,
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
) {
    let Ok((player_transform, player_velocity, mut player_health)) = player_query.single_mut()
    else {
        return;
    };
    let player_position = player_transform.translation.truncate();
    let player_speed_mps = player_velocity.linvel.length();
    let dt = time.delta_secs();

    let mut total_contact_damage = 0.0;
    let mut contact_source_sum = Vec2::ZERO;
    let mut contact_source_weight = 0.0;
    let mut dead_enemies: Vec<(Entity, String, Vec2)> = Vec::new();
    let mut current_colliding_enemies = HashSet::new();
    for (enemy_entity, enemy_transform, enemy_hitbox, enemy_type_id, mut enemy_health) in
        &mut enemy_query
    {
        let enemy_position = enemy_transform.translation.truncate();
        if enemy_health.current <= 0.0 {
            dead_enemies.push((enemy_entity, enemy_type_id.0.clone(), enemy_position));
            continue;
        }

        let Some(enemy_type) = config.enemy_types_by_id.get(&enemy_type_id.0) else {
            continue;
        };

        let combined_radius = PLAYER_CONTACT_HIT_RADIUS_M + enemy_hitbox.radius_m;
        let distance_sq = enemy_transform
            .translation
            .truncate()
            .distance_squared(player_position);
        if distance_sq <= combined_radius * combined_radius {
            current_colliding_enemies.insert(enemy_entity);
            let enemy_contact_damage = enemy_type.contact_damage.max(0.0) * dt;
            total_contact_damage += enemy_contact_damage;
            contact_source_sum += enemy_position * enemy_contact_damage.max(0.001);
            contact_source_weight += enemy_contact_damage.max(0.001);

            if player_speed_mps >= PLAYER_CRASH_MIN_SPEED_MPS {
                if !contact_tracker.currently_colliding.contains(&enemy_entity) {
                    crash_event_writer.write(PlayerEnemyCrashEvent {
                        player_speed_mps,
                        enemy_type_id: enemy_type_id.0.clone(),
                    });
                }
                let crash_damage = (PLAYER_CRASH_DAMAGE_TO_ENEMY_BASE_PER_SECOND
                    + (player_speed_mps * PLAYER_CRASH_DAMAGE_TO_ENEMY_PER_MPS_PER_SECOND))
                    * dt;
                enemy_health.current = (enemy_health.current - crash_damage.max(0.0)).max(0.0);
                if enemy_health.current <= 0.0 {
                    dead_enemies.push((enemy_entity, enemy_type_id.0.clone(), enemy_position));
                }
            }
        }
    }

    let player_invulnerable = debug_guards
        .as_ref()
        .is_some_and(|guards| guards.player_invulnerable);
    if total_contact_damage > 0.0 && !player_invulnerable {
        player_health.current = (player_health.current - total_contact_damage).max(0.0);
        player_damage_writer.write(PlayerDamageEvent {
            amount: total_contact_damage,
            source: PlayerDamageSource::Contact,
            source_world_position: if contact_source_weight > f32::EPSILON {
                Some(contact_source_sum / contact_source_weight)
            } else {
                None
            },
        });
    }
    contact_tracker.currently_colliding = current_colliding_enemies;

    for (enemy_entity, enemy_type_id, world_position) in dead_enemies {
        killed_message_writer.write(EnemyKilledEvent {
            enemy_type_id,
            world_position,
        });
        commands.entity(enemy_entity).try_despawn();
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn handle_segment_boss_defeat_transition(
    mut commands: Commands,
    config: Res<GameConfig>,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
    mut kill_events: MessageReader<EnemyKilledEvent>,
    mut boss_defeated_writer: MessageWriter<SegmentBossDefeatedEvent>,
    mut bootstrap: ResMut<EnemyBootstrapState>,
    mut boss_state: ResMut<SegmentBossEncounterState>,
    mut portal_state: ResMut<SegmentPortalTransitionState>,
    mut player_query: Query<
        (
            &mut Transform,
            Option<&mut Velocity>,
            Option<&mut ExternalForce>,
        ),
        With<PlayerVehicle>,
    >,
    enemy_query: Query<Entity, With<Enemy>>,
    enemy_projectile_query: Query<Entity, With<EnemyProjectile>>,
) {
    if !boss_state.boss_alive || portal_state.pending.is_some() {
        return;
    }

    let mut boss_killed = false;
    for kill in kill_events.read() {
        if kill.enemy_type_id == SEGMENT_BOSS_ENEMY_ID {
            boss_killed = true;
        }
    }
    if !boss_killed {
        return;
    }

    let previous_segment_id = boss_state.active_segment_id.clone();

    let segment_count = config.segments.segment_sequence.len();
    if segment_count == 0 {
        return;
    }
    let next_segment_index = (boss_state.active_segment_index + 1) % segment_count;
    let Some(next_segment_start_x) = config.segment_start_x_for_index(next_segment_index) else {
        return;
    };
    let Some(next_segment_bounds) =
        config.active_segment_bounds_for_distance(next_segment_start_x + 0.001)
    else {
        return;
    };

    let Ok((player_transform, player_velocity, player_external_force)) = player_query.single_mut()
    else {
        return;
    };
    let previous_x = player_transform.translation.x;
    let previous_ground_y = terrain_height_at_x(&config, previous_x);
    let previous_clearance = (player_transform.translation.y - previous_ground_y).clamp(2.4, 16.0);
    let target_x = next_segment_start_x + SEGMENT_BOSS_ENTRY_OFFSET_M;

    if let Some(mut velocity) = player_velocity {
        velocity.linvel = Vec2::ZERO;
        velocity.angvel = 0.0;
    }
    if let Some(mut external_force) = player_external_force {
        external_force.force = Vec2::ZERO;
        external_force.torque = 0.0;
    }

    for enemy_entity in &enemy_query {
        commands.entity(enemy_entity).try_despawn();
    }
    for projectile_entity in &enemy_projectile_query {
        commands.entity(projectile_entity).try_despawn();
    }

    bootstrap.seeded = false;
    boss_state.boss_spawned_for_segment = true;
    boss_state.boss_alive = false;

    let logo_handle = asset_server.load(SEGMENT_PORTAL_LOADING_LOGO_PATH);
    commands.spawn((
        Name::new("SegmentPortalLoadingOverlay"),
        SegmentPortalLoadingOverlay,
        Sprite::from_image(logo_handle.clone()),
        Transform::from_xyz(0.0, 0.0, 300.0),
    ));

    #[cfg(feature = "gaussian_splats")]
    let next_splat_handle = config
        .backgrounds_by_id
        .get(next_segment_bounds.id)
        .and_then(|background| background.splat_asset_id.as_deref())
        .and_then(|splat_id| config.splat_assets_by_id.get(splat_id))
        .map(|splat_cfg| asset_server.load::<PlanarGaussian3d>(splat_cfg.path.clone()));

    portal_state.pending = Some(PendingSegmentPortal {
        previous_segment_id: previous_segment_id.clone(),
        next_segment_index: next_segment_bounds.index,
        next_segment_id: next_segment_bounds.id.to_string(),
        next_segment_start_x,
        next_segment_end_x: next_segment_bounds.end_x,
        target_x,
        previous_clearance_y: previous_clearance,
        started_at_s: time.elapsed_secs_f64(),
        logo_handle,
        #[cfg(feature = "gaussian_splats")]
        next_splat_handle,
    });
    boss_defeated_writer.write(SegmentBossDefeatedEvent {
        segment_id: previous_segment_id.clone(),
    });

    info!(
        "Boss defeated in segment `{}`; queued portal transition to segment `{}` at x={:.1}.",
        previous_segment_id, next_segment_bounds.id, target_x
    );
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn process_segment_portal_transition(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    asset_server: Res<AssetServer>,
    mut bootstrap: ResMut<EnemyBootstrapState>,
    mut boss_state: ResMut<SegmentBossEncounterState>,
    mut portal_state: ResMut<SegmentPortalTransitionState>,
    mut player_query: Query<
        (
            &mut Transform,
            Option<&mut Velocity>,
            Option<&mut ExternalForce>,
        ),
        With<PlayerVehicle>,
    >,
    overlay_query: Query<Entity, With<SegmentPortalLoadingOverlay>>,
) {
    let Some(pending) = portal_state.pending.as_ref() else {
        return;
    };

    let Ok((mut player_transform, mut player_velocity, mut player_force)) =
        player_query.single_mut()
    else {
        return;
    };

    if let Some(velocity) = &mut player_velocity {
        velocity.linvel = Vec2::ZERO;
        velocity.angvel = 0.0;
    }
    if let Some(force) = &mut player_force {
        force.force = Vec2::ZERO;
        force.torque = 0.0;
    }

    let min_time_elapsed =
        (time.elapsed_secs_f64() - pending.started_at_s) >= SEGMENT_PORTAL_LOADING_MIN_SECONDS;
    if !min_time_elapsed {
        return;
    }

    let logo_loaded = asset_server.is_loaded_with_dependencies(pending.logo_handle.id());
    let logo_failed = matches!(
        asset_server.load_state(pending.logo_handle.id()),
        LoadState::Failed(_)
    );
    if !logo_loaded && !logo_failed {
        return;
    }

    #[cfg(feature = "gaussian_splats")]
    {
        if let Some(next_splat_handle) = pending.next_splat_handle.as_ref() {
            let splat_loaded = asset_server.is_loaded_with_dependencies(next_splat_handle.id());
            let splat_failed = matches!(
                asset_server.load_state(next_splat_handle.id()),
                LoadState::Failed(_)
            );
            if !splat_loaded && !splat_failed {
                return;
            }
        }
    }

    let Some(pending) = portal_state.pending.take() else {
        return;
    };

    let target_ground_y = terrain_height_at_x(&config, pending.target_x);
    player_transform.translation.x = pending.target_x;
    player_transform.translation.y = target_ground_y + pending.previous_clearance_y;
    player_transform.rotation = Quat::IDENTITY;

    if let Some(velocity) = &mut player_velocity {
        velocity.linvel = Vec2::ZERO;
        velocity.angvel = 0.0;
    }
    if let Some(force) = &mut player_force {
        force.force = Vec2::ZERO;
        force.torque = 0.0;
    }

    bootstrap.seeded = false;
    boss_state.active_segment_index = pending.next_segment_index;
    boss_state.active_segment_id = pending.next_segment_id.clone();
    boss_state.active_segment_start_x = pending.next_segment_start_x;
    boss_state.active_segment_end_x = pending.next_segment_end_x;
    boss_state.boss_trigger_x = (pending.next_segment_end_x - SEGMENT_BOSS_TRIGGER_BEFORE_END_M)
        .max(pending.next_segment_start_x);
    boss_state.boss_spawned_for_segment = false;
    boss_state.boss_alive = false;

    for entity in &overlay_query {
        commands.entity(entity).try_despawn();
    }

    info!(
        "Boss portal completed: segment `{}` -> `{}` at x={:.1}.",
        pending.previous_segment_id, pending.next_segment_id, pending.target_x
    );
}

fn despawn_far_enemies(
    mut commands: Commands,
    player_query: Query<&Transform, With<PlayerVehicle>>,
    enemy_query: Query<(Entity, &Transform, &EnemyTypeId), With<Enemy>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let min_x = player_transform.translation.x - ENEMY_DESPAWN_BEHIND_M;
    let max_x = player_transform.translation.x + ENEMY_DESPAWN_AHEAD_M;

    for (entity, transform, enemy_type_id) in &enemy_query {
        if enemy_type_id.0 == SEGMENT_BOSS_ENEMY_ID {
            continue;
        }
        if transform.translation.x < min_x || transform.translation.x > max_x {
            commands.entity(entity).try_despawn();
        }
    }
}

fn rearm_bootstrap_when_empty(
    mut bootstrap: ResMut<EnemyBootstrapState>,
    boss_state: Res<SegmentBossEncounterState>,
    enemy_query: Query<Entity, With<Enemy>>,
) {
    if bootstrap.seeded && enemy_query.is_empty() && !boss_state.boss_alive {
        bootstrap.seeded = false;
    }
}

fn shot_pattern_for_behavior(kind: EnemyBehaviorKind) -> ([f32; 3], usize) {
    match kind {
        EnemyBehaviorKind::Walker | EnemyBehaviorKind::Turret | EnemyBehaviorKind::Bomber => {
            ([0.0, 0.0, 0.0], 1)
        }
        EnemyBehaviorKind::Flier => ([ENEMY_FLIER_ARC_ANGLE_RAD, 0.0, 0.0], 1),
        EnemyBehaviorKind::Boss => ([-0.14, 0.0, 0.14], 3),
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

    spawn_enemy_muzzle_flash(commands, muzzle_world, color);

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

fn spawn_enemy_muzzle_flash(commands: &mut Commands, muzzle_world: Vec2, projectile_color: Color) {
    let rgba = projectile_color.to_srgba();
    let flash_color = Color::srgba(rgba.red, rgba.green, rgba.blue, 0.86);
    commands.spawn((
        Name::new("EnemyMuzzleFlashFx"),
        EnemyFadeOutFx {
            remaining_s: ENEMY_MUZZLE_FLASH_LIFETIME_S,
            total_s: ENEMY_MUZZLE_FLASH_LIFETIME_S.max(0.001),
            initial_alpha: flash_color.to_srgba().alpha,
        },
        Sprite::from_color(flash_color, ENEMY_MUZZLE_FLASH_SIZE_M),
        Transform::from_xyz(muzzle_world.x, muzzle_world.y, ENEMY_MUZZLE_FLASH_Z_M),
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

#[allow(clippy::type_complexity)]
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
        (With<Enemy>, Without<EnemyModelVisualActive>),
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

fn update_enemy_fade_out_fx(
    mut commands: Commands,
    time: Res<Time>,
    mut fx_query: Query<(Entity, &mut EnemyFadeOutFx, &mut Sprite)>,
) {
    let dt = time.delta_secs().max(0.000_1);
    for (entity, mut fx, mut sprite) in &mut fx_query {
        fx.remaining_s -= dt;
        let life_t = (fx.remaining_s / fx.total_s.max(0.001)).clamp(0.0, 1.0);
        let mut color = sprite.color;
        color.set_alpha(fx.initial_alpha * life_t);
        sprite.color = color;
        if fx.remaining_s <= 0.0 {
            commands.entity(entity).try_despawn();
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
        EnemyBehaviorKind::Boss => Vec2::new(r * 2.1, r * 1.7),
    }
}

fn color_for_behavior(kind: EnemyBehaviorKind) -> Color {
    match kind {
        EnemyBehaviorKind::Walker => Color::srgb(0.86, 0.57, 0.36),
        EnemyBehaviorKind::Flier => Color::srgb(0.54, 0.74, 0.92),
        EnemyBehaviorKind::Turret => Color::srgb(0.81, 0.54, 0.84),
        EnemyBehaviorKind::Charger => Color::srgb(0.90, 0.41, 0.41),
        EnemyBehaviorKind::Bomber => Color::srgb(0.73, 0.78, 0.85),
        EnemyBehaviorKind::Boss => Color::srgb(0.80, 0.90, 0.34),
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
        "boss" => EnemyBehaviorKind::Boss,
        _ => EnemyBehaviorKind::Walker,
    }
}

fn resolve_enemy_model_entry<'a>(
    registry: &'a AssetRegistry,
    enemy_type_id: &str,
    behavior_kind: EnemyBehaviorKind,
) -> Option<(String, &'a ModelAssetEntry)> {
    let specific_id = format!("enemy_{enemy_type_id}");
    if let Some(entry) = registry.models.get(&specific_id) {
        return Some((specific_id, entry));
    }

    let behavior_id = match behavior_kind {
        EnemyBehaviorKind::Turret => Some("enemy_turret_model"),
        EnemyBehaviorKind::Bomber => Some("enemy_bomber_model"),
        EnemyBehaviorKind::Boss => Some("enemy_drone_flier"),
        _ => None,
    }?;
    registry
        .models
        .get(behavior_id)
        .map(|entry| (behavior_id.to_string(), entry))
}

fn enemy_model_scale_multiplier(model: &EnemyModelScene) -> f32 {
    if model.scene_path.contains("owl_tower") {
        2.0
    } else if model.scene_path.contains("owl_bomber") {
        3.0
    } else if model.scene_path.contains("beetle_rough") {
        2.0
    } else if model.scene_path.contains("bullfinch") {
        2.5
    } else {
        1.0
    }
}

fn enemy_model_rotation(model: &EnemyModelScene) -> Quat {
    if model.scene_path.contains("owl_tower")
        || model.scene_path.contains("owl_bomber")
        || model.scene_path.contains("beetle_rough")
        || model.scene_path.contains("beetle_green")
        || model.scene_path.contains("bullfinch")
    {
        // Imported GLBs here are forward on +Z; rotate so they face world -X (left).
        Quat::from_rotation_y(-FRAC_PI_2)
    } else {
        Quat::IDENTITY
    }
}

fn collect_descendants(root: Entity, children_query: &Query<&Children>, out: &mut Vec<Entity>) {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        let Ok(children) = children_query.get(entity) else {
            continue;
        };
        for child in children.iter() {
            out.push(child);
            stack.push(child);
        }
    }
}

fn mesh_local_bounds(mesh: &Mesh) -> Option<(Vec3, Vec3)> {
    let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?;
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);

    match positions {
        VertexAttributeValues::Float32x3(values) => {
            for [x, y, z] in values {
                let point = Vec3::new(*x, *y, *z);
                min = min.min(point);
                max = max.max(point);
            }
        }
        VertexAttributeValues::Float32x4(values) => {
            for [x, y, z, _w] in values {
                let point = Vec3::new(*x, *y, *z);
                min = min.min(point);
                max = max.max(point);
            }
        }
        _ => return None,
    }

    if min.x.is_finite() && min.y.is_finite() && min.z.is_finite() {
        Some((min, max))
    } else {
        None
    }
}

fn aabb_corners(min: Vec3, max: Vec3) -> [Vec3; 8] {
    [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, max.y, max.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(max.x, max.y, max.z),
    ]
}

fn terrain_height_at_x(config: &GameConfig, x: f32) -> f32 {
    config.terrain_height_at_x(x)
}

fn terrain_tangent_at_x(config: &GameConfig, x: f32) -> Vec2 {
    config.terrain_tangent_at_x(x)
}
