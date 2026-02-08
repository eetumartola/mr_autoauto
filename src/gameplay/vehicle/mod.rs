use crate::assets::{AssetRegistry, ModelAssetEntry};
use crate::config::GameConfig;
use crate::debug::{DebugCameraPanState, DebugGameplayGuards};
use crate::gameplay::combat::TurretTargetingState;
use crate::states::GameState;
use bevy::asset::{LoadState, RenderAssetUsages};
use bevy::camera::visibility::RenderLayers;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::math::primitives::RegularPolygon;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy_rapier2d::prelude::*;
use std::collections::HashSet;
use std::f32::consts::{PI, TAU};
use std::path::Path;

#[cfg(feature = "gaussian_splats")]
use bevy_gaussian_splatting::{
    sort::SortMode, CloudSettings, Gaussian3d, GaussianCamera, PlanarGaussian3d,
    PlanarGaussian3dHandle,
};

mod model;
mod runtime;
mod scene;
mod terrain;

use model::*;
use runtime::*;
use scene::*;
use terrain::*;

const CAMERA_ORTHO_SCALE_METERS: f32 = 0.05;
const GROUND_WIDTH: f32 = 1_200.0;
const WORLD_HALF_WIDTH: f32 = GROUND_WIDTH * 0.5;
const GROUND_SPLINE_SEGMENT_WIDTH_M: f32 = 1.2;
const GROUND_SPLINE_THICKNESS_M: f32 = 3.2;
const GROUND_SPLINE_Z: f32 = 0.1;
const GROUND_CURTAIN_Z: f32 = GROUND_SPLINE_Z - 0.04;
const GROUND_CURTAIN_BOTTOM_Y_M: f32 = -180.0;
const GROUND_CURTAIN_UV_WORLD_UNITS_PER_TILE: f32 = CAMERA_ORTHO_SCALE_METERS * 255.0;
const GROUND_CURTAIN_UV_SCALE: f32 = 1.0 / GROUND_CURTAIN_UV_WORLD_UNITS_PER_TILE;
const GROUND_STRIP_TEXTURE_PRIMARY_PATH: &str = "ground_strip.png";
const GROUND_STRIP_TEXTURE_FALLBACK_PATH: &str = "textures/ground_strip.png";
const GROUND_CURTAIN_TEXTURE_PRIMARY_PATH: &str = "ground_curtain.png";
const GROUND_CURTAIN_TEXTURE_FALLBACK_PATH: &str = "textures/ground_curtain.png";
const BACKGROUND_WIDTH: f32 = 1_400.0;
const BACKGROUND_BAND_HEIGHT: f32 = 60.0;
const BACKGROUND_Y: f32 = 6.0;
const BACKGROUND_CHECKER_WIDTH: f32 = 13.0;
const BACKGROUND_CHECKER_HEIGHT: f32 = 13.0;
#[cfg(feature = "gaussian_splats")]
const SPLAT_CAMERA_Y_M: f32 = 2.0;
#[cfg(feature = "gaussian_splats")]
const SPLAT_CAMERA_Z_M: f32 = 85.0;
#[cfg(feature = "gaussian_splats")]
const SPLAT_CAMERA_TARGET_Z_M: f32 = 0.0;
#[cfg(feature = "gaussian_splats")]
const SPLAT_BACKGROUND_Z_M: f32 = -120.0;
#[cfg(feature = "gaussian_splats")]
const SPLAT_BACKGROUND_Y_OFFSET_M: f32 = -8.5;
#[cfg(feature = "gaussian_splats")]
const SPLAT_BACKGROUND_RENDER_LAYER: usize = 1;
const PLAYER_CHASSIS_SIZE: Vec2 = Vec2::new(3.45, 1.08);
const PLAYER_TURRET_SIZE: Vec2 = Vec2::new(1.42, 0.52);
const PLAYER_TURRET_OFFSET_LOCAL: Vec3 = Vec3::new(0.38, 0.66, 0.4);
const PLAYER_WHEEL_RADIUS_M: f32 = 0.552;
const PLAYER_WHEEL_SPREAD_EXTRA_PER_SIDE_RADII: f32 = 0.4;
const PLAYER_WHEEL_SPREAD_EXTRA_PER_SIDE_M: f32 =
    PLAYER_WHEEL_RADIUS_M * PLAYER_WHEEL_SPREAD_EXTRA_PER_SIDE_RADII;
const PLAYER_CHASSIS_RAISE_EXTRA_M: f32 =
    PLAYER_WHEEL_RADIUS_M * PLAYER_WHEEL_SPREAD_EXTRA_PER_SIDE_RADII;
