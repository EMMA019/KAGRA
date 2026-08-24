// GPU helper utilities
use nalgebra;

/// Directional shadow map resolution
pub(super) const SHADOW_MAP_SIZE: u32 = 2048;

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// オフスクリーン描画の MSAA サンプル数。swapchain は resolve 後の 1x を受け取る。
pub const MSAA_COUNT: u32 = 4;

pub fn msaa_state() -> wgpu::MultisampleState {
    wgpu::MultisampleState {
        count: MSAA_COUNT,
        mask: !0,
        alpha_to_coverage_enabled: false,
    }
}

pub(super) fn make_depth_texture(device: &wgpu::Device, w: u32, h: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: MSAA_COUNT,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

pub(super) fn make_shadow_map(
    device: &wgpu::Device,
) -> (wgpu::Texture, wgpu::TextureView, [wgpu::TextureView; 2]) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Shadow Map"),
        size: wgpu::Extent3d {
            width: SHADOW_MAP_SIZE,
            height: SHADOW_MAP_SIZE,
            depth_or_array_layers: 2,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let sample = tex.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Shadow Array"),
        format: Some(DEPTH_FORMAT),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        aspect: wgpu::TextureAspect::DepthOnly,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(2),
    });
    let layer0 = tex.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Shadow Layer 0"),
        format: Some(DEPTH_FORMAT),
        dimension: Some(wgpu::TextureViewDimension::D2),
        aspect: wgpu::TextureAspect::DepthOnly,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(1),
    });
    let layer1 = tex.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Shadow Layer 1"),
        format: Some(DEPTH_FORMAT),
        dimension: Some(wgpu::TextureViewDimension::D2),
        aspect: wgpu::TextureAspect::DepthOnly,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 1,
        array_layer_count: Some(1),
    });
    (tex, sample, [layer0, layer1])
}

/// light_dir = direction toward the light. Builds ortho light view-proj around target.
pub(super) fn build_light_view_proj(light_dir: [f32; 4], target: [f32; 3]) -> [f32; 16] {
    build_light_view_proj_fit(light_dir, target, 6.0, 8.0, 20.0)
}

/// 影錐を VRM / ワールド AABB に合わせる。`half` はライト空間の半辺。
pub(super) fn build_light_view_proj_fit(
    light_dir: [f32; 4],
    target: [f32; 3],
    half: f32,
    light_dist: f32,
    far: f32,
) -> [f32; 16] {
    use nalgebra::{Matrix4, Point3, Vector3};
    let dir = Vector3::new(light_dir[0], light_dir[1], light_dir[2]);
    let dir = if dir.norm_squared() < 1e-12 {
        Vector3::new(0.0, 1.0, 0.0)
    } else {
        dir.normalize()
    };
    let target = Point3::new(target[0], target[1], target[2]);
    let light_pos = target + dir * light_dist.max(1.0);
    let up = if dir.y.abs() > 0.99 {
        Vector3::new(0.0, 0.0, 1.0)
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    };
    let view = Matrix4::look_at_rh(&light_pos, &target, &up);
    let half = half.max(0.5);
    let proj = Matrix4::new_orthographic(-half, half, -half, half, 0.1, far.max(half + 2.0));
    let wgpu_correction = Matrix4::new(
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 0.5, 0.5,
        0.0, 0.0, 0.0, 1.0,
    );
    let vp = wgpu_correction * proj * view;
    let mut out = [0f32; 16];
    out.copy_from_slice(vp.as_slice());
    out
}

/// `ShadowU.params`: x = cascade count (1 when the spot owns the map),
/// y = 1 if the 2048 map is a spot perspective (shadow the local light, not the sun).
pub(super) fn shadow_u_params(spot_owns_map: bool, cascades: u32) -> [f32; 4] {
    if spot_owns_map {
        [1.0, 1.0, 0.0, 0.0]
    } else {
        [cascades.max(1) as f32, 0.0, 0.0, 0.0]
    }
}

