// kagra-core/src/renderer.rs
// wgpu 0.19 - 矩形/スプライト/テキスト/スキニングメッシュ描画
// Phase 2: DrawCommand 一本化

use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::color::Color;
use crate::text::TextRenderer;
use nalgebra;

// ── 描画コマンド ──────────────────────────────────────────────
#[derive(Clone)]
pub struct RectCommand {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: Color,
}

#[derive(Clone)]
pub struct SpriteCommand {
    pub texture_id: u32,
    pub shader_id: u32,
    pub shader_params: [f32; 4],
    pub dx: f32,
    pub dy: f32,
    pub dw: f32,
    pub dh: f32,
    pub sx: f32,
    pub sy: f32,
    pub sw: f32,
    pub sh: f32,
    pub alpha: f32,
    pub rotation_deg: f32,
    pub pivot_x: f32,
    pub pivot_y: f32,
    pub flip_x: bool,
    pub flip_y: bool,
}

#[derive(Clone)]
pub struct TextCommand {
    pub font_id: u32,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size_px: u32,
    pub color: Color,
}

/// スキニングメッシュ描画コマンド
#[derive(Clone)]
pub struct SkinnedMeshCommand {
    pub texture_id:       u32,
    pub vertex_buffer:    Arc<wgpu::Buffer>,
    pub index_buffer:     Arc<wgpu::Buffer>,
    pub num_indices:      u32,
    /// モーフターゲット用バインドグループ（None = モーフなし）
    pub morph_bind_group: Option<Arc<wgpu::BindGroup>>,
    /// このプリミティブのモーフウェイト（描画直前に GPU バッファへ書き込む）
    pub morph_weights:    [f32; 8],
}

/// Python から変形済み頂点を受け取ってメッシュ描画
#[derive(Clone)]
pub struct MeshCommand {
    pub texture_id: u32,
    pub verts:      Vec<[f32; 5]>,
    pub shader_id:  u32,
    pub shader_params: [f32; 4],
}

/// 3D メッシュ描画コマンド
#[derive(Clone)]
pub struct Mesh3DCommand {
    pub texture_id: u32,
    pub verts:   Vec<[f32; 8]>,   // [x,y,z, nx,ny,nz, u,v]
    pub indices: Vec<u32>,
}

/// フェーズ2の本丸: 描画命令を1本化
#[derive(Clone)]
pub enum DrawCommand {
    Rect(RectCommand),
    Sprite(SpriteCommand),
    Text(TextCommand),
    SkinnedMesh(SkinnedMeshCommand),
    Mesh(MeshCommand),
}

// ── 頂点フォーマット ──────────────────────────────────────────
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ColorVertex {
    position: [f32; 2],
    color: [f32; 4],
}
impl ColorVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SpriteVertex {
    position: [f32; 2],
    uv: [f32; 2],
    alpha: f32,
    _pad: [f32; 3],
}
impl SpriteVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32];
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinnedVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub joints: [u32; 4],
    pub weights: [f32; 4],
}
impl SkinnedVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Uint32x4, 3 => Float32x4];
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

struct GpuTexture {
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

fn make_depth_texture(device: &wgpu::Device, w: u32, h: u32)
    -> (wgpu::Texture, wgpu::TextureView)
{
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn make_texture_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
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

fn make_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    })
}

// ── Renderer ─────────────────────────────────────────────────
pub struct Renderer {
    pub clear_color: Color,

    draw_queue: Vec<DrawCommand>,

    rect_pipeline: wgpu::RenderPipeline,
    sprite_pipeline: wgpu::RenderPipeline,
    skinning_pipeline:    wgpu::RenderPipeline,
    skinning_3d_pipeline: wgpu::RenderPipeline,   // 3D カメラ対応スキニング
    mesh_3d_skinned_queue: Vec<SkinnedMeshCommand>, // 3D スキニングキュー

    texture_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: std::collections::HashMap<u32, GpuTexture>,
    next_texture_id: u32,

    text_renderer: TextRenderer,

    shader_params_bgl:    wgpu::BindGroupLayout,
    shader_params_buffer: wgpu::Buffer,
    shader_params_bg:     wgpu::BindGroup,
    custom_pipelines:     std::collections::HashMap<u32, wgpu::RenderPipeline>,
    next_shader_id:       u32,
    surface_format:       wgpu::TextureFormat,

    #[allow(dead_code)]
    skinning_uniform_bgl: wgpu::BindGroupLayout,
    skinning_uniform_buffer: wgpu::Buffer,
    skinning_bind_group: wgpu::BindGroup,

    // ── ブレンドシェイプ（モーフ）───────────────────────────
    pub morph_weight_buffer:   wgpu::Buffer,
    pub morph_weight_bgl:      wgpu::BindGroupLayout,
    null_morph_bind_group:     wgpu::BindGroup,

    // フォグパラメータ（Python から更新）
    fog_start:   f32,
    fog_end:     f32,
    fog_color:   [f32; 3],  // RGB 0.0-1.0
    fog_enabled: bool,

    // ── Phase 11: 3D 拡張 ───────────────────────────────
    depth_texture: wgpu::Texture,
    depth_view:    wgpu::TextureView,
    pipeline_3d:   wgpu::RenderPipeline,
    camera_3d_buf: wgpu::Buffer,
    camera_3d_bg:  wgpu::BindGroup,
    camera_3d_bgl: wgpu::BindGroupLayout,
    mesh_3d_queue: Vec<Mesh3DCommand>,

    pub device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    screen_width: u32,
    screen_height: u32,
}

