use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) struct GroundProfilePoint {
    pub(super) x: f32,
    pub(super) top: Vec2,
    pub(super) bottom: Vec2,
    pub(super) u: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GroundColliderSegmentSample {
    pub(super) x0: f32,
    pub(super) x1: f32,
    pub(super) top0: Vec2,
    pub(super) top1: Vec2,
}

#[derive(Debug, Clone)]
pub(super) struct GroundProfileSamples {
    pub(super) points: Vec<GroundProfilePoint>,
    pub(super) segments: Vec<GroundColliderSegmentSample>,
}

pub(super) fn build_ground_profile_samples(config: &GameConfig) -> GroundProfileSamples {
    let segment_count = (GROUND_WIDTH / GROUND_SPLINE_SEGMENT_WIDTH_M).ceil() as usize + 2;
    let start_x = -WORLD_HALF_WIDTH - GROUND_SPLINE_SEGMENT_WIDTH_M;
    let node_count = segment_count + 1;

    let mut top_points = Vec::with_capacity(node_count);
    for index in 0..node_count {
        let x = start_x + (index as f32 * GROUND_SPLINE_SEGMENT_WIDTH_M);
        top_points.push(Vec2::new(x, terrain_height_at_x(config, x)));
    }

    let mut points = Vec::with_capacity(node_count);
    let strip_width = GROUND_SPLINE_THICKNESS_M.max(0.001);
    let mut u_along = 0.0_f32;
    for index in 0..node_count {
        if index > 0 {
            u_along += (top_points[index] - top_points[index - 1]).length() / strip_width;
        }
        let tangent = if index == 0 {
            top_points[1] - top_points[0]
        } else if index + 1 == node_count {
            top_points[node_count - 1] - top_points[node_count - 2]
        } else {
            top_points[index + 1] - top_points[index - 1]
        };
        let normal = Vec2::new(-tangent.y, tangent.x).normalize_or_zero();
        let safe_normal = if normal.length_squared() <= f32::EPSILON {
            Vec2::Y
        } else {
            normal
        };
        let top = top_points[index];
        let bottom = top - (safe_normal * GROUND_SPLINE_THICKNESS_M);

        points.push(GroundProfilePoint {
            x: top.x,
            top,
            bottom,
            u: u_along,
        });
    }

    let mut segments = Vec::with_capacity(segment_count);
    for pair in points.windows(2) {
        let left = pair[0];
        let right = pair[1];
        segments.push(GroundColliderSegmentSample {
            x0: left.x,
            x1: right.x,
            top0: left.top,
            top1: right.top,
        });
    }

    GroundProfileSamples { points, segments }
}

pub(super) fn build_ground_strip_mesh(profile: &GroundProfileSamples) -> Mesh {
    let node_count = profile.points.len();
    let mut positions = Vec::with_capacity(node_count * 2);
    let mut normals = Vec::with_capacity(node_count * 2);
    let mut uvs = Vec::with_capacity(node_count * 2);
    let mut indices = Vec::with_capacity((node_count.saturating_sub(1)) * 6);

    for point in &profile.points {
        positions.push([point.top.x, point.top.y, GROUND_SPLINE_Z]);
        positions.push([point.bottom.x, point.bottom.y, GROUND_SPLINE_Z]);
        normals.push([0.0, 0.0, 1.0]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([point.u, 0.0]);
        uvs.push([point.u, 1.0]);
    }

    for index in 0..node_count.saturating_sub(1) {
        let base = (index * 2) as u32;
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

pub(super) fn build_ground_curtain_mesh(profile: &GroundProfileSamples) -> Mesh {
    let node_count = profile.points.len();
    let mut positions = Vec::with_capacity(node_count * 2);
    let mut normals = Vec::with_capacity(node_count * 2);
    let mut uvs = Vec::with_capacity(node_count * 2);
    let mut indices = Vec::with_capacity((node_count.saturating_sub(1)) * 6);

    for point in &profile.points {
        let curtain_top = point.bottom;
        let curtain_bottom = Vec2::new(point.x, GROUND_CURTAIN_BOTTOM_Y_M);
        positions.push([curtain_top.x, curtain_top.y, GROUND_CURTAIN_Z]);
        positions.push([curtain_bottom.x, curtain_bottom.y, GROUND_CURTAIN_Z]);
        normals.push([0.0, 0.0, 1.0]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push(curtain_world_uv(curtain_top));
        uvs.push(curtain_world_uv(curtain_bottom));
    }

    for index in 0..node_count.saturating_sub(1) {
        let base = (index * 2) as u32;
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn curtain_world_uv(world: Vec2) -> [f32; 2] {
    [
        world.x * GROUND_CURTAIN_UV_SCALE,
        world.y * GROUND_CURTAIN_UV_SCALE,
    ]
}

pub(super) fn resolve_ground_texture_path(primary: &str, fallback: &str) -> String {
    let primary_path = Path::new("assets").join(primary);
    if primary_path.exists() {
        return primary.to_string();
    }

    let fallback_path = Path::new("assets").join(fallback);
    if fallback_path.exists() {
        return fallback.to_string();
    }

    primary.to_string()
}

pub(super) fn load_repeating_texture(asset_server: &AssetServer, path: &str) -> Handle<Image> {
    asset_server.load_with_settings(path.to_string(), |settings: &mut ImageLoaderSettings| {
        settings.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            address_mode_w: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        });
    })
}

pub(super) fn terrain_height_at_x(config: &GameConfig, x: f32) -> f32 {
    config.terrain_height_at_x(x)
}