/// スポット用の透視シャドウ。``angle`` は外角（ラジアン）。
pub(super) fn build_spot_view_proj(pos: [f32; 3], dir: [f32; 3], angle: f32, radius: f32) -> [f32; 16] {
    use nalgebra::{Matrix4, Point3, Vector3};
    let origin = Point3::new(pos[0], pos[1], pos[2]);
    let dir = Vector3::new(dir[0], dir[1], dir[2]);
    let dir = if dir.norm_squared() < 1e-12 {
        Vector3::new(0.0, -1.0, 0.0)
    } else {
        dir.normalize()
    };
    let target = origin + dir * 1.5;
    let up = if dir.y.abs() > 0.99 {
        Vector3::new(0.0, 0.0, 1.0)
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    };
    let view = Matrix4::look_at_rh(&origin, &target, &up);
    let fov = (angle * 2.0).clamp(0.08, 2.6);
    let far = radius.max(3.0);
    let proj = Matrix4::new_perspective(1.0, fov, 0.12, far);
    let wgpu_correction = Matrix4::new(
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 0.5, 0.5,
        0.0, 0.0, 0.0, 1.0,
    );
    let vp = wgpu_correction * proj * view;
    let mut out = [0f32; 16];
    out.copy_from_slice(vp.as_slice());
    out
}

pub(super) fn make_msaa_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("MSAA Color Target"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: MSAA_COUNT,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

pub(super) fn make_texture_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Texture BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Mesh3D color pass: diffuse + sampler + tangent-space normal (MToon-style).
pub(super) fn make_mesh3d_tex_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Mesh3D Tex BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
        ],
    })
}

pub(super) fn make_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    })
}

pub(super) fn make_frame_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Frame Color Target"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

pub(super) fn make_default_env_cube(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    upload_env_cube(device, queue, &[[40u8, 42, 48, 255]; 6])
}

pub(super) fn upload_env_cube(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    faces_1x1: &[[u8; 4]; 6],
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Env Cube"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 6,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (i, px) in faces_1x1.iter().enumerate() {
        let mut row = [0u8; 256];
        row[0..4].copy_from_slice(px);
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: i as u32 },
                aspect: wgpu::TextureAspect::All,
            },
            &row,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
    }
    let view = tex.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Env Cube View"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Env Cube Sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    (tex, view, sampler)
}

pub(super) fn upload_env_cube_faces(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    face_size: u32,
    rgba: &[[u8; 4]],
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    upload_env_cube_mips(device, queue, face_size, rgba, 1)
}

pub(super) fn upload_env_cube_mips(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    face_size: u32,
    rgba: &[[u8; 4]],
    mips: u32,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    let chain = if mips > 1 {
        crate::hdri::cube_mip_chain(rgba, face_size, mips)
    } else {
        vec![(face_size, rgba.to_vec())]
    };
    let mip_count = chain.len().max(1) as u32;
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Env Cube"),
        size: wgpu::Extent3d {
            width: face_size,
            height: face_size,
            depth_or_array_layers: 6,
        },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (mip, (size, pixels)) in chain.iter().enumerate() {
        let size = *size;
        let raw_stride = (size * 4) as usize;
        let padded_stride = ((raw_stride + 255) / 256) * 256;
        for face in 0..6 {
            let start = face * size as usize * size as usize;
            let mut bytes = vec![0u8; padded_stride * size as usize];
            for y in 0..size as usize {
                for x in 0..size as usize {
                    let px = pixels[start + y * size as usize + x];
                    let o = y * padded_stride + x * 4;
                    bytes[o..o + 4].copy_from_slice(&px);
                }
            }
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &tex,
                    mip_level: mip as u32,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: face as u32 },
                    aspect: wgpu::TextureAspect::All,
                },
                &bytes,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_stride as u32),
                    rows_per_image: Some(size),
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
    let view = tex.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Env Cube View"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Env Cube Sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    (tex, view, sampler)
}

pub(super) fn normalize_light_dir(x: f32, y: f32, z: f32) -> [f32; 4] {
    let len = (x * x + y * y + z * z).sqrt();
    if len < 1e-8 {
        return [0.0, 1.0, 0.0, 0.0];
    }
    [x / len, y / len, z / len, 0.0]
}

pub(super) fn default_toon_params() -> [f32; 4] {
    // threshold, softness, shade, lit — softness=1 で連続照明
    [0.5, 1.0, 0.55, 1.0]
}

