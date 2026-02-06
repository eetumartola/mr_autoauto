use crate::config::GameConfig;
use crate::states::GameState;
use bevy::prelude::*;
use std::f32::consts::{PI, TAU};

const CAMERA_ORTHO_SCALE_METERS: f32 = 0.05;
const GROUND_WIDTH: f32 = 1_200.0;
const WORLD_HALF_WIDTH: f32 = GROUND_WIDTH * 0.5;
const GROUND_CHECKER_WIDTH: f32 = 2.0;
const TERRAIN_EXTRUSION_DEPTH: f32 = 26.0;
const TERRAIN_RIDGE_HEIGHT: f32 = 0.6;
const BACKGROUND_WIDTH: f32 = 1_400.0;
const BACKGROUND_BAND_HEIGHT: f32 = 60.0;
const BACKGROUND_Y: f32 = 6.0;
const BACKGROUND_CHECKER_WIDTH: f32 = 13.0;
const BACKGROUND_CHECKER_HEIGHT: f32 = 13.0;
const PLAYER_SIZE_METERS: Vec2 = Vec2::new(3.5, 1.9);
const PLAYER_SIZE: Vec2 = PLAYER_SIZE_METERS;
const START_HEIGHT_OFFSET: f32 = 4.0;
const CAMERA_Y: f32 = -2.0;
const CAMERA_Z: f32 = 999.9;
const MAX_ANGULAR_SPEED: f32 = 5.5;
const GROUND_ROTATION_SETTLE_RATE: f32 = 8.0;
const AIR_ANGULAR_DAMPING: f32 = 0.96;
const WHEELIE_ANGLE_THRESHOLD_DEG: f32 = 20.0;
const WHEELIE_MIN_SPEED_MPS: f32 = 2.0;
const CRASH_LANDING_SPEED_THRESHOLD_MPS: f32 = 9.0;
const CRASH_LANDING_ANGLE_THRESHOLD_DEG: f32 = 50.0;

pub struct VehicleGameplayPlugin;

impl Plugin for VehicleGameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VehicleInputState>()
            .init_resource::<VehicleInputBindings>()
            .init_resource::<VehicleTelemetry>()
            .init_resource::<VehicleStuntMetrics>()
            .init_resource::<StuntTrackingState>()
            .add_systems(
                OnEnter(GameState::InRun),
                (
                    configure_camera_units,
                    spawn_vehicle_scene,
                    reset_stunt_metrics,
                ),
            )
            .add_systems(
                Update,
                (
                    read_vehicle_input,
                    update_ground_checker_tiles,
                    apply_vehicle_kinematics,
                    update_stunt_metrics,
                    update_vehicle_telemetry,
                    camera_follow_vehicle,
                )
                    .chain()
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            );
    }
}

fn configure_camera_units(mut camera_query: Query<&mut Projection, With<Camera2d>>) {
    let Ok(mut projection) = camera_query.single_mut() else {
        return;
    };

    if let Projection::Orthographic(ortho) = &mut *projection {
        ortho.scale = CAMERA_ORTHO_SCALE_METERS;
    }
}

#[derive(Component)]
pub struct PlayerVehicle;

#[derive(Component)]
struct GroundVisual;

#[derive(Component)]
struct BackgroundVisual;

#[derive(Component)]
struct GroundCheckerTile {
    world_x: f32,
}

#[derive(Component)]
struct GroundRidgeTile {
    world_x: f32,
}

#[derive(Component, Debug, Clone)]
struct VehicleKinematics {
    velocity: Vec2,
    angular_velocity: f32,
}

#[derive(Component, Debug, Clone, Copy, Default)]
struct GroundContact {
    grounded: bool,
    just_landed: bool,
    landing_impact_speed_mps: f32,
}

#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct VehicleInputState {
    pub accelerate: bool,
    pub brake: bool,
}

