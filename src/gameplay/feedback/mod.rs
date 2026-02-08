use crate::gameplay::enemies::{PlayerDamageEvent, PlayerDamageSource, PlayerEnemyCrashEvent};
use crate::gameplay::pickups::{PickupCollectedEvent, PickupKind};
use crate::gameplay::vehicle::{PlayerVehicle, VehicleLandingEvent};
use crate::states::GameState;
use bevy::prelude::*;

const DAMAGE_INDICATOR_Z: i32 = 240;
const DAMAGE_INDICATOR_BASE_COLOR: Color = Color::srgba(1.0, 0.24, 0.20, 0.85);
const DAMAGE_INDICATOR_DECAY_PER_SECOND: f32 = 2.2;
const CAMERA_SHAKE_DECAY_PER_SECOND: f32 = 1.8;
const CAMERA_SHAKE_MAX_OFFSET_X_M: f32 = 0.9;
const CAMERA_SHAKE_MAX_OFFSET_Y_M: f32 = 0.55;
const FEEDBACK_PARTICLE_Z_M: f32 = 5.1;

pub struct FeedbackGameplayPlugin;

impl Plugin for FeedbackGameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DamageIndicatorState>()
            .init_resource::<CameraShakeState>()
            .add_systems(
                OnEnter(GameState::InRun),
                (reset_feedback_state, spawn_damage_indicator_ui),
            )
            .add_systems(OnExit(GameState::InRun), cleanup_feedback_entities)
            .add_systems(
                Update,
                (
                    collect_feedback_events,
                    decay_damage_indicators,
                    update_damage_indicator_ui,
                    decay_camera_shake,
                    update_feedback_particles,
                )
                    .chain()
                    .run_if(in_state(GameState::InRun)),
            )
            .add_systems(
                PostUpdate,
                apply_camera_shake.run_if(in_state(GameState::InRun)),
            );
    }
}

#[derive(Component)]
struct DamageIndicatorRoot;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
struct DamageIndicatorBar {
    side: DamageIndicatorSide,
}

