// src/renderer.rs
// wgpu 0.19 - 動的バッファ自動拡張版 + エラーハンドリング強化 (KaguraResult)
// 構造体名: RendererV2
// 修正: Surface を内部で所有し、render() は引数なし

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::color::Color;
use crate::instance_renderer::InstanceRenderer;
use crate::text::TextRenderer;
use crate::error::{KaguraError, KaguraResult};
use nalgebra;

/// Skinning uniform: 256 matrices * 16 floats + screen size (2) + pad
const SKIN_UNIFORM_FLOATS: usize = 256 * 16 + 4;
/// morph BindGroup キャッシュの上限（超過分は古いものから破棄）
const MORPH_BG_CACHE_MAX: usize = 128;
/// Mesh3D 再利用バッファの初期容量
const MESH3D_VB_INITIAL: u64 = 256 * 1024;
const MESH3D_IB_INITIAL: u64 = 64 * 1024;
/// Directional shadow map resolution


mod types;
mod shaders;
mod gpu_helpers;

pub use types::{
    DrawCommand, RectCommand, SpriteCommand, TextCommand,
    SkinnedMeshCommand, MeshCommand, PolygonCommand, Mesh3DCommand,
    SkinnedVertex,
};
#[allow(unused_imports)] // 外部公開用の再エクスポート
pub use gpu_helpers::{DEPTH_FORMAT, MSAA_COUNT, msaa_state};

use types::*;
use shaders::*;
use gpu_helpers::*;

struct GpuTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

pub struct RendererV2 {
    pub clear_color: Color,
    draw_queue: Vec<DrawCommand>,

    rect_pipeline: wgpu::RenderPipeline,
    sprite_pipeline: wgpu::RenderPipeline,
    skinning_pipeline: wgpu::RenderPipeline,
    skinning_3d_pipeline: wgpu::RenderPipeline,
    skinning_3d_outline_pipeline: wgpu::RenderPipeline,
    skinning_3d_shadow_pipeline: wgpu::RenderPipeline,
    default_mtoon_buf: wgpu::Buffer,
    mesh_3d_skinned_queue: Vec<SkinnedMeshCommand>,
    pub instance_renderer: Option<InstanceRenderer>,
    pub pending_computes: Vec<Box<dyn FnOnce(&wgpu::Device, &wgpu::Queue, &mut wgpu::CommandEncoder)>>,
    instance_draw_queue: Vec<u32>,

    texture_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: std::collections::HashMap<u32, GpuTexture>,
    next_texture_id: u32,
    text_renderer: TextRenderer,
    fallback_texture_bind_group: wgpu::BindGroup,
    fallback_tex_view: wgpu::TextureView,

    shader_params_bgl: wgpu::BindGroupLayout,
    shader_params_buffer: wgpu::Buffer,
    shader_params_bg: wgpu::BindGroup,
    custom_pipelines: std::collections::HashMap<u32, wgpu::RenderPipeline>,
    next_shader_id: u32,
    pub surface_format: wgpu::TextureFormat,

    skinning_uniform_bgl: wgpu::BindGroupLayout,
    skinning_uniform_buffer: wgpu::Buffer,
    skinning_bind_group: wgpu::BindGroup,
    /// ドロー個別スキンパレット (buffer, bind_group) のプール
    skin_palette_pool: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    /// このフレームで使用済みのパレット数
    skin_palette_used: usize,

    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    pipeline_3d: wgpu::RenderPipeline,
    /// view(64) + proj(64) + light_dir(16) + toon(16) + eye(16) + fog_params(16) + fog_color(16) = 208 bytes
    camera_3d_buf: wgpu::Buffer,
    camera_3d_bg: wgpu::BindGroup,
    camera_3d_bgl: wgpu::BindGroupLayout,
    /// 平行光の方向（光源へ向かう単位ベクトル）。xyz 使用、w は未使用。
    light_dir: [f32; 4],
    /// VRM トゥーン: [threshold, softness, shade, lit]
    toon_params: [f32; 4],
    /// フォグ: [start, end, enabled, 0]
    fog_params: [f32; 4],
    /// フォグ色 RGB 0..1 + pad
    fog_color: [f32; 4],
    shader_clock: Instant,
    mesh_3d_queue: Vec<Mesh3DCommand>,
    skinning_3d_morph_bgl: wgpu::BindGroupLayout,
    /// (base, shade, matcap, normal, uvmask, morph_ptr, blend_ptr)
    morph_bg_cache: std::collections::HashMap<(u32, u32, u32, u32, u32, usize, usize), Arc<wgpu::BindGroup>>,
    morph_bg_order: VecDeque<(u32, u32, u32, u32, u32, usize, usize)>,

    /// Directional shadow map (1x depth, not MSAA)
    shadow_tex: wgpu::Texture,
    shadow_view: wgpu::TextureView,
    shadow_sampler: wgpu::Sampler,
    shadow_vp_buf: wgpu::Buffer,
    shadow_vp_bgl: wgpu::BindGroupLayout,
    shadow_bgl: wgpu::BindGroupLayout,
    shadow_write_bg: wgpu::BindGroup,
    shadow_bg: wgpu::BindGroup,
    shadows_enabled: bool,

    /// 毎フレーム再利用するスキニング用スクラッチ（heap）
    skin_uniform_scratch: Vec<f32>,
    /// 動的 Mesh3D 用の再利用 GPU バッファ
    mesh_3d_vb: wgpu::Buffer,
    mesh_3d_ib: wgpu::Buffer,
    mesh_3d_vb_cap: u64,
    mesh_3d_ib_cap: u64,

    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub screen_width: u32,
    pub screen_height: u32,

    dynamic_vertex_buffer: wgpu::Buffer,
    dynamic_buffer_size: u64,
    staging_belt: wgpu::util::StagingBelt,
    dynamic_offset: u64,

    // ★ Surface とその設定を保持
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

    /// MSAA 描画先。resolve して frame_texture に落とす。
    msaa_texture: wgpu::Texture,
    msaa_view: wgpu::TextureView,
    /// resolve 後の 1x カラー。swapchain コピー / スクリーンショット用。
    frame_texture: wgpu::Texture,
    frame_view: wgpu::TextureView,

    _window_arc: Arc<Window>,
}