impl Renderer {
    pub async fn new(window: &Window, width: u32, height: u32) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
            flags: wgpu::InstanceFlags::default(),
            gles_minor_version: wgpu::Gles3MinorVersion::Automatic,
        });

        let window_static: &'static Window = unsafe { &*(window as *const Window) };
        let surface = instance.create_surface(window_static).map_err(|e| e.to_string())?;

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
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

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
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let texture_bgl = make_texture_bgl(&device);
        let texture_bgl_text = make_texture_bgl(&device);
        let sampler = make_sampler(&device);
        let sampler_text = make_sampler(&device);

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
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
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
            size: 256 * 64 + 16,  // bone_matrices(16384) + screen_size vec4(16)
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

        // ── モーフターゲット BGL ─────────────────────────────────────
        // group(3): binding(0)=差分頂点 storage, binding(1)=ウェイト uniform
        let morph_weight_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Morph Weight Buffer"),
            size:  32,  // f32 × 8
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let morph_weight_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Morph BGL"),
            entries: &[
                // binding(0): 差分頂点 storage buffer（プリミティブごとに異なる）
                wgpu::BindGroupLayoutEntry {
                    binding:    0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding(1): ウェイト uniform（全プリミティブで共用）
                wgpu::BindGroupLayoutEntry {
                    binding:    1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // ── null モーフバインドグループ（モーフなしプリミティブ用）────
        // ダミーの1要素 storage バッファ（0バイト storage は wgpu が拒否するため）
        let null_morph_storage = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label:    Some("Null Morph Storage"),
            contents: bytemuck::cast_slice(&[0.0f32; 4]),  // 16 bytes
            usage:    wgpu::BufferUsages::STORAGE,
        });
        // null_morph 用 BGL（storage + uniform の2バインディング）
        let null_morph_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Null Morph BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false, min_binding_size: None,
                    }, count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false, min_binding_size: None,
                    }, count: None,
                },
            ],
        });
        let null_morph_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:   Some("Null Morph BG"),
            layout:  &null_morph_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: null_morph_storage.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: morph_weight_buffer.as_entire_binding() },
            ],
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
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let text_renderer = TextRenderer::new(texture_bgl_text, sampler_text);

        // ── Shader パラメータ Uniform Buffer ──────────────────
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

        // ── カスタムシェーダーパイプライン ──────────────────
        let make_custom = |device: &wgpu::Device, wgsl: &str,
                           tex_bgl: &wgpu::BindGroupLayout,
                           params_bgl: &wgpu::BindGroupLayout,
                           fmt: wgpu::TextureFormat| {
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
                    module: &module, entry_point: "vs_main",
                    buffers: &[SpriteVertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module, entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: fmt,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            })
        };
        let tmp_bgl = make_texture_bgl(&device);
        let mut custom_pipelines = std::collections::HashMap::new();
        custom_pipelines.insert(1u32, make_custom(&device, SHADER_GRAYSCALE, &tmp_bgl, &shader_params_bgl, format));
        custom_pipelines.insert(2u32, make_custom(&device, SHADER_FLASH,     &tmp_bgl, &shader_params_bgl, format));
        custom_pipelines.insert(3u32, make_custom(&device, SHADER_SPOTLIGHT, &tmp_bgl, &shader_params_bgl, format));
        custom_pipelines.insert(4u32, make_custom(&device, SHADER_GLOW,      &tmp_bgl, &shader_params_bgl, format));
        custom_pipelines.insert(5u32, make_custom(&device, SHADER_TINT,      &tmp_bgl, &shader_params_bgl, format));

        // ── Depth Buffer ────────────────────────────────────
        let (depth_texture, depth_view) = make_depth_texture(&device, width, height);

        // ── 3D Camera Uniform Buffer ─────────────────────────
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
        let camera_3d_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera3D Buf"),
            size: 160,  // view(64) + proj(64) + fog params(32)
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

        // ── 3D スキニングパイプライン ─────────────────────────
        let skinning_3d_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skinning3D Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SKINNING_3D.into()),
        });
        let skin3d_tex_bgl = make_texture_bgl(&device);
        let skinning_3d_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[
                &camera_3d_bgl,        // group(0): カメラ
                &skinning_uniform_bgl, // group(1): ボーン行列 + screen_size
                &skin3d_tex_bgl,       // group(2): テクスチャ
                &morph_weight_bgl,     // group(3): モーフウェイト8本
            ],
            push_constant_ranges: &[],
        });
        let skinning_3d_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skinning3D Pipeline"),
            layout: Some(&skinning_3d_layout),
            vertex: wgpu::VertexState {
                module: &skinning_3d_shader,
                entry_point: "vs_main",
                buffers: &[SkinnedVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &skinning_3d_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // ── 3D Render Pipeline ───────────────────────────────
        let shader_3d = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader3D"),
            source: wgpu::ShaderSource::Wgsl(SHADER_3D.into()),
        });
        let tex_bgl_3d = make_texture_bgl(&device);
        let layout_3d = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&camera_3d_bgl, &tex_bgl_3d],
            push_constant_ranges: &[],
        });
        let pipeline_3d = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Pipeline3D"),
            layout: Some(&layout_3d),
            vertex: wgpu::VertexState {
                module: &shader_3d, entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 32,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x3, 1 => Float32x3, 2 => Float32x2
                    ],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_3d, entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,   // VRM 向き確認中は無効
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
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Ok(Renderer {
            clear_color: Color { r: 0, g: 0, b: 0, a: 255 },
            draw_queue: Vec::new(),
            rect_pipeline,
            sprite_pipeline,
            skinning_pipeline,
            skinning_3d_pipeline,
            mesh_3d_skinned_queue: Vec::new(),
            morph_weight_buffer,
            morph_weight_bgl,
            texture_bgl,
            sampler,
            textures: std::collections::HashMap::new(),
            next_texture_id: 1,
            text_renderer,
            shader_params_bgl,
            shader_params_buffer,
            shader_params_bg,
            custom_pipelines,
            next_shader_id: 10,
            surface_format: format,
            fog_start:   0.0,
            fog_end:     30.0,
            fog_color:   [0.43, 0.71, 0.90],
            fog_enabled: false,
            depth_texture,
            depth_view,
            pipeline_3d,
            camera_3d_buf,
            camera_3d_bg,
            camera_3d_bgl,
            mesh_3d_queue: Vec::new(),
            skinning_uniform_bgl,
            skinning_uniform_buffer,
            skinning_bind_group,
            null_morph_bind_group,
            device,
            queue,
            surface,
            surface_config,
            screen_width: width,
            screen_height: height,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.screen_width = width;
        self.screen_height = height;
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        let (dt, dv) = make_depth_texture(&self.device, width, height);
        self.depth_texture = dt;
        self.depth_view    = dv;
    }

    pub fn width(&self) -> u32 { self.screen_width }
    pub fn height(&self) -> u32 { self.screen_height }

    pub fn load_texture(&mut self, path: &str) -> Result<u32, String> {
        let img = image::open(path)
            .map_err(|e| format!("テクスチャ読み込み失敗: {} ({})", path, e))?
            .into_rgba8();
        let (width, height) = img.dimensions();
        let data = img.into_raw();

        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(path),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Tex BG"),
            layout: &self.texture_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let id = self.next_texture_id;
        self.next_texture_id += 1;
        self.textures.insert(id, GpuTexture { bind_group, width, height });
        Ok(id)
    }

    pub fn texture_size(&self, id: u32) -> Option<(u32, u32)> {
        self.textures.get(&id).map(|t| (t.width, t.height))
    }

    pub fn load_font(&mut self, path: &str) -> Result<u32, String> {
        self.text_renderer.load_font(path)
    }

    pub fn measure_text(&mut self, font_id: u32, text: &str, size_px: u32) -> (f32, f32) {
        self.text_renderer.measure_text(&self.device, &self.queue, font_id, text, size_px)
    }

    pub fn queue_command(&mut self, cmd: DrawCommand) {
        self.draw_queue.push(cmd);
    }

    pub fn queue_rect(&mut self, cmd: RectCommand) { self.queue_command(DrawCommand::Rect(cmd)); }
    pub fn queue_sprite(&mut self, cmd: SpriteCommand) { self.queue_command(DrawCommand::Sprite(cmd)); }
    pub fn queue_text(&mut self, cmd: TextCommand) { self.queue_command(DrawCommand::Text(cmd)); }
    /// 3D スキニングキューに追加する（draw_vrm で使用）
    pub fn queue_skinned_mesh_3d(&mut self, cmd: SkinnedMeshCommand) {
        self.mesh_3d_skinned_queue.push(cmd);
    }

    pub fn queue_skinned_mesh(&mut self, cmd: SkinnedMeshCommand) {
        self.queue_command(DrawCommand::SkinnedMesh(cmd));
    }

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
                module: &module, entry_point: "vs_main",
                buffers: &[SpriteVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module, entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: self.surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        let id = self.next_shader_id;
        self.next_shader_id += 1;
        self.custom_pipelines.insert(id, pipeline);
        Ok(id)
    }

    pub fn update_camera_3d(&mut self, view: &[f32; 16], proj: &[f32; 16]) {
        // view(16) + proj(16) + fog(8) = 40 f32 = 160 bytes
        let mut data = [0f32; 40];
        data[..16].copy_from_slice(view);
        data[16..32].copy_from_slice(proj);
        data[32] = self.fog_start;
        data[33] = self.fog_end;
        data[34] = self.fog_color[0];
        data[35] = self.fog_color[1];
        data[36] = self.fog_color[2];
        data[37] = if self.fog_enabled { 1.0 } else { 0.0 };
        // data[38..39] = padding
        self.queue.write_buffer(&self.camera_3d_buf, 0, bytemuck::cast_slice(&data));
    }

    pub fn queue_mesh_3d(&mut self, cmd: Mesh3DCommand) {
        self.mesh_3d_queue.push(cmd);
    }

    /// モーフターゲットウェイト（8スロット）を GPU に送る
    /// フォグパラメータを設定する（Python から呼ばれる）
    pub fn set_fog(&mut self, start: f32, end: f32, r: f32, g: f32, b: f32, enabled: bool) {
        self.fog_start   = start;
        self.fog_end     = end;
        self.fog_color   = [r, g, b];
        self.fog_enabled = enabled;
    }

    pub fn update_morph_weights(&mut self, weights: &[f32; 8]) {
        self.queue.write_buffer(&self.morph_weight_buffer, 0, bytemuck::cast_slice(weights));
    }

    /// 任意のバッファにデータを書き込む（プリミティブ専用ウェイトバッファ更新用）
    pub fn write_buffer(&self, buf: &wgpu::Buffer, data: &[u8]) {
        self.queue.write_buffer(buf, 0, data);
    }

    pub fn update_skin_uniforms(&mut self, matrices: &[nalgebra::Matrix4<f32>]) {
        let max = matrices.len().min(256);
        let mut data = vec![0f32; 256 * 16 + 4]; // +4 floats for screen_size vec4
        for (i, m) in matrices[..max].iter().enumerate() {
            data[i * 16..(i + 1) * 16].copy_from_slice(m.as_slice());
        }
        // screen_size: vec4<f32> を末尾に書き込み（.xy = 画面幅/高さ, .zw = 未使用）
        data[256 * 16    ] = self.screen_width as f32;
        data[256 * 16 + 1] = self.screen_height as f32;
        data[256 * 16 + 2] = 0.0;
        data[256 * 16 + 3] = 0.0;
        self.queue
            .write_buffer(&self.skinning_uniform_buffer, 0, bytemuck::cast_slice(&data));
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("KAGRA Encoder"),
        });

        let sw = self.screen_width as f32;
        let sh = self.screen_height as f32;

        let cmds = std::mem::take(&mut self.draw_queue);

        let mut rect_cmds:    Vec<RectCommand>    = Vec::new();
        let mut sprite_cmds:  Vec<SpriteCommand>  = Vec::new();
        let mut text_cmds:    Vec<TextCommand>    = Vec::new();
        let mut skinned_cmds: Vec<SkinnedMeshCommand> = Vec::new();
        let mut mesh_cmds_local: Vec<(u32, Vec<[f32;5]>, [f32;4], u32)> = Vec::new();

        for cmd in cmds {
            match cmd {
                DrawCommand::Rect(c)        => rect_cmds.push(c),
                DrawCommand::Sprite(c)      => sprite_cmds.push(c),
                DrawCommand::Text(c)        => text_cmds.push(c),
                DrawCommand::SkinnedMesh(c) => skinned_cmds.push(c),
                DrawCommand::Mesh(c)        => mesh_cmds_local.push((
                    c.texture_id, c.verts, c.shader_params, c.shader_id,
                )),
            }
        }

        let rect_verts: Vec<ColorVertex> = rect_cmds
            .iter()
            .flat_map(|cmd| {
                let x0 = (cmd.x / sw) * 2.0 - 1.0;
                let y0 = -((cmd.y / sh) * 2.0 - 1.0);
                let x1 = ((cmd.x + cmd.w) / sw) * 2.0 - 1.0;
                let y1 = -(((cmd.y + cmd.h) / sh) * 2.0 - 1.0);
                let c = cmd.color.to_f32();
                [
                    ColorVertex { position: [x0, y0], color: c },
                    ColorVertex { position: [x1, y0], color: c },
                    ColorVertex { position: [x0, y1], color: c },
                    ColorVertex { position: [x1, y0], color: c },
                    ColorVertex { position: [x1, y1], color: c },
                    ColorVertex { position: [x0, y1], color: c },
                ]
            })
            .collect();

        let rect_vbuf = if rect_verts.is_empty() {
            None
        } else {
            Some(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Rect VB"),
                contents: bytemuck::cast_slice(&rect_verts),
                usage: wgpu::BufferUsages::VERTEX,
            }))
        };

        let mut sprite_vbufs: Vec<(u32, u32, [f32;4], u32, wgpu::Buffer)> = Vec::new();
        for cmd in &sprite_cmds {
            let (tw, th) = self
                .textures
                .get(&cmd.texture_id)
                .map(|t| (t.width as f32, t.height as f32))
                .unwrap_or((1.0, 1.0));
            let u0 = cmd.sx / tw;
            let v0 = cmd.sy / th;
            let u1 = (cmd.sx + cmd.sw) / tw;
            let v1 = (cmd.sy + cmd.sh) / th;
            let (fu0, fu1) = if cmd.flip_x { (u1, u0) } else { (u0, u1) };
            let (fv0, fv1) = if cmd.flip_y { (v1, v0) } else { (v0, v1) };
            let px = cmd.dw * cmd.pivot_x;
            let py = cmd.dh * cmd.pivot_y;
            let ox = cmd.dx + px;
            let oy = cmd.dy + py;
            let rad = cmd.rotation_deg.to_radians();
            let (sin_r, cos_r) = (rad.sin(), rad.cos());
            let a = cmd.alpha;
            let z = [0.0f32; 3];
            let local = [
                (-px, -py),
                (cmd.dw - px, -py),
                (-px, cmd.dh - py),
                (cmd.dw - px, -py),
                (cmd.dw - px, cmd.dh - py),
                (-px, cmd.dh - py),
            ];
            let uvs = [
                [fu0, fv0],
                [fu1, fv0],
                [fu0, fv1],
                [fu1, fv0],
                [fu1, fv1],
                [fu0, fv1],
            ];
            let mut verts: Vec<SpriteVertex> = Vec::with_capacity(6);
            for i in 0..6 {
                let rx = local[i].0 * cos_r - local[i].1 * sin_r;
                let ry = local[i].0 * sin_r + local[i].1 * cos_r;
                verts.push(SpriteVertex {
                    position: [((ox + rx) / sw) * 2.0 - 1.0, -(((oy + ry) / sh) * 2.0 - 1.0)],
                    uv: uvs[i],
                    alpha: a,
                    _pad: z,
                });
            }
            let vb = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Sprite VB"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
            sprite_vbufs.push((cmd.texture_id, 6, cmd.shader_params, cmd.shader_id, vb));
        }

        // ── Mesh VB 生成 ──────────────────────────────────────
        let mut mesh_vbufs: Vec<(u32, u32, [f32;4], u32, wgpu::Buffer)> = Vec::new();
        for (tex_id, verts, params, shader_id) in &mesh_cmds_local {
            if verts.len() < 3 { continue; }
            let mut sv: Vec<SpriteVertex> = Vec::with_capacity(verts.len());
            for v in verts {
                let nx = (v[0] / sw) * 2.0 - 1.0;
                let ny = -((v[1] / sh) * 2.0 - 1.0);
                sv.push(SpriteVertex { position: [nx, ny], uv: [v[2], v[3]], alpha: v[4], _pad: [0.0;3] });
            }
            let vb = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Mesh VB"),
                contents: bytemuck::cast_slice(&sv),
                usage: wgpu::BufferUsages::VERTEX,
            });
            mesh_vbufs.push((*tex_id, sv.len() as u32, *params, *shader_id, vb));
        }

        let mut text_glyph_data: Vec<(u32, wgpu::Buffer, [f32; 4])> = Vec::new();
        for cmd in &text_cmds {
            let glyphs = self.text_renderer.layout_text(
                &self.device,
                &self.queue,
                cmd.font_id,
                &cmd.text,
                cmd.size_px,
                cmd.x,
                cmd.y,
            );
            let cf = cmd.color.to_f32();
            for (tex_id, gx, gy, gw, gh) in glyphs {
                let x0 = (gx / sw) * 2.0 - 1.0;
                let y0 = -((gy / sh) * 2.0 - 1.0);
                let x1 = ((gx + gw) / sw) * 2.0 - 1.0;
                let y1 = -(((gy + gh) / sh) * 2.0 - 1.0);
                let z = [0.0f32; 3];
                let a = cf[3];
                let verts = [
                    SpriteVertex { position: [x0, y0], uv: [0.0, 0.0], alpha: a, _pad: z },
                    SpriteVertex { position: [x1, y0], uv: [1.0, 0.0], alpha: a, _pad: z },
                    SpriteVertex { position: [x0, y1], uv: [0.0, 1.0], alpha: a, _pad: z },
                    SpriteVertex { position: [x1, y0], uv: [1.0, 0.0], alpha: a, _pad: z },
                    SpriteVertex { position: [x1, y1], uv: [1.0, 1.0], alpha: a, _pad: z },
                    SpriteVertex { position: [x0, y1], uv: [0.0, 1.0], alpha: a, _pad: z },
                ];
                let vb = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Text Glyph VB"),
                    contents: bytemuck::cast_slice(&verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                text_glyph_data.push((tex_id, vb, cf));
            }
        }

        {
            let cc = self.clear_color.to_f64();
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("KAGRA Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: cc[0],
                            g: cc[1],
                            b: cc[2],
                            a: cc[3],
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(vb) = &rect_vbuf {
                rp.set_pipeline(&self.rect_pipeline);
                rp.set_vertex_buffer(0, vb.slice(..));
                rp.draw(0..rect_verts.len() as u32, 0..1);
            }

            let mut cur_shader: u32 = u32::MAX;
            for (tex_id, vert_count, params, shader_id, vb) in &sprite_vbufs {
                if *shader_id != cur_shader {
                    cur_shader = *shader_id;
                    if *shader_id == 0 {
                        rp.set_pipeline(&self.sprite_pipeline);
                    } else if let Some(pl) = self.custom_pipelines.get(shader_id) {
                        self.queue.write_buffer(&self.shader_params_buffer, 0, bytemuck::cast_slice(params));
                        rp.set_pipeline(pl);
                        rp.set_bind_group(1, &self.shader_params_bg, &[]);
                    } else {
                        rp.set_pipeline(&self.sprite_pipeline);
                    }
                } else if *shader_id != 0 {
                    self.queue.write_buffer(&self.shader_params_buffer, 0, bytemuck::cast_slice(params));
                }
                if let Some(gt) = self.textures.get(tex_id) {
                    rp.set_bind_group(0, &gt.bind_group, &[]);
                    rp.set_vertex_buffer(0, vb.slice(..));
                    rp.draw(0..*vert_count, 0..1);
                }
            }
            // Mesh 描画
            let mut cur_mesh_shader: u32 = u32::MAX;
            for (tex_id, vert_count, params, shader_id, vb) in &mesh_vbufs {
                if *shader_id != cur_mesh_shader {
                    cur_mesh_shader = *shader_id;
                    if *shader_id == 0 {
                        rp.set_pipeline(&self.sprite_pipeline);
                    } else if let Some(pl) = self.custom_pipelines.get(shader_id) {
                        self.queue.write_buffer(&self.shader_params_buffer, 0, bytemuck::cast_slice(params));
                        rp.set_pipeline(pl);
                        rp.set_bind_group(1, &self.shader_params_bg, &[]);
                    } else {
                        rp.set_pipeline(&self.sprite_pipeline);
                    }
                }
                if let Some(gt) = self.textures.get(tex_id) {
                    rp.set_bind_group(0, &gt.bind_group, &[]);
                    rp.set_vertex_buffer(0, vb.slice(..));
                    rp.draw(0..*vert_count, 0..1);
                }
            }
            rp.set_pipeline(&self.sprite_pipeline);

            rp.set_pipeline(&self.skinning_pipeline);
            rp.set_bind_group(0, &self.skinning_bind_group, &[]);
            for cmd in &skinned_cmds {
                if let Some(gt) = self.textures.get(&cmd.texture_id) {
                    rp.set_bind_group(1, &gt.bind_group, &[]);
                    rp.set_vertex_buffer(0, cmd.vertex_buffer.slice(..));
                    rp.set_index_buffer(cmd.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    rp.draw_indexed(0..cmd.num_indices, 0, 0..1);
                }
            }

            rp.set_pipeline(&self.sprite_pipeline);
            for (tex_id, vb, _) in &text_glyph_data {
                if let Some(bg) = self.text_renderer.get_bind_group(*tex_id) {
                    rp.set_bind_group(0, bg, &[]);
                    rp.set_vertex_buffer(0, vb.slice(..));
                    rp.draw(0..6, 0..1);
                }
            }
        }

        // ── 3D スキニング描画パス ────────────────────────────
        let mesh_3d_skinned_cmds = std::mem::take(&mut self.mesh_3d_skinned_queue);

        // ── 3D 描画パス ──────────────────────────────────────
        let mesh_3d_cmds = std::mem::take(&mut self.mesh_3d_queue);
        if !mesh_3d_cmds.is_empty() {
            // バッファを先に確保（ライフタイム確保のため）
            let buffers_3d: Vec<(wgpu::Buffer, wgpu::Buffer)> = mesh_3d_cmds.iter()
                .map(|cmd| {
                    let vb = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("3D VB"),
                        contents: bytemuck::cast_slice(&cmd.verts),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let ib = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("3D IB"),
                        contents: bytemuck::cast_slice(&cmd.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    (vb, ib)
                })
                .collect();

            let mut enc3d = self.device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("3D Encoder") }
            );
            {
                let mut rp = enc3d.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("3D Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                rp.set_pipeline(&self.pipeline_3d);
                rp.set_bind_group(0, &self.camera_3d_bg, &[]);
                for (i, cmd) in mesh_3d_cmds.iter().enumerate() {
                    if cmd.verts.is_empty() || cmd.indices.is_empty() { continue; }
                    let (ref vb, ref ib) = buffers_3d[i];
                    if let Some(gt) = self.textures.get(&cmd.texture_id) {
                        rp.set_bind_group(1, &gt.bind_group, &[]);
                        rp.set_vertex_buffer(0, vb.slice(..));
                        rp.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                        rp.draw_indexed(0..cmd.indices.len() as u32, 0, 0..1);
                    }
                }
            }
            // 2D を先に submit → 3D を上に描画（LoadOp::Load）
            self.queue.submit(std::iter::once(encoder.finish()));
            self.queue.submit(std::iter::once(enc3d.finish()));
        } else {
            self.queue.submit(std::iter::once(encoder.finish()));
        }

        // ── 3D スキニング描画パス ────────────────────────────
        if !mesh_3d_skinned_cmds.is_empty() {
            let mut enc_s3d = self.device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("Skinning3D Encoder") });
            {
                let mut rp = enc_s3d.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Skinning3D Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                rp.set_pipeline(&self.skinning_3d_pipeline);
                rp.set_bind_group(0, &self.camera_3d_bg, &[]);
                rp.set_bind_group(1, &self.skinning_bind_group, &[]);
                for cmd in &mesh_3d_skinned_cmds {
                    if let Some(gt) = self.textures.get(&cmd.texture_id) {
                        rp.set_bind_group(2, &gt.bind_group, &[]);

                        // group(3): モーフ差分 storage + ウェイト uniform
                        // None の場合は null バインドグループを使用（ゼロ差分）
                        let morph_bg = cmd.morph_bind_group.as_deref()
                            .unwrap_or(&self.null_morph_bind_group);
                        rp.set_bind_group(3, morph_bg, &[]);

                        rp.set_vertex_buffer(0, cmd.vertex_buffer.slice(..));
                        rp.set_index_buffer(cmd.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        rp.draw_indexed(0..cmd.num_indices, 0, 0..1);
                    }
                }
            }
            self.queue.submit(std::iter::once(enc_s3d.finish()));
        }

        output.present();
        Ok(())
    }
}