const PLAYER_FRONT_HARDPOINT_X_M: f32 = 1.06 + PLAYER_WHEEL_SPREAD_EXTRA_PER_SIDE_M;
const PLAYER_REAR_HARDPOINT_X_M: f32 = -1.08 - PLAYER_WHEEL_SPREAD_EXTRA_PER_SIDE_M;
const PLAYER_FRONT_HARDPOINT_Y_M: f32 = -0.15 + PLAYER_CHASSIS_RAISE_EXTRA_M;
const PLAYER_REAR_HARDPOINT_Y_M: f32 = -0.10 + PLAYER_CHASSIS_RAISE_EXTRA_M;
const PLAYER_CHASSIS_MASS_KG: f32 = 6.0;
const PLAYER_CHASSIS_CENTER_OF_MASS_Y_M: f32 = -0.54;
const PLAYER_REAR_WHEEL_GROUND_EPSILON_M: f32 = 0.05;
const SUSPENSION_FORCE_CLAMP_N: f32 = 240.0;
const WHEEL_FRICTION_MIN_FACTOR: f32 = 0.30;
const START_HEIGHT_OFFSET: f32 = 4.0;
const CAMERA_Y: f32 = -2.0;
const CAMERA_Z: f32 = 999.9;
const CAMERA_LOOKAHEAD_MAX_STEP_MPS: f32 = 24.0;
const CAMERA_FOLLOW_SMOOTH_RATE_HZ: f32 = 10.0;
const GROUND_MAX_ANGULAR_SPEED: f32 = 5.5;
const REAR_TRACTION_ASSIST_FALLBACK_DISTANCE_M: f32 = 0.28;
const AIR_ANGULAR_DAMPING: f32 = 0.96;
const WHEEL_VISUAL_TRAVEL_EXAGGERATION: f32 = 1.8;
const WHEEL_VISUAL_SPRING_LERP_RATE: f32 = 14.0;
const GROUND_RAYCAST_MAX_DISTANCE_M: f32 = 3.0;
const MIN_DRIVEABLE_GROUND_NORMAL_Y: f32 = 0.55;
const MIN_SUSPENSION_DOWN_ALIGNMENT: f32 = 0.28;
const SUSPENSION_MAX_COMPRESSION_SPEED_MPS: f32 = 5.0;
const SUSPENSION_MAX_REBOUND_SPEED_MPS: f32 = 1.8;
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
const PLAYER_MODEL_SETUP_DEPTH_Z: f32 = PLAYER_MODEL_SCENE_Z;
const YARDSTICK_LENGTH_M: f32 = 40.0;
const YARDSTICK_INTERVAL_M: f32 = 5.0;
const YARDSTICK_MAJOR_INTERVAL_M: f32 = 10.0;
const YARDSTICK_BASE_THICKNESS_M: f32 = 0.08;
const YARDSTICK_MINOR_NOTCH_HEIGHT_M: f32 = 0.34;
const YARDSTICK_MAJOR_NOTCH_HEIGHT_M: f32 = 0.62;
const YARDSTICK_NOTCH_THICKNESS_M: f32 = 0.07;
const YARDSTICK_OFFSET_FROM_CAMERA: Vec3 = Vec3::new(-35.0, -20.0, 60.0);
const PLAYER_MODEL_SCENE_Z: f32 = 0.30;
const PLAYER_MODEL_CAMERA_Z_M: f32 = 140.0;
const PLAYER_MODEL_SCALE_MULTIPLIER: f32 = 1.36;
const PLAYER_WHEEL_VISUAL_SCALE: f32 = 1.70;
const PLAYER_VISUAL_RIDE_HEIGHT_OFFSET_M: f32 = 0.46;
const PLAYER_MODEL_WHEEL_FOREGROUND_Z_BIAS_M: f32 = 1.2;

pub struct VehicleGameplayPlugin;

