use crate::config::GameConfig;
use crate::states::GameState;
use bevy::math::primitives::RegularPolygon;
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
const PLAYER_CHASSIS_SIZE: Vec2 = Vec2::new(3.45, 1.08);
const PLAYER_TURRET_SIZE: Vec2 = Vec2::new(1.42, 0.52);
const PLAYER_TURRET_OFFSET_LOCAL: Vec3 = Vec3::new(0.38, 0.66, 0.4);
const PLAYER_WHEEL_RADIUS_M: f32 = 0.552;
const PLAYER_FRONT_HARDPOINT_X_M: f32 = 1.06;
const PLAYER_REAR_HARDPOINT_X_M: f32 = -1.08;
const PLAYER_FRONT_HARDPOINT_Y_M: f32 = -0.15;
const PLAYER_REAR_HARDPOINT_Y_M: f32 = -0.10;
const PLAYER_REAR_WHEEL_GROUND_EPSILON_M: f32 = 0.05;
const SUSPENSION_FORCE_CLAMP_N: f32 = 240.0;
const WHEEL_FRICTION_MIN_FACTOR: f32 = 0.30;
const START_HEIGHT_OFFSET: f32 = 4.0;
const CAMERA_Y: f32 = -2.0;
const CAMERA_Z: f32 = 999.9;
const MAX_ANGULAR_SPEED: f32 = 5.5;
const REAR_TRACTION_ASSIST_FALLBACK_DISTANCE_M: f32 = 0.90;
const AIR_ANGULAR_DAMPING: f32 = 0.96;
const WHEEL_VISUAL_TRAVEL_EXAGGERATION: f32 = 1.8;
const WHEELIE_ANGLE_THRESHOLD_DEG: f32 = 20.0;
const WHEELIE_MIN_SPEED_MPS: f32 = 2.0;
const CRASH_LANDING_SPEED_THRESHOLD_MPS: f32 = 9.0;
const CRASH_LANDING_ANGLE_THRESHOLD_DEG: f32 = 50.0;
const LANDING_DAMAGE_PER_MPS_OVER_THRESHOLD: f32 = 2.4;
const PLAYER_HP_BAR_OFFSET_Y_M: f32 = 1.55;
const PLAYER_HP_BAR_BG_WIDTH_M: f32 = 3.3;
const PLAYER_HP_BAR_BG_HEIGHT_M: f32 = 0.26;
const PLAYER_HP_BAR_FILL_HEIGHT_M: f32 = 0.16;
const PLAYER_HP_BAR_Z_M: f32 = 0.9;

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
            .add_systems(OnExit(GameState::InRun), cleanup_vehicle_scene)
            .add_systems(
                Update,
                (
                    read_vehicle_input,
                    update_ground_checker_tiles,
                    apply_vehicle_kinematics,
                    spin_wheel_pairs,
                    update_stunt_metrics,
                    update_player_health_bar,
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

#[derive(Component, Debug, Clone, Copy)]
pub struct PlayerHealth {
    pub current: f32,
    pub max: f32,
}

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

#[derive(Component, Debug, Clone, Copy)]
struct PlayerHpBarBackground;

#[derive(Component, Debug, Clone, Copy)]
struct PlayerHpBarFill {
    max_width_m: f32,
}

#[derive(Component, Debug, Clone, Copy)]
struct PlayerChassisVisual;

#[derive(Component, Debug, Clone, Copy)]
struct PlayerTurretVisual;

#[derive(Component, Debug, Clone, Copy)]
struct PlayerWheelPairVisual {
    axle: WheelAxle,
    radius_m: f32,
    driven: bool,
    hardpoint_local: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WheelAxle {
    Front,
    Rear,
}

#[derive(Component, Debug, Clone)]
struct VehicleKinematics {
    velocity: Vec2,
    angular_velocity: f32,
}

#[derive(Component, Debug, Clone, Copy)]
struct VehicleSuspensionState {
    front_spring_length_m: f32,
    rear_spring_length_m: f32,
    front_prev_compression_m: f32,
    rear_prev_compression_m: f32,
    front_grounded: bool,
    rear_grounded: bool,
}

#[derive(Debug, Clone, Copy)]
struct WheelSuspensionSample {
    compression_m: f32,
    compression_ratio: f32,
    support_force_n: f32,
    gap_to_ground_m: f32,
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    config: Res<GameConfig>,
    existing_player: Query<Entity, With<PlayerVehicle>>,
    existing_ground: Query<Entity, With<GroundVisual>>,
    existing_background: Query<Entity, With<BackgroundVisual>>,
) {
    if existing_player.is_empty() {
        let Some(vehicle) = config.vehicles_by_id.get(&config.game.app.default_vehicle) else {
            return;
        };

        let player_entity = commands
            .spawn((
                Name::new("PlayerVehicle"),
                PlayerVehicle,
                PlayerHealth {
                    current: vehicle.health,
                    max: vehicle.health,
                },
                VehicleKinematics {
                    velocity: Vec2::ZERO,
                    angular_velocity: 0.0,
                },
                VehicleSuspensionState {
                    front_spring_length_m: vehicle.suspension_rest_length_m,
                    rear_spring_length_m: vehicle.suspension_rest_length_m,
                    front_prev_compression_m: 0.0,
                    rear_prev_compression_m: 0.0,
                    front_grounded: true,
                    rear_grounded: true,
                },
                GroundContact {
                    grounded: true,
                    just_landed: false,
                    landing_impact_speed_mps: 0.0,
                },
                Transform::from_xyz(
                    0.0,
                    rear_wheel_root_contact_y(&config, 0.0, 0.0, vehicle.suspension_rest_length_m)
                        + START_HEIGHT_OFFSET,
                    10.0,
                ),
                GlobalTransform::default(),
                Visibility::Inherited,
                InheritedVisibility::VISIBLE,
                ViewVisibility::default(),
            ))
            .id();

        let wheel_mesh = meshes.add(RegularPolygon::new(PLAYER_WHEEL_RADIUS_M, 6));
        let front_wheel_material =
            materials.add(ColorMaterial::from(Color::srgb(0.70, 0.80, 0.90)));
        let rear_wheel_material = materials.add(ColorMaterial::from(Color::srgb(0.62, 0.73, 0.84)));

        commands.entity(player_entity).with_children(|parent| {
            parent.spawn((
                Name::new("PlayerChassis"),
                PlayerChassisVisual,
                Sprite::from_color(Color::srgb(0.93, 0.34, 0.24), PLAYER_CHASSIS_SIZE),
                Transform::from_xyz(0.0, -0.02, 0.22),
            ));

            parent.spawn((
                Name::new("PlayerTurretBody"),
                PlayerTurretVisual,
                Sprite::from_color(Color::srgb(0.98, 0.44, 0.24), PLAYER_TURRET_SIZE),
                Transform::from_translation(PLAYER_TURRET_OFFSET_LOCAL),
            ));

            // Side-view wheel entities represent synchronized left/right tire pairs in the 2D solve.
            parent.spawn((
                Name::new("PlayerWheelPairFront"),
                PlayerWheelPairVisual {
                    axle: WheelAxle::Front,
                    radius_m: PLAYER_WHEEL_RADIUS_M,
                    driven: false,
                    hardpoint_local: Vec2::new(
                        PLAYER_FRONT_HARDPOINT_X_M,
                        PLAYER_FRONT_HARDPOINT_Y_M,
                    ),
                },
                Mesh2d(wheel_mesh.clone()),
                MeshMaterial2d(front_wheel_material.clone()),
                Transform::from_xyz(
                    PLAYER_FRONT_HARDPOINT_X_M,
                    PLAYER_FRONT_HARDPOINT_Y_M - vehicle.suspension_rest_length_m,
                    0.26,
                ),
            ));

            parent.spawn((
                Name::new("PlayerWheelPairRear"),
                PlayerWheelPairVisual {
                    axle: WheelAxle::Rear,
                    radius_m: PLAYER_WHEEL_RADIUS_M,
                    driven: true,
                    hardpoint_local: Vec2::new(
                        PLAYER_REAR_HARDPOINT_X_M,
                        PLAYER_REAR_HARDPOINT_Y_M,
                    ),
                },
                Mesh2d(wheel_mesh.clone()),
                MeshMaterial2d(rear_wheel_material.clone()),
                Transform::from_xyz(
                    PLAYER_REAR_HARDPOINT_X_M,
                    PLAYER_REAR_HARDPOINT_Y_M - vehicle.suspension_rest_length_m,
                    0.26,
                ),
            ));

            parent.spawn((
                Name::new("PlayerHpBarBackground"),
                PlayerHpBarBackground,
                Sprite::from_color(
                    Color::srgba(0.06, 0.08, 0.10, 0.85),
                    Vec2::new(PLAYER_HP_BAR_BG_WIDTH_M, PLAYER_HP_BAR_BG_HEIGHT_M),
                ),
                Transform::from_xyz(0.0, PLAYER_HP_BAR_OFFSET_Y_M, PLAYER_HP_BAR_Z_M),
            ));

            parent.spawn((
                Name::new("PlayerHpBarFill"),
                PlayerHpBarFill {
                    max_width_m: PLAYER_HP_BAR_BG_WIDTH_M - 0.04,
                },
                Sprite::from_color(
                    Color::srgba(0.14, 0.88, 0.25, 0.94),
                    Vec2::new(PLAYER_HP_BAR_BG_WIDTH_M - 0.04, PLAYER_HP_BAR_FILL_HEIGHT_M),
                ),
                Transform::from_xyz(0.0, PLAYER_HP_BAR_OFFSET_Y_M, PLAYER_HP_BAR_Z_M + 0.01),
            ));
        });
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

fn cleanup_vehicle_scene(
    mut commands: Commands,
    player_query: Query<Entity, With<PlayerVehicle>>,
    ground_query: Query<Entity, With<GroundVisual>>,
    background_query: Query<Entity, With<BackgroundVisual>>,
) {
    for entity in &player_query {
        commands.entity(entity).despawn();
    }
    for entity in &ground_query {
        commands.entity(entity).despawn();
    }
    for entity in &background_query {
        commands.entity(entity).despawn();
    }
}

fn spin_wheel_pairs(
    time: Res<Time>,
    config: Res<GameConfig>,
    player_query: Query<(&VehicleKinematics, &VehicleSuspensionState), With<PlayerVehicle>>,
    mut wheel_query: Query<(&PlayerWheelPairVisual, &mut Transform)>,
) {
    let Ok((kinematics, suspension)) = player_query.single() else {
        return;
    };
    let Some(vehicle) = config.vehicles_by_id.get(&config.game.app.default_vehicle) else {
        return;
    };

    let dt = time.delta_secs();
    let rest_length = vehicle.suspension_rest_length_m.max(0.01);
    let min_length = (rest_length - vehicle.suspension_max_compression_m.max(0.01)).max(0.02);
    let max_length = rest_length + vehicle.suspension_max_extension_m.max(0.0);
    let visual_min_length = (min_length - 0.08).max(0.02);
    let visual_max_length = max_length + 0.08;

    for (wheel, mut transform) in &mut wheel_query {
        let spring_length_m = match wheel.axle {
            WheelAxle::Front => suspension.front_spring_length_m,
            WheelAxle::Rear => suspension.rear_spring_length_m,
        };
        let visual_spring_length = (rest_length
            + ((spring_length_m - rest_length) * WHEEL_VISUAL_TRAVEL_EXAGGERATION))
            .clamp(visual_min_length, visual_max_length);
        transform.translation.x = wheel.hardpoint_local.x;
        transform.translation.y = wheel.hardpoint_local.y - visual_spring_length;

        let axle_scale = match wheel.axle {
            WheelAxle::Front => 0.97,
            WheelAxle::Rear => 1.0,
        };
        let drive_spin_multiplier = if wheel.driven { 1.0 } else { 0.995 };
        let angular_speed_rad_s =
            (kinematics.velocity.x / wheel.radius_m.max(0.01)) * axle_scale * drive_spin_multiplier;
        transform.rotate_z(-(angular_speed_rad_s * dt));
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
        (
            &mut Transform,
            &mut VehicleKinematics,
            &mut VehicleSuspensionState,
            &mut GroundContact,
            &mut PlayerHealth,
        ),
        With<PlayerVehicle>,
    >,
) {
    let Ok((mut transform, mut kinematics, mut suspension, mut contact, mut health)) =
        player_query.single_mut()
    else {
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

    let dt = time.delta_secs().max(0.000_1);
    let throttle = if input_state.accelerate { 1.0 } else { 0.0 };
    let brake = if input_state.brake { 1.0 } else { 0.0 };
    let (_, _, z_rot_rad) = transform.rotation.to_euler(EulerRot::XYZ);
    let root_position = transform.translation.truncate();

    let rest_length = vehicle.suspension_rest_length_m.max(0.01);
    let min_length = (rest_length - vehicle.suspension_max_compression_m.max(0.01)).max(0.02);
    let max_length = rest_length + vehicle.suspension_max_extension_m.max(0.0);
    let max_compression = (rest_length - min_length).max(0.001);

    let (front_spring_length, front_sample, front_wheel_grounded) = sample_wheel_suspension(
        &config,
        root_position,
        z_rot_rad,
        Vec2::new(PLAYER_FRONT_HARDPOINT_X_M, PLAYER_FRONT_HARDPOINT_Y_M),
        suspension.front_prev_compression_m,
        rest_length,
        min_length,
        max_length,
        max_compression,
        vehicle.suspension_stiffness,
        vehicle.suspension_damping,
        dt,
    );
    let (rear_spring_length, rear_sample, rear_wheel_grounded) = sample_wheel_suspension(
        &config,
        root_position,
        z_rot_rad,
        Vec2::new(PLAYER_REAR_HARDPOINT_X_M, PLAYER_REAR_HARDPOINT_Y_M),
        suspension.rear_prev_compression_m,
        rest_length,
        min_length,
        max_length,
        max_compression,
        vehicle.suspension_stiffness,
        vehicle.suspension_damping,
        dt,
    );

    suspension.front_spring_length_m = front_spring_length;
    suspension.rear_spring_length_m = rear_spring_length;
    suspension.front_prev_compression_m = front_sample.compression_m;
    suspension.rear_prev_compression_m = rear_sample.compression_m;
    suspension.front_grounded = front_wheel_grounded;
    suspension.rear_grounded = rear_wheel_grounded;

    let grounded_wheel_ratio =
        (front_wheel_grounded as u32 + rear_wheel_grounded as u32) as f32 * 0.5;

    let drive_accel = (vehicle.acceleration * vehicle.linear_speed_scale) / vehicle.linear_inertia;
    let brake_accel =
        (vehicle.brake_strength * vehicle.linear_speed_scale) / vehicle.linear_inertia;
    let rear_grip_factor = vehicle.tire_longitudinal_grip
        * (vehicle.tire_slip_grip_floor
            + ((1.0 - vehicle.tire_slip_grip_floor) * rear_sample.compression_ratio))
            .clamp(0.0, 1.0);

    let rear_assist_distance_m = vehicle.rear_drive_traction_assist_distance_m.max(0.0);
    let rear_assist_min_factor = vehicle
        .rear_drive_traction_assist_min_factor
        .clamp(0.0, 1.0);
    let chassis_supporting_drive = front_wheel_grounded || rear_wheel_grounded || contact.grounded;
    let effective_assist_distance_m = if chassis_supporting_drive && !rear_wheel_grounded {
        rear_assist_distance_m.max(REAR_TRACTION_ASSIST_FALLBACK_DISTANCE_M)
    } else {
        rear_assist_distance_m
    };
    let rear_assist_factor = if rear_wheel_grounded {
        1.0
    } else if effective_assist_distance_m > f32::EPSILON
        && rear_sample.gap_to_ground_m <= effective_assist_distance_m
    {
        let proximity = 1.0 - (rear_sample.gap_to_ground_m / effective_assist_distance_m);
        rear_assist_min_factor + ((1.0 - rear_assist_min_factor) * proximity.clamp(0.0, 1.0))
    } else if chassis_supporting_drive {
        rear_assist_min_factor * 0.55
    } else {
        0.0
    };
    let rear_drive_factor = rear_grip_factor * rear_assist_factor;
    let brake_ground_factor = grounded_wheel_ratio.max(WHEEL_FRICTION_MIN_FACTOR);
    let mut longitudinal_accel = throttle * drive_accel * rear_drive_factor;
    if brake > 0.0 {
        if kinematics.velocity.x > 0.25 {
            longitudinal_accel -= brake * brake_accel * brake_ground_factor;
        } else {
            longitudinal_accel -= brake * brake_accel * rear_drive_factor;
        }
    }
    kinematics.velocity.x += longitudinal_accel * dt;

    let damping = if front_wheel_grounded || rear_wheel_grounded {
        let ground_damping_scale = (0.45 + (grounded_wheel_ratio * 0.55)).clamp(0.45, 1.0);
        f32::exp(-(vehicle.ground_coast_damping * ground_damping_scale) * dt)
    } else {
        let air_damping =
            vehicle.air_base_damping + (environment.drag * vehicle.air_env_drag_factor);
        f32::exp(-air_damping * dt)
    };
    kinematics.velocity.x *= damping;

    let support_force_n = front_sample.support_force_n + rear_sample.support_force_n;
    kinematics.velocity.y += (support_force_n / vehicle.linear_inertia) * dt;
    kinematics.velocity.y -= environment.gravity * vehicle.gravity_scale * dt;

    let suspension_pitch_torque = (PLAYER_FRONT_HARDPOINT_X_M * front_sample.support_force_n)
        + (PLAYER_REAR_HARDPOINT_X_M * rear_sample.support_force_n);
    kinematics.angular_velocity += (suspension_pitch_torque / vehicle.rotational_inertia) * dt;

    if !(front_wheel_grounded || rear_wheel_grounded) {
        kinematics.angular_velocity +=
            ((throttle - brake) * vehicle.air_pitch_torque / vehicle.rotational_inertia) * dt;
    }
    kinematics.angular_velocity = kinematics
        .angular_velocity
        .clamp(-MAX_ANGULAR_SPEED, MAX_ANGULAR_SPEED);

    transform.rotate_z(kinematics.angular_velocity * dt);
    if front_wheel_grounded || rear_wheel_grounded {
        kinematics.angular_velocity *= 0.94;
    } else {
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

    let (_, _, z_rot_after_integration) = transform.rotation.to_euler(EulerRot::XYZ);
    let mut root_after_move = transform.translation.truncate();
    let mut front_clearance = wheel_ground_clearance(
        &config,
        root_after_move,
        z_rot_after_integration,
        Vec2::new(PLAYER_FRONT_HARDPOINT_X_M, PLAYER_FRONT_HARDPOINT_Y_M),
        suspension.front_spring_length_m,
        PLAYER_WHEEL_RADIUS_M,
    );
    let mut rear_clearance = wheel_ground_clearance(
        &config,
        root_after_move,
        z_rot_after_integration,
        Vec2::new(PLAYER_REAR_HARDPOINT_X_M, PLAYER_REAR_HARDPOINT_Y_M),
        suspension.rear_spring_length_m,
        PLAYER_WHEEL_RADIUS_M,
    );
    let penetration_correction_m = (-front_clearance).max(-rear_clearance).max(0.0);
    if penetration_correction_m > 0.0 {
        transform.translation.y += penetration_correction_m;
        root_after_move.y += penetration_correction_m;
        front_clearance += penetration_correction_m;
        rear_clearance += penetration_correction_m;
    }

    let front_grounded_after = front_clearance <= PLAYER_REAR_WHEEL_GROUND_EPSILON_M;
    let rear_grounded_after = rear_clearance <= PLAYER_REAR_WHEEL_GROUND_EPSILON_M;
    let grounded_now =
        front_grounded_after || rear_grounded_after || penetration_correction_m > 0.0;
    suspension.front_grounded = front_grounded_after;
    suspension.rear_grounded = rear_grounded_after;

    if grounded_now {
        if kinematics.velocity.y < 0.0 {
            if !was_grounded {
                contact.just_landed = true;
                contact.landing_impact_speed_mps = -kinematics.velocity.y;
                let impact_over_threshold =
                    (contact.landing_impact_speed_mps - CRASH_LANDING_SPEED_THRESHOLD_MPS).max(0.0);
                if impact_over_threshold > 0.0 {
                    let damage = impact_over_threshold * LANDING_DAMAGE_PER_MPS_OVER_THRESHOLD;
                    health.current = (health.current - damage).max(0.0);
                }
            }
            kinematics.velocity.y = 0.0;
        }

        contact.grounded = true;
    } else {
        contact.grounded = false;
    }
}

fn update_player_health_bar(
    player_query: Query<&PlayerHealth, With<PlayerVehicle>>,
    mut hp_fill_query: Query<(&PlayerHpBarFill, &mut Transform, &mut Sprite)>,
) {
    let Ok(player_health) = player_query.single() else {
        return;
    };

    let health_fraction = (player_health.current / player_health.max).clamp(0.0, 1.0);
    for (bar_fill, mut transform, mut sprite) in &mut hp_fill_query {
        transform.scale.x = health_fraction.max(0.001);
        transform.translation.x = -((1.0 - health_fraction) * bar_fill.max_width_m * 0.5);

        let red = 0.92 - (0.78 * health_fraction);
        let green = 0.20 + (0.67 * health_fraction);
        sprite.color = Color::srgba(red, green, 0.20, 0.96);
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

#[allow(clippy::too_many_arguments)]
fn sample_wheel_suspension(
    config: &GameConfig,
    root_position: Vec2,
    root_z_rotation: f32,
    hardpoint_local: Vec2,
    prev_compression_m: f32,
    rest_length_m: f32,
    min_length_m: f32,
    max_length_m: f32,
    max_compression_m: f32,
    stiffness: f32,
    damping: f32,
    dt: f32,
) -> (f32, WheelSuspensionSample, bool) {
    let hardpoint_world = wheel_hardpoint_world(root_position, root_z_rotation, hardpoint_local);
    let ground_y = terrain_height_at_x(config, hardpoint_world.x);
    let contact_length = hardpoint_world.y - (ground_y + PLAYER_WHEEL_RADIUS_M);
    let grounded = contact_length <= (max_length_m + PLAYER_REAR_WHEEL_GROUND_EPSILON_M);
    let gap_to_ground_m = (contact_length - max_length_m).max(0.0);
    let spring_length_m = if grounded {
        contact_length.clamp(min_length_m, max_length_m)
    } else {
        max_length_m
    };

    let compression_m = (rest_length_m - spring_length_m).clamp(0.0, max_compression_m);
    let compression_velocity_mps = (compression_m - prev_compression_m) / dt.max(0.000_1);
    let support_force_n = if grounded {
        (compression_m * stiffness - compression_velocity_mps * damping)
            .clamp(0.0, SUSPENSION_FORCE_CLAMP_N)
    } else {
        0.0
    };
    let compression_ratio = (compression_m / max_compression_m.max(0.001)).clamp(0.0, 1.0);

    (
        spring_length_m,
        WheelSuspensionSample {
            compression_m,
            compression_ratio,
            support_force_n,
            gap_to_ground_m,
        },
        grounded,
    )
}

fn rear_wheel_root_contact_y(
    config: &GameConfig,
    root_x: f32,
    root_z_rotation: f32,
    rear_spring_length_m: f32,
) -> f32 {
    let rear_hardpoint_world = wheel_hardpoint_world(
        Vec2::new(root_x, 0.0),
        root_z_rotation,
        Vec2::new(PLAYER_REAR_HARDPOINT_X_M, PLAYER_REAR_HARDPOINT_Y_M),
    );
    let rear_ground_y = terrain_height_at_x(config, rear_hardpoint_world.x);
    rear_ground_y + PLAYER_WHEEL_RADIUS_M - (rear_hardpoint_world.y - rear_spring_length_m)
}

fn wheel_hardpoint_world(root_position: Vec2, root_z_rotation: f32, hardpoint_local: Vec2) -> Vec2 {
    root_position + (Mat2::from_angle(root_z_rotation) * hardpoint_local)
}

fn wheel_center_world(
    root_position: Vec2,
    root_z_rotation: f32,
    hardpoint_local: Vec2,
    spring_length_m: f32,
) -> Vec2 {
    let hardpoint_world = wheel_hardpoint_world(root_position, root_z_rotation, hardpoint_local);
    Vec2::new(hardpoint_world.x, hardpoint_world.y - spring_length_m)
}

fn wheel_ground_clearance(
    config: &GameConfig,
    root_position: Vec2,
    root_z_rotation: f32,
    hardpoint_local: Vec2,
    spring_length_m: f32,
    wheel_radius_m: f32,
) -> f32 {
    let wheel_center = wheel_center_world(
        root_position,
        root_z_rotation,
        hardpoint_local,
        spring_length_m,
    );
    let ground_y = terrain_height_at_x(config, wheel_center.x);
    (wheel_center.y - wheel_radius_m) - ground_y
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