const RECT_SHADER: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) color: vec4<f32> }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) color: vec4<f32> }
@vertex fn vs_main(in: VI) -> VO { return VO(vec4<f32>(in.position, 0.0, 1.0), in.color); }
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> { return in.color; }
"#;

const SPRITE_SHADER: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32 }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32 }
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@vertex fn vs_main(in: VI) -> VO { return VO(vec4<f32>(in.position, 0.0, 1.0), in.uv, in.alpha); }
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    c.a *= in.alpha;
    return c;
}
"#;

const SHADER_GRAYSCALE: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32 }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32 }
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(1) @binding(0) var<uniform> params: vec4<f32>;
@vertex fn vs_main(in: VI) -> VO { return VO(vec4<f32>(in.position, 0.0, 1.0), in.uv, in.alpha); }
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    let gray = dot(c.rgb, vec3<f32>(0.299, 0.587, 0.114));
    c = vec4<f32>(mix(c.rgb, vec3<f32>(gray), params.x), c.a * in.alpha);
    return c;
}
"#;

const SHADER_FLASH: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32 }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32 }
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(1) @binding(0) var<uniform> params: vec4<f32>;
@vertex fn vs_main(in: VI) -> VO { return VO(vec4<f32>(in.position, 0.0, 1.0), in.uv, in.alpha); }
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    let flash = vec3<f32>(
        select(1.0, params.y, params.y > 0.0),
        select(1.0, params.z, params.z > 0.0),
        select(1.0, params.w, params.w > 0.0)
    );
    c = vec4<f32>(mix(c.rgb, flash, params.x), c.a * in.alpha);
    return c;
}
"#;

