use super::*;

pub(super) fn request_vehicle_model_scene_dump_hotkey(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<VehicleModelDebugState>,
) {
    if keyboard.just_pressed(KeyCode::KeyN) {
        state.dump_requested = true;
    }
}

pub(super) fn dump_loaded_vehicle_model_scene_info(
    _asset_server: Res<AssetServer>,
    _scenes: ResMut<Assets<Scene>>,
    mut state: ResMut<VehicleModelDebugState>,
    _model_scene_query: Query<&PlayerVehicleModelScene>,
) {
    if !state.dump_requested {
        return;
    }
    state.dump_requested = false;
}

#[allow(clippy::type_complexity)]
pub(super) fn configure_player_vehicle_model_visuals(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut model_root_query: Query<(
        Entity,
        &PlayerVehicleModelScene,
        &mut PlayerVehicleModelRuntime,
        &mut Transform,
    )>,
    children_query: Query<&Children>,
    node_query: Query<
        (Option<&Name>, Option<&Mesh3d>, Option<&Transform>),
        Without<PlayerVehicleModelScene>,
    >,
) {
    let Some(vehicle) = config.vehicles_by_id.get(&config.game.app.default_vehicle) else {
        return;
    };

    for (scene_entity, model, mut runtime, mut scene_transform) in &mut model_root_query {
        if runtime.configured {
            continue;
        }

        let mut descendants = Vec::new();
        collect_descendants(scene_entity, &children_query, &mut descendants);
        if descendants.is_empty() {
            continue;
        }

        let snapshots: Vec<ModelSceneNodeSnapshot> = descendants
            .iter()
            .filter_map(|entity| {
                node_query
                    .get(*entity)
                    .ok()
                    .map(|(name, mesh, local_transform)| ModelSceneNodeSnapshot {
                        entity: *entity,
                        name: name.map(|value| value.as_str().to_string()),
                        mesh: mesh.map(|value| value.0.clone()),
                        local_transform: local_transform.copied(),
                    })
            })
            .collect();

        let Some(chassis_node) = find_named_node(&snapshots, &model.expected_root_node) else {
            continue;
        };
        let Some((chassis_min, chassis_max)) = snapshot_mesh_bounds(chassis_node, &meshes) else {
            continue;
        };
        let chassis_size = chassis_max - chassis_min;
        let source_chassis_extent_m = chassis_size.max_element().max(0.001);

        let mut wheel_nodes = Vec::new();
        for (wheel_index, expected_wheel) in model.expected_wheel_nodes.iter().take(4).enumerate() {
            let Some(wheel_node) = find_named_node(&snapshots, expected_wheel) else {
                wheel_nodes.clear();
                break;
            };
            let Some(mesh_handle) = wheel_node.mesh.clone() else {
                wheel_nodes.clear();
                break;
            };
            let Some((wheel_min, wheel_max)) = snapshot_mesh_bounds(wheel_node, &meshes) else {
                wheel_nodes.clear();
                break;
            };
            let Some(base_local_transform) = wheel_node.local_transform else {
                wheel_nodes.clear();
                break;
            };
            let center = (wheel_min + wheel_max) * 0.5;
            wheel_nodes.push(MatchedModelWheelNode {
                center,
                min: wheel_min,
                max: wheel_max,
                entity: wheel_node.entity,
                base_local_transform,
                axle: if wheel_index < 2 {
                    WheelAxle::Front
                } else {
                    WheelAxle::Rear
                },
                mesh_handle,
            });
        }
        if wheel_nodes.is_empty() {
            continue;
        }

        let mut tinted_meshes = HashSet::new();
        for wheel in &wheel_nodes {
            if !tinted_meshes.insert(wheel.mesh_handle.id()) {
                continue;
            }
            if let Some(mesh) = meshes.get_mut(&wheel.mesh_handle) {
                // Bevy's StandardMaterial shader multiplies base color by vertex color.
                apply_uniform_vertex_color(mesh, [0.2, 0.2, 0.2, 1.0]);
            }
        }

        let average_wheel_center = wheel_nodes
            .iter()
            .map(|wheel| wheel.center)
            .fold(Vec3::ZERO, |acc, value| acc + value)
            / wheel_nodes.len() as f32;

        let turret_snapshot = model.expected_turret_node.as_ref().and_then(|turret_name| {
            find_named_node(&snapshots, turret_name)
                .cloned()
                .map(|snapshot| (turret_name.clone(), snapshot))
        });
        let turret_bounds = turret_snapshot
            .as_ref()
            .and_then(|(turret_name, snapshot)| {
                snapshot_mesh_bounds(snapshot, &meshes).map(|(min, max)| {
                    (
                        turret_name.as_str(),
                        snapshot.entity,
                        snapshot.local_transform,
                        min,
                        max,
                    )
                })
            });

        let front_center = (wheel_nodes[0].center + wheel_nodes[1].center) * 0.5;
        let rear_center = (wheel_nodes[2].center + wheel_nodes[3].center) * 0.5;
        let left_center = (wheel_nodes[0].center + wheel_nodes[2].center) * 0.5;
        let right_center = (wheel_nodes[1].center + wheel_nodes[3].center) * 0.5;

        let source_forward = (front_center - rear_center).normalize_or_zero();
        let source_lateral_guess = (right_center - left_center).normalize_or_zero();
        if source_forward.length_squared() <= f32::EPSILON
            || source_lateral_guess.length_squared() <= f32::EPSILON
        {
            continue;
        }

        let mut source_up_hint = turret_bounds
            .as_ref()
            .map(|(_, _, _, min, max)| ((*min + *max) * 0.5) - average_wheel_center)
            .unwrap_or(Vec3::Y);
        source_up_hint -= source_forward * source_up_hint.dot(source_forward);
        if source_up_hint.length_squared() <= f32::EPSILON {
            source_up_hint = source_lateral_guess.cross(source_forward);
            source_up_hint -= source_forward * source_up_hint.dot(source_forward);
        }
        let source_up = source_up_hint.normalize_or_zero();
        if source_up.length_squared() <= f32::EPSILON {
            continue;
        }

        // Rotate model so source forward aligns with gameplay +X, then twist around +X so up aligns with +Y.
        let align_forward = Quat::from_rotation_arc(source_forward, Vec3::X);
        let up_after_forward = align_forward * source_up;
        let up_after_forward_projected =
            (up_after_forward - (Vec3::X * up_after_forward.dot(Vec3::X))).normalize_or_zero();
        if up_after_forward_projected.length_squared() <= f32::EPSILON {
            continue;
        }
        let twist_sign = Vec3::X
            .dot(up_after_forward_projected.cross(Vec3::Y))
            .signum();
        let twist_angle = up_after_forward_projected.angle_between(Vec3::Y)
            * if twist_sign == 0.0 { 1.0 } else { twist_sign };
        let twist = Quat::from_axis_angle(Vec3::X, twist_angle);
        let model_rotation = twist * align_forward;
        let rotation_matrix = Mat3::from_quat(model_rotation);
        let scene_out_of_plane_axis = (model_rotation.inverse() * Vec3::Z).normalize_or_zero();

        let source_wheelbase_m = front_center.distance(rear_center).max(0.001);
        let desired_wheelbase_m = (PLAYER_FRONT_HARDPOINT_X_M - PLAYER_REAR_HARDPOINT_X_M).abs();
        let wheelbase_scale = desired_wheelbase_m / source_wheelbase_m;

        let (forward_extent_m, up_extent_m) = chassis_node
            .mesh
            .as_ref()
            .and_then(|handle| meshes.get(handle))
            .map(|mesh| {
                (
                    mesh_projected_extent(mesh, source_forward).unwrap_or(source_chassis_extent_m),
                    mesh_projected_extent(mesh, source_up).unwrap_or(source_chassis_extent_m),
                )
            })
            .unwrap_or((source_chassis_extent_m, source_chassis_extent_m));
        let forward_scale = PLAYER_CHASSIS_SIZE.x / forward_extent_m.max(0.001);
        let up_scale = PLAYER_CHASSIS_SIZE.y / up_extent_m.max(0.001);
        let scale = (((wheelbase_scale * 0.55) + (forward_scale * 0.30) + (up_scale * 0.15))
            * PLAYER_MODEL_SCALE_MULTIPLIER)
            .clamp(0.01, 500.0);

        let desired_wheel_center_x = (PLAYER_FRONT_HARDPOINT_X_M + PLAYER_REAR_HARDPOINT_X_M) * 0.5;
        let desired_wheel_center_y = ((PLAYER_FRONT_HARDPOINT_Y_M + PLAYER_REAR_HARDPOINT_Y_M)
            * 0.5)
            - vehicle.suspension_rest_length_m
            + PLAYER_VISUAL_RIDE_HEIGHT_OFFSET_M;
        let scaled_rotated_wheel_center = rotation_matrix * (average_wheel_center * scale);

        scene_transform.translation = Vec3::new(
            desired_wheel_center_x - scaled_rotated_wheel_center.x,
            desired_wheel_center_y - scaled_rotated_wheel_center.y,
            PLAYER_MODEL_SETUP_DEPTH_Z - scaled_rotated_wheel_center.z,
        );
        scene_transform.rotation = model_rotation;
        scene_transform.scale = Vec3::splat(scale);

        if let Some((_turret_name, turret_entity, turret_local_transform, turret_min, turret_max)) =
            turret_bounds.as_ref()
        {
            let turret_pivot = Vec3::new(
                (turret_min.x + turret_max.x) * 0.5,
                turret_min.y,
                (turret_min.z + turret_max.z) * 0.5,
            );
            if let Some(local_transform) = *turret_local_transform {
                let aim_axis_local = (local_transform.rotation.inverse() * scene_out_of_plane_axis)
                    .normalize_or_zero();
                commands
                    .entity(*turret_entity)
                    .insert(PlayerVehicleModelTurretNode {
                        base_translation: local_transform.translation,
                        base_rotation: local_transform.rotation,
                        base_scale: local_transform.scale,
                        pivot_local: turret_pivot,
                        aim_axis_local: if aim_axis_local.length_squared() > f32::EPSILON {
                            aim_axis_local
                        } else {
                            Vec3::Z
                        },
                    });
            }
        }

        for wheel in &wheel_nodes {
            let source_wheel_radius_local =
                wheel_estimated_radius_from_bounds(wheel.min, wheel.max);
            let source_wheel_radius_after_scene_scale = source_wheel_radius_local * scale;
            let desired_visual_wheel_radius = PLAYER_WHEEL_RADIUS_M * PLAYER_WHEEL_VISUAL_SCALE;
            let visual_scale_multiplier = if source_wheel_radius_after_scene_scale > f32::EPSILON {
                (desired_visual_wheel_radius / source_wheel_radius_after_scene_scale)
                    .clamp(0.05, 20.0)
            } else {
                1.0
            };

            let spin_axis_local = (wheel.base_local_transform.rotation.inverse()
                * scene_out_of_plane_axis)
                .normalize_or_zero();
            commands
                .entity(wheel.entity)
                .insert(PlayerVehicleModelWheelNode {
                    axle: wheel.axle,
                    base_translation: wheel.base_local_transform.translation,
                    base_rotation: wheel.base_local_transform.rotation,
                    base_scale: wheel.base_local_transform.scale,
                    pivot_local: wheel.center,
                    visual_scale_multiplier,
                    spin_axis_local: if spin_axis_local.length_squared() > f32::EPSILON {
                        spin_axis_local
                    } else {
                        Vec3::Z
                    },
                });
        }

        runtime.configured = true;
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

#[derive(Clone)]
struct ModelSceneNodeSnapshot {
    entity: Entity,
    name: Option<String>,
    mesh: Option<Handle<Mesh>>,
    local_transform: Option<Transform>,
}

#[derive(Clone)]
struct MatchedModelWheelNode {
    center: Vec3,
    min: Vec3,
    max: Vec3,
    entity: Entity,
    base_local_transform: Transform,
    axle: WheelAxle,
    mesh_handle: Handle<Mesh>,
}

fn find_named_node<'a>(
    snapshots: &'a [ModelSceneNodeSnapshot],
    expected_name: &str,
) -> Option<&'a ModelSceneNodeSnapshot> {
    snapshots.iter().find(|node| {
        node.name
            .as_deref()
            .map(|name| model_node_name_matches(name, expected_name))
            .unwrap_or(false)
    })
}

