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

pub(super) fn make_shadow_map(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Shadow Map"),
        size: wgpu::Extent3d {
            width: SHADOW_MAP_SIZE,
            height: SHADOW_MAP_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
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
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Env Cube"),
        size: wgpu::Extent3d {
            width: face_size,
            height: face_size,
            depth_or_array_layers: 6,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let raw_stride = (face_size * 4) as usize;
    let padded_stride = ((raw_stride + 255) / 256) * 256;
    for face in 0..6 {
        let start = face * face_size as usize * face_size as usize;
        let mut bytes = vec![0u8; padded_stride * face_size as usize];
        for y in 0..face_size as usize {
            for x in 0..face_size as usize {
                let px = rgba[start + y * face_size as usize + x];
                let o = y * padded_stride + x * 4;
                bytes[o..o + 4].copy_from_slice(&px);
            }
        }
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: face as u32 },
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_stride as u32),
                rows_per_image: Some(face_size),
            },
            wgpu::Extent3d {
                width: face_size,
                height: face_size,
                depth_or_array_layers: 1,
            },
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
}