impl Plugin for VehicleGameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VehicleInputState>()
            .init_resource::<VehicleInputBindings>()
            .init_resource::<VehicleTelemetry>()
            .init_resource::<CameraFollowState>()
            .init_resource::<VehicleStuntMetrics>()
            .init_resource::<StuntTrackingState>()
            .init_resource::<VehicleModelDebugState>()
            .add_message::<VehicleStuntEvent>()
            .add_message::<VehicleLandingEvent>()
            .add_systems(
                OnEnter(GameState::InRun),
                (
                    configure_camera_units,
                    spawn_vehicle_scene,
                    reset_stunt_metrics,
                    reset_camera_follow_state,
                ),
            )
            .add_systems(OnExit(GameState::InRun), cleanup_vehicle_scene)
            .add_systems(
                Update,
                (
                    request_vehicle_model_scene_dump_hotkey,
                    dump_loaded_vehicle_model_scene_info,
                )
                    .run_if(in_state(GameState::InRun))
                    .run_if(resource_exists::<GameConfig>),
            )
            .add_systems(
                Update,
                (
                    read_vehicle_input,
                    update_ground_spline_segments,
                    sync_rapier_gravity_from_config,
                    #[cfg(feature = "gaussian_splats")]
                    sort_splat_background_by_z_once,
                    apply_vehicle_kinematics,
                    configure_player_vehicle_model_visuals,
                    spin_wheel_pairs,
                    sync_player_vehicle_visual_aim_and_model_wheels,
                    update_stunt_metrics,
                    update_player_health_bar,
                    update_vehicle_telemetry,
                    camera_follow_vehicle,
                    sync_vehicle_model_camera_with_gameplay_camera,
                    #[cfg(feature = "gaussian_splats")]
                    sync_splat_background_runtime_from_config,
                    #[cfg(feature = "gaussian_splats")]
                    update_splat_background_parallax,
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
struct PlayerVehicleModelCamera;

#[derive(Component, Debug, Clone, Copy)]
pub struct PlayerHealth {
    pub current: f32,
    pub max: f32,
}

#[derive(Component)]
struct GroundVisual;

#[derive(Component)]
struct BackgroundVisual;

#[cfg(feature = "gaussian_splats")]
#[derive(Component)]
struct SplatBackgroundCloud;

#[cfg(feature = "gaussian_splats")]
#[derive(Component)]
struct SplatBackgroundSorted;

#[cfg(feature = "gaussian_splats")]
#[derive(Component, Debug, Clone, Copy)]
struct SplatBackgroundCamera {
    parallax: f32,
    loop_length_m: f32,
}

#[derive(Component)]
struct GroundSplineSegment {
    x0: f32,
    x1: f32,
}

#[derive(Component)]
struct GroundPhysicsCollider;

#[derive(Component)]
struct GroundStripVisual;

#[derive(Component)]
struct GroundCurtainVisual;

#[derive(Component)]
struct YardstickVisualRoot;

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
struct PlayerVehiclePlaceholderVisual;

#[derive(Component, Debug, Clone, Copy)]
struct PlayerWheelPairVisual {
    axle: WheelAxle,
    radius_m: f32,
    driven: bool,
    hardpoint_local: Vec2,
}

#[derive(Component, Debug, Clone)]
struct PlayerVehicleModelScene {
    model_id: String,
    scene_path: String,
    expected_root_node: String,
    expected_wheel_nodes: Vec<String>,
    expected_turret_node: Option<String>,
}

#[derive(Debug, Clone)]
struct PlayerVehicleModelSceneSpawn {
    handle: Handle<Scene>,
    scene_metadata: PlayerVehicleModelScene,
}

#[derive(Resource, Debug, Default)]
struct VehicleModelDebugState {
    dump_requested: bool,
}

#[derive(Component, Debug, Clone, Default)]
struct PlayerVehicleModelRuntime {
    configured: bool,
}

#[derive(Component, Debug, Clone, Copy)]
struct PlayerVehicleModelWheelNode {
    axle: WheelAxle,
    base_translation: Vec3,
    base_rotation: Quat,
    base_scale: Vec3,
    pivot_local: Vec3,
    visual_scale_multiplier: f32,
    spin_axis_local: Vec3,
}

#[derive(Component, Debug, Clone, Copy)]
struct PlayerVehicleModelTurretNode {
    base_translation: Vec3,
    base_rotation: Quat,
    base_scale: Vec3,
    pivot_local: Vec3,
    aim_axis_local: Vec3,
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

#[derive(Resource, Debug, Clone, Default)]
struct CameraFollowState {
    initialized: bool,
    look_ahead_m: f32,
}

#[derive(Resource, Debug, Clone)]
pub struct VehicleStuntMetrics {
    pub airtime_current_s: f32,
    pub airtime_total_s: f32,
    pub airtime_best_s: f32,
    pub wheelie_current_s: f32,
    pub wheelie_total_s: f32,
    pub wheelie_best_s: f32,
    pub flip_count: u32,
    pub big_jump_count: u32,
    pub huge_jump_count: u32,
    pub long_wheelie_count: u32,
    pub crash_count: u32,
    pub max_speed_mps: f32,
    pub last_landing_impact_speed_mps: f32,
}

impl Default for VehicleStuntMetrics {
    fn default() -> Self {
        Self {
            airtime_current_s: 0.0,
            airtime_total_s: 0.0,
            airtime_best_s: 0.0,
            wheelie_current_s: 0.0,
            wheelie_total_s: 0.0,
            wheelie_best_s: 0.0,
            flip_count: 0,
            big_jump_count: 0,
            huge_jump_count: 0,
            long_wheelie_count: 0,
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
    wheelie_long_awarded_this_streak: bool,
}

#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub enum VehicleStuntEvent {
    AirtimeBig { duration_s: f32 },
    AirtimeHuge { duration_s: f32 },
    WheelieLong { duration_s: f32 },
    Flip { total_flips: u32 },
}

#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct VehicleLandingEvent {
    pub world_position: Vec2,
    pub impact_speed_mps: f32,
    pub was_crash: bool,
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