fn snapshot_mesh_bounds(
    snapshot: &ModelSceneNodeSnapshot,
    meshes: &Assets<Mesh>,
) -> Option<(Vec3, Vec3)> {
    let mesh_handle = snapshot.mesh.as_ref()?;
    let mesh = meshes.get(mesh_handle)?;
    mesh_local_bounds(mesh)
}

fn model_node_name_matches(actual: &str, expected: &str) -> bool {
    actual == expected || actual.starts_with(format!("{expected}.").as_str())
}

fn mesh_projected_extent(mesh: &Mesh, axis: Vec3) -> Option<f32> {
    let axis = axis.normalize_or_zero();
    if axis.length_squared() <= f32::EPSILON {
        return None;
    }

    let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?;
    let mut min_proj = f32::INFINITY;
    let mut max_proj = f32::NEG_INFINITY;

    match positions {
        VertexAttributeValues::Float32x3(values) => {
            for [x, y, z] in values {
                let proj = Vec3::new(*x, *y, *z).dot(axis);
                min_proj = min_proj.min(proj);
                max_proj = max_proj.max(proj);
            }
        }
        VertexAttributeValues::Float32x4(values) => {
            for [x, y, z, _w] in values {
                let proj = Vec3::new(*x, *y, *z).dot(axis);
                min_proj = min_proj.min(proj);
                max_proj = max_proj.max(proj);
            }
        }
        _ => return None,
    }

    if min_proj.is_finite() && max_proj.is_finite() {
        Some((max_proj - min_proj).abs())
    } else {
        None
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

pub(super) fn spin_wheel_pairs(
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
        let target_y = wheel.hardpoint_local.y - visual_spring_length;
        let spring_lerp = (WHEEL_VISUAL_SPRING_LERP_RATE * dt).clamp(0.0, 1.0);
        transform.translation.y = transform.translation.y.lerp(target_y, spring_lerp);

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
pub(super) fn sync_player_vehicle_visual_aim_and_model_wheels(
    targeting: Option<Res<TurretTargetingState>>,
    wheel_pair_query: Query<
        (&PlayerWheelPairVisual, &Transform, &GlobalTransform),
        (
            Without<PlayerTurretVisual>,
            Without<PlayerVehicleModelWheelNode>,
            Without<PlayerVehicleModelTurretNode>,
        ),
    >,
    mut placeholder_turret_query: Query<
        &mut Transform,
        (
            With<PlayerTurretVisual>,
            Without<PlayerWheelPairVisual>,
            Without<PlayerVehicleModelWheelNode>,
            Without<PlayerVehicleModelTurretNode>,
        ),
    >,
    mut model_wheel_query: Query<
        (&PlayerVehicleModelWheelNode, &ChildOf, &mut Transform),
        (
            Without<PlayerWheelPairVisual>,
            Without<PlayerTurretVisual>,
            Without<PlayerVehicleModelTurretNode>,
        ),
    >,
    mut model_turret_query: Query<
        (&PlayerVehicleModelTurretNode, &mut Transform),
        (
            Without<PlayerWheelPairVisual>,
            Without<PlayerVehicleModelWheelNode>,
            Without<PlayerTurretVisual>,
        ),
    >,
    global_transform_query: Query<&GlobalTransform>,
) {
    let aim_direction_local = targeting
        .as_ref()
        .map(|state| state.aim_direction_local.normalize_or_zero())
        .filter(|direction| direction.length_squared() > f32::EPSILON)
        .unwrap_or(Vec2::X);
    let aim_angle_rad = aim_direction_local.y.atan2(aim_direction_local.x);

    for mut transform in &mut placeholder_turret_query {
        transform.translation =
            PLAYER_TURRET_OFFSET_LOCAL + (Vec3::Y * PLAYER_VISUAL_RIDE_HEIGHT_OFFSET_M);
        transform.rotation = Quat::from_rotation_z(aim_angle_rad);
    }

    let mut front_spin_angle_rad = 0.0;
    let mut rear_spin_angle_rad = 0.0;
    let mut front_wheel_pivot_world = None;
    let mut rear_wheel_pivot_world = None;
    for (wheel, transform, global_transform) in &wheel_pair_query {
        let spin_angle = transform.rotation.to_euler(EulerRot::XYZ).2;
        match wheel.axle {
            WheelAxle::Front => {
                front_spin_angle_rad = spin_angle;
                front_wheel_pivot_world = Some(global_transform.translation());
            }
            WheelAxle::Rear => {
                rear_spin_angle_rad = spin_angle;
                rear_wheel_pivot_world = Some(global_transform.translation());
            }
        }
    }

    for (wheel_node, child_of, mut transform) in &mut model_wheel_query {
        let (spin_angle_rad, desired_wheel_pivot_world) = match wheel_node.axle {
            WheelAxle::Front => (front_spin_angle_rad, front_wheel_pivot_world),
            WheelAxle::Rear => (rear_spin_angle_rad, rear_wheel_pivot_world),
        };
        let effective_scale = wheel_node.base_scale * wheel_node.visual_scale_multiplier;
        let spin_delta = Quat::from_axis_angle(wheel_node.spin_axis_local, spin_angle_rad);
        let rotation = wheel_node.base_rotation * spin_delta;
        transform.rotation = rotation;
        transform.scale = effective_scale;
        if let Some(mut desired_pivot_world) = desired_wheel_pivot_world {
            desired_pivot_world.z += PLAYER_MODEL_WHEEL_FOREGROUND_Z_BIAS_M;
            if let Ok(parent_global) = global_transform_query.get(child_of.0) {
                let parent_to_world = parent_global.affine();
                let pivot_in_parent = parent_to_world
                    .inverse()
                    .transform_point3a(desired_pivot_world.into());
                let rotated_pivot_local = rotation * (effective_scale * wheel_node.pivot_local);
                transform.translation = Vec3::from(pivot_in_parent) - rotated_pivot_local;
                continue;
            }
        }

        let (fallback_translation, _) = rotate_local_transform_around_pivot(
            wheel_node.base_translation,
            wheel_node.base_rotation,
            wheel_node.base_scale,
            effective_scale,
            wheel_node.pivot_local,
            spin_delta,
        );
        transform.translation = fallback_translation;
    }

    for (turret_node, mut transform) in &mut model_turret_query {
        let aim_delta = Quat::from_axis_angle(turret_node.aim_axis_local, aim_angle_rad);
        let (translation, rotation) = rotate_local_transform_around_pivot(
            turret_node.base_translation,
            turret_node.base_rotation,
            turret_node.base_scale,
            turret_node.base_scale,
            turret_node.pivot_local,
            aim_delta,
        );
        transform.translation = translation;
        transform.rotation = rotation;
        transform.scale = turret_node.base_scale;
    }
}

#[allow(clippy::type_complexity)]
fn wheel_estimated_radius_from_bounds(min: Vec3, max: Vec3) -> f32 {
    let extents = (max - min).abs();
    let (diameter_a, diameter_b) = if extents.x <= extents.y && extents.x <= extents.z {
        (extents.y, extents.z)
    } else if extents.y <= extents.x && extents.y <= extents.z {
        (extents.x, extents.z)
    } else {
        (extents.x, extents.y)
    };
    ((diameter_a + diameter_b) * 0.25).max(0.001)
}

fn rotate_local_transform_around_pivot(
    base_translation: Vec3,
    base_rotation: Quat,
    base_scale: Vec3,
    target_scale: Vec3,
    pivot_local: Vec3,
    delta_local_rotation: Quat,
) -> (Vec3, Quat) {
    let rotation = base_rotation * delta_local_rotation;
    let base_pivot_world = base_rotation * (base_scale * pivot_local);
    let rotated_pivot_world = rotation * (target_scale * pivot_local);
    let translation = base_translation + (base_pivot_world - rotated_pivot_world);
    (translation, rotation)
}

fn apply_uniform_vertex_color(mesh: &mut Mesh, color: [f32; 4]) {
    let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
        return;
    };
    let vertex_count = match positions {
        VertexAttributeValues::Float32x3(values) => values.len(),
        VertexAttributeValues::Float32x4(values) => values.len(),
        _ => return,
    };
    if vertex_count == 0 {
        return;
    }
    let colors = vec![color; vertex_count];
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
}