#[derive(Component, Debug, Clone, Copy)]
struct FeedbackParticle {
    velocity_mps: Vec2,
    gravity_mps2: f32,
    drag_per_second: f32,
    remaining_s: f32,
    total_s: f32,
    initial_alpha: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DamageIndicatorSide {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
struct DamageIndicatorState {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl DamageIndicatorState {
    fn side_intensity(self, side: DamageIndicatorSide) -> f32 {
        match side {
            DamageIndicatorSide::Left => self.left,
            DamageIndicatorSide::Right => self.right,
            DamageIndicatorSide::Top => self.top,
            DamageIndicatorSide::Bottom => self.bottom,
        }
    }

    fn bump(&mut self, side: DamageIndicatorSide, value: f32) {
        let target = value.clamp(0.0, 1.0);
        match side {
            DamageIndicatorSide::Left => self.left = self.left.max(target),
            DamageIndicatorSide::Right => self.right = self.right.max(target),
            DamageIndicatorSide::Top => self.top = self.top.max(target),
            DamageIndicatorSide::Bottom => self.bottom = self.bottom.max(target),
        }
    }
}

#[derive(Resource, Debug, Clone, Copy)]
struct CameraShakeState {
    trauma: f32,
    rng_state: u64,
}

impl Default for CameraShakeState {
    fn default() -> Self {
        Self {
            trauma: 0.0,
            rng_state: 0x8A37_2BC1_D9E4_1023,
        }
    }
}

fn reset_feedback_state(
    mut indicators: ResMut<DamageIndicatorState>,
    mut shake_state: ResMut<CameraShakeState>,
) {
    *indicators = DamageIndicatorState::default();
    *shake_state = CameraShakeState::default();
}

fn spawn_damage_indicator_ui(
    mut commands: Commands,
    existing_root_query: Query<Entity, With<DamageIndicatorRoot>>,
) {
    if !existing_root_query.is_empty() {
        return;
    }

    commands
        .spawn((
            Name::new("DamageIndicatorRoot"),
            DamageIndicatorRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                ..default()
            },
            ZIndex(DAMAGE_INDICATOR_Z),
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("DamageIndicatorLeft"),
                DamageIndicatorBar {
                    side: DamageIndicatorSide::Left,
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Percent(30.0),
                    width: Val::Px(26.0),
                    height: Val::Percent(40.0),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Visibility::Hidden,
            ));
            parent.spawn((
                Name::new("DamageIndicatorRight"),
                DamageIndicatorBar {
                    side: DamageIndicatorSide::Right,
                },
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(0.0),
                    top: Val::Percent(30.0),
                    width: Val::Px(26.0),
                    height: Val::Percent(40.0),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Visibility::Hidden,
            ));
            parent.spawn((
                Name::new("DamageIndicatorTop"),
                DamageIndicatorBar {
                    side: DamageIndicatorSide::Top,
                },
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(0.0),
                    left: Val::Percent(30.0),
                    width: Val::Percent(40.0),
                    height: Val::Px(20.0),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Visibility::Hidden,
            ));
            parent.spawn((
                Name::new("DamageIndicatorBottom"),
                DamageIndicatorBar {
                    side: DamageIndicatorSide::Bottom,
                },
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(0.0),
                    left: Val::Percent(30.0),
                    width: Val::Percent(40.0),
                    height: Val::Px(18.0),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Visibility::Hidden,
            ));
        });
}

#[allow(clippy::type_complexity)]
fn cleanup_feedback_entities(
    mut commands: Commands,
    cleanup_query: Query<Entity, Or<(With<DamageIndicatorRoot>, With<FeedbackParticle>)>>,
) {
    for entity in &cleanup_query {
        commands.entity(entity).try_despawn();
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_feedback_events(
    mut commands: Commands,
    mut damage_events: MessageReader<PlayerDamageEvent>,
    mut crash_events: MessageReader<PlayerEnemyCrashEvent>,
    mut landing_events: MessageReader<VehicleLandingEvent>,
    mut pickup_events: MessageReader<PickupCollectedEvent>,
    player_query: Query<&Transform, With<PlayerVehicle>>,
    mut indicators: ResMut<DamageIndicatorState>,
    mut shake: ResMut<CameraShakeState>,
) {
    let player_position = player_query
        .single()
        .ok()
        .map(|transform| transform.translation.truncate());

    for event in damage_events.read() {
        let side = resolve_damage_side(event, player_position);
        let intensity = (event.amount * 0.055).clamp(0.12, 0.95);
        indicators.bump(side, intensity);

        let mut trauma_bump = (event.amount * 0.014).clamp(0.03, 0.22);
        if event.source == PlayerDamageSource::ProjectileBomb {
            trauma_bump += 0.10;
        }
        shake.trauma = (shake.trauma + trauma_bump).clamp(0.0, 1.0);
    }

    for event in crash_events.read() {
        let trauma_bump = ((event.player_speed_mps - 1.5) * 0.015).clamp(0.04, 0.20);
        shake.trauma = (shake.trauma + trauma_bump).clamp(0.0, 1.0);
    }

    for event in landing_events.read() {
        spawn_landing_dust_particles(
            &mut commands,
            event.world_position,
            event.impact_speed_mps,
            &mut shake.rng_state,
        );
        let landing_trauma = if event.was_crash {
            (event.impact_speed_mps * 0.015).clamp(0.08, 0.30)
        } else {
            (event.impact_speed_mps * 0.007).clamp(0.02, 0.14)
        };
        shake.trauma = (shake.trauma + landing_trauma).clamp(0.0, 1.0);
    }

    for event in pickup_events.read() {
        spawn_pickup_sparkle_particles(
            &mut commands,
            event.world_position,
            event.kind,
            &mut shake.rng_state,
        );
    }
}

fn decay_damage_indicators(time: Res<Time>, mut indicators: ResMut<DamageIndicatorState>) {
    let dt = time.delta_secs().max(0.000_1);
    let decay = (1.0 - (DAMAGE_INDICATOR_DECAY_PER_SECOND * dt)).clamp(0.0, 1.0);
    indicators.left *= decay;
    indicators.right *= decay;
    indicators.top *= decay;
    indicators.bottom *= decay;
}

fn update_damage_indicator_ui(
    indicators: Res<DamageIndicatorState>,
    mut bar_query: Query<(&DamageIndicatorBar, &mut BackgroundColor, &mut Visibility)>,
) {
    for (bar, mut background, mut visibility) in &mut bar_query {
        let intensity = indicators.side_intensity(bar.side).clamp(0.0, 1.0);
        if intensity <= 0.01 {
            *visibility = Visibility::Hidden;
            continue;
        }

        *visibility = Visibility::Inherited;
        let tint = DAMAGE_INDICATOR_BASE_COLOR.to_srgba();
        let alpha = (tint.alpha * intensity).clamp(0.0, 1.0);
        *background = BackgroundColor(Color::srgba(tint.red, tint.green, tint.blue, alpha));
    }
}

fn decay_camera_shake(time: Res<Time>, mut shake: ResMut<CameraShakeState>) {
    let dt = time.delta_secs().max(0.000_1);
    shake.trauma = (shake.trauma - (CAMERA_SHAKE_DECAY_PER_SECOND * dt)).max(0.0);
}

fn apply_camera_shake(
    mut shake: ResMut<CameraShakeState>,
    mut camera_query: Query<&mut Transform, With<Camera2d>>,
) {
    if shake.trauma <= f32::EPSILON {
        return;
    }

    let shake_amount = shake.trauma * shake.trauma;
    let offset_x =
        next_signed_unit_random(&mut shake.rng_state) * CAMERA_SHAKE_MAX_OFFSET_X_M * shake_amount;
    let offset_y =
        next_signed_unit_random(&mut shake.rng_state) * CAMERA_SHAKE_MAX_OFFSET_Y_M * shake_amount;

    for mut camera_transform in &mut camera_query {
        camera_transform.translation.x += offset_x;
        camera_transform.translation.y += offset_y;
    }
}

fn update_feedback_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particle_query: Query<(Entity, &mut Transform, &mut Sprite, &mut FeedbackParticle)>,
) {
    let dt = time.delta_secs().max(0.000_1);
    for (entity, mut transform, mut sprite, mut particle) in &mut particle_query {
        particle.velocity_mps.y -= particle.gravity_mps2 * dt;
        let drag = f32::exp(-(particle.drag_per_second.max(0.0) * dt));
        particle.velocity_mps *= drag;
        transform.translation += (particle.velocity_mps * dt).extend(0.0);

        particle.remaining_s -= dt;
        let life_t = (particle.remaining_s / particle.total_s.max(0.001)).clamp(0.0, 1.0);
        let mut color = sprite.color;
        color.set_alpha(particle.initial_alpha * life_t);
        sprite.color = color;
        transform.scale = Vec3::splat(0.45 + (0.55 * life_t));

        if particle.remaining_s <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

fn resolve_damage_side(
    event: &PlayerDamageEvent,
    player_position: Option<Vec2>,
) -> DamageIndicatorSide {
    let Some(player_position) = player_position else {
        return fallback_damage_side(event.source);
    };
    let Some(source_position) = event.source_world_position else {
        return fallback_damage_side(event.source);
    };

    let delta = source_position - player_position;
    if delta.length_squared() <= f32::EPSILON {
        return fallback_damage_side(event.source);
    }

    if delta.x.abs() >= delta.y.abs() {
        if delta.x >= 0.0 {
            DamageIndicatorSide::Right
        } else {
            DamageIndicatorSide::Left
        }
    } else if delta.y >= 0.0 {
        DamageIndicatorSide::Top
    } else {
        DamageIndicatorSide::Bottom
    }
}

fn fallback_damage_side(source: PlayerDamageSource) -> DamageIndicatorSide {
    match source {
        PlayerDamageSource::Contact => DamageIndicatorSide::Bottom,
        PlayerDamageSource::ProjectileBullet
        | PlayerDamageSource::ProjectileMissile
        | PlayerDamageSource::ProjectileBomb => DamageIndicatorSide::Right,
    }
}

fn spawn_landing_dust_particles(
    commands: &mut Commands,
    world_position: Vec2,
    impact_speed_mps: f32,
    rng_state: &mut u64,
) {
    let impact = impact_speed_mps.max(0.0);
    let count = ((impact * 0.7).round() as i32).clamp(6, 16) as usize;
    for _ in 0..count {
        let x_jitter = next_signed_unit_random(rng_state) * (0.55 + (impact * 0.02));
        let launch_speed = lerp(1.8, 6.4, next_unit_random(rng_state)) * (0.6 + impact * 0.03);
        let vx = next_signed_unit_random(rng_state) * launch_speed;
        let vy = lerp(1.0, 4.6, next_unit_random(rng_state));
        let size = lerp(0.10, 0.24, next_unit_random(rng_state));
        let lifetime = lerp(0.24, 0.46, next_unit_random(rng_state));
        let alpha = lerp(0.35, 0.72, next_unit_random(rng_state));

        commands.spawn((
            Name::new("LandingDustFx"),
            FeedbackParticle {
                velocity_mps: Vec2::new(vx, vy),
                gravity_mps2: 11.5,
                drag_per_second: 2.4,
                remaining_s: lifetime,
                total_s: lifetime,
                initial_alpha: alpha,
            },
            Sprite::from_color(Color::srgba(0.72, 0.56, 0.38, alpha), Vec2::splat(size)),
            Transform::from_xyz(
                world_position.x + x_jitter,
                world_position.y + 0.08,
                FEEDBACK_PARTICLE_Z_M,
            ),
        ));
    }
}

fn spawn_pickup_sparkle_particles(
    commands: &mut Commands,
    world_position: Vec2,
    kind: PickupKind,
    rng_state: &mut u64,
) {
    let (count, color) = match kind {
        PickupKind::Coin => (8, Color::srgba(1.0, 0.86, 0.26, 0.95)),
        PickupKind::Health => (7, Color::srgba(0.32, 1.0, 0.42, 0.95)),
    };

    for _ in 0..count {
        let angle = next_unit_random(rng_state) * std::f32::consts::TAU;
        let speed = lerp(3.0, 9.0, next_unit_random(rng_state));
        let velocity = Vec2::new(angle.cos() * speed, angle.sin().abs() * speed + 2.2);
        let size = lerp(0.06, 0.14, next_unit_random(rng_state));
        let lifetime = lerp(0.16, 0.34, next_unit_random(rng_state));

        commands.spawn((
            Name::new("PickupSparkleFx"),
            FeedbackParticle {
                velocity_mps: velocity,
                gravity_mps2: 14.0,
                drag_per_second: 3.4,
                remaining_s: lifetime,
                total_s: lifetime,
                initial_alpha: color.to_srgba().alpha,
            },
            Sprite::from_color(color, Vec2::splat(size)),
            Transform::from_xyz(
                world_position.x,
                world_position.y + 0.20,
                FEEDBACK_PARTICLE_Z_M + 0.02,
            ),
        ));
    }
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

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + ((b - a) * t.clamp(0.0, 1.0))
}
