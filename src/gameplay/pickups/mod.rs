use crate::config::{GameConfig, PickupConfig};
use crate::gameplay::combat::EnemyKilledEvent;
use crate::gameplay::vehicle::{PlayerHealth, PlayerVehicle};
use crate::states::GameState;
use bevy::math::primitives::RegularPolygon;
use bevy::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};

const PICKUP_Z_M: f32 = 7.2;

pub struct PickupGameplayPlugin;

impl Plugin for PickupGameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PickupRngState>()
            .add_message::<PickupCollectedEvent>()
            .add_systems(OnEnter(GameState::InRun), reset_pickup_rng_state)
            .add_systems(OnExit(GameState::InRun), cleanup_pickups)
            .add_systems(
                Update,
                (
                    spawn_pickups_from_enemy_kills,
                    simulate_pickups,
                    collect_pickups,
                )
                    .chain()
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupKind {
    Coin,
    Health,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct PickupCollectedEvent {
    pub kind: PickupKind,
    pub score_added: u32,
    pub health_restored: f32,
    pub world_position: Vec2,
}

#[derive(Component, Debug, Clone, Copy)]
struct PickupDrop {
    kind: PickupKind,
    velocity_mps: Vec2,
    lifetime_s: f32,
    pickup_radius_m: f32,
    ground_clearance_m: f32,
    score_value: u32,
    heal_amount: f32,
    spin_speed_rad_s: f32,
}

#[derive(Resource, Debug, Clone, Copy)]
struct PickupRngState {
    seed: u64,
}

impl Default for PickupRngState {
    fn default() -> Self {
        Self {
            seed: 0xD40A_A15E_7D12_9321,
        }
    }
}

fn reset_pickup_rng_state(mut rng_state: ResMut<PickupRngState>) {
    rng_state.seed ^= unix_timestamp_seconds();
}

fn cleanup_pickups(mut commands: Commands, pickup_query: Query<Entity, With<PickupDrop>>) {
    for entity in &pickup_query {
        commands.entity(entity).try_despawn();
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_pickups_from_enemy_kills(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut kill_events: MessageReader<EnemyKilledEvent>,
    mut rng_state: ResMut<PickupRngState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let pickup_cfg = &config.game.pickups;
    for event in kill_events.read() {
        let kill_score = config
            .enemy_types_by_id
            .get(&event.enemy_type_id)
            .map(|enemy| enemy.kill_score)
            .unwrap_or(pickup_cfg.coin_score_min);
        let coin_score_value = ((kill_score as f32 * pickup_cfg.coin_score_scale).round() as u32)
            .max(pickup_cfg.coin_score_min)
            .max(1);

        spawn_coin_drop(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut rng_state.seed,
            pickup_cfg,
            event.world_position,
            coin_score_value,
        );

        let health_roll = next_unit_random(&mut rng_state.seed);
        if health_roll <= pickup_cfg.health_drop_chance {
            spawn_health_drop(
                &mut commands,
                &mut rng_state.seed,
                pickup_cfg,
                event.world_position,
                pickup_cfg.health_drop_heal_amount,
            );
        }
    }
}

#[allow(clippy::type_complexity)]
fn simulate_pickups(
    mut commands: Commands,
    time: Res<Time>,
    config: Res<GameConfig>,
    player_query: Query<&Transform, (With<PlayerVehicle>, Without<PickupDrop>)>,
    mut pickup_query: Query<
        (Entity, &mut Transform, &mut PickupDrop),
        (With<PickupDrop>, Without<PlayerVehicle>),
    >,
) {
    let dt = time.delta_secs().max(0.000_1);
    let pickup_cfg = &config.game.pickups;
    let player_x = player_query
        .single()
        .map(|transform| transform.translation.x)
        .unwrap_or(0.0);

    for (entity, mut transform, mut pickup) in &mut pickup_query {
        pickup.lifetime_s -= dt;
        if pickup.lifetime_s <= 0.0
            || transform.translation.x < (player_x - pickup_cfg.despawn_behind_player_m)
        {
            commands.entity(entity).try_despawn();
            continue;
        }

        pickup.velocity_mps.y -= pickup_cfg.gravity_mps2 * dt;
        transform.translation += (pickup.velocity_mps * dt).extend(0.0);
        transform.rotate_z(pickup.spin_speed_rad_s * dt);

        let ground_y =
            terrain_height_at_x(&config, transform.translation.x) + pickup.ground_clearance_m;
        if transform.translation.y <= ground_y {
            transform.translation.y = ground_y;
            if pickup.velocity_mps.y < 0.0 {
                pickup.velocity_mps.y = -pickup.velocity_mps.y * pickup_cfg.bounce_damping;
                if pickup.velocity_mps.y.abs() < pickup_cfg.ground_stop_speed_mps {
                    pickup.velocity_mps.y = 0.0;
                }
            }
            pickup.velocity_mps.x *= pickup_cfg.ground_slide_damping;
        }
    }
}

#[allow(clippy::type_complexity)]
fn collect_pickups(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut pickup_events: MessageWriter<PickupCollectedEvent>,
    mut player_query: Query<
        (&Transform, &mut PlayerHealth),
        (With<PlayerVehicle>, Without<PickupDrop>),
    >,
    pickup_query: Query<
        (Entity, &Transform, &PickupDrop),
        (With<PickupDrop>, Without<PlayerVehicle>),
    >,
) {
    let Ok((player_transform, mut player_health)) = player_query.single_mut() else {
        return;
    };
    let player_position = player_transform.translation.truncate();

    for (entity, pickup_transform, pickup) in &pickup_query {
        let pickup_position = pickup_transform.translation.truncate();
        if pickup_position.distance_squared(player_position)
            > (config.game.pickups.collection_radius_m + pickup.pickup_radius_m).powi(2)
        {
            continue;
        }

        let mut score_added = 0;
        let mut health_restored = 0.0;
        match pickup.kind {
            PickupKind::Coin => {
                score_added = pickup.score_value.max(1);
            }
            PickupKind::Health => {
                let missing_health = (player_health.max - player_health.current).max(0.0);
                if missing_health > 0.0 {
                    let restore_amount = pickup.heal_amount.max(0.0).min(missing_health);
                    player_health.current += restore_amount;
                    health_restored = restore_amount;
                }
            }
        }

        pickup_events.write(PickupCollectedEvent {
            kind: pickup.kind,
            score_added,
            health_restored,
            world_position: pickup_position,
        });
        commands.entity(entity).try_despawn();
    }
}

fn spawn_coin_drop(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    seed: &mut u64,
    pickup_cfg: &PickupConfig,
    world_position: Vec2,
    score_value: u32,
) {
    let horizontal_velocity = next_signed_unit_random(seed) * pickup_cfg.drop_horizontal_spread_mps;
    let vertical_velocity = lerp(
        pickup_cfg.drop_vertical_speed_min_mps,
        pickup_cfg.drop_vertical_speed_max_mps,
        next_unit_random(seed),
    );
    let jitter = Vec2::new(
        next_signed_unit_random(seed) * pickup_cfg.coin_jitter_x_m,
        pickup_cfg.coin_jitter_y_m,
    );
    let spin_speed = lerp(
        pickup_cfg.coin_spin_speed_min_rad_s,
        pickup_cfg.coin_spin_speed_max_rad_s,
        next_unit_random(seed),
    );

    commands.spawn((
        Name::new("PickupCoin"),
        PickupDrop {
            kind: PickupKind::Coin,
            velocity_mps: Vec2::new(horizontal_velocity, vertical_velocity),
            lifetime_s: pickup_cfg.despawn_seconds,
            pickup_radius_m: pickup_cfg.coin_pickup_radius_m,
            ground_clearance_m: pickup_cfg.coin_radius_m,
            score_value,
            heal_amount: 0.0,
            spin_speed_rad_s: spin_speed,
        },
        Mesh2d(meshes.add(RegularPolygon::new(pickup_cfg.coin_radius_m, 18))),
        MeshMaterial2d(materials.add(ColorMaterial::from(Color::srgb(0.96, 0.79, 0.18)))),
        Transform::from_xyz(
            world_position.x + jitter.x,
            world_position.y + jitter.y,
            PICKUP_Z_M,
        ),
    ));
}

fn spawn_health_drop(
    commands: &mut Commands,
    seed: &mut u64,
    pickup_cfg: &PickupConfig,
    world_position: Vec2,
    heal_amount: f32,
) {
    let horizontal_velocity =
        next_signed_unit_random(seed) * (pickup_cfg.drop_horizontal_spread_mps * 0.8);
    let vertical_velocity = lerp(
        pickup_cfg.drop_vertical_speed_min_mps,
        pickup_cfg.drop_vertical_speed_max_mps,
        next_unit_random(seed),
    );
    let jitter = Vec2::new(
        next_signed_unit_random(seed) * pickup_cfg.health_jitter_x_m,
        pickup_cfg.health_jitter_y_m,
    );
    let spin_speed = lerp(
        pickup_cfg.health_spin_speed_min_rad_s,
        pickup_cfg.health_spin_speed_max_rad_s,
        next_unit_random(seed),
    );
    let health_box_size = Vec2::splat(pickup_cfg.health_box_size_m);

    commands.spawn((
        Name::new("PickupHealth"),
        PickupDrop {
            kind: PickupKind::Health,
            velocity_mps: Vec2::new(horizontal_velocity, vertical_velocity),
            lifetime_s: pickup_cfg.despawn_seconds,
            pickup_radius_m: pickup_cfg.health_pickup_radius_m,
            ground_clearance_m: health_box_size.y * 0.5,
            score_value: 0,
            heal_amount,
            spin_speed_rad_s: spin_speed,
        },
        Sprite::from_color(Color::srgb(0.20, 0.82, 0.33), health_box_size),
        Transform::from_xyz(
            world_position.x + jitter.x,
            world_position.y + jitter.y,
            PICKUP_Z_M,
        ),
    ));
}

fn next_signed_unit_random(seed: &mut u64) -> f32 {
    (next_unit_random(seed) * 2.0) - 1.0
}

fn next_unit_random(seed: &mut u64) -> f32 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 32) as u32) as f32 / u32::MAX as f32
}

fn terrain_height_at_x(config: &GameConfig, x: f32) -> f32 {
    let terrain = &config.game.terrain;
    terrain.base_height - terrain.ground_lowering_m
        + (x * terrain.ramp_slope)
        + (x * terrain.wave_a_frequency).sin() * terrain.wave_a_amplitude
        + (x * terrain.wave_b_frequency).sin() * terrain.wave_b_amplitude
        + (x * terrain.wave_c_frequency).sin() * terrain.wave_c_amplitude
}

fn unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + ((b - a) * t.clamp(0.0, 1.0))
}
