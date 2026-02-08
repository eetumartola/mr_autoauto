use super::*;

pub(super) fn update_ground_spline_segments(
    config: Res<GameConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut collider_query: Query<(&GroundSplineSegment, &mut Collider), With<GroundPhysicsCollider>>,
    strip_query: Query<&Mesh2d, With<GroundStripVisual>>,
    curtain_query: Query<&Mesh2d, With<GroundCurtainVisual>>,
) {
    if !config.is_changed() {
        return;
    }

    let profile = build_ground_profile_samples(&config);

    if let Ok(strip_mesh) = strip_query.single() {
        if let Some(mesh) = meshes.get_mut(&strip_mesh.0) {
            *mesh = build_ground_strip_mesh(&profile);
        }
    }

    if let Ok(curtain_mesh) = curtain_query.single() {
        if let Some(mesh) = meshes.get_mut(&curtain_mesh.0) {
            *mesh = build_ground_curtain_mesh(&profile);
        }
    }

    for (segment, mut collider) in &mut collider_query {
        let top0 = Vec2::new(segment.x0, terrain_height_at_x(&config, segment.x0));
        let top1 = Vec2::new(segment.x1, terrain_height_at_x(&config, segment.x1));
        *collider = Collider::segment(top0, top1);
    }
}

pub(super) fn reset_stunt_metrics(
    mut metrics: ResMut<VehicleStuntMetrics>,
    mut tracking: ResMut<StuntTrackingState>,
) {
    *metrics = VehicleStuntMetrics::default();
    *tracking = StuntTrackingState::default();
}