impl RendererV2 {
    pub async fn new(window: Arc<Window>, width: u32, height: u32, transparent: bool) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
            flags: wgpu::InstanceFlags::default(),
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
        });

        // ★ 同じインスタンスで Surface を作成
        let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or("GPUアダプターが見つかりません")?;

        log::info!("GPU: {}", adapter.get_info().name);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("KAGRA Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| e.to_string())?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().find(|f| f.is_srgb()).copied().unwrap_or(caps.formats[0]);
        let alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PostMultiplied) {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else {
            caps.alpha_modes[0]
        };
        let surface_config = wgpu::SurfaceConfiguration {
            // オフスクリーン frame_texture からコピーするため COPY_DST が必要
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let surface_format = format;
        let (frame_texture, frame_view) = make_frame_texture(&device, width, height, surface_format);
        let (msaa_texture, msaa_view) = make_msaa_texture(&device, width, height, surface_format);

        // ---------- シェーダとパイプラインの作成（以降は元のコードと同じ）----------
        let rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Rect Shader"),
            source: wgpu::ShaderSource::Wgsl(RECT_SHADER.into()),
        });
        let rect_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Rect Pipeline"),
            layout: Some(&rect_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &rect_shader,
                entry_point: "vs_main",
                buffers: &[ColorVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &rect_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: msaa_state(),
            multiview: None,
        });

        let texture_bgl = make_texture_bgl(&device);
        let texture_bgl_text = make_texture_bgl(&device);
        let sampler = make_sampler(&device);
        let sampler_text = make_sampler(&device);

        let fallback_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Fallback White"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture { texture: &fallback_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &[255u8, 255u8, 255u8, 255u8],
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        let fallback_view = fallback_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let fallback_texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Fallback Tex BG"),
            layout: &texture_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&fallback_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });

        let sprite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sprite Shader"),
            source: wgpu::ShaderSource::Wgsl(SPRITE_SHADER.into()),
        });
        let sprite_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&texture_bgl],
            push_constant_ranges: &[],
        });
        let sprite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sprite Pipeline"),
            layout: Some(&sprite_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sprite_shader,
                entry_point: "vs_main",
                buffers: &[SpriteVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &sprite_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: msaa_state(),
            multiview: None,
        });

        let skinning_uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Skinning Uniform BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let skinning_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Skinning Uniform Buffer"),
            size: 256 * 64 + 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let skinning_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skinning BG"),
            layout: &skinning_uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: skinning_uniform_buffer.as_entire_binding(),
            }],
        });

        let skinning_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skinning Shader"),
            source: wgpu::ShaderSource::Wgsl(SKINNING_SHADER.into()),
        });
        let skin_tex_bgl = make_texture_bgl(&device);
        let skinning_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&skinning_uniform_bgl, &skin_tex_bgl],
            push_constant_ranges: &[],
        });
        let skinning_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skinning Pipeline"),
            layout: Some(&skinning_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &skinning_shader,
                entry_point: "vs_main",
                buffers: &[SkinnedVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &skinning_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: msaa_state(),
            multiview: None,
        });

        let text_renderer = TextRenderer::new(texture_bgl_text, sampler_text);

        let shader_params_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shader Params BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let shader_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shader Params Buffer"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader_params_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shader Params BG"),
            layout: &shader_params_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: shader_params_buffer.as_entire_binding(),
            }],
        });

        let tmp_bgl = make_texture_bgl(&device);
        let mut custom_pipelines = std::collections::HashMap::new();
        custom_pipelines.insert(1u32, Self::make_custom_pipeline(&device, SHADER_GRAYSCALE, &tmp_bgl, &shader_params_bgl, surface_format));
        custom_pipelines.insert(2u32, Self::make_custom_pipeline(&device, SHADER_FLASH, &tmp_bgl, &shader_params_bgl, surface_format));
        custom_pipelines.insert(3u32, Self::make_custom_pipeline(&device, SHADER_SPOTLIGHT, &tmp_bgl, &shader_params_bgl, surface_format));
        custom_pipelines.insert(4u32, Self::make_custom_pipeline(&device, SHADER_GLOW, &tmp_bgl, &shader_params_bgl, surface_format));
        custom_pipelines.insert(5u32, Self::make_custom_pipeline(&device, SHADER_TINT, &tmp_bgl, &shader_params_bgl, surface_format));

        let (depth_texture, depth_view) = make_depth_texture(&device, width, height);

        let camera_3d_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera3D BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        // view+proj+light_dir+toon+eye+fog_params+fog_color = 208
        let camera_3d_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera3D Buf"),
            size: 208,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_3d_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera3D BG"),
            layout: &camera_3d_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_3d_buf.as_entire_binding(),
            }],
        });
        // 従来ハードコード値と同じデフォルト（見た目回帰を避ける）
        let light_dir = normalize_light_dir(0.3, 1.0, 0.5);
        // softness=1 → 連続 half-Lambert（従来式）。threshold は未使用。
        let toon_params = default_toon_params();
        let cam_eye = [0.0f32, 1.5, 3.0, 0.0];
        // デフォルト無効（ゴールデン互換）
        let fog_params = [5.0f32, 20.0, 0.0, 0.0];
        let fog_color = [110.0 / 255.0, 180.0 / 255.0, 230.0 / 255.0, 1.0];
        queue.write_buffer(&camera_3d_buf, 128, bytemuck::cast_slice(&light_dir));
        queue.write_buffer(&camera_3d_buf, 144, bytemuck::cast_slice(&toon_params));
        queue.write_buffer(&camera_3d_buf, 160, bytemuck::cast_slice(&cam_eye));
        queue.write_buffer(&camera_3d_buf, 176, bytemuck::cast_slice(&fog_params));
        queue.write_buffer(&camera_3d_buf, 192, bytemuck::cast_slice(&fog_color));

        let skinning_3d_morph_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Skinning3D Morph BGL"),
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
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
            ],
        });

        let default_mtoon = crate::mtoon::MtoonGpu::default();
        let default_mtoon_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Default MToon UBO"),
            contents: bytemuck::bytes_of(&default_mtoon),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        // --- Directional shadow map ---
        let (shadow_tex, shadow_view) = make_shadow_map(&device);
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Comparison Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let shadow_vp_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow VP BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Sample BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Depth,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let shadow_vp_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow VP Buf"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let initial_shadow_vp = build_light_view_proj(light_dir, [0.0, 1.0, 0.0]);
        queue.write_buffer(&shadow_vp_buf, 0, bytemuck::cast_slice(&initial_shadow_vp));
        let shadow_write_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Write BG"),
            layout: &shadow_vp_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: shadow_vp_buf.as_entire_binding(),
            }],
        });
        let shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Sample BG"),
            layout: &shadow_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shadow_vp_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
            ],
        });

        let skinning_3d_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skinning3D Shader (Blend)"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SKINNING_3D_BLEND.into()),
        });
        let skinning_3d_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skinning3D Layout"),
            bind_group_layouts: &[
                &camera_3d_bgl,
                &skinning_uniform_bgl,
                &skinning_3d_morph_bgl,
                &shadow_bgl,
            ],
            push_constant_ranges: &[],
        });
        let skinning_3d_shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skinning3D Shadow Layout"),
            bind_group_layouts: &[
                &camera_3d_bgl,
                &skinning_uniform_bgl,
                &skinning_3d_morph_bgl,
                &shadow_vp_bgl,
            ],
            push_constant_ranges: &[],
        });
        let skinning_3d_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skinning3D Pipeline"),
            layout: Some(&skinning_3d_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &skinning_3d_shader,
                entry_point: "vs_main",
                buffers: &[SkinnedVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &skinning_3d_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: msaa_state(),
            multiview: None,
        });
        let skinning_3d_outline_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skinning3D Outline Pipeline"),
            layout: Some(&skinning_3d_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &skinning_3d_shader,
                entry_point: "vs_outline",
                buffers: &[SkinnedVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &skinning_3d_shader,
                entry_point: "fs_outline",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 1,
                    slope_scale: 1.0,
                    clamp: 0.0,
                },
            }),
            multisample: msaa_state(),
            multiview: None,
        });
        let skinning_3d_shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skinning3D Shadow Pipeline"),
            layout: Some(&skinning_3d_shadow_layout),
            vertex: wgpu::VertexState {
                module: &skinning_3d_shader,
                entry_point: "vs_shadow",
                buffers: &[SkinnedVertex::desc()],
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 1.5,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let shader_3d = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader3D"),
            source: wgpu::ShaderSource::Wgsl(SHADER_3D.into()),
        });
        let tex_bgl_3d = make_texture_bgl(&device);
        let layout_3d = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&camera_3d_bgl, &tex_bgl_3d, &shadow_bgl],
            push_constant_ranges: &[],
        });
        let pipeline_3d = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Pipeline3D"),
            layout: Some(&layout_3d),
            vertex: wgpu::VertexState {
                module: &shader_3d,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 32,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_3d,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: msaa_state(),
            multiview: None,
        });

        let initial_buffer_size = 64 * 1024 * 1024;
        let dynamic_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dynamic Vertex Buffer"),
            size: initial_buffer_size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let staging_belt = wgpu::util::StagingBelt::new(16 * 1024 * 1024);
        let dynamic_offset = 0;

        let mesh_3d_vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mesh3D VB (pooled)"),
            size: MESH3D_VB_INITIAL,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mesh_3d_ib = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mesh3D IB (pooled)"),
            size: MESH3D_IB_INITIAL,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let initial_alpha = if transparent { 0 } else { 255 };
        Ok(RendererV2 {
            clear_color: Color { r: 0, g: 0, b: 0, a: initial_alpha },
            draw_queue: Vec::new(),
            rect_pipeline,
            sprite_pipeline,
            skinning_pipeline,
            skinning_3d_pipeline,
            skinning_3d_outline_pipeline,
            skinning_3d_shadow_pipeline,
            default_mtoon_buf,
            mesh_3d_skinned_queue: Vec::new(),
            instance_renderer: None,
            pending_computes: Vec::new(),
            instance_draw_queue: Vec::new(),
            texture_bgl,
            sampler,
            textures: std::collections::HashMap::new(),
            next_texture_id: 1,
            text_renderer,
            fallback_texture_bind_group,
            fallback_tex_view: fallback_view,
            shader_params_bgl,
            shader_params_buffer,
            shader_params_bg,
            custom_pipelines,
            next_shader_id: 10,
            surface_format,
            skinning_uniform_bgl,
            skinning_uniform_buffer,
            skinning_bind_group,
            skin_palette_pool: Vec::new(),
            skin_palette_used: 0,
            depth_texture,
            depth_view,
            pipeline_3d,
            camera_3d_buf,
            camera_3d_bg,
            camera_3d_bgl,
            light_dir,
            toon_params,
            fog_params,
            fog_color,
            shader_clock: Instant::now(),
            mesh_3d_queue: Vec::new(),
            skinning_3d_morph_bgl,
            morph_bg_cache: std::collections::HashMap::new(),
            morph_bg_order: VecDeque::new(),
            shadow_tex,
            shadow_view,
            shadow_sampler,
            shadow_vp_buf,
            shadow_vp_bgl,
            shadow_bgl,
            shadow_write_bg,
            shadow_bg,
            shadows_enabled: true,
            skin_uniform_scratch: vec![0f32; SKIN_UNIFORM_FLOATS],
            mesh_3d_vb,
            mesh_3d_ib,
            mesh_3d_vb_cap: MESH3D_VB_INITIAL,
            mesh_3d_ib_cap: MESH3D_IB_INITIAL,
            device,
            queue,
            screen_width: width,
            screen_height: height,
            dynamic_vertex_buffer,
            dynamic_buffer_size: initial_buffer_size,
            staging_belt,
            dynamic_offset,
            surface,
            surface_config,
            msaa_texture,
            msaa_view,
            frame_texture,
            frame_view,
            _window_arc: window,
        })
    }

    fn make_custom_pipeline(
        device: &wgpu::Device,
        wgsl: &str,
        tex_bgl: &wgpu::BindGroupLayout,
        params_bgl: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Custom"),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[tex_bgl, params_bgl],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Custom Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "vs_main",
                buffers: &[SpriteVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: msaa_state(),
            multiview: None,
        })
    }

    fn ensure_buffer_space(&mut self, needed: u64) -> KaguraResult<()> {
        if self.dynamic_offset + needed <= self.dynamic_buffer_size {
            return Ok(());
        }
        let new_size = (self.dynamic_buffer_size * 2).max(needed);
        log::warn!("Dynamic vertex buffer resize: {} -> {} bytes", self.dynamic_buffer_size, new_size);
        let new_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dynamic Vertex Buffer (resized)"),
            size: new_size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.dynamic_vertex_buffer = new_buffer;
        self.dynamic_buffer_size = new_size;
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        self.screen_width = width;
        self.screen_height = height;
        let (dt, dv) = make_depth_texture(&self.device, width, height);
        self.depth_texture = dt;
        self.depth_view = dv;
        if let Some(ir) = self.instance_renderer.as_ref() {
            ir.resize(&self.queue, width, height);
        }
        // ★ Surface の設定も更新
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        let (ft, fv) = make_frame_texture(&self.device, width, height, self.surface_format);
        self.frame_texture = ft;
        self.frame_view = fv;
        let (mt, mv) = make_msaa_texture(&self.device, width, height, self.surface_format);
        self.msaa_texture = mt;
        self.msaa_view = mv;
    }

    pub fn width(&self) -> u32 { self.screen_width }
    pub fn height(&self) -> u32 { self.screen_height }

    pub fn load_texture(&mut self, path: &str) -> Result<u32, String> {
        let img = image::open(path).map_err(|e| format!("テクスチャ読み込み失敗: {} ({})", path, e))?.into_rgba8();
        let (width, height) = img.dimensions();
        let data = img.into_raw();
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(path),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &data,
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * width), rows_per_image: Some(height) },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Tex BG"),
            layout: &self.texture_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        let id = self.next_texture_id;
        self.next_texture_id += 1;
        self.textures.insert(id, GpuTexture { texture: tex, bind_group, width, height });
        Ok(id)
    }

    pub fn load_gltf_image(&mut self, data: &[u8], ext: &str) -> Result<u32, String> {
        if let Some(fmt) = crate::gltf_common::image_format_from_ext(ext) {
            if let Ok(id) = self.load_texture_from_memory(data, fmt) {
                return Ok(id);
            }
        }
        match image::load_from_memory(data) {
            Ok(img) => self.upload_rgba8(img.into_rgba8()),
            Err(e) => Err(format!("画像デコード失敗: {}", e)),
        }
    }

    fn upload_rgba8(&mut self, img: image::RgbaImage) -> Result<u32, String> {
        let (width, height) = img.dimensions();
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Memory Texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &img,
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * width), rows_per_image: Some(height) },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Tex BG"),
            layout: &self.texture_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });
        let id = self.next_texture_id;
        self.next_texture_id += 1;
        self.textures.insert(id, GpuTexture { texture: tex, bind_group, width, height });
        Ok(id)
    }

    pub fn load_texture_from_memory(&mut self, data: &[u8], format: image::ImageFormat) -> Result<u32, String> {
        let img = image::load_from_memory_with_format(data, format)
            .or_else(|_| image::load_from_memory(data))
            .map_err(|e| format!("画像デコード失敗: {}", e))?
            .into_rgba8();
        self.upload_rgba8(img)
    }

    pub fn texture_size(&self, id: u32) -> Option<(u32, u32)> {
        self.textures.get(&id).map(|t| (t.width, t.height))
    }

    fn get_texture_bind_group(&self, tex_id: u32) -> &wgpu::BindGroup {
        self.textures.get(&tex_id)
            .map(|t| &t.bind_group)
            .or_else(|| self.text_renderer.get_bind_group(tex_id))
            .unwrap_or(&self.fallback_texture_bind_group)
    }

    pub fn load_font(&mut self, path: &str) -> Result<u32, String> {
        self.text_renderer.load_font(path)
    }

    pub fn measure_text(&mut self, font_id: u32, text: &str, size_px: u32) -> (f32, f32) {
        self.text_renderer.measure_text(&self.device, &self.queue, font_id, text, size_px)
    }

    pub fn queue_command(&mut self, cmd: DrawCommand) { self.draw_queue.push(cmd); }
    pub fn queue_rect(&mut self, cmd: RectCommand) { self.queue_command(DrawCommand::Rect(cmd)); }
    pub fn queue_polygon(&mut self, cmd: PolygonCommand) { self.queue_command(DrawCommand::Polygon(cmd)); }
    pub fn queue_sprite(&mut self, cmd: SpriteCommand) { self.queue_command(DrawCommand::Sprite(cmd)); }
    pub fn queue_text(&mut self, cmd: TextCommand) { self.queue_command(DrawCommand::Text(cmd)); }
    pub fn queue_skinned_mesh_3d(&mut self, cmd: SkinnedMeshCommand) { self.mesh_3d_skinned_queue.push(cmd); }

    /// スキンパレット付きで 3D スキンメッシュをキューする。
    ///
    /// `update_skin_uniforms` は共有バッファ 1 本への `write_buffer` なので、
    /// 1 フレームに複数スキンを描くと最後の書き込みが全ドローに適用されてしまう
    /// （腕・指など「最後のパレットに居ないジョイント」がバインドポーズで固まる）。
    /// ここではドローごとに専用バッファ／バインドグループを割り当てる。
    pub fn queue_skinned_mesh_3d_with_palette(
        &mut self,
        mut cmd: SkinnedMeshCommand,
        matrices: &[nalgebra::Matrix4<f32>],
    ) {
        cmd.skin_slot = Some(self.alloc_skin_palette(matrices));
        self.mesh_3d_skinned_queue.push(cmd);
    }

    /// パレットプールの次のスロットに行列を書き込み、スロット番号を返す。
    /// プールはフレーム間で再利用され、`render()` 完了時に使用数がリセットされる。
    fn alloc_skin_palette(&mut self, matrices: &[nalgebra::Matrix4<f32>]) -> usize {
        if self.skin_palette_used == self.skin_palette_pool.len() {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Skin Palette Buffer"),
                size: 256 * 64 + 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Skin Palette BG"),
                layout: &self.skinning_uniform_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });
            self.skin_palette_pool.push((buffer, bind_group));
        }
        let slot = self.skin_palette_used;
        self.skin_palette_used += 1;

        let max = matrices.len().min(256);
        let data = &mut self.skin_uniform_scratch;
        data.fill(0.0);
        for (i, m) in matrices[..max].iter().enumerate() {
            data[i * 16..(i + 1) * 16].copy_from_slice(m.as_slice());
        }
        data[256 * 16] = self.screen_width as f32;
        data[256 * 16 + 1] = self.screen_height as f32;
        self.queue.write_buffer(
            &self.skin_palette_pool[slot].0,
            0,
            bytemuck::cast_slice(data.as_slice()),
        );
        slot
    }

    /// コマンドのスロットに対応するバインドグループ（未設定なら共有 BG）。
    fn skin_bind_group_for(&self, cmd: &SkinnedMeshCommand) -> &wgpu::BindGroup {
        cmd.skin_slot
            .and_then(|s| self.skin_palette_pool.get(s))
            .map(|(_, bg)| bg)
            .unwrap_or(&self.skinning_bind_group)
    }

    pub fn create_instance_batch(&mut self, texture_id: u32, capacity: u32, sprite_w: f32, sprite_h: f32) -> u32 {
        if self.instance_renderer.is_none() {
            self.instance_renderer = Some(InstanceRenderer::new(&self.device, self.surface_format, self.screen_width, self.screen_height));
        }
        if let Some(ir) = self.instance_renderer.as_mut() {
            ir.create_batch(&self.device, texture_id, capacity, sprite_w, sprite_h)
        } else { 0 }
    }

    pub fn update_instance_batch(&mut self, batch_id: u32, data: &[[f32; 6]]) {
        if let Some(ir) = self.instance_renderer.as_mut() { ir.update_batch(&self.queue, batch_id, data); }
    }

    pub fn queue_instance_batch(&mut self, batch_id: u32) { self.instance_draw_queue.push(batch_id); }
    pub fn queue_skinned_mesh(&mut self, cmd: SkinnedMeshCommand) { self.queue_command(DrawCommand::SkinnedMesh(cmd)); }

    pub fn load_shader_src(&mut self, wgsl_src: &str) -> Result<u32, String> {
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("User Shader"),
            source: wgpu::ShaderSource::Wgsl(wgsl_src.into()),
        });
        let tex_bgl = make_texture_bgl(&self.device);
        let layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&tex_bgl, &self.shader_params_bgl],
            push_constant_ranges: &[],
        });
        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("User Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "vs_main",
                buffers: &[SpriteVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: msaa_state(),
            multiview: None,
        });
        let id = self.next_shader_id;
        self.next_shader_id += 1;
        self.custom_pipelines.insert(id, pipeline);
        Ok(id)
    }

    pub fn update_camera_3d(&mut self, view: &[f32; 16], proj: &[f32; 16]) {
        // light_dir / toon / eye は別途書くので、ここは先頭 128 バイトだけ更新する
        let mut data = [0f32; 32];
        data[..16].copy_from_slice(view);
        data[16..].copy_from_slice(proj);
        self.queue.write_buffer(&self.camera_3d_buf, 0, bytemuck::cast_slice(&data));
        // view is column-major flat [16]
        // eye = -R^T * t where t=(m03,m13,m23)=(v[12],v[13],v[14])
        let v = view;
        let eye = [
            -(v[0] * v[12] + v[4] * v[13] + v[8] * v[14]),
            -(v[1] * v[12] + v[5] * v[13] + v[9] * v[14]),
            -(v[2] * v[12] + v[6] * v[13] + v[10] * v[14]),
            self.shader_clock.elapsed().as_secs_f32(),
        ];
        self.queue.write_buffer(&self.camera_3d_buf, 160, bytemuck::cast_slice(&eye));
    }

    /// 平行光の方向を設定する（光源へ向かうベクトル）。正規化はこちらで行う。
    pub fn set_light_dir(&mut self, x: f32, y: f32, z: f32) {
        self.light_dir = normalize_light_dir(x, y, z);
        self.queue.write_buffer(
            &self.camera_3d_buf,
            128,
            bytemuck::cast_slice(&self.light_dir),
        );
        self.update_shadow_vp();
    }

    pub fn set_shadow_enabled(&mut self, enabled: bool) {
        self.shadows_enabled = enabled;
    }

    fn update_shadow_vp(&mut self) {
        let vp = build_light_view_proj(self.light_dir, [0.0, 1.0, 0.0]);
        self.queue.write_buffer(&self.shadow_vp_buf, 0, bytemuck::cast_slice(&vp));
    }

    /// VRM トゥーン階調パラメータ。
    /// - threshold: 明暗境界（0〜1、half-Lambert 空間）
    /// - softness: 0=硬い2階調、大きいほど柔らかい。≥0.999 で従来の連続照明
    /// - shade: 影側の明るさ
    /// - lit: 光側の明るさ
    pub fn set_toon_params(&mut self, threshold: f32, softness: f32, shade: f32, lit: f32) {
        self.toon_params = [
            threshold.clamp(0.0, 1.0),
            softness.max(0.0),
            shade.clamp(0.0, 2.0),
            lit.clamp(0.0, 2.0),
        ];
        self.queue.write_buffer(
            &self.camera_3d_buf,
            144,
            bytemuck::cast_slice(&self.toon_params),
        );
    }

    /// 3D 距離フォグ。enabled=false で無効（デフォルト）。
    pub fn set_fog(&mut self, start: f32, end: f32, r: u8, g: u8, b: u8, enabled: bool) {
        let start = start.max(0.0);
        let end = end.max(start + 1e-3);
        self.fog_params = [start, end, if enabled { 1.0 } else { 0.0 }, 0.0];
        self.fog_color = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
        self.queue.write_buffer(
            &self.camera_3d_buf,
            176,
            bytemuck::cast_slice(&self.fog_params),
        );
        self.queue.write_buffer(
            &self.camera_3d_buf,
            192,
            bytemuck::cast_slice(&self.fog_color),
        );
    }

    pub fn queue_mesh_3d(&mut self, cmd: Mesh3DCommand) { self.mesh_3d_queue.push(cmd); }

    pub fn update_skin_uniforms(&mut self, matrices: &[nalgebra::Matrix4<f32>]) {
        let max = matrices.len().min(256);
        let data = &mut self.skin_uniform_scratch;
        // 未使用ボーン枠はゼロのままにする
        data.fill(0.0);
        for (i, m) in matrices[..max].iter().enumerate() {
            data[i * 16..(i + 1) * 16].copy_from_slice(m.as_slice());
        }
        data[256 * 16] = self.screen_width as f32;
        data[256 * 16 + 1] = self.screen_height as f32;
        self.queue.write_buffer(
            &self.skinning_uniform_buffer,
            0,
            bytemuck::cast_slice(data.as_slice()),
        );
    }

    pub fn unload_texture(&mut self, id: u32) -> Result<(), String> {
        if self.textures.remove(&id).is_some() {
            self.morph_bg_cache.retain(|(tid, shade, matcap, normal, uvmask, _, _), _| {
                *tid != id && *shade != id && *matcap != id && *normal != id && *uvmask != id
            });
            self.morph_bg_order.retain(|(tid, shade, matcap, normal, uvmask, _, _)| {
                *tid != id && *shade != id && *matcap != id && *normal != id && *uvmask != id
            });
            log::debug!("Texture {} unloaded", id);
            Ok(())
        } else {
            Err(format!("Texture with id {} not found", id))
        }
    }

    fn ensure_mesh_3d_capacity(&mut self, vb_bytes: u64, ib_bytes: u64) {
        if vb_bytes > self.mesh_3d_vb_cap {
            let new_cap = vb_bytes.next_power_of_two().max(MESH3D_VB_INITIAL);
            log::warn!("Mesh3D VB resize: {} -> {} bytes", self.mesh_3d_vb_cap, new_cap);
            self.mesh_3d_vb = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Mesh3D VB (pooled)"),
                size: new_cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.mesh_3d_vb_cap = new_cap;
        }
        if ib_bytes > self.mesh_3d_ib_cap {
            let new_cap = ib_bytes.next_power_of_two().max(MESH3D_IB_INITIAL);
            log::warn!("Mesh3D IB resize: {} -> {} bytes", self.mesh_3d_ib_cap, new_cap);
            self.mesh_3d_ib = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Mesh3D IB (pooled)"),
                size: new_cap,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.mesh_3d_ib_cap = new_cap;
        }
    }

    // ---------- メイン描画関数 ----------
    /// `screenshot_path` が Some なら、このフレームの結果を PNG に書き出す。
    pub fn render(&mut self, screenshot_path: Option<&str>) -> KaguraResult<()> {
        let output = self.surface.get_current_texture().map_err(|e| KaguraError::Gpu(e.to_string()))?;
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("KAGRA Encoder"),
        });

        for compute_fn in std::mem::take(&mut self.pending_computes) {
            compute_fn(&self.device, &self.queue, &mut encoder);
        }

        let (rect_cmds, poly_cmds, sprite_cmds, text_cmds, skinned_cmds, mesh_cmds) = self.split_draw_commands();
        // MSAA ターゲットに描画し、最後に 1x frame_texture へ resolve する
        let view = self.msaa_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 2D パスが空でもクリアされるように、毎フレーム先頭で塗りつぶす
        {
            let clear_color = wgpu::Color {
                r: self.clear_color.r as f64 / 255.0,
                g: self.clear_color.g as f64 / 255.0,
                b: self.clear_color.b as f64 / 255.0,
                a: self.clear_color.a as f64 / 255.0,
            };
            let _rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Frame Clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            drop(_rp);
        }

        self.draw_rects_and_polygons(&mut encoder, &view, &rect_cmds, &poly_cmds)?;
        self.draw_sprites_and_meshes(&mut encoder, &view, &sprite_cmds, &mesh_cmds)?;
        self.draw_text_glyphs(&mut encoder, &view, &text_cmds)?;
        self.draw_skinned_meshes_2d(&mut encoder, &view, &skinned_cmds);
        let mesh_3d_cmds = std::mem::take(&mut self.mesh_3d_queue);
        let skinned_3d_cmds = std::mem::take(&mut self.mesh_3d_skinned_queue);
        // Shadow pass first so mesh3d / VRM can sample the map
        self.update_shadow_vp();
        let morph_bgs = self.build_skinned_morph_bgs(&skinned_3d_cmds);
        self.draw_shadow_pass(&mut encoder, &skinned_3d_cmds, &morph_bgs);
        self.draw_meshes_3d(&mut encoder, &view, &mesh_3d_cmds);
        self.draw_skinned_meshes_3d(&mut encoder, &view, &skinned_3d_cmds, &morph_bgs);
        let instance_batches = std::mem::take(&mut self.instance_draw_queue);
        self.draw_instance_batches(&mut encoder, &view, &instance_batches);

        // MSAA → 1x resolve（スクリーンショット / swapchain 用）
        {
            let resolve_view = self.frame_texture.create_view(&wgpu::TextureViewDescriptor::default());
            let _rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("MSAA Resolve"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_view,
                    resolve_target: Some(&resolve_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            drop(_rp);
        }

        let extent = wgpu::Extent3d {
            width: self.screen_width,
            height: self.screen_height,
            depth_or_array_layers: 1,
        };
        encoder.copy_texture_to_texture(
            wgpu::ImageCopyTexture {
                texture: &self.frame_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyTexture {
                texture: &output.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            extent,
        );

        let readback = if let Some(path) = screenshot_path {
            Some((path.to_string(), self.encode_frame_readback(&mut encoder)))
        } else {
            None
        };

        self.staging_belt.finish();
        self.queue.submit(std::iter::once(encoder.finish()));

        if let Some((path, (buffer, _padded))) = readback {
            self.finish_frame_readback(buffer, &path)?;
        }

        output.present();
        self.staging_belt.recall();
        self.dynamic_offset = 0;
        self.skin_palette_used = 0;

        Ok(())
    }

    fn encode_frame_readback(&self, encoder: &mut wgpu::CommandEncoder) -> (wgpu::Buffer, u32) {
        let width = self.screen_width;
        let height = self.screen_height;
        let unpadded = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = (unpadded + align - 1) / align * align;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Screenshot Readback"),
            size: (padded as u64) * (height as u64),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.frame_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        (buffer, padded)
    }

    fn finish_frame_readback(&self, buffer: wgpu::Buffer, path: &str) -> KaguraResult<()> {
        let width = self.screen_width;
        let height = self.screen_height;
        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|_| KaguraError::Gpu("screenshot map channel closed".into()))?
            .map_err(|e| KaguraError::Gpu(format!("screenshot map failed: {:?}", e)))?;

        let data = slice.get_mapped_range();
        let unpadded = (width * 4) as usize;
        let padded = {
            let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
            (unpadded + align - 1) / align * align
        };
        let mut rgba = vec![0u8; unpadded * height as usize];
        let swizzle_bgra = matches!(
            self.surface_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        for y in 0..height as usize {
            let src = &data[y * padded .. y * padded + unpadded];
            let dst = &mut rgba[y * unpadded .. (y + 1) * unpadded];
            if swizzle_bgra {
                for (i, px) in src.chunks_exact(4).enumerate() {
                    dst[i * 4] = px[2];
                    dst[i * 4 + 1] = px[1];
                    dst[i * 4 + 2] = px[0];
                    dst[i * 4 + 3] = px[3];
                }
            } else {
                dst.copy_from_slice(src);
            }
        }
        drop(data);
        buffer.unmap();

        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        image::save_buffer(path, &rgba, width, height, image::ColorType::Rgba8)
            .map_err(|e| KaguraError::Other(format!("screenshot save failed: {}", e)))?;
        log::info!("screenshot saved: {}", path);
        Ok(())
    }

    fn split_draw_commands(&mut self) -> (Vec<RectCommand>, Vec<PolygonCommand>, Vec<SpriteCommand>, Vec<TextCommand>, Vec<SkinnedMeshCommand>, Vec<MeshCommand>) {
        let mut rect = Vec::new();
        let mut poly = Vec::new();
        let mut sprite = Vec::new();
        let mut text = Vec::new();
        let mut skinned = Vec::new();
        let mut mesh = Vec::new();
        for cmd in self.draw_queue.drain(..) {
            match cmd {
                DrawCommand::Rect(c) => rect.push(c),
                DrawCommand::Polygon(c) => poly.push(c),
                DrawCommand::Sprite(c) => sprite.push(c),
                DrawCommand::Text(c) => text.push(c),
                DrawCommand::SkinnedMesh(c) => skinned.push(c),
                DrawCommand::Mesh(c) => mesh.push(c),
            }
        }
        (rect, poly, sprite, text, skinned, mesh)
    }

    fn draw_rects_and_polygons(&mut self, encoder: &mut wgpu::CommandEncoder, target_view: &wgpu::TextureView, rects: &[RectCommand], polys: &[PolygonCommand]) -> KaguraResult<()> {
        if rects.is_empty() && polys.is_empty() { return Ok(()); }
        let sw = self.screen_width as f32;
        let sh = self.screen_height as f32;
        let mut vertices = Vec::<ColorVertex>::new();
        for cmd in rects {
            let x0 = (cmd.x / sw) * 2.0 - 1.0;
            let y0 = -((cmd.y / sh) * 2.0 - 1.0);
            let x1 = ((cmd.x + cmd.w) / sw) * 2.0 - 1.0;
            let y1 = -(((cmd.y + cmd.h) / sh) * 2.0 - 1.0);
            let c = cmd.color.to_f32();
            vertices.extend(&[
                ColorVertex { position: [x0, y0], color: c },
                ColorVertex { position: [x1, y0], color: c },
                ColorVertex { position: [x0, y1], color: c },
                ColorVertex { position: [x1, y0], color: c },
                ColorVertex { position: [x1, y1], color: c },
                ColorVertex { position: [x0, y1], color: c },
            ]);
        }
        for cmd in polys {
            if cmd.verts.len() < 3 { continue; }
            let c = cmd.color.to_f32();
            let v0 = cmd.verts[0];
            let x0 = (v0[0] / sw) * 2.0 - 1.0;
            let y0 = -((v0[1] / sh) * 2.0 - 1.0);
            for i in 1..cmd.verts.len() - 1 {
                let v1 = cmd.verts[i];
                let v2 = cmd.verts[i + 1];
                let x1 = (v1[0] / sw) * 2.0 - 1.0;
                let y1 = -((v1[1] / sh) * 2.0 - 1.0);
                let x2 = (v2[0] / sw) * 2.0 - 1.0;
                let y2 = -((v2[1] / sh) * 2.0 - 1.0);
                vertices.extend(&[
                    ColorVertex { position: [x0, y0], color: c },
                    ColorVertex { position: [x1, y1], color: c },
                    ColorVertex { position: [x2, y2], color: c },
                ]);
            }
        }
        let vertex_size = std::mem::size_of::<ColorVertex>() as u64;
        let total_size = vertices.len() as u64 * vertex_size;
        self.ensure_buffer_space(total_size)?;
        let size = std::num::NonZeroU64::new(total_size).unwrap();
        let mut buffer_view = self.staging_belt.write_buffer(encoder, &self.dynamic_vertex_buffer, self.dynamic_offset, size, &self.device);
        self.dynamic_offset += total_size;
        buffer_view.copy_from_slice(bytemuck::cast_slice(&vertices));
        let clear_color = wgpu::Color { r: self.clear_color.r as f64 / 255.0, g: self.clear_color.g as f64 / 255.0, b: self.clear_color.b as f64 / 255.0, a: self.clear_color.a as f64 / 255.0 };
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Rect/Poly Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear_color), store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rp.set_pipeline(&self.rect_pipeline);
        let offset = self.dynamic_offset - total_size;
        rp.set_vertex_buffer(0, self.dynamic_vertex_buffer.slice(offset..offset + total_size));
        rp.draw(0..vertices.len() as u32, 0..1);
        Ok(())
    }

    fn draw_sprites_and_meshes(&mut self, encoder: &mut wgpu::CommandEncoder, target_view: &wgpu::TextureView, sprites: &[SpriteCommand], meshes: &[MeshCommand]) -> KaguraResult<()> {
        struct BatchItem { texture_id: u32, shader_id: u32, params: [f32; 4], vertices: Vec<SpriteVertex> }
        let sw = self.screen_width as f32;
        let sh = self.screen_height as f32;
        let mut items = Vec::new();
        for cmd in sprites {
            let (tw, th) = self.textures.get(&cmd.texture_id).map(|t| (t.width as f32, t.height as f32)).unwrap_or((1.0, 1.0));
            let u0 = cmd.sx / tw; let v0 = cmd.sy / th;
            let u1 = (cmd.sx + cmd.sw) / tw; let v1 = (cmd.sy + cmd.sh) / th;
            let (fu0, fu1) = if cmd.flip_x { (u1, u0) } else { (u0, u1) };
            let (fv0, fv1) = if cmd.flip_y { (v1, v0) } else { (v0, v1) };
            let px = cmd.dw * cmd.pivot_x; let py = cmd.dh * cmd.pivot_y;
            let ox = cmd.dx + px; let oy = cmd.dy + py;
            let rad = cmd.rotation_deg.to_radians(); let (sin_r, cos_r) = (rad.sin(), rad.cos());
            let a = cmd.alpha;
            let local = [(-px, -py), (cmd.dw - px, -py), (-px, cmd.dh - py), (cmd.dw - px, -py), (cmd.dw - px, cmd.dh - py), (-px, cmd.dh - py)];
            let uvs = [(fu0, fv0), (fu1, fv0), (fu0, fv1), (fu1, fv0), (fu1, fv1), (fu0, fv1)];
            let mut verts = Vec::with_capacity(6);
            for i in 0..6 {
                let rx = local[i].0 * cos_r - local[i].1 * sin_r;
                let ry = local[i].0 * sin_r + local[i].1 * cos_r;
                let px_world = (ox + rx) / sw * 2.0 - 1.0;
                let py_world = -((oy + ry) / sh * 2.0 - 1.0);
                verts.push(SpriteVertex { position: [px_world, py_world], uv: [uvs[i].0, uvs[i].1], alpha: a, _pad: [0.0; 3] });
            }
            items.push(BatchItem { texture_id: cmd.texture_id, shader_id: cmd.shader_id, params: cmd.shader_params, vertices: verts });
        }
        for cmd in meshes {
            let mut verts = Vec::with_capacity(cmd.verts.len());
            for v in &cmd.verts {
                let px = (v[0] / sw) * 2.0 - 1.0;
                let py = -((v[1] / sh) * 2.0 - 1.0);
                verts.push(SpriteVertex { position: [px, py], uv: [v[2], v[3]], alpha: v[4], _pad: [0.0; 3] });
            }
            items.push(BatchItem { texture_id: cmd.texture_id, shader_id: cmd.shader_id, params: cmd.shader_params, vertices: verts });
        }
        if items.is_empty() { return Ok(()); }
        items.sort_by_key(|i| (i.texture_id, i.shader_id));
        let mut current_tex = None;
        let mut current_shader = None;
        let mut current_params = [0.0; 4];
        let mut batch_vertices = Vec::new();
        for item in items {
            if current_tex != Some(item.texture_id) || current_shader != Some(item.shader_id) || current_params != item.params {
                if !batch_vertices.is_empty() {
                    self.flush_sprite_batch(encoder, target_view, current_tex.unwrap(), current_shader.unwrap(), current_params, &batch_vertices)?;
                    batch_vertices.clear();
                }
                current_tex = Some(item.texture_id);
                current_shader = Some(item.shader_id);
                current_params = item.params;
            }
            batch_vertices.extend(item.vertices);
        }
        if !batch_vertices.is_empty() {
            self.flush_sprite_batch(encoder, target_view, current_tex.unwrap(), current_shader.unwrap(), current_params, &batch_vertices)?;
        }
        Ok(())
    }

    fn flush_sprite_batch(&mut self, encoder: &mut wgpu::CommandEncoder, target_view: &wgpu::TextureView, tex_id: u32, shader_id: u32, params: [f32; 4], vertices: &[SpriteVertex]) -> KaguraResult<()> {
        if vertices.is_empty() { return Ok(()); }
        let vertex_size = std::mem::size_of::<SpriteVertex>() as u64;
        let total_size = vertices.len() as u64 * vertex_size;
        self.ensure_buffer_space(total_size)?;
        let size = std::num::NonZeroU64::new(total_size).unwrap();
        {
            let mut buffer_view = self.staging_belt.write_buffer(encoder, &self.dynamic_vertex_buffer, self.dynamic_offset, size, &self.device);
            buffer_view.copy_from_slice(bytemuck::cast_slice(vertices));
        }
        self.dynamic_offset += total_size;
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Sprite Batch Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let pipeline = if shader_id == 0 { &self.sprite_pipeline } else { self.custom_pipelines.get(&shader_id).unwrap_or(&self.sprite_pipeline) };
        rp.set_pipeline(pipeline);
        if shader_id != 0 {
            self.queue.write_buffer(&self.shader_params_buffer, 0, bytemuck::cast_slice(&params));
            rp.set_bind_group(1, &self.shader_params_bg, &[]);
        }
        rp.set_bind_group(0, self.get_texture_bind_group(tex_id), &[]);
        let offset = self.dynamic_offset - total_size;
        rp.set_vertex_buffer(0, self.dynamic_vertex_buffer.slice(offset..offset + total_size));
        rp.draw(0..vertices.len() as u32, 0..1);
        Ok(())
    }

    fn draw_text_glyphs(&mut self, encoder: &mut wgpu::CommandEncoder, target_view: &wgpu::TextureView, text_cmds: &[TextCommand]) -> KaguraResult<()> {
        if text_cmds.is_empty() { return Ok(()); }
        let sw = self.screen_width as f32;
        let sh = self.screen_height as f32;
        let mut glyph_batches = Vec::new();
        for cmd in text_cmds {
            let glyphs = self.text_renderer.layout_text(&self.device, &self.queue, cmd.font_id, &cmd.text, cmd.size_px, cmd.x, cmd.y);
            let color = cmd.color.to_f32();
            for (tex_id, gx, gy, gw, gh) in glyphs {
                let x0 = (gx / sw) * 2.0 - 1.0;
                let y0 = -((gy / sh) * 2.0 - 1.0);
                let x1 = ((gx + gw) / sw) * 2.0 - 1.0;
                let y1 = -(((gy + gh) / sh) * 2.0 - 1.0);
                let a = color[3];
                let verts = [
                    SpriteVertex { position: [x0, y0], uv: [0.0, 0.0], alpha: a, _pad: [0.0; 3] },
                    SpriteVertex { position: [x1, y0], uv: [1.0, 0.0], alpha: a, _pad: [0.0; 3] },
                    SpriteVertex { position: [x0, y1], uv: [0.0, 1.0], alpha: a, _pad: [0.0; 3] },
                    SpriteVertex { position: [x1, y0], uv: [1.0, 0.0], alpha: a, _pad: [0.0; 3] },
                    SpriteVertex { position: [x1, y1], uv: [1.0, 1.0], alpha: a, _pad: [0.0; 3] },
                    SpriteVertex { position: [x0, y1], uv: [0.0, 1.0], alpha: a, _pad: [0.0; 3] },
                ];
                glyph_batches.push((tex_id, verts));
            }
        }
        if glyph_batches.is_empty() { return Ok(()); }
        glyph_batches.sort_by_key(|(tid, _)| *tid);
        let mut current_tex = None;
        let mut all_vertices = Vec::new();
        for (tex_id, verts) in glyph_batches {
            if current_tex != Some(tex_id) {
                if !all_vertices.is_empty() {
                    self.flush_sprite_batch(encoder, target_view, current_tex.unwrap(), 0, [0.0; 4], &all_vertices)?;
                    all_vertices.clear();
                }
                current_tex = Some(tex_id);
            }
            all_vertices.extend_from_slice(&verts);
        }
        if !all_vertices.is_empty() {
            self.flush_sprite_batch(encoder, target_view, current_tex.unwrap(), 0, [0.0; 4], &all_vertices)?;
        }
        Ok(())
    }

    fn draw_skinned_meshes_2d(&mut self, encoder: &mut wgpu::CommandEncoder, target_view: &wgpu::TextureView, cmds: &[SkinnedMeshCommand]) {
        if cmds.is_empty() { return; }
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Skinned 2D Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rp.set_pipeline(&self.skinning_pipeline);
        rp.set_bind_group(0, &self.skinning_bind_group, &[]);
        for cmd in cmds {
            let bg = self.get_texture_bind_group(cmd.texture_id);
            rp.set_bind_group(1, bg, &[]);
            rp.set_vertex_buffer(0, cmd.vertex_buffer.slice(..));
            rp.set_index_buffer(cmd.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            rp.draw_indexed(0..cmd.num_indices, 0, 0..1);
        }
    }

    fn draw_meshes_3d(&mut self, encoder: &mut wgpu::CommandEncoder, target_view: &wgpu::TextureView, cmds: &[Mesh3DCommand]) {
        if cmds.is_empty() { return; }

        const VERT_STRIDE: u64 = (8 * std::mem::size_of::<f32>()) as u64; // [f32; 8]
        const INDEX_STRIDE: u64 = std::mem::size_of::<u32>() as u64;

        let mut total_vb = 0u64;
        let mut total_ib = 0u64;
        for cmd in cmds {
            if cmd.verts.is_empty() || cmd.indices.is_empty() {
                continue;
            }
            total_vb += cmd.verts.len() as u64 * VERT_STRIDE;
            total_ib += cmd.indices.len() as u64 * INDEX_STRIDE;
        }
        if total_vb == 0 || total_ib == 0 {
            return;
        }
        self.ensure_mesh_3d_capacity(total_vb, total_ib);

        // (texture_id, vb_offset, ib_offset, index_count)
        let mut draws: Vec<(u32, u64, u64, u32)> = Vec::with_capacity(cmds.len());
        let mut vb_off = 0u64;
        let mut ib_off = 0u64;
        for cmd in cmds {
            if cmd.verts.is_empty() || cmd.indices.is_empty() {
                continue;
            }
            let v_bytes = cmd.verts.len() as u64 * VERT_STRIDE;
            let i_bytes = cmd.indices.len() as u64 * INDEX_STRIDE;
            self.queue.write_buffer(
                &self.mesh_3d_vb,
                vb_off,
                bytemuck::cast_slice(&cmd.verts),
            );
            self.queue.write_buffer(
                &self.mesh_3d_ib,
                ib_off,
                bytemuck::cast_slice(&cmd.indices),
            );
            draws.push((cmd.texture_id, vb_off, ib_off, cmd.indices.len() as u32));
            vb_off += v_bytes;
            ib_off += i_bytes;
        }

        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("3D Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.depth_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rp.set_pipeline(&self.pipeline_3d);
        rp.set_bind_group(0, &self.camera_3d_bg, &[]);
        rp.set_bind_group(2, &self.shadow_bg, &[]);
        for (texture_id, v_off, i_off, index_count) in draws {
            let bg = self.get_texture_bind_group(texture_id);
            rp.set_bind_group(1, bg, &[]);
            rp.set_vertex_buffer(0, self.mesh_3d_vb.slice(v_off..));
            rp.set_index_buffer(self.mesh_3d_ib.slice(i_off..), wgpu::IndexFormat::Uint32);
            rp.draw_indexed(0..index_count, 0, 0..1);
        }
    }

    fn build_skinned_morph_bgs(&mut self, cmds: &[SkinnedMeshCommand]) -> Vec<Option<Arc<wgpu::BindGroup>>> {
        let mut morph_bgs: Vec<Option<Arc<wgpu::BindGroup>>> = Vec::with_capacity(cmds.len());
        for cmd in cmds {
            let shade_tex_id = cmd.shade_texture_id.unwrap_or(cmd.texture_id);
            let matcap_id = cmd.matcap_texture_id.unwrap_or(0);
            let normal_id = cmd.normal_texture_id.unwrap_or(0);
            let uvmask_id = cmd.uv_mask_texture_id.unwrap_or(0);
            let morph_ptr = Arc::as_ptr(&cmd.morph_delta_buffer) as *const () as usize;
            let blend_ptr = Arc::as_ptr(&cmd.blend_weights_buffer) as *const () as usize;
            let key = (cmd.texture_id, shade_tex_id, matcap_id, normal_id, uvmask_id, morph_ptr, blend_ptr);
            if let Some(bg) = self.morph_bg_cache.get(&key) {
                morph_bgs.push(Some(bg.clone()));
                continue;
            }
            let tex = match self.textures.get(&cmd.texture_id) {
                Some(t) => t,
                None => {
                    morph_bgs.push(None);
                    continue;
                }
            };
            let shade_tex = self.textures.get(&shade_tex_id).unwrap_or(tex);
            let tex_view = tex.texture.create_view(&wgpu::TextureViewDescriptor::default());
            let shade_view = shade_tex.texture.create_view(&wgpu::TextureViewDescriptor::default());
            let matcap_view;
            let matcap_ref = if let Some(t) = cmd.matcap_texture_id.and_then(|id| self.textures.get(&id)) {
                matcap_view = t.texture.create_view(&wgpu::TextureViewDescriptor::default());
                &matcap_view
            } else {
                &self.fallback_tex_view
            };
            let normal_view;
            let normal_ref = if let Some(t) = cmd.normal_texture_id.and_then(|id| self.textures.get(&id)) {
                normal_view = t.texture.create_view(&wgpu::TextureViewDescriptor::default());
                &normal_view
            } else {
                &self.fallback_tex_view
            };
            let uvmask_view;
            let uvmask_ref = if let Some(t) = cmd.uv_mask_texture_id.and_then(|id| self.textures.get(&id)) {
                uvmask_view = t.texture.create_view(&wgpu::TextureViewDescriptor::default());
                &uvmask_view
            } else {
                &self.fallback_tex_view
            };
            let morph_info: [u32; 4] = [cmd.num_morph_targets, 0, 0, 0];
            let info_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("MorphInfo"),
                contents: bytemuck::cast_slice(&morph_info),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let mtoon_resource = match cmd.mtoon_buffer.as_ref() {
                Some(b) => b.as_entire_binding(),
                None => self.default_mtoon_buf.as_entire_binding(),
            };
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Morph BG"),
                layout: &self.skinning_3d_morph_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&tex_view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    wgpu::BindGroupEntry { binding: 2, resource: cmd.morph_delta_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: cmd.blend_weights_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: info_buf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 5, resource: mtoon_resource },
                    wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&shade_view) },
                    wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(matcap_ref) },
                    wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(normal_ref) },
                    wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::TextureView(uvmask_ref) },
                ],
            });
            let bg = Arc::new(bg);
            while self.morph_bg_cache.len() >= MORPH_BG_CACHE_MAX {
                if let Some(old) = self.morph_bg_order.pop_front() {
                    self.morph_bg_cache.remove(&old);
                } else {
                    break;
                }
            }
            self.morph_bg_order.push_back(key);
            self.morph_bg_cache.insert(key, bg.clone());
            morph_bgs.push(Some(bg));
        }
        morph_bgs
    }

    fn draw_shadow_pass(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        cmds: &[SkinnedMeshCommand],
        morph_bgs: &[Option<Arc<wgpu::BindGroup>>],
    ) {
        if !self.shadows_enabled || cmds.is_empty() {
            // Still clear so receivers see an empty (fully lit) map
            let _rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Clear"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            return;
        }
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Shadow Pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.skinning_3d_shadow_pipeline);
            rp.set_bind_group(0, &self.camera_3d_bg, &[]);
            rp.set_bind_group(3, &self.shadow_write_bg, &[]);
            for (i, cmd) in cmds.iter().enumerate() {
                let morph_bg = match morph_bgs.get(i).and_then(|b| b.as_ref()) {
                    Some(bg) => bg,
                    None => continue,
                };
                rp.set_bind_group(1, self.skin_bind_group_for(cmd), &[]);
                rp.set_bind_group(2, morph_bg, &[]);
                rp.set_vertex_buffer(0, cmd.vertex_buffer.slice(..));
                rp.set_index_buffer(cmd.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..cmd.num_indices, 0, 0..1);
            }
        }
    }

    fn draw_skinned_meshes_3d(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        cmds: &[SkinnedMeshCommand],
        morph_bgs: &[Option<Arc<wgpu::BindGroup>>],
    ) {
        if cmds.is_empty() { return; }
        {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Skinned 3D Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.depth_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.skinning_3d_pipeline);
            rp.set_bind_group(0, &self.camera_3d_bg, &[]);
            rp.set_bind_group(3, &self.shadow_bg, &[]);
            for (i, cmd) in cmds.iter().enumerate() {
                let morph_bg = match morph_bgs.get(i).and_then(|b| b.as_ref()) {
                    Some(bg) => bg,
                    None => continue,
                };
                rp.set_bind_group(1, self.skin_bind_group_for(cmd), &[]);
                rp.set_bind_group(2, morph_bg, &[]);
                rp.set_vertex_buffer(0, cmd.vertex_buffer.slice(..));
                rp.set_index_buffer(cmd.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..cmd.num_indices, 0, 0..1);
            }
        }
        let need_outline = cmds.iter().any(|c| c.outline_width > 1e-5);
        if need_outline {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Skinned 3D Outline Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.depth_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.skinning_3d_outline_pipeline);
            rp.set_bind_group(0, &self.camera_3d_bg, &[]);
            rp.set_bind_group(3, &self.shadow_bg, &[]);
            for (i, cmd) in cmds.iter().enumerate() {
                if cmd.outline_width <= 1e-5 {
                    continue;
                }
                let morph_bg = match morph_bgs.get(i).and_then(|b| b.as_ref()) {
                    Some(bg) => bg,
                    None => continue,
                };
                rp.set_bind_group(1, self.skin_bind_group_for(cmd), &[]);
                rp.set_bind_group(2, morph_bg, &[]);
                rp.set_vertex_buffer(0, cmd.vertex_buffer.slice(..));
                rp.set_index_buffer(cmd.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..cmd.num_indices, 0, 0..1);
            }
        }
    }

    fn draw_instance_batches(&mut self, encoder: &mut wgpu::CommandEncoder, target_view: &wgpu::TextureView, batch_ids: &[u32]) {
        if batch_ids.is_empty() || self.instance_renderer.is_none() { return; }
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Instance Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let ir = self.instance_renderer.as_ref().unwrap();
        for &bid in batch_ids { ir.draw_batch(&mut rp, bid); }
    }
}

