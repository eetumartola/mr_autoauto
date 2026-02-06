use crate::config::GameConfig;
use crate::states::GameState;
use bevy::prelude::*;

const GROUND_Y: f32 = -170.0;
const GROUND_THICKNESS: f32 = 40.0;
const GROUND_WIDTH: f32 = 24_000.0;
const WORLD_HALF_WIDTH: f32 = GROUND_WIDTH * 0.5;
const GROUND_CHECKER_WIDTH: f32 = 120.0;
const BACKGROUND_WIDTH: f32 = 28_000.0;
const BACKGROUND_BAND_HEIGHT: f32 = 1_200.0;
const BACKGROUND_Y: f32 = 120.0;
const BACKGROUND_CHECKER_WIDTH: f32 = 260.0;
const BACKGROUND_CHECKER_HEIGHT: f32 = 260.0;
const PLAYER_SIZE: Vec2 = Vec2::new(70.0, 38.0);
const START_HEIGHT_OFFSET: f32 = 4.0;
const HORIZONTAL_LOOK_AHEAD_FACTOR: f32 = 1.1;
const LOOK_AHEAD_MIN: f32 = -220.0;
const LOOK_AHEAD_MAX: f32 = 420.0;
const CAMERA_Y: f32 = -40.0;
const CAMERA_Z: f32 = 999.9;
const GRAVITY_SCALE: f32 = 45.0;
const LINEAR_SPEED_SCALE: f32 = 7.5;
const GROUND_COAST_DAMPING_PER_SEC: f32 = 0.22;
const AIR_BASE_DAMPING_PER_SEC: f32 = 0.10;
const AIR_ENV_DRAG_FACTOR: f32 = 0.45;
const MAX_FORWARD_SPEED: f32 = 340.0;
const MAX_REVERSE_SPEED: f32 = 185.0;
const MAX_FALL_SPEED: f32 = 260.0;
const MAX_ANGULAR_SPEED: f32 = 5.5;
const GROUND_ROTATION_SETTLE_RATE: f32 = 8.0;
const AIR_ANGULAR_DAMPING: f32 = 0.96;

pub struct VehicleGameplayPlugin;

impl Plugin for VehicleGameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VehicleInputState>()
            .init_resource::<VehicleInputBindings>()
            .init_resource::<VehicleTelemetry>()
            .add_systems(OnEnter(GameState::InRun), spawn_vehicle_scene)
            .add_systems(
                Update,
                (
                    read_vehicle_input,
                    apply_vehicle_kinematics,
                    update_vehicle_telemetry,
                    camera_follow_vehicle,
                )
                    .chain()
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            );
    }
}

#[derive(Component)]
pub struct PlayerVehicle;

#[derive(Component)]
struct GroundVisual;

#[derive(Component)]
struct BackgroundVisual;

#[derive(Component, Debug, Clone)]
struct VehicleKinematics {
    velocity: Vec2,
    angular_velocity: f32,
}

#[derive(Component, Debug, Clone, Copy, Default)]
struct GroundContact {
    grounded: bool,
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
            GroundContact { grounded: true },
            Sprite::from_color(Color::srgb(0.93, 0.34, 0.24), PLAYER_SIZE),
            Transform::from_xyz(
                0.0,
                GROUND_Y + (PLAYER_SIZE.y * 0.5) + START_HEIGHT_OFFSET,
                10.0,
            ),
        ));
    }

    if existing_ground.is_empty() {
        let ground_entity = commands
            .spawn((
                Name::new("GroundVisual"),
                GroundVisual,
                Sprite::from_color(
                    Color::srgb(0.16, 0.18, 0.20),
                    Vec2::new(GROUND_WIDTH, GROUND_THICKNESS),
                ),
                Transform::from_xyz(0.0, GROUND_Y - (GROUND_THICKNESS * 0.5), 0.0),
            ))
            .id();

        let ground_checker_count = (GROUND_WIDTH / GROUND_CHECKER_WIDTH).ceil() as i32 + 2;
        let ground_start_x = -WORLD_HALF_WIDTH;

        commands.entity(ground_entity).with_children(|parent| {
            for index in 0..ground_checker_count {
                let x = ground_start_x + ((index as f32 + 0.5) * GROUND_CHECKER_WIDTH);
                let color = if index % 2 == 0 {
                    Color::srgb(0.24, 0.27, 0.31)
                } else {
                    Color::srgb(0.12, 0.14, 0.16)
                };

                parent.spawn((
                    Name::new("GroundCheckerTile"),
                    Sprite::from_color(color, Vec2::new(GROUND_CHECKER_WIDTH, GROUND_THICKNESS)),
                    Transform::from_xyz(x, 0.0, 0.1),
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

    let drive_accel = vehicle.acceleration * LINEAR_SPEED_SCALE;
    let brake_accel = vehicle.brake_strength * LINEAR_SPEED_SCALE;
    kinematics.velocity.x += (throttle * drive_accel - brake * brake_accel) * dt;

    let damping = if contact.grounded {
        f32::exp(-GROUND_COAST_DAMPING_PER_SEC * dt)
    } else {
        let air_damping = AIR_BASE_DAMPING_PER_SEC + (environment.drag * AIR_ENV_DRAG_FACTOR);
        f32::exp(-air_damping * dt)
    };
    kinematics.velocity.x *= damping;

    kinematics.velocity.y -= environment.gravity * GRAVITY_SCALE * dt;

    if !contact.grounded {
        kinematics.angular_velocity += (throttle - brake) * vehicle.air_pitch_torque * dt;
        kinematics.angular_velocity = kinematics
            .angular_velocity
            .clamp(-MAX_ANGULAR_SPEED, MAX_ANGULAR_SPEED);
        transform.rotate_z(kinematics.angular_velocity * dt);
        kinematics.angular_velocity *= AIR_ANGULAR_DAMPING;
    }

    kinematics.velocity.x = kinematics
        .velocity
        .x
        .clamp(-MAX_REVERSE_SPEED, MAX_FORWARD_SPEED);
    kinematics.velocity.y = kinematics.velocity.y.clamp(-MAX_FALL_SPEED, MAX_FALL_SPEED);

    transform.translation += (kinematics.velocity * dt).extend(0.0);

    let ground_contact_y = GROUND_Y + (PLAYER_SIZE.y * 0.5);
    if transform.translation.y <= ground_contact_y {
        transform.translation.y = ground_contact_y;
        if kinematics.velocity.y < 0.0 {
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
    player_query: Query<&Transform, With<PlayerVehicle>>,
    mut camera_query: Query<&mut Transform, (With<Camera2d>, Without<PlayerVehicle>)>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok(mut camera_transform) = camera_query.single_mut() else {
        return;
    };

    camera_transform.translation.x = player_transform.translation.x
        + (telemetry.speed_mps * HORIZONTAL_LOOK_AHEAD_FACTOR)
            .clamp(LOOK_AHEAD_MIN, LOOK_AHEAD_MAX);
    camera_transform.translation.y = CAMERA_Y;
    camera_transform.translation.z = CAMERA_Z;
}