const SHADER_SPOTLIGHT: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32 }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32, @location(2) screen_pos: vec2<f32> }
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(1) @binding(0) var<uniform> params: vec4<f32>;
@vertex fn vs_main(in: VI) -> VO {
    var out: VO;
    out.pos = vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.uv; out.alpha = in.alpha;
    out.screen_pos = in.position * 0.5 + 0.5;
    return out;
}
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    let spot = vec2<f32>(params.x, 1.0 - params.y);
    let dist = distance(in.screen_pos, spot);
    let falloff = 1.0 - smoothstep(params.z * 0.5, params.z, dist);
    let light = 0.15 + falloff * params.w;
    c = vec4<f32>(c.rgb * clamp(light, 0.0, 1.5), c.a * in.alpha);
    return c;
}
"#;

const SHADER_GLOW: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32 }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32 }
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(1) @binding(0) var<uniform> params: vec4<f32>;
@vertex fn vs_main(in: VI) -> VO { return VO(vec4<f32>(in.position, 0.0, 1.0), in.uv, in.alpha); }
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    let edge = min(min(in.uv.x, 1.0 - in.uv.x), min(in.uv.y, 1.0 - in.uv.y));
    let rim = (1.0 - smoothstep(0.0, 0.12, edge)) * params.w;
    c = vec4<f32>(c.rgb + vec3<f32>(params.x, params.y, params.z) * rim, c.a * in.alpha);
    return c;
}
"#;