#[cfg(test)]
mod light_dir_tests {
    use super::{default_toon_params, normalize_light_dir};

    #[test]
    fn normalizes_and_rejects_zero() {
        let d = normalize_light_dir(0.0, 2.0, 0.0);
        assert!((d[1] - 1.0).abs() < 1e-6);
        let z = normalize_light_dir(0.0, 0.0, 0.0);
        assert!((z[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn default_toon_is_continuous() {
        let t = default_toon_params();
        assert!(t[1] >= 0.999);
        assert!((t[2] - 0.55).abs() < 1e-6);
        assert!((t[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn shadow_fit_default_matches_legacy() {
        let a = super::build_light_view_proj([0.0, 1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let b = super::build_light_view_proj_fit([0.0, 1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 6.0, 8.0, 20.0);
        for i in 0..16 {
            assert!((a[i] - b[i]).abs() < 1e-5);
        }
    }

    #[test]
    fn spot_view_proj_is_finite() {
        let vp = super::build_spot_view_proj([0.0, 3.0, 0.0], [0.0, -1.0, 0.0], 0.85, 14.0);
        assert!(vp.iter().all(|v| v.is_finite()));
        assert!(vp.iter().any(|v| v.abs() > 1e-5));
    }

    #[test]
    fn shadow_u_params_marks_spot_owned_map() {
        let spot = super::shadow_u_params(true, 2);
        assert!((spot[0] - 1.0).abs() < 1e-6);
        assert!(spot[1] > 0.5);
        let sun = super::shadow_u_params(false, 2);
        assert!((sun[0] - 2.0).abs() < 1e-6);
        assert!(sun[1] < 0.5);
    }

    fn mul_vp(m: &[f32; 16], p: [f32; 3]) -> [f32; 4] {
        let (x, y, z) = (p[0], p[1], p[2]);
        [
            m[0] * x + m[4] * y + m[8] * z + m[12],
            m[1] * x + m[5] * y + m[9] * z + m[13],
            m[2] * x + m[6] * y + m[10] * z + m[14],
            m[3] * x + m[7] * y + m[11] * z + m[15],
        ]
    }

    #[test]
    fn spot_maps_floor_under_lamp_into_ndc() {
        let vp = super::build_spot_view_proj([0.0, 2.8, 0.0], [0.0, -1.0, 0.0], 0.85, 10.0);
        let clip = mul_vp(&vp, [0.0, 0.0, 0.0]);
        assert!(clip[3].abs() > 1e-4, "w={}", clip[3]);
        let ndc = [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]];
        assert!(ndc[0] > -0.15 && ndc[0] < 0.15, "x={}", ndc[0]);
        assert!(ndc[1] > -0.15 && ndc[1] < 0.15, "y={}", ndc[1]);
        assert!(ndc[2] > 0.0 && ndc[2] < 1.0, "z={}", ndc[2]);
        let side = mul_vp(&vp, [1.2, 0.0, 0.0]);
        let sx = side[0] / side[3];
        let sy = side[1] / side[3];
        assert!(sx.abs() < 1.0 && sy.abs() < 1.0, "side ndc {} {}", sx, sy);
    }

    fn ndc(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
        let c = mul_vp(m, p);
        [c[0] / c[3], c[1] / c[3], c[2] / c[3]]
    }

    #[test]
    fn spot_maps_indoor_golden_side_lamp_into_ndc() {
        // tests/render_golden_scene.py IndoorSpot
        let vp = super::build_spot_view_proj(
            [-2.4, 2.7, 0.15],
            [0.82, -0.55, 0.0],
            0.72,
            12.0,
        );
        let box_c = ndc(&vp, [0.0, 0.95, 0.0]);
        assert!(box_c[0].abs() < 0.85 && box_c[1].abs() < 0.85, "box xy {:?}", box_c);
        assert!(box_c[2] > 0.0 && box_c[2] < 1.0, "box z={}", box_c[2]);
        let floor = ndc(&vp, [1.4, 0.0, 0.0]);
        assert!(floor[0].abs() < 0.95 && floor[1].abs() < 0.95, "floor xy {:?}", floor);
        assert!(floor[2] > 0.0 && floor[2] < 1.0, "floor z={}", floor[2]);
    }
}