pub(super) fn update_stunt_metrics(
    time: Res<Time>,
    config: Res<GameConfig>,
    mut metrics: ResMut<VehicleStuntMetrics>,
    mut tracking: ResMut<StuntTrackingState>,
    mut stunt_events: MessageWriter<VehicleStuntEvent>,
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

    let commentator_thresholds = &config.commentator.thresholds;
    if contact.grounded {
        if !tracking.was_grounded {
            let landed_airtime_s = metrics.airtime_current_s;
            if landed_airtime_s >= commentator_thresholds.airtime_huge_jump.max(0.01) {
                metrics.huge_jump_count = metrics.huge_jump_count.saturating_add(1);
                metrics.big_jump_count = metrics.big_jump_count.saturating_add(1);
                stunt_events.write(VehicleStuntEvent::AirtimeHuge {
                    duration_s: landed_airtime_s,
                });
            } else if landed_airtime_s >= commentator_thresholds.airtime_big_jump.max(0.01) {
                metrics.big_jump_count = metrics.big_jump_count.saturating_add(1);
                stunt_events.write(VehicleStuntEvent::AirtimeBig {
                    duration_s: landed_airtime_s,
                });
            }
        }
        metrics.airtime_current_s = 0.0;
        tracking.airborne_rotation_accum_rad = 0.0;
    } else {
        metrics.airtime_current_s += dt;
        metrics.airtime_total_s += dt;
        metrics.airtime_best_s = metrics.airtime_best_s.max(metrics.airtime_current_s);

        if !tracking.was_grounded {
            tracking.airborne_rotation_accum_rad +=
                shortest_angle_delta_rad(angle_rad, tracking.previous_angle_rad).abs();
            while tracking.airborne_rotation_accum_rad >= TAU {
                metrics.flip_count = metrics.flip_count.saturating_add(1);
                stunt_events.write(VehicleStuntEvent::Flip {
                    total_flips: metrics.flip_count,
                });
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
        metrics.wheelie_total_s += dt;
        metrics.wheelie_best_s = metrics.wheelie_best_s.max(metrics.wheelie_current_s);
        if !tracking.wheelie_long_awarded_this_streak
            && metrics.wheelie_current_s >= commentator_thresholds.wheelie_long.max(0.01)
        {
            metrics.long_wheelie_count = metrics.long_wheelie_count.saturating_add(1);
            tracking.wheelie_long_awarded_this_streak = true;
            stunt_events.write(VehicleStuntEvent::WheelieLong {
                duration_s: metrics.wheelie_current_s,
            });
        }
    } else {
        metrics.wheelie_current_s = 0.0;
        tracking.wheelie_long_awarded_this_streak = false;
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

pub(super) fn read_vehicle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    bindings: Res<VehicleInputBindings>,
    mut input_state: ResMut<VehicleInputState>,
) {
    input_state.accelerate = bindings.accelerate.iter().any(|key| keyboard.pressed(*key));
    input_state.brake = bindings.brake.iter().any(|key| keyboard.pressed(*key));
}

pub(super) fn sync_rapier_gravity_from_config(
    config: Res<GameConfig>,
    mut rapier_config_query: Query<&mut RapierConfiguration, With<DefaultRapierContext>>,
    mut player_gravity_query: Query<&mut GravityScale, With<PlayerVehicle>>,
) {
    let Some(environment) = config
        .environments_by_id
        .get(&config.game.app.starting_environment)
    else {
        return;
    };

    if let Ok(mut rapier_config) = rapier_config_query.single_mut() {
        rapier_config.gravity = Vec2::new(0.0, -environment.gravity.max(0.0));
    }

    if let Some(vehicle) = config.vehicles_by_id.get(&config.game.app.default_vehicle) {
        if let Ok(mut gravity_scale) = player_gravity_query.single_mut() {
            gravity_scale.0 = vehicle.gravity_scale.max(0.01);
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn apply_vehicle_kinematics(
    time: Res<Time>,
    config: Res<GameConfig>,
    input_state: Res<VehicleInputState>,
    debug_guards: Option<Res<DebugGameplayGuards>>,
    rapier_context: ReadRapierContext,
    mut landing_events: MessageWriter<VehicleLandingEvent>,
    mut player_query: Query<
        (
            Entity,
            &Transform,
            &mut Velocity,
            &mut ExternalForce,
            &mut Damping,
            Option<&ReadMassProperties>,
            &mut VehicleKinematics,
            &mut VehicleSuspensionState,
            &mut GroundContact,
            &mut PlayerHealth,
        ),
        With<PlayerVehicle>,
    >,
) {
    let Ok(rapier_context) = rapier_context.single() else {
        return;
    };
    let Ok((
        player_entity,
        transform,
        mut velocity,
        mut external_force,
        mut damping,
        mass_properties,
        mut kinematics,
        mut suspension,
        mut contact,
        mut health,
    )) = player_query.single_mut()
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
    let player_invulnerable = debug_guards
        .as_ref()
        .is_some_and(|guards| guards.player_invulnerable);
    let throttle = if input_state.accelerate { 1.0 } else { 0.0 };
    let brake = if input_state.brake { 1.0 } else { 0.0 };
    let (_, _, z_rot_rad) = transform.rotation.to_euler(EulerRot::XYZ);
    let body_center = transform.translation.truncate();
    *external_force = ExternalForce::default();

    let rest_length = vehicle.suspension_rest_length_m.max(0.01);
    let min_length = (rest_length - vehicle.suspension_max_compression_m.max(0.01)).max(0.02);
    let max_length = rest_length + vehicle.suspension_max_extension_m.max(0.0);
    let max_compression = (rest_length - min_length).max(0.001);

    let (front_spring_length, front_sample, front_wheel_grounded) = sample_wheel_suspension(
        &rapier_context,
        player_entity,
        body_center,
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
        &rapier_context,
        player_entity,
        body_center,
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
    let front_grip_factor = vehicle.tire_longitudinal_grip
        * (vehicle.tire_slip_grip_floor
            + ((1.0 - vehicle.tire_slip_grip_floor) * front_sample.compression_ratio))
            .clamp(0.0, 1.0);
    let rear_grip_factor = vehicle.tire_longitudinal_grip
        * (vehicle.tire_slip_grip_floor
            + ((1.0 - vehicle.tire_slip_grip_floor) * rear_sample.compression_ratio))
            .clamp(0.0, 1.0);
    let front_drive_ratio = vehicle.front_drive_ratio.clamp(0.0, 1.0);
    let rear_drive_ratio = 1.0 - front_drive_ratio;

    let rear_assist_distance_m = vehicle.rear_drive_traction_assist_distance_m.max(0.0);
    let rear_assist_min_factor = vehicle
        .rear_drive_traction_assist_min_factor
        .clamp(0.0, 1.0);
    let chassis_up_alignment = (Mat2::from_angle(z_rot_rad) * Vec2::Y)
        .dot(Vec2::Y)
        .clamp(-1.0, 1.0);
    let chassis_drive_alignment = ((chassis_up_alignment + 1.0) * 0.5).clamp(0.0, 1.0);
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
    } else {
        0.0
    };
    let front_assist_factor = if front_wheel_grounded { 1.0 } else { 0.0 };
    let front_drive_factor = front_grip_factor * front_assist_factor * chassis_drive_alignment;
    let rear_drive_factor = rear_grip_factor * rear_assist_factor * chassis_drive_alignment;
    let brake_ground_factor = grounded_wheel_ratio.max(WHEEL_FRICTION_MIN_FACTOR);
    let mut front_longitudinal_accel =
        throttle * drive_accel * front_drive_ratio * front_drive_factor;
    let mut rear_longitudinal_accel = throttle * drive_accel * rear_drive_ratio * rear_drive_factor;
    if brake > 0.0 {
        if velocity.linvel.x > 0.25 {
            let braking_accel = brake * brake_accel * brake_ground_factor;
            let front_brake_weight = if front_wheel_grounded { 0.6 } else { 0.0 };
            let rear_brake_weight = if rear_wheel_grounded { 0.4 } else { 0.0 };
            let brake_weight_sum = front_brake_weight + rear_brake_weight;
            if brake_weight_sum > f32::EPSILON {
                front_longitudinal_accel -= braking_accel * (front_brake_weight / brake_weight_sum);
                rear_longitudinal_accel -= braking_accel * (rear_brake_weight / brake_weight_sum);
            } else {
                front_longitudinal_accel -= braking_accel * 0.5;
                rear_longitudinal_accel -= braking_accel * 0.5;
            }
        } else {
            front_longitudinal_accel -=
                brake * brake_accel * front_drive_ratio * front_drive_factor;
            rear_longitudinal_accel -= brake * brake_accel * rear_drive_ratio * rear_drive_factor;
        }
    }

    let ground_damping_scale = (0.45 + (grounded_wheel_ratio * 0.55)).clamp(0.45, 1.0);
    if front_wheel_grounded || rear_wheel_grounded {
        damping.linear_damping = (vehicle.ground_coast_damping * ground_damping_scale).max(0.02);
        damping.angular_damping = (vehicle.ground_coast_damping * 2.9).max(0.34);
    } else {
        let air_damping =
            vehicle.air_base_damping + (environment.drag * vehicle.air_env_drag_factor);
        damping.linear_damping = air_damping.max(0.01);
        damping.angular_damping = (air_damping * AIR_ANGULAR_DAMPING).max(0.02);
    }

    let front_hardpoint_world = wheel_hardpoint_world(
        body_center,
        z_rot_rad,
        Vec2::new(PLAYER_FRONT_HARDPOINT_X_M, PLAYER_FRONT_HARDPOINT_Y_M),
    );
    let rear_hardpoint_world = wheel_hardpoint_world(
        body_center,
        z_rot_rad,
        Vec2::new(PLAYER_REAR_HARDPOINT_X_M, PLAYER_REAR_HARDPOINT_Y_M),
    );

    let suspension_front_force = front_sample.support_force_n;
    if suspension_front_force > f32::EPSILON {
        *external_force += ExternalForce::at_point(
            Vec2::Y * suspension_front_force,
            front_hardpoint_world,
            body_center,
        );
    }
    let suspension_rear_force = rear_sample.support_force_n;
    if suspension_rear_force > f32::EPSILON {
        *external_force += ExternalForce::at_point(
            Vec2::Y * suspension_rear_force,
            rear_hardpoint_world,
            body_center,
        );
    }

    let body_mass = mass_properties
        .map(|props| props.mass)
        .unwrap_or(vehicle.linear_inertia.max(0.5))
        .max(0.25);
    let forward_direction = Mat2::from_angle(z_rot_rad) * Vec2::X;
    if front_longitudinal_accel.abs() > f32::EPSILON {
        let front_drive_force_n = front_longitudinal_accel * body_mass;
        *external_force += ExternalForce::at_point(
            forward_direction * front_drive_force_n,
            front_hardpoint_world,
            body_center,
        );
    }
    if rear_longitudinal_accel.abs() > f32::EPSILON {
        let rear_drive_force_n = rear_longitudinal_accel * body_mass;
        *external_force += ExternalForce::at_point(
            forward_direction * rear_drive_force_n,
            rear_hardpoint_world,
            body_center,
        );
    }

    let front_grounded_after = front_wheel_grounded;
    let rear_grounded_after = rear_wheel_grounded;
    let grounded_now = front_grounded_after || rear_grounded_after;
    suspension.front_grounded = front_grounded_after;
    suspension.rear_grounded = rear_grounded_after;

    let air_control_factor = environment.air_control.max(0.0);
    let air_rotation_input = throttle - brake;
    if !grounded_now && air_rotation_input.abs() > f32::EPSILON {
        velocity.angvel =
            air_rotation_input * vehicle.air_max_rotation_speed.max(0.1) * air_control_factor;
    }

    if grounded_now {
        if !was_grounded && velocity.linvel.y < 0.0 {
            contact.just_landed = true;
            contact.landing_impact_speed_mps = -velocity.linvel.y;
            let landing_crash =
                contact.landing_impact_speed_mps >= CRASH_LANDING_SPEED_THRESHOLD_MPS;
            landing_events.write(VehicleLandingEvent {
                world_position: body_center,
                impact_speed_mps: contact.landing_impact_speed_mps,
                was_crash: landing_crash,
            });
            let impact_over_threshold =
                (contact.landing_impact_speed_mps - CRASH_LANDING_SPEED_THRESHOLD_MPS).max(0.0);
            if impact_over_threshold > 0.0 && !player_invulnerable {
                let damage = impact_over_threshold * LANDING_DAMAGE_PER_MPS_OVER_THRESHOLD;
                health.current = (health.current - damage).max(0.0);
            }
        }

        contact.grounded = true;
    } else {
        contact.grounded = false;
    }

    velocity.linvel.x = velocity
        .linvel
        .x
        .clamp(-vehicle.max_reverse_speed, vehicle.max_forward_speed);
    velocity.linvel.y = velocity
        .linvel
        .y
        .clamp(-vehicle.max_fall_speed, vehicle.max_fall_speed);
    if grounded_now {
        velocity.angvel = velocity
            .angvel
            .clamp(-GROUND_MAX_ANGULAR_SPEED, GROUND_MAX_ANGULAR_SPEED);
    }

    kinematics.velocity = velocity.linvel;
    kinematics.angular_velocity = velocity.angvel;
}

pub(super) fn update_player_health_bar(
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

pub(super) fn update_vehicle_telemetry(
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

pub(super) fn reset_camera_follow_state(mut state: ResMut<CameraFollowState>) {
    *state = CameraFollowState::default();
}

pub(super) fn camera_follow_vehicle(
    time: Res<Time>,
    telemetry: Res<VehicleTelemetry>,
    config: Res<GameConfig>,
    debug_camera_pan: Option<Res<DebugCameraPanState>>,
    mut follow_state: ResMut<CameraFollowState>,
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
    let pan_offset = debug_camera_pan.map(|pan| pan.offset_x_m).unwrap_or(0.0);
    let target_look_ahead_m = (telemetry.speed_mps * vehicle.camera_look_ahead_factor)
        .clamp(vehicle.camera_look_ahead_min, vehicle.camera_look_ahead_max);
    let dt = time.delta_secs().max(0.000_1);
    let max_look_ahead_step = CAMERA_LOOKAHEAD_MAX_STEP_MPS * dt;
    if !follow_state.initialized {
        follow_state.initialized = true;
        follow_state.look_ahead_m = target_look_ahead_m;
        camera_transform.translation.x =
            player_transform.translation.x + target_look_ahead_m + pan_offset;
    } else {
        follow_state.look_ahead_m = move_towards(
            follow_state.look_ahead_m,
            target_look_ahead_m,
            max_look_ahead_step,
        );
        let target_camera_x =
            player_transform.translation.x + follow_state.look_ahead_m + pan_offset;
        let camera_blend = (CAMERA_FOLLOW_SMOOTH_RATE_HZ * dt).clamp(0.0, 1.0);
        camera_transform.translation.x = camera_transform
            .translation
            .x
            .lerp(target_camera_x, camera_blend);
    }
    camera_transform.translation.y = CAMERA_Y;
    camera_transform.translation.z = CAMERA_Z;
}

pub(super) fn sync_vehicle_model_camera_with_gameplay_camera(
    gameplay_camera_query: Query<&Transform, (With<Camera2d>, Without<PlayerVehicleModelCamera>)>,
    mut model_camera_query: Query<&mut Transform, With<PlayerVehicleModelCamera>>,
) {
    let Ok(gameplay_transform) = gameplay_camera_query.single() else {
        return;
    };
    let Ok(mut model_camera_transform) = model_camera_query.single_mut() else {
        return;
    };

    model_camera_transform.translation.x = gameplay_transform.translation.x;
    model_camera_transform.translation.y = gameplay_transform.translation.y;
    model_camera_transform.translation.z = PLAYER_MODEL_CAMERA_Z_M;
    model_camera_transform.look_at(
        Vec3::new(
            gameplay_transform.translation.x,
            gameplay_transform.translation.y,
            0.0,
        ),
        Vec3::Y,
    );
}

#[cfg(feature = "gaussian_splats")]
#[allow(clippy::type_complexity)]
pub(super) fn update_splat_background_parallax(
    gameplay_camera_query: Query<&Transform, (With<Camera2d>, Without<SplatBackgroundCamera>)>,
    mut splat_camera_query: Query<
        (&SplatBackgroundCamera, &mut Transform),
        (With<SplatBackgroundCamera>, Without<Camera2d>),
    >,
) {
    let Ok(gameplay_camera) = gameplay_camera_query.single() else {
        return;
    };

    for (settings, mut transform) in &mut splat_camera_query {
        let mut parallax_x = gameplay_camera.translation.x * settings.parallax;
        if settings.loop_length_m > f32::EPSILON {
            parallax_x = parallax_x.rem_euclid(settings.loop_length_m);
        }
        transform.translation.x = parallax_x;
    }
}

#[cfg(feature = "gaussian_splats")]
pub(super) fn sync_splat_background_runtime_from_config(
    config: Res<GameConfig>,
    mut camera_query: Query<&mut SplatBackgroundCamera>,
    mut cloud_query: Query<&mut Transform, With<SplatBackgroundCloud>>,
) {
    let Some(first_segment) = config.segments.segment_sequence.first() else {
        return;
    };
    let Some(background_cfg) = config.backgrounds_by_id.get(&first_segment.id) else {
        return;
    };

    for mut camera in &mut camera_query {
        camera.parallax = background_cfg.parallax;
        camera.loop_length_m = background_cfg.loop_length_m.max(0.0);
    }

    for mut transform in &mut cloud_query {
        transform.translation = Vec3::new(
            background_cfg.offset_x_m,
            SPLAT_BACKGROUND_Y_OFFSET_M + background_cfg.offset_y_m,
            SPLAT_BACKGROUND_Z_M + background_cfg.offset_z_m,
        );
        transform.scale = Vec3::new(
            background_cfg.scale_x,
            background_cfg.scale_y,
            background_cfg.scale_z,
        );
    }
}

#[cfg(feature = "gaussian_splats")]
#[allow(clippy::type_complexity)]
pub(super) fn sort_splat_background_by_z_once(
    mut commands: Commands,
    mut cloud_assets: ResMut<Assets<PlanarGaussian3d>>,
    cloud_query: Query<
        (Entity, &PlanarGaussian3dHandle),
        (With<SplatBackgroundCloud>, Without<SplatBackgroundSorted>),
    >,
) {
    for (entity, cloud_handle) in &cloud_query {
        let Some(cloud) = cloud_assets.get_mut(&cloud_handle.0) else {
            continue;
        };

        let mut packed: Vec<Gaussian3d> = cloud.iter().collect();
        packed.sort_by(|left, right| {
            right.position_visibility.position[2].total_cmp(&left.position_visibility.position[2])
        });
        *cloud = PlanarGaussian3d::from(packed);

        commands.entity(entity).insert(SplatBackgroundSorted);
        info!("Sorted splat background by +z once; runtime sort disabled for that cloud.");
    }
}

#[cfg(feature = "gaussian_splats")]
pub(super) fn spawn_gaussian_splat_background(
    commands: &mut Commands,
    config: &GameConfig,
    asset_server: &AssetServer,
) -> bool {
    let Some(first_segment) = config.segments.segment_sequence.first() else {
        return false;
    };
    let Some(background_cfg) = config.backgrounds_by_id.get(&first_segment.id) else {
        return false;
    };
    let Some(splat_asset_id) = background_cfg.splat_asset_id.as_deref() else {
        return false;
    };
    let Some(splat_asset_cfg) = config.splat_assets_by_id.get(splat_asset_id) else {
        warn!(
            "Background `{}` references missing splat asset `{splat_asset_id}`.",
            background_cfg.id
        );
        return false;
    };

    let splat_handle: Handle<PlanarGaussian3d> = asset_server.load(splat_asset_cfg.path.clone());
    let parallax = background_cfg.parallax;
    let offset_x_m = background_cfg.offset_x_m;
    let offset_y_m = background_cfg.offset_y_m;
    let offset_z_m = background_cfg.offset_z_m;
    let loop_length_m = background_cfg.loop_length_m.max(0.0);
    let root_entity = commands
        .spawn((
            Name::new("BackgroundVisual"),
            BackgroundVisual,
            Transform::default(),
            GlobalTransform::default(),
            Visibility::Inherited,
            InheritedVisibility::VISIBLE,
            ViewVisibility::default(),
        ))
        .id();

    commands.entity(root_entity).with_children(|parent| {
        parent.spawn((
            Name::new("SplatBackgroundCloud"),
            SplatBackgroundCloud,
            PlanarGaussian3dHandle(splat_handle),
            RenderLayers::layer(SPLAT_BACKGROUND_RENDER_LAYER),
            CloudSettings {
                sort_mode: SortMode::None,
                aabb: false,
                visualize_bounding_box: false,
                ..default()
            },
            Transform::from_xyz(
                offset_x_m,
                SPLAT_BACKGROUND_Y_OFFSET_M + offset_y_m,
                SPLAT_BACKGROUND_Z_M + offset_z_m,
            )
            .with_scale(Vec3::new(
                background_cfg.scale_x,
                background_cfg.scale_y,
                background_cfg.scale_z,
            )),
        ));

        parent.spawn((
            Name::new("SplatBackgroundCamera"),
            SplatBackgroundCamera {
                parallax,
                loop_length_m,
            },
            RenderLayers::layer(SPLAT_BACKGROUND_RENDER_LAYER),
            GaussianCamera::default(),
            Camera3d::default(),
            Camera {
                order: 0,
                ..default()
            },
            Transform::from_xyz(0.0, SPLAT_CAMERA_Y_M, SPLAT_CAMERA_Z_M).looking_at(
                Vec3::new(0.0, SPLAT_CAMERA_Y_M, SPLAT_CAMERA_TARGET_Z_M),
                Vec3::Y,
            ),
        ));
    });

    info!(
        "Spawned splat background `{}` from `{}` with parallax {}.",
        splat_asset_id, splat_asset_cfg.path, parallax
    );
    true
}

#[allow(clippy::too_many_arguments)]
fn sample_wheel_suspension(
    rapier_context: &RapierContext<'_>,
    player_entity: Entity,
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
    let wheel_down_world = (Mat2::from_angle(root_z_rotation) * Vec2::NEG_Y).normalize_or_zero();
    let down_alignment = wheel_down_world.dot(Vec2::NEG_Y);
    if wheel_down_world.length_squared() <= f32::EPSILON {
        return (
            max_length_m,
            WheelSuspensionSample {
                compression_m: 0.0,
                compression_ratio: 0.0,
                support_force_n: 0.0,
                gap_to_ground_m: GROUND_RAYCAST_MAX_DISTANCE_M,
            },
            false,
        );
    }

    let ray_length = max_length_m + PLAYER_WHEEL_RADIUS_M + GROUND_RAYCAST_MAX_DISTANCE_M;
    let ray_filter = QueryFilter::only_fixed()
        .exclude_sensors()
        .exclude_rigid_body(player_entity);
    let hit = rapier_context.cast_ray_and_get_normal(
        hardpoint_world,
        wheel_down_world,
        ray_length,
        false,
        ray_filter,
    );
    let hit_toi = hit.map(|(_, intersection)| intersection.time_of_impact);
    let hit_normal = hit
        .map(|(_, intersection)| intersection.normal.normalize_or_zero())
        .unwrap_or(Vec2::Y);

    let contact_length = hit_toi
        .map(|toi| (toi - PLAYER_WHEEL_RADIUS_M).max(0.0))
        .unwrap_or(max_length_m + GROUND_RAYCAST_MAX_DISTANCE_M);
    let grounded = contact_length <= (max_length_m + PLAYER_REAR_WHEEL_GROUND_EPSILON_M)
        && hit_normal.y >= MIN_DRIVEABLE_GROUND_NORMAL_Y
        && down_alignment >= MIN_SUSPENSION_DOWN_ALIGNMENT;
    let gap_to_ground_m = (contact_length - max_length_m).max(0.0);
    let target_spring_length_m = if grounded {
        contact_length.clamp(min_length_m, max_length_m)
    } else {
        max_length_m
    };
    let prev_spring_length_m =
        (rest_length_m - prev_compression_m).clamp(min_length_m, max_length_m);
    let max_spring_step_m = if target_spring_length_m < prev_spring_length_m {
        SUSPENSION_MAX_COMPRESSION_SPEED_MPS * dt
    } else {
        SUSPENSION_MAX_REBOUND_SPEED_MPS * dt
    };
    let spring_length_m = move_towards(
        prev_spring_length_m,
        target_spring_length_m,
        max_spring_step_m.max(0.001),
    )
    .clamp(min_length_m, max_length_m);

    let compression_m = (rest_length_m - spring_length_m).clamp(0.0, max_compression_m);
    let compression_velocity_mps = (compression_m - prev_compression_m) / dt.max(0.000_1);
    let support_force_n = if grounded {
        ((compression_m * stiffness) + (compression_velocity_mps * damping))
            .clamp(0.0, SUSPENSION_FORCE_CLAMP_N)
            * hit_normal.y.clamp(0.0, 1.0)
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

pub(super) fn rear_wheel_root_contact_y(
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

fn shortest_angle_delta_rad(current: f32, previous: f32) -> f32 {
    (current - previous + PI).rem_euclid(TAU) - PI
}

fn move_towards(current: f32, target: f32, max_delta: f32) -> f32 {
    if (target - current).abs() <= max_delta {
        target
    } else {
        current + (target - current).signum() * max_delta
    }
}