const SHADER_TINT: &str = r#"
struct VI { @location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) alpha: f32 }
struct VO { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) alpha: f32 }
@group(0) @binding(0) var t_diffuse: texture_2d<f32>;
@group(0) @binding(1) var s_diffuse: sampler;
@group(1) @binding(0) var<uniform> params: vec4<f32>;
@vertex fn vs_main(in: VI) -> VO { return VO(vec4<f32>(in.position, 0.0, 1.0), in.uv, in.alpha); }
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    c = vec4<f32>(mix(c.rgb, c.rgb * vec3<f32>(params.x, params.y, params.z), params.w), c.a * in.alpha);
    return c;
}
"#;

const SKINNING_SHADER: &str = r#"
struct Uniforms {
    bone_matrices: array<mat4x4<f32>, 256>,
    screen_size: vec4<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(1) @binding(0) var texture: texture_2d<f32>;
@group(1) @binding(1) var tex_sampler: sampler;
struct VI { @location(0) position: vec3<f32>, @location(1) uv: vec2<f32>,
            @location(2) joints: vec4<u32>, @location(3) weights: vec4<f32> }
struct VO { @builtin(position) clip_position: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs_main(in: VI) -> VO {
    var m = mat4x4<f32>(vec4(0.0),vec4(0.0),vec4(0.0),vec4(0.0));
    m += uniforms.bone_matrices[in.joints[0]] * in.weights[0];
    m += uniforms.bone_matrices[in.joints[1]] * in.weights[1];
    m += uniforms.bone_matrices[in.joints[2]] * in.weights[2];
    m += uniforms.bone_matrices[in.joints[3]] * in.weights[3];
    let wp = m * vec4<f32>(in.position, 1.0);
    let half_w = uniforms.screen_size.x * 0.5;
    let half_h = uniforms.screen_size.y * 0.5;
    var out: VO;
    out.clip_position = vec4<f32>((wp.x/half_w)-1.0, -((wp.y/half_h)-1.0), wp.z/1000.0, 1.0);
    out.uv = in.uv;
    return out;
}
@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    return textureSample(texture, tex_sampler, in.uv);
}
"#;


const SHADER_3D: &str = r#"
struct Camera {
    view:      mat4x4<f32>,
    proj:      mat4x4<f32>,
    fog_start: f32, fog_end: f32,
    fog_r: f32, fog_g: f32, fog_b: f32, fog_on: f32,
    _pad0: f32, _pad1: f32,
};
@group(0) @binding(0) var<uniform> cam: Camera;
@group(1) @binding(0) var t_diffuse: texture_2d<f32>;
@group(1) @binding(1) var s_diffuse: sampler;

struct VI {
    @location(0) position: vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @location(2) uv:       vec2<f32>,
}
struct VO {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv:    vec2<f32>,
    @location(1) light: f32,
    @location(2) depth: f32,   // フォグ用カメラ距離
}

@vertex fn vs_main(in: VI) -> VO {
    var out: VO;
    let pos4     = vec4<f32>(in.position, 1.0);
    let view_pos = transpose(cam.view) * pos4;
    out.clip_pos = transpose(cam.proj) * view_pos;
    out.uv       = in.uv;
    let light_dir = normalize(vec3<f32>(0.3, 1.0, 0.5));
    out.light    = clamp(dot(normalize(in.normal), light_dir), 0.2, 1.0);
    out.depth    = -view_pos.z;   // カメラからの距離（正値）
    return out;
}

@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    if c.a < 0.01 { discard; }
    var col = c.rgb * in.light;
    // リニアフォグ
    if cam.fog_on > 0.5 {
        let fog_f = clamp((in.depth - cam.fog_start) / max(cam.fog_end - cam.fog_start, 0.001),
                          0.0, 1.0);
        let fog_c = vec3<f32>(cam.fog_r, cam.fog_g, cam.fog_b);
        col = mix(col, fog_c, fog_f);
    }
    return vec4<f32>(col, c.a);
}
"#;