#[derive(Resource, Debug, Clone)]
struct VehicleInputBindings {
    accelerate: Vec<KeyCode>,
    brake: Vec<KeyCode>,
}

impl Default for VehicleInputBindings {
    fn default() -> Self {
        Self {
            accelerate: vec![KeyCode::KeyD, KeyCode::ArrowRight],
            brake: vec![KeyCode::KeyA, KeyCode::ArrowLeft],
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct VehicleTelemetry {
    pub distance_m: f32,
    pub speed_mps: f32,
    pub grounded: bool,
}

#[derive(Resource, Debug, Clone)]
pub struct VehicleStuntMetrics {
    pub airtime_current_s: f32,
    pub airtime_best_s: f32,
    pub wheelie_current_s: f32,
    pub wheelie_best_s: f32,
    pub flip_count: u32,
    pub crash_count: u32,
    pub max_speed_mps: f32,
    pub last_landing_impact_speed_mps: f32,
}

impl Default for VehicleStuntMetrics {
    fn default() -> Self {
        Self {
            airtime_current_s: 0.0,
            airtime_best_s: 0.0,
            wheelie_current_s: 0.0,
            wheelie_best_s: 0.0,
            flip_count: 0,
            crash_count: 0,
            max_speed_mps: 0.0,
            last_landing_impact_speed_mps: 0.0,
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
struct StuntTrackingState {
    initialized: bool,
    was_grounded: bool,
    previous_angle_rad: f32,
    airborne_rotation_accum_rad: f32,
}

impl Default for VehicleTelemetry {
    fn default() -> Self {
        Self {
            distance_m: 0.0,
            speed_mps: 0.0,
            grounded: true,
        }
    }
}

fn spawn_vehicle_scene(
    mut commands: Commands,
    config: Res<GameConfig>,
    existing_player: Query<Entity, With<PlayerVehicle>>,
    existing_ground: Query<Entity, With<GroundVisual>>,
    existing_background: Query<Entity, With<BackgroundVisual>>,
) {
    if existing_player.is_empty() {
        commands.spawn((
            Name::new("PlayerVehicle"),
            PlayerVehicle,
            VehicleKinematics {
                velocity: Vec2::ZERO,
                angular_velocity: 0.0,
            },
            GroundContact {
                grounded: true,
                just_landed: false,
                landing_impact_speed_mps: 0.0,
            },
            Sprite::from_color(Color::srgb(0.93, 0.34, 0.24), PLAYER_SIZE),
            Transform::from_xyz(
                0.0,
                terrain_height_at_x(&config, 0.0) + (PLAYER_SIZE.y * 0.5) + START_HEIGHT_OFFSET,
                10.0,
            ),
        ));
    }

    if existing_ground.is_empty() {
        let ground_entity = commands
            .spawn((
                Name::new("GroundVisual"),
                GroundVisual,
                Transform::default(),
                GlobalTransform::default(),
                Visibility::Inherited,
            ))
            .id();

        let ground_checker_count = (GROUND_WIDTH / GROUND_CHECKER_WIDTH).ceil() as i32 + 2;
        let ground_start_x = -WORLD_HALF_WIDTH;

        commands.entity(ground_entity).with_children(|parent| {
            for index in 0..ground_checker_count {
                let x = ground_start_x + ((index as f32 + 0.5) * GROUND_CHECKER_WIDTH);
                let body_color = if index % 2 == 0 {
                    Color::srgb(0.28, 0.33, 0.39)
                } else {
                    Color::srgb(0.18, 0.22, 0.27)
                };
                let top_y = terrain_height_at_x(&config, x);
                let body_center_y = top_y - (TERRAIN_EXTRUSION_DEPTH * 0.5);

                parent.spawn((
                    Name::new("GroundCheckerTile"),
                    GroundCheckerTile { world_x: x },
                    Sprite::from_color(
                        body_color,
                        Vec2::new(GROUND_CHECKER_WIDTH + 1.0, TERRAIN_EXTRUSION_DEPTH),
                    ),
                    Transform::from_xyz(x, body_center_y, 0.1),
                ));

                parent.spawn((
                    Name::new("GroundRidgeTile"),
                    GroundRidgeTile { world_x: x },
                    Sprite::from_color(
                        Color::srgb(0.58, 0.66, 0.73),
                        Vec2::new(GROUND_CHECKER_WIDTH + 1.0, TERRAIN_RIDGE_HEIGHT),
                    ),
                    Transform::from_xyz(x, top_y - (TERRAIN_RIDGE_HEIGHT * 0.5), 0.2),
                ));
            }
        });
    }

    if existing_background.is_empty() {
        let background_entity = commands
            .spawn((
                Name::new("BackgroundVisual"),
                BackgroundVisual,
                Sprite::from_color(
                    Color::srgb(0.07, 0.09, 0.12),
                    Vec2::new(BACKGROUND_WIDTH, BACKGROUND_BAND_HEIGHT),
                ),
                Transform::from_xyz(BACKGROUND_WIDTH * 0.0, BACKGROUND_Y, -20.0),
            ))
            .id();

        let bg_checker_count = (BACKGROUND_WIDTH / BACKGROUND_CHECKER_WIDTH).ceil() as i32 + 2;
        let bg_start_x = -(BACKGROUND_WIDTH * 0.5);

        commands.entity(background_entity).with_children(|parent| {
            for index in 0..bg_checker_count {
                let x = bg_start_x + ((index as f32 + 0.5) * BACKGROUND_CHECKER_WIDTH);
                let color = if index % 2 == 0 {
                    Color::srgb(0.10, 0.13, 0.17)
                } else {
                    Color::srgb(0.06, 0.08, 0.11)
                };

                parent.spawn((
                    Name::new("BackgroundCheckerTile"),
                    Sprite::from_color(
                        color,
                        Vec2::new(BACKGROUND_CHECKER_WIDTH, BACKGROUND_CHECKER_HEIGHT),
                    ),
                    Transform::from_xyz(x, 0.0, 0.1),
                ));
            }
        });
    }
}

#[allow(clippy::type_complexity)]
fn update_ground_checker_tiles(
    config: Res<GameConfig>,
    mut terrain_tiles: Query<
        (
            &mut Transform,
            Option<&GroundCheckerTile>,
            Option<&GroundRidgeTile>,
        ),
        Or<(With<GroundCheckerTile>, With<GroundRidgeTile>)>,
    >,
) {
    if !config.is_changed() {
        return;
    }

    for (mut transform, ground_tile, ridge_tile) in &mut terrain_tiles {
        if let Some(tile) = ground_tile {
            let top_y = terrain_height_at_x(&config, tile.world_x);
            transform.translation.y = top_y - (TERRAIN_EXTRUSION_DEPTH * 0.5);
        }
        if let Some(tile) = ridge_tile {
            let top_y = terrain_height_at_x(&config, tile.world_x);
            transform.translation.y = top_y - (TERRAIN_RIDGE_HEIGHT * 0.5);
        }
    }
}

fn reset_stunt_metrics(
    mut metrics: ResMut<VehicleStuntMetrics>,
    mut tracking: ResMut<StuntTrackingState>,
) {
    *metrics = VehicleStuntMetrics::default();
    *tracking = StuntTrackingState::default();
}

fn update_stunt_metrics(
    time: Res<Time>,
    mut metrics: ResMut<VehicleStuntMetrics>,
    mut tracking: ResMut<StuntTrackingState>,
    player_query: Query<(&Transform, &VehicleKinematics, &GroundContact), With<PlayerVehicle>>,
) {
    let Ok((transform, kinematics, contact)) = player_query.single() else {
        return;
    };

    let (_, _, angle_rad) = transform.rotation.to_euler(EulerRot::XYZ);
    let dt = time.delta_secs();

    if !tracking.initialized {
        tracking.initialized = true;
        tracking.previous_angle_rad = angle_rad;
        tracking.was_grounded = contact.grounded;
    }

    metrics.max_speed_mps = metrics.max_speed_mps.max(kinematics.velocity.length());

    if contact.grounded {
        metrics.airtime_current_s = 0.0;
        tracking.airborne_rotation_accum_rad = 0.0;
    } else {
        metrics.airtime_current_s += dt;
        metrics.airtime_best_s = metrics.airtime_best_s.max(metrics.airtime_current_s);

        if !tracking.was_grounded {
            tracking.airborne_rotation_accum_rad +=
                shortest_angle_delta_rad(angle_rad, tracking.previous_angle_rad).abs();
            while tracking.airborne_rotation_accum_rad >= TAU {
                metrics.flip_count = metrics.flip_count.saturating_add(1);
                tracking.airborne_rotation_accum_rad -= TAU;
            }
        }
    }

    let angle_deg = angle_rad.abs().to_degrees();
    if contact.grounded
        && angle_deg >= WHEELIE_ANGLE_THRESHOLD_DEG
        && kinematics.velocity.x.abs() >= WHEELIE_MIN_SPEED_MPS
    {
        metrics.wheelie_current_s += dt;
        metrics.wheelie_best_s = metrics.wheelie_best_s.max(metrics.wheelie_current_s);
    } else {
        metrics.wheelie_current_s = 0.0;
    }

    if contact.just_landed {
        metrics.last_landing_impact_speed_mps = contact.landing_impact_speed_mps;
        if contact.landing_impact_speed_mps >= CRASH_LANDING_SPEED_THRESHOLD_MPS
            || angle_deg >= CRASH_LANDING_ANGLE_THRESHOLD_DEG
        {
            metrics.crash_count = metrics.crash_count.saturating_add(1);
        }
    }

    tracking.previous_angle_rad = angle_rad;
    tracking.was_grounded = contact.grounded;
}

fn read_vehicle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    bindings: Res<VehicleInputBindings>,
    mut input_state: ResMut<VehicleInputState>,
) {
    input_state.accelerate = bindings.accelerate.iter().any(|key| keyboard.pressed(*key));
    input_state.brake = bindings.brake.iter().any(|key| keyboard.pressed(*key));
}

fn apply_vehicle_kinematics(
    time: Res<Time>,
    config: Res<GameConfig>,
    input_state: Res<VehicleInputState>,
    mut player_query: Query<
        (&mut Transform, &mut VehicleKinematics, &mut GroundContact),
        With<PlayerVehicle>,
    >,
) {
    let Ok((mut transform, mut kinematics, mut contact)) = player_query.single_mut() else {
        return;
    };
    let was_grounded = contact.grounded;
    contact.just_landed = false;
    contact.landing_impact_speed_mps = 0.0;

    let Some(vehicle) = config.vehicles_by_id.get(&config.game.app.default_vehicle) else {
        return;
    };

    let Some(environment) = config
        .environments_by_id
        .get(&config.game.app.starting_environment)
    else {
        return;
    };

    let dt = time.delta_secs();
    let throttle = if input_state.accelerate { 1.0 } else { 0.0 };
    let brake = if input_state.brake { 1.0 } else { 0.0 };

    let drive_accel = (vehicle.acceleration * vehicle.linear_speed_scale) / vehicle.linear_inertia;
    let brake_accel =
        (vehicle.brake_strength * vehicle.linear_speed_scale) / vehicle.linear_inertia;
    kinematics.velocity.x += (throttle * drive_accel - brake * brake_accel) * dt;

    let damping = if contact.grounded {
        f32::exp(-vehicle.ground_coast_damping * dt)
    } else {
        let air_damping =
            vehicle.air_base_damping + (environment.drag * vehicle.air_env_drag_factor);
        f32::exp(-air_damping * dt)
    };
    kinematics.velocity.x *= damping;

    kinematics.velocity.y -= environment.gravity * vehicle.gravity_scale * dt;

    if !contact.grounded {
        kinematics.angular_velocity +=
            ((throttle - brake) * vehicle.air_pitch_torque / vehicle.rotational_inertia) * dt;
        kinematics.angular_velocity = kinematics
            .angular_velocity
            .clamp(-MAX_ANGULAR_SPEED, MAX_ANGULAR_SPEED);
        transform.rotate_z(kinematics.angular_velocity * dt);
        kinematics.angular_velocity *= AIR_ANGULAR_DAMPING;
    }

    kinematics.velocity.x = kinematics
        .velocity
        .x
        .clamp(-vehicle.max_reverse_speed, vehicle.max_forward_speed);
    kinematics.velocity.y = kinematics
        .velocity
        .y
        .clamp(-vehicle.max_fall_speed, vehicle.max_fall_speed);

    transform.translation += (kinematics.velocity * dt).extend(0.0);

    let ground_contact_y =
        terrain_height_at_x(&config, transform.translation.x) + (PLAYER_SIZE.y * 0.5);
    if transform.translation.y <= ground_contact_y {
        transform.translation.y = ground_contact_y;
        if kinematics.velocity.y < 0.0 {
            if !was_grounded {
                contact.just_landed = true;
                contact.landing_impact_speed_mps = -kinematics.velocity.y;
            }
            kinematics.velocity.y = 0.0;
        }

        contact.grounded = true;
        kinematics.angular_velocity = 0.0;

        let (_, _, z_rot) = transform.rotation.to_euler(EulerRot::XYZ);
        let settled_z = z_rot * (1.0 - (GROUND_ROTATION_SETTLE_RATE * dt).clamp(0.0, 1.0));
        transform.rotation = Quat::from_rotation_z(settled_z);
    } else {
        contact.grounded = false;
    }
}

fn update_vehicle_telemetry(
    mut telemetry: ResMut<VehicleTelemetry>,
    player_query: Query<(&Transform, &VehicleKinematics, &GroundContact), With<PlayerVehicle>>,
) {
    let Ok((transform, kinematics, contact)) = player_query.single() else {
        return;
    };

    telemetry.distance_m = transform.translation.x.max(0.0);
    telemetry.speed_mps = kinematics.velocity.x;
    telemetry.grounded = contact.grounded;
}

fn camera_follow_vehicle(
    telemetry: Res<VehicleTelemetry>,
    config: Res<GameConfig>,
    player_query: Query<&Transform, With<PlayerVehicle>>,
    mut camera_query: Query<&mut Transform, (With<Camera2d>, Without<PlayerVehicle>)>,
) {
    let Some(vehicle) = config.vehicles_by_id.get(&config.game.app.default_vehicle) else {
        return;
    };

    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok(mut camera_transform) = camera_query.single_mut() else {
        return;
    };

    camera_transform.translation.x = player_transform.translation.x
        + (telemetry.speed_mps * vehicle.camera_look_ahead_factor)
            .clamp(vehicle.camera_look_ahead_min, vehicle.camera_look_ahead_max);
    camera_transform.translation.y = CAMERA_Y;
    camera_transform.translation.z = CAMERA_Z;
}

fn terrain_height_at_x(config: &GameConfig, x: f32) -> f32 {
    let terrain = &config.game.terrain;
    terrain.base_height
        + (x * terrain.ramp_slope)
        + (x * terrain.wave_a_frequency).sin() * terrain.wave_a_amplitude
        + (x * terrain.wave_b_frequency).sin() * terrain.wave_b_amplitude
}

fn shortest_angle_delta_rad(current: f32, previous: f32) -> f32 {
    (current - previous + PI).rem_euclid(TAU) - PI
}
