use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_vehicle_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    config: Res<GameConfig>,
    asset_registry: Option<Res<AssetRegistry>>,
    asset_server: Res<AssetServer>,
    camera_query: Query<Entity, With<Camera2d>>,
    existing_player: Query<Entity, With<PlayerVehicle>>,
    existing_ground: Query<Entity, With<GroundVisual>>,
    existing_background: Query<Entity, With<BackgroundVisual>>,
    existing_yardstick: Query<Entity, With<YardstickVisualRoot>>,
    existing_model_camera: Query<Entity, With<PlayerVehicleModelCamera>>,
) {
    #[cfg(not(feature = "gaussian_splats"))]
    let _ = &asset_server;

    if existing_model_camera.is_empty() {
        commands.spawn((
            Name::new("PlayerVehicleModelCamera"),
            PlayerVehicleModelCamera,
            Camera3d::default(),
            Projection::Orthographic(OrthographicProjection {
                scale: CAMERA_ORTHO_SCALE_METERS,
                ..OrthographicProjection::default_3d()
            }),
            Camera {
                order: 2,
                clear_color: bevy::camera::ClearColorConfig::None,
                ..default()
            },
            Transform::from_xyz(0.0, CAMERA_Y, PLAYER_MODEL_CAMERA_Z_M)
                .looking_at(Vec3::new(0.0, CAMERA_Y, 0.0), Vec3::Y),
        ));
    }

    if existing_player.is_empty() {
        let Some(vehicle) = config.vehicles_by_id.get(&config.game.app.default_vehicle) else {
            return;
        };
        let model_scene = asset_registry
            .as_ref()
            .and_then(|registry| {
                resolve_vehicle_model_entry(registry, &config.game.app.default_vehicle)
            })
            .and_then(|(model_id, model_entry)| {
                model_entry
                    .handle
                    .as_ref()
                    .map(|handle| PlayerVehicleModelSceneSpawn {
                        handle: handle.clone(),
                        scene_metadata: PlayerVehicleModelScene {
                            model_id: model_id.to_string(),
                            scene_path: model_entry.scene_path.clone(),
                            expected_root_node: model_entry.hierarchy.root_node.clone(),
                            expected_wheel_nodes: model_entry.hierarchy.wheel_nodes.clone(),
                            expected_turret_node: model_entry.hierarchy.turret_node.clone(),
                        },
                    })
            });
        let chassis_half_extents =
            Vec2::new(PLAYER_CHASSIS_SIZE.x * 0.48, PLAYER_CHASSIS_SIZE.y * 0.36);
        let chassis_size = chassis_half_extents * 2.0;
        let chassis_base_principal_inertia = (PLAYER_CHASSIS_MASS_KG
            * ((chassis_size.x * chassis_size.x) + (chassis_size.y * chassis_size.y)))
            / 12.0;
        let chassis_principal_inertia =
            chassis_base_principal_inertia * vehicle.rotational_inertia.max(0.05);

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
            .insert((
                RigidBody::Dynamic,
                Collider::cuboid(chassis_half_extents.x, chassis_half_extents.y),
                ColliderMassProperties::MassProperties(MassProperties {
                    local_center_of_mass: Vec2::new(0.0, PLAYER_CHASSIS_CENTER_OF_MASS_Y_M),
                    mass: PLAYER_CHASSIS_MASS_KG,
                    principal_inertia: chassis_principal_inertia,
                }),
                Friction::coefficient(1.20),
                Restitution::coefficient(0.02),
                GravityScale(vehicle.gravity_scale),
                Velocity::zero(),
                ExternalForce::default(),
                Damping {
                    linear_damping: vehicle.air_base_damping.max(0.01),
                    angular_damping: vehicle.air_base_damping.max(0.01),
                },
                Ccd::enabled(),
                Sleeping::disabled(),
            ))
            .id();

        let wheel_mesh = meshes.add(RegularPolygon::new(PLAYER_WHEEL_RADIUS_M, 6));
        let front_wheel_material =
            materials.add(ColorMaterial::from(Color::srgb(0.70, 0.80, 0.90)));
        let rear_wheel_material = materials.add(ColorMaterial::from(Color::srgb(0.62, 0.73, 0.84)));

        commands.entity(player_entity).with_children(|parent| {
            if let Some(model) = &model_scene {
                parent.spawn((
                    Name::new("PlayerVehicleModelScene"),
                    model.scene_metadata.clone(),
                    PlayerVehicleModelRuntime::default(),
                    SceneRoot(model.handle.clone()),
                    Transform::from_xyz(
                        0.0,
                        -0.02 + PLAYER_VISUAL_RIDE_HEIGHT_OFFSET_M,
                        PLAYER_MODEL_SCENE_Z,
                    ),
                ));
            }

            parent.spawn((
                Name::new("PlayerChassis"),
                PlayerChassisVisual,
                PlayerVehiclePlaceholderVisual,
                Sprite::from_color(Color::srgb(0.93, 0.34, 0.24), PLAYER_CHASSIS_SIZE),
                Transform::from_xyz(0.0, -0.02 + PLAYER_VISUAL_RIDE_HEIGHT_OFFSET_M, 0.00),
                if DRAW_PLAYER_GAMEPLAY_BOX_VISUALS {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ));

            parent.spawn((
                Name::new("PlayerTurretBody"),
                PlayerTurretVisual,
                PlayerVehiclePlaceholderVisual,
                Sprite::from_color(Color::srgb(0.98, 0.44, 0.24), PLAYER_TURRET_SIZE),
                Transform::from_translation(
                    PLAYER_TURRET_OFFSET_LOCAL + (Vec3::Y * PLAYER_VISUAL_RIDE_HEIGHT_OFFSET_M),
                ),
                if DRAW_PLAYER_GAMEPLAY_BOX_VISUALS {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
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
                    0.80,
                )
                .with_scale(Vec3::splat(PLAYER_WHEEL_VISUAL_SCALE)),
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
                    0.80,
                )
                .with_scale(Vec3::splat(PLAYER_WHEEL_VISUAL_SCALE)),
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
                InheritedVisibility::VISIBLE,
                ViewVisibility::default(),
            ))
            .id();

        let ground_profile = build_ground_profile_samples(&config);
        let strip_mesh = meshes.add(build_ground_strip_mesh(&ground_profile));
        let curtain_mesh = meshes.add(build_ground_curtain_mesh(&ground_profile));
        let strip_texture_path = resolve_ground_texture_path(
            GROUND_STRIP_TEXTURE_PRIMARY_PATH,
            GROUND_STRIP_TEXTURE_FALLBACK_PATH,
        );
        let curtain_texture_path = resolve_ground_texture_path(
            GROUND_CURTAIN_TEXTURE_PRIMARY_PATH,
            GROUND_CURTAIN_TEXTURE_FALLBACK_PATH,
        );
        let strip_texture = load_repeating_texture(&asset_server, &strip_texture_path);
        let curtain_texture = load_repeating_texture(&asset_server, &curtain_texture_path);
        let strip_material = materials.add(ColorMaterial {
            color: Color::WHITE,
            texture: Some(strip_texture),
            ..default()
        });
        let curtain_material = materials.add(ColorMaterial {
            color: Color::WHITE,
            texture: Some(curtain_texture),
            ..default()
        });

        commands.entity(ground_entity).with_children(|parent| {
            parent.spawn((
                Name::new("GroundSplineStrip"),
                GroundStripVisual,
                Mesh2d(strip_mesh),
                MeshMaterial2d(strip_material),
                Transform::default(),
            ));

            parent.spawn((
                Name::new("GroundSplineCurtain"),
                GroundCurtainVisual,
                Mesh2d(curtain_mesh),
                MeshMaterial2d(curtain_material),
                Transform::default(),
            ));

            for segment in ground_profile.segments.iter().copied() {
                parent.spawn((
                    Name::new("GroundSplineColliderSegment"),
                    GroundSplineSegment {
                        x0: segment.x0,
                        x1: segment.x1,
                    },
                    GroundPhysicsCollider,
                    RigidBody::Fixed,
                    Collider::segment(segment.top0, segment.top1),
                    Friction::coefficient(1.35),
                    Restitution::coefficient(0.0),
                    Transform::default(),
                    GlobalTransform::default(),
                ));
            }
        });
    }

    if existing_background.is_empty() {
        let spawned_splat_background = {
            #[cfg(feature = "gaussian_splats")]
            {
                spawn_gaussian_splat_background(&mut commands, &config, &asset_server)
            }
            #[cfg(not(feature = "gaussian_splats"))]
            {
                false
            }
        };

        if !spawned_splat_background {
            let background_entity = commands
                .spawn((
                    Name::new("BackgroundVisual"),
                    BackgroundVisual,
                    Sprite::from_color(
                        Color::srgb(0.07, 0.09, 0.12),
                        Vec2::new(BACKGROUND_WIDTH, BACKGROUND_BAND_HEIGHT),
                    ),
                    Transform::from_xyz(BACKGROUND_WIDTH * 0.0, BACKGROUND_Y, -20.0),
                    Visibility::Inherited,
                    InheritedVisibility::VISIBLE,
                    ViewVisibility::default(),
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

    if existing_yardstick.is_empty() {
        let Ok(camera_entity) = camera_query.single() else {
            return;
        };

        let yardstick_root = commands
            .spawn((
                Name::new("YardstickVisualRoot"),
                YardstickVisualRoot,
                Transform::from_translation(YARDSTICK_OFFSET_FROM_CAMERA),
                GlobalTransform::default(),
                Visibility::Inherited,
                InheritedVisibility::VISIBLE,
                ViewVisibility::default(),
            ))
            .id();

        commands.entity(camera_entity).add_child(yardstick_root);
        commands.entity(yardstick_root).with_children(|parent| {
            parent.spawn((
                Name::new("YardstickBase"),
                Sprite::from_color(
                    Color::srgba(0.82, 0.86, 0.92, 0.90),
                    Vec2::new(YARDSTICK_LENGTH_M, YARDSTICK_BASE_THICKNESS_M),
                ),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));

            let notch_count = (YARDSTICK_LENGTH_M / YARDSTICK_INTERVAL_M).round() as i32;
            for notch_index in 0..=notch_count {
                let distance_m = notch_index as f32 * YARDSTICK_INTERVAL_M;
                let x = -YARDSTICK_LENGTH_M * 0.5 + distance_m;
                let is_major = (distance_m % YARDSTICK_MAJOR_INTERVAL_M).abs() < 0.01;
                let notch_height = if is_major {
                    YARDSTICK_MAJOR_NOTCH_HEIGHT_M
                } else {
                    YARDSTICK_MINOR_NOTCH_HEIGHT_M
                };

                parent.spawn((
                    Name::new("YardstickNotch"),
                    Sprite::from_color(
                        Color::srgba(0.82, 0.86, 0.92, if is_major { 0.94 } else { 0.72 }),
                        Vec2::new(YARDSTICK_NOTCH_THICKNESS_M, notch_height),
                    ),
                    Transform::from_xyz(x, (YARDSTICK_BASE_THICKNESS_M + notch_height) * 0.5, 0.01),
                ));
            }
        });
    }
}

pub(super) fn cleanup_vehicle_scene(
    mut commands: Commands,
    player_query: Query<Entity, With<PlayerVehicle>>,
    ground_query: Query<Entity, With<GroundVisual>>,
    background_query: Query<Entity, With<BackgroundVisual>>,
    yardstick_query: Query<Entity, With<YardstickVisualRoot>>,
    model_camera_query: Query<Entity, With<PlayerVehicleModelCamera>>,
) {
    for entity in &player_query {
        commands.entity(entity).try_despawn();
    }
    for entity in &ground_query {
        commands.entity(entity).try_despawn();
    }
    for entity in &background_query {
        commands.entity(entity).try_despawn();
    }
    for entity in &yardstick_query {
        commands.entity(entity).try_despawn();
    }
    for entity in &model_camera_query {
        commands.entity(entity).try_despawn();
    }
}

fn resolve_vehicle_model_entry<'a>(
    registry: &'a AssetRegistry,
    vehicle_id: &str,
) -> Option<(String, &'a ModelAssetEntry)> {
    let preferred_id = format!("vehicle_{vehicle_id}");
    if let Some(entry) = registry.models.get(&preferred_id) {
        return Some((preferred_id, entry));
    }

    if let Some(entry) = registry.models.get(vehicle_id) {
        return Some((vehicle_id.to_string(), entry));
    }

    None
}