const SHADER_SKINNING_3D: &str = r#"
// 3D スキニング + モーフターゲット シェーダー
// group(0): Camera (View + Projection, row-major)
// group(1): BoneMatrices (256本) + screen_size
// group(2): Texture
// group(3): MorphDeltas (storage) + MorphWeights (uniform, 8スロット)

struct Camera {
    view:      mat4x4<f32>,
    proj:      mat4x4<f32>,
    fog_start: f32, fog_end: f32,
    fog_r: f32, fog_g: f32, fog_b: f32, fog_on: f32,
    _pad0: f32, _pad1: f32,
};
@group(0) @binding(0) var<uniform> cam: Camera;

struct SkinUniforms {
    bone_matrices: array<mat4x4<f32>, 256>,
    screen_size:   vec4<f32>,
};
@group(1) @binding(0) var<uniform> skin: SkinUniforms;

@group(2) @binding(0) var t_diffuse: texture_2d<f32>;
@group(2) @binding(1) var s_diffuse: sampler;

// group(3): モーフターゲット
// morph_deltas: [target0_v0(xyz pad), target0_v1, ..., target1_v0, ...]
// morph_weights: 8スロットのウェイト
@group(3) @binding(0) var<storage, read> morph_deltas:  array<vec4<f32>>;
@group(3) @binding(1) var<uniform>       morph_weights: array<vec4<f32>, 2>; // vec4 x 2 = 8 floats

struct VI {
    @builtin(vertex_index) vid:    u32,
    @location(0) position: vec3<f32>,
    @location(1) uv:       vec2<f32>,
    @location(2) joints:   vec4<u32>,
    @location(3) weights:  vec4<f32>,
}
struct VO {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv:    vec2<f32>,
    @location(1) light: f32,
    @location(2) depth: f32,
}

@vertex fn vs_main(in: VI) -> VO {
    // ── 1. モーフ差分を合成（スキニング前）────────────────────
    // WGSL ではループ変数で array<f32,N> をインデックスできないため8スロット展開
    var morph_offset = vec3<f32>(0.0);
    let n = arrayLength(&morph_deltas);

    if n > 8u {
        let vc = n / 8u;   // 1スロット分の頂点数
        let mw0 = morph_weights[0];  // slot 0-3
        let mw1 = morph_weights[1];  // slot 4-7

        if mw0.x > 0.0001 { let i0 = 0u*vc+in.vid; if i0<n { morph_offset += morph_deltas[i0].xyz * mw0.x; } }
        if mw0.y > 0.0001 { let i1 = 1u*vc+in.vid; if i1<n { morph_offset += morph_deltas[i1].xyz * mw0.y; } }
        if mw0.z > 0.0001 { let i2 = 2u*vc+in.vid; if i2<n { morph_offset += morph_deltas[i2].xyz * mw0.z; } }
        if mw0.w > 0.0001 { let i3 = 3u*vc+in.vid; if i3<n { morph_offset += morph_deltas[i3].xyz * mw0.w; } }
        if mw1.x > 0.0001 { let i4 = 4u*vc+in.vid; if i4<n { morph_offset += morph_deltas[i4].xyz * mw1.x; } }
        if mw1.y > 0.0001 { let i5 = 5u*vc+in.vid; if i5<n { morph_offset += morph_deltas[i5].xyz * mw1.y; } }
        if mw1.z > 0.0001 { let i6 = 6u*vc+in.vid; if i6<n { morph_offset += morph_deltas[i6].xyz * mw1.z; } }
        if mw1.w > 0.0001 { let i7 = 7u*vc+in.vid; if i7<n { morph_offset += morph_deltas[i7].xyz * mw1.w; } }
    }

    let morphed_pos = in.position + morph_offset;

    // ── 2. スキニング ─────────────────────────────────────────
    var m = mat4x4<f32>(
        vec4<f32>(0.0,0.0,0.0,0.0), vec4<f32>(0.0,0.0,0.0,0.0),
        vec4<f32>(0.0,0.0,0.0,0.0), vec4<f32>(0.0,0.0,0.0,0.0)
    );
    m += skin.bone_matrices[in.joints[0]] * in.weights[0];
    m += skin.bone_matrices[in.joints[1]] * in.weights[1];
    m += skin.bone_matrices[in.joints[2]] * in.weights[2];
    m += skin.bone_matrices[in.joints[3]] * in.weights[3];

    let world_pos = m * vec4<f32>(morphed_pos, 1.0);

    // ── 3. カメラ変換（行優先）───────────────────────────────
    let view_pos = transpose(cam.view) * world_pos;
    let clip_pos = transpose(cam.proj) * view_pos;

    // 簡易ライティング（モーフ後の法線は近似）
    let world_nrm = normalize((m * vec4<f32>(0.0, 1.0, 0.0, 0.0)).xyz);
    let light_dir = normalize(vec3<f32>(0.3, 1.0, 0.5));
    let light = clamp(dot(world_nrm, light_dir), 0.25, 1.0);

    let view_pos_s = transpose(cam.view) * world_pos;
    var out: VO;
    out.clip_pos = clip_pos;
    out.uv       = in.uv;
    out.light    = light;
    out.depth    = -view_pos_s.z;
    return out;
}

@fragment fn fs_main(in: VO) -> @location(0) vec4<f32> {
    var c = textureSample(t_diffuse, s_diffuse, in.uv);
    if c.a < 0.05 { discard; }
    var col = c.rgb * in.light;
    if cam.fog_on > 0.5 {
        let fog_f = clamp((in.depth - cam.fog_start) / max(cam.fog_end - cam.fog_start, 0.001),
                          0.0, 1.0);
        col = mix(col, vec3<f32>(cam.fog_r, cam.fog_g, cam.fog_b), fog_f);
    }
    return vec4<f32>(col, c.a);
}
"#;
