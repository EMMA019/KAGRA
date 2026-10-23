//! wgpu による描画（feature = "render"）。
//!
//! 3D パス（`scene3d::Scene3D`）を深度付きで描いてから、2D パス
//! （`scene::DrawList`）を HUD として上に重ねる。両方とも同じレンダーパスで、
//! 2D 側は深度テストを常に通して書き込まない。Android / iOS / Web /
//! オフスクリーンで同じコードを通す。
//!
//! WebGL2 でも動く範囲に収めてある。ストレージバッファを使わず、インスタンスは
//! 頂点バッファで渡し、base instance には頼らない（GLES に無いため）。

mod bloom;
mod color_space;
mod post;
mod target;

pub use target::SurfaceSource;

use crate::scene::{DrawList, ImageQuad};
use crate::scene3d::{MeshData, MeshId, Scene3D};
use crate::world_doc::load_png_albedo;
use bloom::BloomPass;
use post::PostGraph;

const MAX_TEXTURE_SIDE: u32 = 8192;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    color: [f32; 4],
}

/// 3D シェーダの `Globals` と同じ並び。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    light: [f32; 4],
    fog_color: [f32; 4],
    fog_range: [f32; 4],
    camera_pos: [f32; 4],
    /// x = IBL, y = exposure, z = elapsed seconds (water scroll), w = ACES (>0.5 on). Matches shader Globals.env.
    env: [f32; 4],
    light_pos: [[f32; 4]; 4],
    light_col: [[f32; 4]; 4],
    light_dir: [[f32; 4]; 4],
    light_view_proj: [[f32; 4]; 4],
    /// Cascades 1 and 2. Count 1 repeats the historical matrix in both slots.
    cascade1: [[f32; 4]; 4],
    cascade2: [[f32; 4]; 4],
    /// x = shadow radius. 1 = historical 3×3 PCF.
    /// y > 0.5: this pass writes an unorm swapchain, so `present_rgb` grades it.
    /// z = camera near, w = camera far (depth prepass only).
    shadow: [f32; 4],
    /// x = cascade count (1, 2, or 3). y > 0.5 enables the PCSS blocker search.
    /// z > 1 stores eye/z in the depth color for unorm SSAO. w is unused.
    cascade: [f32; 4],
    hemi_sky: [f32; 4],
    /// w > 0.5 samples the prefiltered env strip.
    hemi_ground: [f32; 4],
}

/// インスタンスごとの頂点属性（location 2..7, 9..11）。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    model: [[f32; 4]; 4],
    color: [f32; 4],
    material: f32,
    _pad: [f32; 3],
    /// Location 9: rgb = MToon shadeColor, a = shadingToony. Zeros if not toon.
    mtoon: [f32; 4],
    /// Location 10: rgb = rimColor, a = rimFresnelPower.
    mtoon2: [f32; 4],
    /// Location 11: rgb = outlineColor, a = outlineWidth.
    mtoon3: [f32; 4],
    /// Location 12: x = shadingShift, y = rimLift.
    mtoon4: [f32; 4],
    /// Location 15. 0 = legacy cutout, >0 = MASK, -1 = OPAQUE, <= -1.5 = BLEND.
    alpha_cut: f32,
    _pad_cut: [f32; 3],
}

#[derive(Debug)]
struct GpuMesh {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    index_count: u32,
    albedo_bind: Option<wgpu::BindGroup>,
    _albedo_tex: Option<wgpu::Texture>,
    _matcap_tex: Option<wgpu::Texture>,
    _normal_tex: Option<wgpu::Texture>,
    _orm_tex: Option<wgpu::Texture>,
    _emissive_tex: Option<wgpu::Texture>,
    sample_rough: bool,
    sample_metal: bool,
    sample_emissive: bool,
    mtoon: [f32; 4],
    mtoon2: [f32; 4],
    mtoon3: [f32; 4],
    mtoon4: [f32; 4],
    alpha_cut: f32,
    double_sided: bool,
    has_normal: bool,
    normal_scale: f32,
    emissive: f32,
    emissive_color: [f32; 3],
    clearcoat: f32,
    clearcoat_roughness: f32,
    _sampler: Option<wgpu::Sampler>,
    /// Authored image bytes (albedo, matcap, normal, ORM, emissive).
    tex_bytes: u64,
}

#[derive(Debug)]
enum Target {
    Surface {
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
    },
    Offscreen {
        texture: wgpu::Texture,
    },
}

#[derive(Debug)]
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    target: Target,
    depth: wgpu::TextureView,

    pipeline: wgpu::RenderPipeline,
    screen_buf: wgpu::Buffer,
    screen_bind: wgpu::BindGroup,
    vbuf: wgpu::Buffer,
    vbuf_capacity: usize,

    pipeline3d: wgpu::RenderPipeline,
    /// Color target of linear eye depth. Drawn only when a post pass samples it.
    pipeline_depth: wgpu::RenderPipeline,
    /// glTF doubleSided. Same shader, cull disabled.
    pipeline_nocull: wgpu::RenderPipeline,
    /// 天球用。深度は書いて読まず、裏面カリングもしない。
    pipeline_sky: wgpu::RenderPipeline,
    /// MToon アウトライン（backface push-out、カリング Front）。
    pipeline_outline: wgpu::RenderPipeline,
    /// Depth-only sun map. One pipeline per cascade entry (`vs_shadow`, `_1`, `_2`).
    pipeline_shadow: [wgpu::RenderPipeline; 3],
    /// Double-sided casters. Same entries, cull off.
    pipeline_shadow_nocull: [wgpu::RenderPipeline; 3],
    /// Same draws, plus a color target so PCSS can read device z.
    pipeline_shadow_pcss: [wgpu::RenderPipeline; 3],
    pipeline_shadow_pcss_nocull: [wgpu::RenderPipeline; 3],
    globals_buf: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    _shadow_tex: wgpu::Texture,
    shadow_view: wgpu::TextureView,
    _shadow_raw: wgpu::Texture,
    shadow_raw_view: wgpu::TextureView,
    shadow_bind: wgpu::BindGroup,
    inst_buf: wgpu::Buffer,
    inst_capacity: usize,
    meshes: Vec<GpuMesh>,
    albedo_layout: wgpu::BindGroupLayout,
    /// Kept alive for the default 1×1 bind group. Per-mesh samplers live on `GpuMesh`.
    _albedo_sampler: wgpu::Sampler,
    default_albedo_bind: wgpu::BindGroup,
    _white_albedo: wgpu::Texture,
    _white_view: wgpu::TextureView,
    _black_view: wgpu::TextureView,
    _flat_view: wgpu::TextureView,
    _orm_default: wgpu::Texture,
    _orm_view: wgpu::TextureView,
    env_tex: wgpu::Texture,
    env_bind: wgpu::BindGroup,
    /// Seconds for shader env.z (water scroll). Frame accum, not Instant (wasm).
    elapsed: f32,
    /// Threshold bloom: linear-HDR frame → extract/blur → composite → target.
    bloom: BloomPass,
    /// SSAO / DOF / motion / TAA. Off unless a dump or `set_post` enables them.
    post: PostGraph,
    /// TextQuad → カバレッジ Quad のキャッシュ。GPU 状態を持たない。
    text: crate::font::TextRaster,
    /// walker glTF spec → SpringBone 布シミュ（フレーム跨ぎで保持）。
    cloth: std::collections::HashMap<String, crate::spring::SpringState>,
    /// Which `WorldDoc` mesh table is on the GPU. A new dump reuploads.
    mesh_key: String,
    /// Last `render_frame`. CPU time is filled off wasm. GPU time only when
    /// the device exposes timestamp queries.
    last_profile: crate::profile::FrameProfile,
    timestamps: Option<TimestampClock>,
    /// Adapter backend name (`webgpu`, `gl`, `vulkan`, …).
    backend_name: String,
    /// Dump `quality` from the last world draw. Empty until a dump arrives.
    picture_tier: String,
    /// One line for the page: tier, backend, fxaa, ssao, cascades, path.
    picture_line: String,
    /// Instanced point sprites and fat lines. Absent dumps draw nothing.
    strokes: StrokeGpu,
}

#[derive(Clone, Debug)]
struct TimestampClock {
    query: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    staging: wgpu::Buffer,
}

impl Renderer {
    /// 画面付き。`source` のハンドルはレンダラより長生きさせること。
    pub async fn new_for_surface(
        source: SurfaceSource,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let instance = new_instance().await;
        let surface = source.create(&instance)?;
        Self::from_surface(instance, surface, width, height).await
    }

    /// Desktop winit (or any window that wgpu can present to). Same wgpu 30
    /// `Renderer` as Android / iOS / canvas / offscreen. Not kagra-core
    /// `RendererV2`. The instance gets the window's display handle so GLES /
    /// X11 can present (Vulkan ignores it).
    ///
    /// Desktop-only. wgpu's `WgpuHasDisplayHandle` is `Send + Sync`; wasm
    /// canvas/`JsValue` handles are not. Wasm stays on `new_for_surface`.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new_for_window(
        window: impl wgpu::DisplayAndWindowHandle + Clone + std::fmt::Debug + 'static,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let mut desc = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(window.clone()));
        desc.backends = wgpu::Backends::all();
        let instance = wgpu::Instance::new(desc);
        let surface = instance.create_surface(window).map_err(|e| e.to_string())?;
        Self::from_surface(instance, surface, width, height).await
    }

    async fn from_surface(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let (adapter, device, queue) = request_device(&instance, Some(&surface)).await?;

        // present_mode / alpha_mode / color_space の妥当な組み合わせは
        // バックエンド依存なので、既定構成をもらってから必要な所だけ直す。
        let mut config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .ok_or("surface is not supported by this adapter")?;
        let caps = surface.get_capabilities(&adapter);
        // 8-bit only. An fp16 canvas is an HDR surface: Chrome tone-maps it and
        // a 0..1 picture collapses to black on a phone. WebGPU also has no
        // sRGB canvas format, so prefer sRGB when a backend offers it and
        // encode in the shader otherwise.
        config.format =
            choose_surface_format(&caps.formats).ok_or("surface advertises no color format")?;
        if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            // PreMultiplied + a clear alpha of 0 composites as "nothing" and
            // the page looks black even though the pass ran.
            config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
        }
        // WebGL2 canvases only allow RENDER_ATTACHMENT. Asking for COPY_DST
        // makes configure fail, and the next get_current_texture aborts.
        config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        if caps.usages.contains(wgpu::TextureUsages::COPY_DST) {
            config.usage |= wgpu::TextureUsages::COPY_DST;
        }
        surface.configure(&device, &config);
        // Do not call get_current_texture here. On WebGPU the canvas allows one
        // current texture per task; acquiring it and dropping it makes every
        // later present Lost, which is a black canvas with no exception.

        let format = config.format;
        let (width, height) = (config.width, config.height);
        note_surface(&adapter, &config);
        let hdr = hdr_for_adapter(&adapter);
        let backend = adapter.get_info().backend.to_str().to_string();
        Ok(Self::assemble(
            device,
            queue,
            format,
            width,
            height,
            Target::Surface { surface, config },
            hdr,
            backend,
        ))
    }

    /// 画面なし。テストやスクリーンショット用。
    pub async fn new_offscreen(width: u32, height: u32) -> Result<Self, String> {
        Self::new_offscreen_format(width, height, wgpu::TextureFormat::Rgba8UnormSrgb).await
    }

    /// Phone WebGPU canvas: an 8-bit frame and an 8-bit target, even when the
    /// adapter can filter `Rgba16Float`.
    pub async fn new_phone_offscreen(width: u32, height: u32) -> Result<Self, String> {
        let instance = new_instance().await;
        let (adapter, device, queue) = request_device(&instance, None).await?;
        let (width, height) = (
            width.clamp(1, MAX_TEXTURE_SIDE),
            height.clamp(1, MAX_TEXTURE_SIDE),
        );
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = create_offscreen_texture(&device, format, width, height);
        let backend = adapter.get_info().backend.to_str().to_string();
        Ok(Self::assemble(
            device,
            queue,
            format,
            width,
            height,
            Target::Offscreen { texture },
            false,
            backend,
        ))
    }

    /// Offscreen target of an explicit format. sRGB encodes on store. Unorm
    /// encodes in the composite shader. Tests use both to lock one OETF.
    async fn new_offscreen_format(
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        let instance = new_instance().await;
        let (adapter, device, queue) = request_device(&instance, None).await?;
        let (width, height) = (
            width.clamp(1, MAX_TEXTURE_SIDE),
            height.clamp(1, MAX_TEXTURE_SIDE),
        );
        let texture = create_offscreen_texture(&device, format, width, height);
        let hdr = hdr_for_adapter(&adapter);
        let backend = adapter.get_info().backend.to_str().to_string();
        Ok(Self::assemble(
            device,
            queue,
            format,
            width,
            height,
            Target::Offscreen { texture },
            hdr,
            backend,
        ))
    }

    /// Phone WebGPU canvas: an 8-bit frame and an 8-bit target, even when the
    /// adapter can filter `Rgba16Float`. Tests use this to lock the direct path.
    #[cfg(test)]
    async fn new_ldr_unorm(width: u32, height: u32) -> Result<Self, String> {
        let instance = new_instance().await;
        let (adapter, device, queue) = request_device(&instance, None).await?;
        let (width, height) = (
            width.clamp(1, MAX_TEXTURE_SIDE),
            height.clamp(1, MAX_TEXTURE_SIDE),
        );
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = create_offscreen_texture(&device, format, width, height);
        let backend = adapter.get_info().backend.to_str().to_string();
        Ok(Self::assemble(
            device,
            queue,
            format,
            width,
            height,
            Target::Offscreen { texture },
            false,
            backend,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn assemble(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        target: Target,
        hdr: bool,
        backend: String,
    ) -> Self {
        if !hdr {
            note_ldr_fallback();
        }
        let frame_format = if hdr {
            bloom::HDR_FRAME_FORMAT
        } else {
            bloom::LDR_FRAME_FORMAT
        };
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kagra-shared 2d"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let screen_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kagra-shared screen"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kagra-shared screen layout"),
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

        let screen_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kagra-shared screen bind"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_buf.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kagra-shared 2d layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });

        // HUD は 3D の上に無条件で乗せる。深度は読むだけで書かない。
        let hud_depth = wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: Default::default(),
            bias: Default::default(),
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kagra-shared 2d pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(hud_depth),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let vbuf_capacity = 6 * 256;
        let vbuf = create_vertex_buffer(&device, vbuf_capacity);

        // ---- 3D ----
        let shader3d = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kagra-shared 3d"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader3d.wgsl").into()),
        });

        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kagra-shared globals"),
            size: std::mem::size_of::<Globals>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kagra-shared globals layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kagra-shared globals bind"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buf.as_entire_binding(),
            }],
        });

        let albedo_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kagra-shared tex layout"),
            entries: &[
                // 0/1 albedo, 2/3 matcap, 4/5 normal. 無いメッシュはデフォルト白/黒/フラット。
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // 6/7 linear ORM, 8/9 emissive. Defaults until an instance flag samples them.
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 8,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 9,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let albedo_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kagra-shared albedo sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let white_albedo = create_albedo_texture(&device, &queue, 1, 1, &[255, 255, 255, 255]);
        let white_view = white_albedo.create_view(&Default::default());
        let black_matcap = create_albedo_texture(&device, &queue, 1, 1, &[0, 0, 0, 255]);
        let black_view = black_matcap.create_view(&Default::default());
        // Flat tangent normal is data (128 → 0.5). An sRGB decode would tilt it.
        let flat_normal = create_linear_texture(&device, &queue, 1, 1, &[128, 128, 255, 255]);
        let flat_view = flat_normal.create_view(&Default::default());
        let orm_default = create_linear_texture(&device, &queue, 1, 1, &[255, 255, 0, 255]);
        let orm_view = orm_default.create_view(&Default::default());
        let default_albedo_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kagra-shared default tex"),
            layout: &albedo_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&white_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&albedo_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&black_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&albedo_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&flat_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&albedo_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&orm_view),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::Sampler(&albedo_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::TextureView(&black_view),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::Sampler(&albedo_sampler),
                },
            ],
        });

        let (shadow_tex, shadow_view) = create_shadow_map(&device);
        let (shadow_raw, shadow_raw_view) = create_shadow_raw(&device);
        // Nearest compare. depth32float linear filtering is the WebGPU
        // feature `depth32float-filterable`, which wgpu 30 does not expose.
        // A linear compare sampler is valid on lavapipe and invalid on
        // Pixel-class WebGPU, where the taps come back as black/white bands.
        // The shader PCF (textureSampleCompareLevel) is the soft edge.
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kagra-shared shadow compare"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        // Nearest. A linear sample of this Rgba16Float copy is
        // `float32-filterable`, which the phone WebGPU path does not require.
        // PCSS is opt-in and reads explicit LOD 0.
        let shadow_raw_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kagra-shared shadow raw"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kagra-shared shadow layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shadow_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kagra-shared shadow bind"),
            layout: &shadow_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&shadow_raw_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&shadow_raw_sampler),
                },
            ],
        });

        let env_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kagra-shared env layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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
        });
        let env_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kagra-shared env sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let env_px = vec![128u8; 64 * 64 * 4];
        let env_tex = create_linear_texture(&device, &queue, 64, 64, &env_px);
        let env_view = env_tex.create_view(&Default::default());
        let env_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kagra-shared env bind"),
            layout: &env_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&env_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&env_sampler),
                },
            ],
        });

        let layout3d = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kagra-shared 3d layout"),
            bind_group_layouts: &[
                Some(&globals_layout),
                Some(&albedo_layout),
                Some(&shadow_layout),
                Some(&env_layout),
            ],
            immediate_size: 0,
        });
        let layout_shadow = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kagra-shared shadow layout"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });

        // vertex_attr_array! の一時配列を名前付きに固定しないと、layout の
        // attributes 参照が文の終わりで死ぬ。
        // UV is location 8 so instance locations 2..7 stay put (WebGL2 / no base_instance).
        // Location 9 is MToon shade+toony at offset 96 (16-byte aligned).
        // 13 = linear vertex color, 14 = tangent. Offsets follow Vertex3 (64 bytes).
        let mesh_attrs = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            8 => Float32x2,
            13 => Float32x4,
            14 => Float32x4,
        ];
        let inst_attrs = [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 0,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 4,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 48,
                shader_location: 5,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 64,
                shader_location: 6,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 80,
                shader_location: 7,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 96,
                shader_location: 9,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 112,
                shader_location: 10,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 128,
                shader_location: 11,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 144,
                shader_location: 12,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 160,
                shader_location: 15,
            },
        ];
        let vertex_buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<crate::scene3d::Vertex3>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &mesh_attrs,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &inst_attrs,
            }),
        ];

        // 3D は線形フレームへ（トーンは bloom composite が適用）。
        // 2 枚目はエミッシブマスク。8bit ブルームが読む。HDR 抽出は無視する。
        // フォーマットはフレームと揃える（WebGL2 の MRT は混在に弱い）。
        let scene_target = wgpu::ColorTargetState {
            format: frame_format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        };
        let hdr_target = [Some(scene_target.clone()), Some(scene_target)];

        let pipeline3d = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kagra-shared 3d pipeline"),
            layout: Some(&layout3d),
            vertex: wgpu::VertexState {
                module: &shader3d,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &vertex_buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader3d,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &hdr_target,
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let pipeline_nocull = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kagra-shared 3d pipeline nocull"),
            layout: Some(&layout3d),
            vertex: wgpu::VertexState {
                module: &shader3d,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &vertex_buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader3d,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &hdr_target,
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let depth_color_target = [Some(wgpu::ColorTargetState {
            format: frame_format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let pipeline_depth = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kagra-shared depth prepass"),
            layout: Some(&layout3d),
            vertex: wgpu::VertexState {
                module: &shader3d,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &vertex_buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader3d,
                entry_point: Some("fs_depth"),
                compilation_options: Default::default(),
                targets: &depth_color_target,
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let pipeline_sky = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kagra-shared sky pipeline"),
            layout: Some(&layout3d),
            vertex: wgpu::VertexState {
                module: &shader3d,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &vertex_buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader3d,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &hdr_target,
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let pipeline_outline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kagra-shared outline pipeline"),
            layout: Some(&layout3d),
            vertex: wgpu::VertexState {
                module: &shader3d,
                entry_point: Some("vs_outline"),
                compilation_options: Default::default(),
                buffers: &vertex_buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader3d,
                entry_point: Some("fs_outline"),
                compilation_options: Default::default(),
                targets: &hdr_target,
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let raw_target = [Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba16Float,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let shadow_depth = wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            // depth32float: 1 constant unit is ~2^-23. 8192 units is
            // about 0.001 of the depth range, which is the bias a 24-bit
            // map gets from a constant of 2. Clamp so a steep caster
            // cannot shove its shadow off the receiver.
            bias: wgpu::DepthBiasState {
                constant: 8192,
                slope_scale: 1.5,
                clamp: 0.008,
            },
        };
        let shadow_vs = ["vs_shadow", "vs_shadow_1", "vs_shadow_2"];
        let mut depth_only = Vec::with_capacity(3);
        let mut depth_nocull = Vec::with_capacity(3);
        let mut with_raw = Vec::with_capacity(3);
        let mut raw_nocull = Vec::with_capacity(3);
        for entry in shadow_vs {
            let vertex = wgpu::VertexState {
                module: &shader3d,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                buffers: &vertex_buffers,
            };
            let culled = wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            };
            let open = wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            };
            let raw_fs = wgpu::FragmentState {
                module: &shader3d,
                entry_point: Some("fs_shadow_depth"),
                compilation_options: Default::default(),
                targets: &raw_target,
            };
            depth_only.push(
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("kagra-shared shadow pipeline"),
                    layout: Some(&layout_shadow),
                    vertex: vertex.clone(),
                    fragment: None,
                    primitive: culled,
                    depth_stencil: Some(shadow_depth.clone()),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                }),
            );
            depth_nocull.push(
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("kagra-shared shadow pipeline nocull"),
                    layout: Some(&layout_shadow),
                    vertex: vertex.clone(),
                    fragment: None,
                    primitive: open,
                    depth_stencil: Some(shadow_depth.clone()),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                }),
            );
            with_raw.push(
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("kagra-shared shadow raw pipeline"),
                    layout: Some(&layout_shadow),
                    vertex: vertex.clone(),
                    fragment: Some(raw_fs.clone()),
                    primitive: culled,
                    depth_stencil: Some(shadow_depth.clone()),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                }),
            );
            raw_nocull.push(
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("kagra-shared shadow raw pipeline nocull"),
                    layout: Some(&layout_shadow),
                    vertex,
                    fragment: Some(raw_fs),
                    primitive: open,
                    depth_stencil: Some(shadow_depth.clone()),
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                }),
            );
        }
        let pipeline_shadow: [wgpu::RenderPipeline; 3] = depth_only
            .try_into()
            .unwrap_or_else(|_| unreachable!("three shadow pipelines"));
        let pipeline_shadow_nocull: [wgpu::RenderPipeline; 3] = depth_nocull
            .try_into()
            .unwrap_or_else(|_| unreachable!("three nocull shadow pipelines"));
        let pipeline_shadow_pcss: [wgpu::RenderPipeline; 3] = with_raw
            .try_into()
            .unwrap_or_else(|_| unreachable!("three pcss pipelines"));
        let pipeline_shadow_pcss_nocull: [wgpu::RenderPipeline; 3] = raw_nocull
            .try_into()
            .unwrap_or_else(|_| unreachable!("three nocull pcss pipelines"));

        let inst_capacity = 256;
        let inst_buf = create_instance_buffer(&device, inst_capacity);
        let depth = create_depth_view(&device, width, height);
        let bloom = BloomPass::new(&device, format, width, height, hdr);
        let timestamps = create_timestamps(&device);
        let post = PostGraph::new(&device, format, bloom.frame_format(), width, height);
        let strokes = create_stroke_gpu(&device, &queue, frame_format);

        let me = Self {
            device,
            queue,
            format,
            width,
            height,
            target,
            depth,
            pipeline,
            screen_buf,
            screen_bind,
            vbuf,
            vbuf_capacity,
            pipeline3d,
            pipeline_depth,
            pipeline_nocull,
            pipeline_sky,
            pipeline_outline,
            pipeline_shadow,
            pipeline_shadow_nocull,
            pipeline_shadow_pcss,
            pipeline_shadow_pcss_nocull,
            globals_buf,
            globals_bind,
            _shadow_tex: shadow_tex,
            shadow_view,
            _shadow_raw: shadow_raw,
            shadow_raw_view,
            shadow_bind,
            inst_buf,
            inst_capacity,
            meshes: Vec::new(),
            albedo_layout,
            _albedo_sampler: albedo_sampler,
            default_albedo_bind,
            _white_albedo: white_albedo,
            _white_view: white_view,
            _black_view: black_view,
            _flat_view: flat_view,
            _orm_default: orm_default,
            _orm_view: orm_view,
            env_tex,
            env_bind,
            elapsed: 0.0,
            bloom,
            post,
            text: crate::font::TextRaster::new(),
            cloth: std::collections::HashMap::new(),
            mesh_key: String::new(),
            last_profile: crate::profile::FrameProfile::default(),
            timestamps,
            backend_name: backend,
            picture_tier: String::new(),
            picture_line: String::new(),
            strokes,
        };
        me.upload_screen();
        me
    }

    /// メッシュを GPU に載せる。`MeshId` は登録順の連番。
    pub fn upload_mesh(&mut self, mesh: &MeshData) -> MeshId {
        let vbuf = create_init_buffer(
            &self.device,
            &self.queue,
            "kagra-shared mesh vertices",
            bytemuck::cast_slice(&mesh.vertices),
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        let ibuf = create_init_buffer(
            &self.device,
            &self.queue,
            "kagra-shared mesh indices",
            bytemuck::cast_slice(&mesh.indices),
            wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        );
        // albedo / matcap / normal を 1 つの bind group に（無いスロットはデフォルト）。
        let tex_ok = |a: &Option<crate::scene3d::AlbedoRgba>| -> bool {
            a.as_ref()
                .is_some_and(|alb| alb.width > 0 && alb.height > 0 && !alb.rgba.is_empty())
        };
        let has_any = mesh.albedo_ktx.is_some()
            || tex_ok(&mesh.albedo)
            || tex_ok(&mesh.matcap)
            || tex_ok(&mesh.normal)
            || tex_ok(&mesh.orm)
            || tex_ok(&mesh.emissive_map);
        let (albedo_bind, albedo_tex, matcap_tex, normal_tex, orm_tex, emissive_tex, sampler) =
            if has_any {
                let alb_tex = upload_albedo(&self.device, &self.queue, mesh);
                let mc_tex = mesh
                    .matcap
                    .as_ref()
                    .filter(|_| tex_ok(&mesh.matcap))
                    .map(|a| {
                        create_albedo_texture(&self.device, &self.queue, a.width, a.height, &a.rgba)
                    });
                let nm_tex = mesh
                    .normal
                    .as_ref()
                    .filter(|_| tex_ok(&mesh.normal))
                    .map(|a| {
                        create_linear_texture(&self.device, &self.queue, a.width, a.height, &a.rgba)
                    });
                let alb_view = alb_tex
                    .as_ref()
                    .map(|t| t.create_view(&Default::default()))
                    .unwrap_or_else(|| self._white_view.clone());
                let mc_view = mc_tex
                    .as_ref()
                    .map(|t| t.create_view(&Default::default()))
                    .unwrap_or_else(|| self._black_view.clone());
                let nm_view = nm_tex
                    .as_ref()
                    .map(|t| t.create_view(&Default::default()))
                    .unwrap_or_else(|| self._flat_view.clone());
                let orm_tex = mesh.orm.as_ref().filter(|_| tex_ok(&mesh.orm)).map(|a| {
                    create_linear_texture(&self.device, &self.queue, a.width, a.height, &a.rgba)
                });
                let em_tex = mesh
                    .emissive_map
                    .as_ref()
                    .filter(|_| tex_ok(&mesh.emissive_map))
                    .map(|a| {
                        create_albedo_texture(&self.device, &self.queue, a.width, a.height, &a.rgba)
                    });
                let orm_view = orm_tex
                    .as_ref()
                    .map(|t| t.create_view(&Default::default()))
                    .unwrap_or_else(|| self._orm_view.clone());
                let em_view = em_tex
                    .as_ref()
                    .map(|t| t.create_view(&Default::default()))
                    .unwrap_or_else(|| self._black_view.clone());
                let sampler = mesh_sampler(&self.device, &mesh.sampler);
                let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("kagra-shared mesh tex"),
                    layout: &self.albedo_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&alb_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&mc_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(&nm_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 5,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 6,
                            resource: wgpu::BindingResource::TextureView(&orm_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 7,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 8,
                            resource: wgpu::BindingResource::TextureView(&em_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 9,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                });
                (
                    Some(bind),
                    alb_tex,
                    mc_tex,
                    nm_tex,
                    orm_tex,
                    em_tex,
                    Some(sampler),
                )
            } else {
                (None, None, None, None, None, None, None)
            };
        let mtoon = mesh
            .mtoon
            .map(crate::scene3d::MtoonShade::gpu)
            .unwrap_or([0.0; 4]);
        let mtoon2 = mesh
            .mtoon
            .map(crate::scene3d::MtoonShade::gpu_rim)
            .unwrap_or([0.0; 4]);
        let mtoon3 = mesh
            .mtoon
            .map(crate::scene3d::MtoonShade::gpu_outline)
            .unwrap_or([0.0; 4]);
        let mtoon4 = mesh
            .mtoon
            .map(crate::scene3d::MtoonShade::gpu_shift_lift)
            .unwrap_or([0.0; 4]);
        self.meshes.push(GpuMesh {
            vbuf,
            ibuf,
            index_count: mesh.indices.len() as u32,
            albedo_bind,
            _albedo_tex: albedo_tex,
            _matcap_tex: matcap_tex,
            _normal_tex: normal_tex,
            _orm_tex: orm_tex,
            _emissive_tex: emissive_tex,
            sample_rough: mesh.orm_rough && tex_ok(&mesh.orm),
            sample_metal: mesh.orm_metal && tex_ok(&mesh.orm),
            sample_emissive: tex_ok(&mesh.emissive_map),
            mtoon,
            mtoon2,
            mtoon3,
            mtoon4,
            alpha_cut: mesh.alpha_cut,
            double_sided: mesh.double_sided,
            has_normal: tex_ok(&mesh.normal),
            normal_scale: mesh.normal_scale.unwrap_or(1.0),
            emissive: mesh.emissive,
            emissive_color: mesh.emissive_color,
            clearcoat: mesh.clearcoat,
            clearcoat_roughness: mesh.clearcoat_roughness,
            _sampler: sampler,
            tex_bytes: image_bytes(&mesh.albedo)
                + image_bytes(&mesh.matcap)
                + image_bytes(&mesh.normal)
                + image_bytes(&mesh.orm)
                + image_bytes(&mesh.emissive_map),
        });
        MeshId(self.meshes.len() as u32 - 1)
    }

    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    /// Profile of the last `render_frame`. Empty until a frame has been drawn.
    pub fn frame_profile(&self) -> crate::profile::FrameProfile {
        self.last_profile.clone()
    }

    fn texture_bytes(&self) -> u64 {
        let meshes: u64 = self.meshes.iter().map(|m| m.tex_bytes).sum();
        let shadow = (crate::scene3d::SHADOW_MAP_SIZE as u64)
            .saturating_mul(crate::scene3d::SHADOW_MAP_SIZE as u64)
            * 4;
        meshes + shadow + 64 * 64 * 4
    }

    /// Drop uploaded meshes so the next world draw can upload `compile_meshes`.
    /// Driving and collectathon attach their own meshes first; a packaged
    /// `World` scene cannot skin glTF on top of that list.
    pub fn clear_meshes(&mut self) {
        self.meshes.clear();
        self.cloth.clear();
        self.mesh_key.clear();
    }

    pub fn aspect(&self) -> f32 {
        self.width as f32 / self.height.max(1) as f32
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn is_offscreen(&self) -> bool {
        matches!(self.target, Target::Offscreen { .. })
    }

    /// Threshold bloom. `intensity` 0 = composite がそのままフレームを通す
    /// （絵は変わらない）。Defaults off so the picture never silently changes;
    /// games / play_world turn it on.
    pub fn set_bloom(&mut self, threshold: f32, intensity: f32) {
        self.bloom.set_params(threshold, intensity);
    }

    /// FXAA edge smoothing on the composite output. Default on.
    pub fn set_fxaa(&mut self, enabled: bool) {
        self.bloom.set_fxaa(enabled);
    }

    /// SSAO radius in pixels (0 = off), TAA, optional DOF `(focus, aperture)`,
    /// and a camera-motion shutter in 0..1. Off leaves the bloom composite alone.
    pub fn set_post(&mut self, ssao: f32, taa: bool, dof: Option<(f32, f32)>, motion: f32) {
        self.post.set(ssao, taa, dof, motion);
    }

    fn upload_screen(&self) {
        let data = [self.width as f32, self.height as f32, 0.0, 0.0];
        self.queue
            .write_buffer(&self.screen_buf, 0, bytemuck::cast_slice(&data));
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.clamp(1, MAX_TEXTURE_SIDE);
        let height = height.clamp(1, MAX_TEXTURE_SIDE);
        if width == self.width && height == self.height {
            return;
        }
        self.width = width;
        self.height = height;
        match &mut self.target {
            Target::Surface { surface, config } => {
                config.width = width;
                config.height = height;
                surface.configure(&self.device, config);
            }
            Target::Offscreen { texture } => {
                *texture = create_offscreen_texture(&self.device, self.format, width, height);
            }
        }
        self.depth = create_depth_view(&self.device, width, height);
        self.bloom.resize(&self.device, width, height);
        self.post.resize(&self.device, width, height);
        self.upload_screen();
    }

    /// HUD だけを描く。3D を持たない呼び出し側のための薄い入口。
    pub fn render(&mut self, list: &DrawList) -> Result<(), String> {
        self.render_frame(None, list)
    }

    /// 3D を描いてから HUD を重ね、1 枚として present する。
    pub fn render_frame(&mut self, world: Option<&Scene3D>, hud: &DrawList) -> Result<(), String> {
        puffin::GlobalProfiler::lock().new_frame();
        puffin::profile_function!();
        // Instant::now can panic on wasm32 before a time source exists.
        #[cfg(not(target_arch = "wasm32"))]
        let cpu_start = std::time::Instant::now();
        let vertices = self.build_vertices(hud);
        self.ensure_vertex_capacity(vertices.len());
        if !vertices.is_empty() {
            self.queue
                .write_buffer(&self.vbuf, 0, bytemuck::cast_slice(&vertices));
        }

        // Frame format == swapchain means Chrome's WebGPU samples an
        // Rgba16Float target as white, and drawing 3D straight at that
        // surface skips FXAA. Any 3D frame on that path draws a separate
        // Rgba8Unorm and always runs the post graph (grade, 8-bit bloom,
        // FXAA). SSAO joins only when the radius is on.
        let direct = self.bloom.frame_format() == self.format;
        let want_ssao = world.is_some() && self.post.ssao_enabled();
        let direct_post = direct && world.is_some();
        let present_in_shader =
            color_space::present_in_fragment(direct && !direct_post, self.format.is_srgb());
        let mut unjittered_vp = glam::Mat4::IDENTITY;
        let mut drew_scene = false;
        let mut stroke_points = 0u32;
        let mut stroke_lines = 0u32;

        // 3D のインスタンスを 1 本のバッファに連結し、バッチごとの開始位置を覚える。
        // base instance は GLES に無いので、描画時は毎回 0.. で数え、バッファの
        // スライス位置でずらす。
        // (mesh, byte_offset, instance_count, is_sky, casts_shadow)
        let mut draws: Vec<(usize, u64, u32, bool, bool)> = Vec::new();
        let low_tier = self.picture_tier == "low";
        if let Some(scene) = world {
            let mut raw: Vec<InstanceRaw> = Vec::with_capacity(scene.instance_count());
            for batch in &scene.batches {
                let mesh = batch.mesh.0 as usize;
                if mesh >= self.meshes.len() || batch.instances.is_empty() {
                    continue;
                }
                let offset =
                    (raw.len() * std::mem::size_of::<InstanceRaw>()) as wgpu::BufferAddress;
                let mat0 = batch.instances[0].material;
                let is_sky = mat0 == crate::scene3d::Material::Sky;
                let casts_shadow = !is_sky && mat0 != crate::scene3d::Material::Water;
                for inst in &batch.instances {
                    let gpu = self.meshes.get(mesh);
                    let (mtoon, mtoon2, mtoon3, mtoon4) =
                        if inst.material == crate::scene3d::Material::Toon {
                            match gpu {
                                Some(g) => (g.mtoon, g.mtoon2, g.mtoon3, g.mtoon4),
                                None => ([0.55, 0.50, 0.52, 0.85], [0.0; 4], [0.0; 4], [0.0; 4]),
                            }
                        } else {
                            let (use_r, use_m, use_e) = match gpu {
                                Some(g) => (
                                    if g.sample_rough { 1.0 } else { 0.0 },
                                    if g.sample_metal { 1.0 } else { 0.0 },
                                    if g.sample_emissive { 1.0 } else { 0.0 },
                                ),
                                None => (0.0, 0.0, 0.0),
                            };
                            let (em_color, em_strength) = if inst.emissive_authored {
                                (inst.emissive_color, inst.emissive)
                            } else if let Some(g) = gpu {
                                if g.emissive > 0.0 || g.emissive_color != [0.0; 3] {
                                    (g.emissive_color, g.emissive)
                                } else {
                                    (inst.emissive_color, inst.emissive)
                                }
                            } else {
                                (inst.emissive_color, inst.emissive)
                            };
                            // w = normal-map scale (0 = no map). The low tier skips the map.
                            let n_w = match gpu {
                                Some(g) if g.has_normal && !low_tier => g.normal_scale,
                                _ => 0.0,
                            };
                            (
                                [0.0; 4],
                                [use_r, use_m, use_e, 0.0],
                                [em_color[0], em_color[1], em_color[2], 0.0],
                                [inst.metallic, inst.roughness, em_strength, n_w],
                            )
                        };
                    let alpha_cut = gpu.map(|g| g.alpha_cut).unwrap_or(0.0);
                    let (coat, coat_rough) = if inst.clearcoat > 0.0 {
                        (inst.clearcoat, inst.clearcoat_roughness)
                    } else {
                        gpu.map(|g| (g.clearcoat, g.clearcoat_roughness))
                            .unwrap_or((0.0, 0.0))
                    };
                    raw.push(InstanceRaw {
                        model: inst.model.to_cols_array_2d(),
                        // Author bytes are sRGB. Lighting stays linear on every target.
                        color: color_space::srgb_u8_to_linear(inst.color),
                        material: inst.material as u8 as f32,
                        // location 7: material id, clearcoat, clearcoat roughness.
                        _pad: [coat, coat_rough, 0.0],
                        mtoon,
                        mtoon2,
                        mtoon3,
                        mtoon4,
                        alpha_cut,
                        _pad_cut: [0.0; 3],
                    });
                }
                draws.push((
                    mesh,
                    offset,
                    batch.instances.len() as u32,
                    is_sky,
                    casts_shadow,
                ));
            }
            if !raw.is_empty() {
                self.ensure_instance_capacity(raw.len());
                self.queue
                    .write_buffer(&self.inst_buf, 0, bytemuck::cast_slice(&raw));
            }
            let depth_span = if direct && want_ssao {
                post::AO_DEPTH_SPAN
            } else {
                0.0
            };
            unjittered_vp = self.upload_globals(scene, present_in_shader, depth_span, direct);
            let (points, lines) =
                self.strokes
                    .prepare(&self.device, &self.queue, scene, self.width, self.height);
            stroke_points = points;
            stroke_lines = lines;
            drew_scene = true;
        }

        let clear_bytes = world.map(|s| s.clear).unwrap_or(hud.clear);
        let (exposure, tonemap, tonemap_code) = match world {
            Some(scene) => (scene.exposure.max(0.0), scene.tonemap, scene.tonemap_code),
            None => (1.0, true, 0.0),
        };
        // 3D clear is linear in the HDR frame. The direct unorm fallback has no
        // composite, so that clear is graded here with the same OETF as present_rgb.
        // HUD-only direct stores the code; composite (or an sRGB target) encodes once.
        let clear = if world.is_some() && present_in_shader {
            color_space::grade_curve(clear_bytes, exposure, tonemap, tonemap_code)
        } else if present_in_shader {
            color_space::display_u8(clear_bytes, false)
        } else {
            color_space::srgb_u8_to_linear(clear_bytes)
        };
        let frame = match &self.target {
            Target::Surface { surface, .. } => {
                use wgpu::CurrentSurfaceTexture as Cst;
                match surface.get_current_texture() {
                    // Suboptimal でもそのフレームは描ける。次フレームで作り直す。
                    Cst::Success(f) | Cst::Suboptimal(f) => Some(f),
                    Cst::Outdated | Cst::Lost => {
                        // 回転・リサイズ直後。再構成して次フレームに任せる。
                        // A configure the browser rejected stays Lost forever
                        // and presents black, so say so once.
                        note_once(&format!(
                            "kagra: surface lost (format {:?}, {}x{})",
                            self.format, self.width, self.height
                        ));
                        self.reconfigure();
                        return Ok(());
                    }
                    Cst::Timeout | Cst::Occluded => return Ok(()),
                    Cst::Validation => return Err("surface validation error".into()),
                }
            }
            Target::Offscreen { .. } => None,
        };

        let target_texture: &wgpu::Texture = match (&frame, &self.target) {
            (Some(f), _) => &f.texture,
            (None, Target::Offscreen { texture }) => texture,
            _ => return Err("no render target".into()),
        };
        let view = target_texture.create_view(&Default::default());
        // 3D は線形 HDR フレームへ。Bloom が最終ターゲットへ合成し、HUD は
        // その後に重ねる（トーン後の色を保つ）。
        // A 3D frame always lands in the offscreen target. Sampling the
        // swapchain itself comes back white on Chrome's WebGPU. HUD-only
        // frames still clear the swapchain directly.
        let frame_view = self.bloom.frame_view();
        let emit_view = self.bloom.emit_view();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kagra-shared frame"),
            });
        let clock = self.timestamps.clone();
        if let Some(ts) = &clock {
            encoder.write_timestamp(&ts.query, 0);
        }
        if !draws.is_empty() {
            let cascade_count = world.map(|s| s.shadow_cascades.clamp(1, 3)).unwrap_or(1);
            // PCSS raw is Rgba16Float. The direct unorm path does not sample it.
            let pcss = world.is_some_and(|s| s.shadow_pcss) && !direct;
            self.encode_cascades(&mut encoder, &draws, cascade_count, pcss);
        }
        // Pass 1: 3D（スカイ + メッシュ）→ 線形フレーム + エミッシブマスク。
        {
            let color_view = if direct && world.is_none() {
                &view
            } else {
                frame_view
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kagra-shared 3d pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: color_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: clear[0] as f64,
                                g: clear[1] as f64,
                                b: clear[2] as f64,
                                a: clear[3] as f64,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: emit_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            if !draws.is_empty() {
                pass.set_bind_group(0, &self.globals_bind, &[]);
                pass.set_bind_group(2, &self.shadow_bind, &[]);
                pass.set_bind_group(3, &self.env_bind, &[]);
                // 空を先に。深度を書かないので、このあと地面が上に乗る。
                pass.set_pipeline(&self.pipeline_sky);
                for (mesh, offset, count, is_sky, _cast) in &draws {
                    if !*is_sky {
                        continue;
                    }
                    let m = &self.meshes[*mesh];
                    pass.set_bind_group(
                        1,
                        m.albedo_bind.as_ref().unwrap_or(&self.default_albedo_bind),
                        &[],
                    );
                    pass.set_vertex_buffer(0, m.vbuf.slice(..));
                    pass.set_vertex_buffer(1, self.inst_buf.slice(*offset..));
                    pass.set_index_buffer(m.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.index_count, 0, 0..*count);
                }
                for (mesh, offset, count, is_sky, _cast) in &draws {
                    if *is_sky {
                        continue;
                    }
                    let m = &self.meshes[*mesh];
                    pass.set_pipeline(if m.double_sided {
                        &self.pipeline_nocull
                    } else {
                        &self.pipeline3d
                    });
                    pass.set_bind_group(
                        1,
                        m.albedo_bind.as_ref().unwrap_or(&self.default_albedo_bind),
                        &[],
                    );
                    pass.set_vertex_buffer(0, m.vbuf.slice(..));
                    pass.set_vertex_buffer(1, self.inst_buf.slice(*offset..));
                    pass.set_index_buffer(m.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.index_count, 0, 0..*count);
                }
                // MToon アウトライン: 本体の後にカリング Front で押し出し背面。
                // outline_width > 0 の Toon メッシュだけ（非 Toon は mtoon3 がゼロ）。
                pass.set_pipeline(&self.pipeline_outline);
                for (mesh, offset, count, is_sky, _cast) in &draws {
                    if *is_sky {
                        continue;
                    }
                    if self.meshes[*mesh].mtoon3[3] <= 0.0 {
                        continue;
                    }
                    let m = &self.meshes[*mesh];
                    pass.set_bind_group(
                        1,
                        m.albedo_bind.as_ref().unwrap_or(&self.default_albedo_bind),
                        &[],
                    );
                    pass.set_vertex_buffer(0, m.vbuf.slice(..));
                    pass.set_vertex_buffer(1, self.inst_buf.slice(*offset..));
                    pass.set_index_buffer(m.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.index_count, 0, 0..*count);
                }
            }
            self.strokes.draw(&mut pass, stroke_points, stroke_lines);
        }
        // Depth color for SSAO / DOF / motion. The color target blends, so
        // device depth cannot live in its alpha. Load the depth buffer the
        // color pass just wrote; do not clear it. HDR stores meters. The
        // direct SSAO path stores eye/AO_DEPTH_SPAN in this unorm target.
        if drew_scene && self.post.needs_depth() && (!direct || want_ssao) {
            let depth_far = world.map(|s| s.camera.far).unwrap_or(1000.0).max(1.0) as f64;
            let depth_color = self.post.scene_depth_view();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kagra-shared depth color"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: depth_color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: depth_far,
                            g: depth_far,
                            b: depth_far,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if !draws.is_empty() {
                pass.set_pipeline(&self.pipeline_depth);
                pass.set_bind_group(0, &self.globals_bind, &[]);
                pass.set_bind_group(2, &self.shadow_bind, &[]);
                pass.set_bind_group(3, &self.env_bind, &[]);
                for (mesh, offset, count, _is_sky, _cast) in &draws {
                    let m = &self.meshes[*mesh];
                    pass.set_bind_group(
                        1,
                        m.albedo_bind.as_ref().unwrap_or(&self.default_albedo_bind),
                        &[],
                    );
                    pass.set_vertex_buffer(0, m.vbuf.slice(..));
                    pass.set_vertex_buffer(1, self.inst_buf.slice(*offset..));
                    pass.set_index_buffer(m.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.index_count, 0, 0..*count);
                }
            }
        }
        // Pass 2: optional SSAO / DOF / motion, then bloom grade, then TAA or FXAA.
        // The 8-bit path always grades here (FXAA when the dump leaves it on,
        // which is the default). SSAO runs only when the radius is on.
        // No TAA, no PCSS, no DOF. 8-bit bloom is the luma/emissive mip chain.
        if direct_post {
            if drew_scene && want_ssao {
                let near = world.map(|s| s.camera.near).unwrap_or(0.1);
                let far = world.map(|s| s.camera.far).unwrap_or(1000.0);
                self.post.apply_ssao_only(
                    &self.device,
                    &self.queue,
                    &mut encoder,
                    self.bloom.frame_view(),
                    near,
                    far,
                    post::AO_DEPTH_SPAN,
                );
            }
            self.bloom.apply(
                &self.device,
                &self.queue,
                &mut encoder,
                (target_texture, &view),
                exposure,
                tonemap,
                tonemap_code,
                None,
                true,
            );
        } else if !direct {
            if drew_scene && self.post.scene_active() {
                let near = world.map(|s| s.camera.near).unwrap_or(0.1);
                let far = world.map(|s| s.camera.far).unwrap_or(1000.0);
                self.post.apply_scene(
                    &self.device,
                    &self.queue,
                    &mut encoder,
                    self.bloom.frame_view(),
                    unjittered_vp,
                    near,
                    far,
                );
            }
            let taa = drew_scene && self.post.taa_enabled();
            let vig = self.bloom.vignette();
            self.bloom.apply(
                &self.device,
                &self.queue,
                &mut encoder,
                (target_texture, &view),
                exposure,
                tonemap,
                tonemap_code,
                if taa { Some(0.0) } else { None },
                !taa,
            );
            if taa {
                self.post.apply_display(
                    &self.device,
                    &self.queue,
                    &mut encoder,
                    self.bloom.composite_view(),
                    &view,
                    vig,
                );
            }
        }
        // Pass 3: HUD → 最終ターゲット（トーン後の色をそのまま重ねる）。
        if !vertices.is_empty() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kagra-shared hud pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.screen_bind, &[]);
            pass.set_vertex_buffer(0, self.vbuf.slice(..));
            pass.draw(0..vertices.len() as u32, 0..1);
        }
        if let Some(ts) = &clock {
            encoder.write_timestamp(&ts.query, 1);
            encoder.resolve_query_set(&ts.query, 0..2, &ts.resolve, 0);
            encoder.copy_buffer_to_buffer(&ts.resolve, 0, &ts.staging, 0, 16);
        }
        self.queue.submit(Some(encoder.finish()));
        #[cfg(not(target_arch = "wasm32"))]
        let cpu_ms = Some(cpu_start.elapsed().as_secs_f32() * 1000.0);
        #[cfg(target_arch = "wasm32")]
        let cpu_ms = None;
        let gpu_ms = clock
            .as_ref()
            .and_then(|ts| read_gpu_ms(&self.device, &self.queue, &ts.staging));
        if drew_scene {
            self.post.store_view(unjittered_vp);
        }
        if let Some(f) = frame {
            self.queue.present(f);
        }
        let (mut draws, mut triangles) = draw_counts(&self.meshes, &draws);
        if stroke_points > 0 {
            draws = draws.saturating_add(1);
            triangles = triangles.saturating_add(stroke_points.saturating_mul(2));
        }
        if stroke_lines > 0 {
            draws = draws.saturating_add(1);
            triangles = triangles.saturating_add(stroke_lines.saturating_mul(2));
        }
        self.last_profile = crate::profile::FrameProfile {
            cpu_ms,
            gpu_ms,
            draws,
            triangles,
            texture_bytes: self.texture_bytes(),
        };
        let cascades = world.map(|s| s.shadow_cascades.clamp(1, 3)).unwrap_or(1);
        self.picture_line = picture_line(
            &self.picture_tier,
            &self.backend_name,
            self.bloom.fxaa_enabled(),
            self.bloom.is_active(),
            self.post.ssao_radius(),
            cascades,
            direct,
            direct_post,
        );
        Ok(())
    }

    /// One line the page can print: tier, backend, and which passes ran.
    pub fn picture_status(&self) -> String {
        if self.picture_line.is_empty() {
            format!("pending {}", self.backend_name)
        } else {
            self.picture_line.clone()
        }
    }

    fn reconfigure(&mut self) {
        if let Target::Surface { surface, config } = &self.target {
            surface.configure(&self.device, config);
        }
    }

    fn build_vertices(&mut self, list: &DrawList) -> Vec<Vertex> {
        let mut out = Vec::with_capacity((list.quads.len() + list.texts.len() * 64) * 6);
        for q in &list.quads {
            out.extend(quad_vertices(q, self.format));
        }
        for t in &list.texts {
            for q in self.text.text_quads(t) {
                out.extend(quad_vertices(&q, self.format));
            }
        }
        out
    }

    fn ensure_vertex_capacity(&mut self, needed: usize) {
        if needed <= self.vbuf_capacity {
            return;
        }
        let cap = needed.next_power_of_two();
        self.vbuf = create_vertex_buffer(&self.device, cap);
        self.vbuf_capacity = cap;
    }

    fn ensure_instance_capacity(&mut self, needed: usize) {
        if needed <= self.inst_capacity {
            return;
        }
        let cap = needed.next_power_of_two();
        self.inst_buf = create_instance_buffer(&self.device, cap);
        self.inst_capacity = cap;
    }

    /// One shadow pass per cascade. The first pass clears the whole 2048 map
    /// (a viewport does not scissor the clear). Later passes load and draw
    /// into their tile. Count 1 keeps the full-map viewport.
    fn encode_cascades(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        draws: &[(usize, u64, u32, bool, bool)],
        count: u32,
        pcss: bool,
    ) {
        let count = count.clamp(1, 3);
        for index in 0..count {
            let (vx, vy, vw, vh) = crate::scene3d::cascade_viewport(index, count);
            let load_depth = if index == 0 {
                wgpu::LoadOp::Clear(1.0)
            } else {
                wgpu::LoadOp::Load
            };
            let load_color = if index == 0 {
                wgpu::LoadOp::Clear(wgpu::Color {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                })
            } else {
                wgpu::LoadOp::Load
            };
            let color = [Some(wgpu::RenderPassColorAttachment {
                view: &self.shadow_raw_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: load_color,
                    store: wgpu::StoreOp::Store,
                },
            })];
            let empty: &[Option<wgpu::RenderPassColorAttachment>] = &[];
            let colors: &[Option<wgpu::RenderPassColorAttachment>] =
                if pcss { &color } else { empty };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kagra-shared shadow"),
                color_attachments: colors,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: load_depth,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if count > 1 {
                pass.set_viewport(vx, vy, vw, vh, 0.0, 1.0);
            }
            pass.set_bind_group(0, &self.globals_bind, &[]);
            for (mesh, offset, n, _is_sky, casts) in draws {
                if !*casts {
                    continue;
                }
                let m = &self.meshes[*mesh];
                let pipeline = match (pcss, m.double_sided) {
                    (true, true) => &self.pipeline_shadow_pcss_nocull[index as usize],
                    (true, false) => &self.pipeline_shadow_pcss[index as usize],
                    (false, true) => &self.pipeline_shadow_nocull[index as usize],
                    (false, false) => &self.pipeline_shadow[index as usize],
                };
                pass.set_pipeline(pipeline);
                pass.set_vertex_buffer(0, m.vbuf.slice(..));
                pass.set_vertex_buffer(1, self.inst_buf.slice(*offset..));
                pass.set_index_buffer(m.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.index_count, 0, 0..*n);
            }
        }
    }

    fn upload_globals(
        &mut self,
        scene: &Scene3D,
        present_in_shader: bool,
        depth_span: f32,
        direct: bool,
    ) -> glam::Mat4 {
        let cam = &scene.camera;
        let mut light_pos = [[0.0f32; 4]; 4];
        let mut light_col = [[0.0f32; 4]; 4];
        let mut light_dir = [[0.0f32; 4]; 4];
        for (i, lit) in scene.local_lights.iter().enumerate() {
            light_pos[i] = [
                lit.position.x,
                lit.position.y,
                lit.position.z,
                lit.intensity.max(0.0),
            ];
            light_col[i] = [
                lit.color[0],
                lit.color[1],
                lit.color[2],
                lit.radius.max(0.0),
            ];
            light_dir[i] = [
                lit.direction.x,
                lit.direction.y,
                lit.direction.z,
                if lit.spot { 1.0 } else { 0.0 },
            ];
        }
        let view = cam.view();
        let proj = cam.projection(self.aspect());
        let unjittered = proj * view;
        let vp = if !direct && !present_in_shader && self.post.taa_enabled() {
            let j = post::ndc_jitter(self.post.sample_index(), self.width, self.height);
            post::apply_projection_jitter(proj, j) * view
        } else {
            unjittered
        };
        let cascade_count = scene.shadow_cascades.clamp(1, 3);
        let vps = crate::scene3d::scene_cascade_view_projs(scene, cascade_count);
        let g = Globals {
            view_proj: vp.to_cols_array_2d(),
            light: [
                scene.light_dir.x,
                scene.light_dir.y,
                scene.light_dir.z,
                scene.ambient.clamp(0.0, 1.0),
            ],
            fog_color: color_space::srgb_u8_to_linear(scene.fog_color),
            fog_range: [scene.fog_start, scene.fog_end, 0.0, 0.0],
            camera_pos: [cam.eye.x, cam.eye.y, cam.eye.z, 0.0],
            env: [
                scene.ibl.max(0.0),
                scene.exposure.max(0.0),
                self.elapsed,
                if scene.tonemap { 1.0 } else { 0.0 },
            ],
            light_pos,
            light_col,
            light_dir,
            light_view_proj: vps[0].to_cols_array_2d(),
            cascade1: vps[1].to_cols_array_2d(),
            cascade2: vps[2].to_cols_array_2d(),
            shadow: [
                scene.shadow_softness.max(0.0),
                if present_in_shader { 1.0 } else { 0.0 },
                cam.near.max(1e-3),
                cam.far.max(cam.near + 0.01),
            ],
            cascade: [
                cascade_count as f32,
                if scene.shadow_pcss && !direct {
                    1.0
                } else {
                    0.0
                },
                depth_span,
                if scene.sky_atmosphere { 1.0 } else { 0.0 },
            ],
            hemi_sky: [
                scene.hemi_sky[0],
                scene.hemi_sky[1],
                scene.hemi_sky[2],
                if scene.tonemap {
                    scene.tonemap_code
                } else {
                    0.0
                },
            ],
            hemi_ground: [
                scene.hemi_ground[0],
                scene.hemi_ground[1],
                scene.hemi_ground[2],
                if scene.env_pixels.is_some() { 1.0 } else { 0.0 },
            ],
        };
        if let Some(px) = &scene.env_pixels {
            if px.len() >= 64 * 64 * 4 {
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.env_tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &px[..64 * 64 * 4],
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(64),
                    },
                    wgpu::Extent3d {
                        width: 64,
                        height: 64,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        self.queue
            .write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&g));
        self.elapsed += 1.0 / 60.0;
        unjittered
    }

    /// オフスクリーンの内容を RGBA8 で読み出す。
    pub fn read_rgba(&self) -> Result<Vec<u8>, String> {
        let Target::Offscreen { texture } = &self.target else {
            return Err("read_rgba requires an offscreen renderer".into());
        };

        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let unpadded = self.width * 4;
        let padded = unpadded.div_ceil(align) * align;

        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kagra-shared readback"),
            size: (padded * self.height) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kagra-shared readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        rx.recv()
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

        let mapped = slice.get_mapped_range().map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity((unpadded * self.height) as usize);
        for row in 0..self.height {
            let start = (row * padded) as usize;
            out.extend_from_slice(&mapped[start..start + unpadded as usize]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(out)
    }

    /// Upload primitive `compile_meshes()` (box/sphere/capsule/plane/quad). Prefer
    /// `upload_world_meshes` so heightfield + glTF slots match `compile_scene`.
    pub fn upload_compile_meshes(&mut self) -> Result<(), String> {
        if !self.meshes.is_empty() {
            return Err("upload_compile_meshes requires an empty mesh list".into());
        }
        let mut meshes = crate::world_doc::compile_meshes();
        meshes.sort_by_key(|(id, _)| id.0);
        self.upload_mesh_list(meshes)
    }

    /// Upload `WorldDoc::compile_meshes` (primitives + heightfield + glTF).
    /// Call on a renderer with no meshes yet (a fresh window / offscreen).
    pub fn upload_world_meshes(&mut self, doc: &crate::world_doc::WorldDoc) -> Result<(), String> {
        if !self.meshes.is_empty() {
            return Err("upload_world_meshes requires an empty mesh list".into());
        }
        let mut meshes = doc.compile_meshes();
        meshes.sort_by_key(|(id, _)| id.0);
        self.upload_mesh_list(meshes)
    }

    /// Overwrite CPU-skinned walker vertices. Topology stays; COPY_DST vertex buf.
    pub fn update_mesh(&mut self, id: MeshId, mesh: &MeshData) -> Result<(), String> {
        let i = id.0 as usize;
        if i >= self.meshes.len() {
            return Err(format!("update_mesh: {i} out of range"));
        }
        let vbytes: &[u8] = bytemuck::cast_slice(&mesh.vertices);
        let need = vbytes.len() as u64;
        if need == 0 {
            return Ok(());
        }
        if need > self.meshes[i].vbuf.size() {
            self.meshes[i].vbuf = create_init_buffer(
                &self.device,
                &self.queue,
                "kagra-shared mesh vertices",
                vbytes,
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            );
        } else {
            self.queue.write_buffer(&self.meshes[i].vbuf, 0, vbytes);
        }
        Ok(())
    }

    /// Re-skin walker glTF slots after a live tick. Primitive / heightfield stay.
    /// SpringBone 布シミュ（`self.cloth`、spec ごと）を 1 ステップ進めてから
    /// スキンするので、フレームを跨いで布が実際に揺れる。
    pub fn update_world_gltf(&mut self, doc: &crate::world_doc::WorldDoc) -> Result<(), String> {
        use crate::world_doc::GltfSlot;
        for (i, slot) in doc.gltf_slots().into_iter().enumerate() {
            let GltfSlot::Skinned {
                spec,
                part,
                clip,
                hair,
                morph,
                expression,
                look_yaw,
                look_pitch,
                anim_blend,
                overlay_bones,
                overlay_weight,
                anim,
                clip_name,
                fade_from,
                fade_from_clip,
                fade_from_weight,
                ..
            } = slot
            else {
                continue;
            };
            let Some(parts) = crate::world_doc::load_skinned_parts(&spec) else {
                continue;
            };
            let Some(skin) = parts.get(part) else {
                continue;
            };
            let mesh = {
                let sim = self
                    .cloth
                    .entry(spec.clone())
                    .or_insert_with(|| skin.springs.clone());
                sim.wind = doc.wind;
                let pose = crate::gltf_load::WalkerPose {
                    clip: if clip > 0.0 { Some(clip) } else { None },
                    hair,
                    expression,
                    morph,
                    look_yaw,
                    look_pitch,
                    anim_blend,
                    overlay_bones,
                    overlay_weight,
                    anim,
                    clip_name,
                    fade_from,
                    fade_from_clip,
                    fade_from_weight,
                };
                crate::gltf_load::sample_skinned_cloth_pose(
                    skin,
                    &pose,
                    sim,
                    crate::scene::FIXED_DT,
                )
            };
            let id = crate::scene3d::MeshId(crate::world_doc::MESH_GLTF_BASE + i as u32);
            self.update_mesh(id, &mesh)?;
        }
        Ok(())
    }

    fn upload_mesh_list(
        &mut self,
        meshes: Vec<(crate::scene3d::MeshId, crate::scene3d::MeshData)>,
    ) -> Result<(), String> {
        for (expect, (id, mesh)) in meshes.into_iter().enumerate() {
            let expect = expect as u32;
            if id.0 != expect {
                return Err(format!(
                    "compile mesh ids must be dense, got {} want {expect}",
                    id.0
                ));
            }
            let got = self.upload_mesh(&mesh);
            if got != id {
                return Err(format!(
                    "compile mesh id mismatch: compiled {} uploaded {}",
                    id.0, got.0
                ));
            }
        }
        Ok(())
    }

    /// Draw a compiled `WorldDoc` to the current target (window or offscreen).
    /// Uploads world meshes on first call (heightfield + glTF + primitives).
    pub fn draw_world_doc(&mut self, doc: &crate::world_doc::WorldDoc) -> Result<(), String> {
        self.draw_world_doc_with_hud(doc, &DrawList::default())
    }

    /// `draw_world_doc` に HUD（quad + テキスト）を重ねる版。
    pub fn draw_world_doc_with_hud(
        &mut self,
        doc: &crate::world_doc::WorldDoc,
        hud: &DrawList,
    ) -> Result<(), String> {
        let key = doc.gpu_mesh_key();
        if self.meshes.is_empty() || self.mesh_key != key {
            // Driving meshes, or a previous world's glTF table, are not this doc.
            self.clear_meshes();
            self.upload_world_meshes(doc)?;
            self.mesh_key = key;
        } else {
            // ライブ tick: walker glTF は clip/hair/morph/look で毎フレーム
            // 再スキンする（初回アップロードのみだと T-pose のまま固定）。
            // 静的 dump（glTF スロット無し）では実質 no-op。
            self.update_world_gltf(doc)?;
        }
        // Wasm and the desktop window draw here. Omitted post fields keep
        // whatever the host already set (the window example's set_bloom).
        self.apply_dump_grade(doc);
        self.apply_dump_post(doc);
        let scene = doc.compile_scene(self.aspect());
        self.render_frame(Some(&scene), hud)
    }

    /// Live windows keep their own bloom until a dump actually sets a post field.
    fn apply_dump_grade(&mut self, doc: &crate::world_doc::WorldDoc) {
        let filled_bloom = doc.bloom.is_none() && doc.bloom_intensity() > 1e-4;
        if doc.bloom.is_none()
            && doc.bloom_threshold.is_none()
            && doc.vignette.is_none()
            && doc.lut.is_none()
            && doc.lut_strength.is_none()
            && doc.fxaa.is_none()
            && !filled_bloom
        {
            return;
        }
        self.bloom.set_grade(
            doc.bloom_threshold(),
            doc.bloom_intensity(),
            doc.vignette_strength(),
            doc.fxaa_enabled(),
            doc.lut_preset(),
            doc.lut_mix(),
        );
    }

    fn apply_dump_post(&mut self, doc: &crate::world_doc::WorldDoc) {
        self.picture_tier = match doc.quality.as_deref().map(str::trim) {
            Some(q) if !q.is_empty() => q.to_string(),
            _ => "legacy".to_string(),
        };
        self.post.set(
            doc.ssao_radius(),
            doc.taa_enabled(),
            doc.dof(),
            doc.motion_shutter(),
        );
    }

    /// Draw a compiled `WorldDoc` into the offscreen target and read RGBA8.
    /// No kagra-core window. Capsules / boxes are the primitives.
    pub fn render_world_doc(
        &mut self,
        doc: &crate::world_doc::WorldDoc,
    ) -> Result<Vec<u8>, String> {
        self.render_world_doc_with_hud(doc, &DrawList::default())
    }

    /// `render_world_doc` に HUD（quad + テキスト）を重ねる版。
    pub fn render_world_doc_with_hud(
        &mut self,
        doc: &crate::world_doc::WorldDoc,
        hud: &DrawList,
    ) -> Result<Vec<u8>, String> {
        if !self.is_offscreen() {
            return Err("render_world_doc requires an offscreen renderer".into());
        }
        // Dump-driven post. Omitted fields stay the legacy picture (bloom off,
        // vignette 1, LUT off, FXAA on). The live window calls set_bloom
        // itself and does not come through here, so its default is unchanged.
        self.bloom.set_grade(
            doc.bloom_threshold(),
            doc.bloom_intensity(),
            doc.vignette_strength(),
            doc.fxaa_enabled(),
            doc.lut_preset(),
            doc.lut_mix(),
        );
        self.draw_world_doc_with_hud(doc, hud)?;
        let mut rgba = self.read_rgba()?;
        blit_hud_images(&mut rgba, self.width(), self.height(), &hud.images);
        Ok(rgba)
    }
}

/// wgpu 30 offscreen RGBA8 of a compiled `WorldDoc`. Linux CI / no desktop window.
///
/// Skips at the call site when `new_offscreen` has no adapter. Does not touch
/// kagra-core `RendererV2` or the `(-12800,-12800)` fake-headless path.
pub fn render_world_doc(
    doc: &crate::world_doc::WorldDoc,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    render_world_doc_with_hud(doc, width, height, &DrawList::default())
}

/// `render_world_doc` に HUD（quad + テキスト）を重ねる版。
pub fn render_world_doc_with_hud(
    doc: &crate::world_doc::WorldDoc,
    width: u32,
    height: u32,
    hud: &DrawList,
) -> Result<Vec<u8>, String> {
    let mut renderer = pollster::block_on(Renderer::new_offscreen(width, height))?;
    renderer.render_world_doc_with_hud(doc, hud)
}

fn blit_hud_images(rgba: &mut [u8], width: u32, height: u32, images: &[ImageQuad]) {
    if images.is_empty() {
        return;
    }
    let (dw, dh) = (width as i32, height as i32);
    for img in images {
        let Some(alb) = load_png_albedo(&img.path) else {
            continue;
        };
        if alb.width == 0 || alb.height == 0 {
            continue;
        }
        let (dx0, dy0, dw_i, dh_i) = (
            img.x.round() as i32,
            img.y.round() as i32,
            img.w.max(1.0).round() as i32,
            img.h.max(1.0).round() as i32,
        );
        for oy in 0..dh_i {
            let dy = dy0 + oy;
            if dy < 0 || dy >= dh {
                continue;
            }
            let v = (oy as f32 + 0.5) / dh_i as f32;
            let sy = ((v * alb.height as f32) as u32).min(alb.height - 1);
            for ox in 0..dw_i {
                let dx = dx0 + ox;
                if dx < 0 || dx >= dw {
                    continue;
                }
                let u = (ox as f32 + 0.5) / dw_i as f32;
                let sx = ((u * alb.width as f32) as u32).min(alb.width - 1);
                let si = ((sy * alb.width + sx) * 4) as usize;
                let src = &alb.rgba[si..si + 4];
                let a = src[3] as u32;
                if a == 0 {
                    continue;
                }
                let di = ((dy as u32 * width + dx as u32) * 4) as usize;
                if a == 255 {
                    rgba[di..di + 4].copy_from_slice(src);
                    continue;
                }
                let ia = 255 - a;
                for c in 0..3 {
                    rgba[di + c] = ((src[c] as u32 * a + rgba[di + c] as u32 * ia) / 255) as u8;
                }
                rgba[di + 3] = 255;
            }
        }
    }
}

fn note_once(msg: &str) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static ONCE: AtomicBool = AtomicBool::new(false);
    if ONCE.swap(true, Ordering::Relaxed) {
        return;
    }
    note_renderer(msg);
}

fn note_renderer(msg: &str) {
    #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
    web_sys::console::warn_1(&msg.into());
    #[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
    eprintln!("{msg}");
}

fn note_ldr_fallback() {
    note_renderer("kagra: HDR bloom disabled, drawing an 8-bit frame");
}

fn note_surface(adapter: &wgpu::Adapter, config: &wgpu::SurfaceConfiguration) {
    let info = adapter.get_info();
    note_renderer(&format!(
        "kagra: backend {} ({}) surface {:?} alpha {:?} {}x{}",
        info.backend.to_str(),
        info.name,
        config.format,
        config.alpha_mode,
        config.width,
        config.height
    ));
}

/// 8-bit swapchain. Skip fp16 even when the backend lists it: that format is
/// an HDR canvas, and a tonemapped game frame presents as black on a phone.
fn choose_surface_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    let ldr = |f: &wgpu::TextureFormat| {
        matches!(
            f,
            wgpu::TextureFormat::Rgba8UnormSrgb
                | wgpu::TextureFormat::Bgra8UnormSrgb
                | wgpu::TextureFormat::Rgba8Unorm
                | wgpu::TextureFormat::Bgra8Unorm
        )
    };
    formats
        .iter()
        .copied()
        .find(|f| f.is_srgb() && ldr(f))
        .or_else(|| formats.iter().copied().find(ldr))
        .or_else(|| formats.first().copied())
}

/// WebGPU's `get_texture_format_features` reports the spec guarantee, not what
/// sampling an Rgba16Float target actually returns. Chrome's WebGPU (SwiftShader
/// and some phone GPUs) samples that target as white and the whole frame clips.
/// Bloom stays off there; WebGL2 still uses the float target when the
/// extension is real.
fn hdr_for_adapter(adapter: &wgpu::Adapter) -> bool {
    // WebGPU reports Rgba16Float as guaranteed-filterable. Sampling that
    // target comes back white on Chrome's WebGPU (SwiftShader and some
    // phone GPUs) and the tonemap clips the whole frame. WebGL2 still
    // uses the float target when the extension is actually there.
    if adapter.get_info().backend == wgpu::Backend::BrowserWebGpu {
        return false;
    }
    bloom::hdr_frame_supported(adapter)
}

/// Which browser backend `new_instance` should build. The page sets this
/// before `attachCanvas` (`?backend=gl`, or after a blank WebGPU frame).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum GraphicsBackend {
    Auto = 0,
    WebGpu = 1,
    Gl = 2,
}

static GRAPHICS_BACKEND: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_graphics_backend(backend: GraphicsBackend) {
    GRAPHICS_BACKEND.store(backend as u8, std::sync::atomic::Ordering::Relaxed);
}

pub fn parse_graphics_backend(name: &str) -> GraphicsBackend {
    match name.trim().to_ascii_lowercase().as_str() {
        "gl" | "webgl" | "webgl2" => GraphicsBackend::Gl,
        "webgpu" | "gpu" => GraphicsBackend::WebGpu,
        _ => GraphicsBackend::Auto,
    }
}

fn graphics_backend() -> GraphicsBackend {
    match GRAPHICS_BACKEND.load(std::sync::atomic::Ordering::Relaxed) {
        1 => GraphicsBackend::WebGpu,
        2 => GraphicsBackend::Gl,
        _ => GraphicsBackend::Auto,
    }
}

async fn new_instance() -> wgpu::Instance {
    // サーフェスごとに display handle を渡すので、インスタンス側には持たせない。
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    let prefer = graphics_backend();
    desc.backends = match prefer {
        GraphicsBackend::Gl => wgpu::Backends::GL,
        GraphicsBackend::WebGpu => wgpu::Backends::BROWSER_WEBGPU,
        GraphicsBackend::Auto => wgpu::Backends::all(),
    };
    // wgpu 30 locks the backend at instance creation. If `navigator.gpu`
    // exists it picks WebGPU and never falls through, then reports
    // "gl support not compiled in" when `requestAdapter()` is null.
    // Detecting a real adapter first lets WebGL2 start.
    #[cfg(target_arch = "wasm32")]
    if prefer == GraphicsBackend::Auto {
        return wgpu::util::new_instance_with_webgpu_detection(desc).await;
    }
    wgpu::Instance::new(desc)
}

fn image_bytes(img: &Option<crate::scene3d::AlbedoRgba>) -> u64 {
    img.as_ref().map(|a| a.rgba.len() as u64).unwrap_or(0)
}

const POINT_CORNERS: [[f32; 2]; 6] = [
    [-1.0, -1.0],
    [1.0, -1.0],
    [1.0, 1.0],
    [-1.0, -1.0],
    [1.0, 1.0],
    [-1.0, 1.0],
];

const LINE_CORNERS: [[f32; 2]; 6] = [
    [0.0, -1.0],
    [1.0, -1.0],
    [1.0, 1.0],
    [0.0, -1.0],
    [1.0, 1.0],
    [0.0, 1.0],
];

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct StrokeView {
    view_proj: [[f32; 4]; 4],
    cam_right: [f32; 4],
    cam_up: [f32; 4],
    viewport: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PointRaw {
    pos: [f32; 4],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LineRaw {
    a: [f32; 4],
    b: [f32; 4],
    color: [f32; 4],
}

#[derive(Debug)]
struct StrokeGpu {
    pipeline_points: wgpu::RenderPipeline,
    pipeline_lines: wgpu::RenderPipeline,
    stroke_buf: wgpu::Buffer,
    stroke_bind: wgpu::BindGroup,
    point_corner_buf: wgpu::Buffer,
    line_corner_buf: wgpu::Buffer,
    point_inst_buf: wgpu::Buffer,
    point_inst_capacity: usize,
    line_inst_buf: wgpu::Buffer,
    line_inst_capacity: usize,
}

fn instance_buf(device: &wgpu::Device, label: &str, stride: usize, count: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (stride * count.max(1)) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_stroke_gpu(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    frame_format: wgpu::TextureFormat,
) -> StrokeGpu {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("kagra-shared strokes"),
        source: wgpu::ShaderSource::Wgsl(include_str!("strokes.wgsl").into()),
    });
    let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("kagra-shared stroke layout"),
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
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("kagra-shared stroke pipeline layout"),
        bind_group_layouts: &[Some(&bind_layout)],
        immediate_size: 0,
    });
    let stroke_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("kagra-shared stroke view"),
        size: std::mem::size_of::<StrokeView>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let stroke_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("kagra-shared stroke bind"),
        layout: &bind_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: stroke_buf.as_entire_binding(),
        }],
    });
    let add = wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
    };
    let target = wgpu::ColorTargetState {
        format: frame_format,
        blend: Some(add),
        write_mask: wgpu::ColorWrites::ALL,
    };
    let targets = [Some(target.clone()), Some(target)];
    let depth = wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::LessEqual),
        stencil: Default::default(),
        bias: Default::default(),
    };
    let corner_attr = [wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 0,
        shader_location: 0,
    }];
    let point_attr = [
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 0,
            shader_location: 1,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 16,
            shader_location: 2,
        },
    ];
    let line_attr = [
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 0,
            shader_location: 1,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 16,
            shader_location: 2,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 32,
            shader_location: 3,
        },
    ];
    let point_buffers = [
        Some(wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &corner_attr,
        }),
        Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PointRaw>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &point_attr,
        }),
    ];
    let line_buffers = [
        Some(wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &corner_attr,
        }),
        Some(wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineRaw>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &line_attr,
        }),
    ];
    let pipeline_points = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("kagra-shared points"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_point"),
            compilation_options: Default::default(),
            buffers: &point_buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_point"),
            compilation_options: Default::default(),
            targets: &targets,
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(depth.clone()),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    let pipeline_lines = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("kagra-shared fat lines"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_line"),
            compilation_options: Default::default(),
            buffers: &line_buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_line"),
            compilation_options: Default::default(),
            targets: &targets,
        }),
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(depth),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    let point_inst_capacity = 64;
    let line_inst_capacity = 64;
    StrokeGpu {
        pipeline_points,
        pipeline_lines,
        stroke_buf,
        stroke_bind,
        point_corner_buf: create_init_buffer(
            device,
            queue,
            "kagra-shared point corners",
            bytemuck::cast_slice(&POINT_CORNERS),
            wgpu::BufferUsages::VERTEX,
        ),
        line_corner_buf: create_init_buffer(
            device,
            queue,
            "kagra-shared line corners",
            bytemuck::cast_slice(&LINE_CORNERS),
            wgpu::BufferUsages::VERTEX,
        ),
        point_inst_buf: instance_buf(
            device,
            "kagra-shared point instances",
            32,
            point_inst_capacity,
        ),
        point_inst_capacity,
        line_inst_buf: instance_buf(
            device,
            "kagra-shared line instances",
            48,
            line_inst_capacity,
        ),
        line_inst_capacity,
    }
}

impl StrokeGpu {
    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &crate::scene3d::Scene3D,
        width: u32,
        height: u32,
    ) -> (u32, u32) {
        let cam = &scene.camera;
        let (forward, right, up) = crate::scene3d::screen_basis(cam.eye, cam.target, cam.up);
        let ahead = |p: [f32; 3]| (glam::Vec3::from_array(p) - cam.eye).dot(forward) > cam.near;
        let points: Vec<PointRaw> = crate::strokes::collect_points(&scene.points)
            .into_iter()
            .filter(|p| ahead(p.position))
            .map(|p| PointRaw {
                pos: [p.position[0], p.position[1], p.position[2], p.size],
                color: color_space::srgb_u8_to_linear(p.color),
            })
            .collect();
        let lines: Vec<LineRaw> = crate::strokes::collect_segments(&scene.lines)
            .into_iter()
            .filter(|s| ahead(s.start) && ahead(s.end))
            .map(|s| LineRaw {
                a: [s.start[0], s.start[1], s.start[2], s.width],
                b: [s.end[0], s.end[1], s.end[2], 0.0],
                color: color_space::srgb_u8_to_linear(s.color),
            })
            .collect();
        if points.is_empty() && lines.is_empty() {
            return (0, 0);
        }
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        let view = StrokeView {
            view_proj: cam.view_projection(aspect).to_cols_array_2d(),
            cam_right: [right.x, right.y, right.z, 0.0],
            cam_up: [up.x, up.y, up.z, 0.0],
            viewport: [width.max(1) as f32, height.max(1) as f32, 0.0, 0.0],
        };
        queue.write_buffer(&self.stroke_buf, 0, bytemuck::bytes_of(&view));
        if !points.is_empty() {
            if points.len() > self.point_inst_capacity {
                let cap = points.len().next_power_of_two();
                self.point_inst_buf = instance_buf(device, "kagra-shared point instances", 32, cap);
                self.point_inst_capacity = cap;
            }
            queue.write_buffer(&self.point_inst_buf, 0, bytemuck::cast_slice(&points));
        }
        if !lines.is_empty() {
            if lines.len() > self.line_inst_capacity {
                let cap = lines.len().next_power_of_two();
                self.line_inst_buf = instance_buf(device, "kagra-shared line instances", 48, cap);
                self.line_inst_capacity = cap;
            }
            queue.write_buffer(&self.line_inst_buf, 0, bytemuck::cast_slice(&lines));
        }
        (points.len() as u32, lines.len() as u32)
    }

    fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, n_points: u32, n_lines: u32) {
        if n_points == 0 && n_lines == 0 {
            return;
        }
        pass.set_bind_group(0, &self.stroke_bind, &[]);
        if n_points > 0 {
            pass.set_pipeline(&self.pipeline_points);
            pass.set_vertex_buffer(0, self.point_corner_buf.slice(..));
            pass.set_vertex_buffer(1, self.point_inst_buf.slice(..));
            pass.draw(0..6, 0..n_points);
        }
        if n_lines > 0 {
            pass.set_pipeline(&self.pipeline_lines);
            pass.set_vertex_buffer(0, self.line_corner_buf.slice(..));
            pass.set_vertex_buffer(1, self.line_inst_buf.slice(..));
            pass.draw(0..6, 0..n_lines);
        }
    }
}

fn draw_counts(meshes: &[GpuMesh], draws: &[(usize, u64, u32, bool, bool)]) -> (u32, u32) {
    let mut draw_count = 0u32;
    let mut triangles = 0u32;
    for (mesh, _, n, _, _) in draws {
        draw_count += 1;
        let idx = meshes.get(*mesh).map(|m| m.index_count).unwrap_or(0);
        triangles = triangles.saturating_add(idx / 3 * *n);
    }
    (draw_count, triangles)
}

fn create_timestamps(device: &wgpu::Device) -> Option<TimestampClock> {
    let want = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
    if !device.features().contains(want) {
        return None;
    }
    let query = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("kagra-shared timestamps"),
        ty: wgpu::QueryType::Timestamp,
        count: 2,
    });
    let resolve = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("kagra-shared timestamp resolve"),
        size: 256,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("kagra-shared timestamp read"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    Some(TimestampClock {
        query,
        resolve,
        staging,
    })
}

fn read_gpu_ms(device: &wgpu::Device, queue: &wgpu::Queue, staging: &wgpu::Buffer) -> Option<f32> {
    let (tx, rx) = std::sync::mpsc::channel();
    staging
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |res| {
            let _ = tx.send(res);
        });
    if device.poll(wgpu::PollType::wait_indefinitely()).is_err() {
        return None;
    }
    if !matches!(rx.recv(), Ok(Ok(()))) {
        return None;
    }
    let view = match staging.slice(..).get_mapped_range() {
        Ok(view) => view,
        Err(_) => {
            staging.unmap();
            return None;
        }
    };
    let ms = if view.len() < 16 {
        None
    } else {
        let t0 = u64::from_le_bytes([
            view[0], view[1], view[2], view[3], view[4], view[5], view[6], view[7],
        ]);
        let t1 = u64::from_le_bytes([
            view[8], view[9], view[10], view[11], view[12], view[13], view[14], view[15],
        ]);
        if t1 <= t0 {
            None
        } else {
            let period = queue.get_timestamp_period();
            let ms = (t1 - t0) as f32 * period / 1_000_000.0;
            if ms.is_finite() && (0.0..10_000.0).contains(&ms) {
                Some(ms)
            } else {
                None
            }
        }
    };
    drop(view);
    staging.unmap();
    ms
}

async fn request_device(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'static>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue), String> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: surface,
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        })
        .await
        .map_err(|e| e.to_string())?;

    // WebGL2 は制限が厳しいので、wasm ではそこに合わせる。
    let limits = if cfg!(target_arch = "wasm32") {
        wgpu::Limits::downlevel_webgl2_defaults()
    } else {
        wgpu::Limits::default()
    };

    // wgpu 30 has no DEPTH32FLOAT_FILTERABLE feature. Comparison itself is
    // core; linear filtering of that compare is not. The shadow sampler
    // stays nearest so mobile WebGPU does not reject the draw.
    // Wasm does not request timestamp queries. Compression bits are still
    // requested when this adapter already lists them, including on wasm:
    // WebGPU will not create a compressed texture unless the feature was
    // required at device creation. A missing bit leaves KTX2 on the RGBA path.
    let mut required_features = wgpu::Features::empty();
    let want_ts = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
    if !cfg!(target_arch = "wasm32") && adapter.features().contains(want_ts) {
        required_features |= want_ts;
    }
    for bit in [
        wgpu::Features::TEXTURE_COMPRESSION_BC,
        wgpu::Features::TEXTURE_COMPRESSION_ETC2,
        wgpu::Features::TEXTURE_COMPRESSION_ASTC,
    ] {
        if adapter.features().contains(bit) {
            required_features |= bit;
        }
    }
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("kagra-shared"),
            required_features,
            required_limits: limits.using_resolution(adapter.limits()),
            ..Default::default()
        })
        .await
        .map_err(|e| e.to_string())?;
    // Validation errors on wasm otherwise become an uncatchable abort, and
    // the page stays black. Log them; the blank-frame check can still fall
    // back to the other backend.
    device.on_uncaptured_error(std::sync::Arc::new(|err: wgpu::Error| {
        note_renderer(&format!("kagra: wgpu error: {err}"));
    }));
    Ok((adapter, device, queue))
}

fn complete_mip_count(width: u32, height: u32) -> u32 {
    let mut w = width.max(1);
    let mut h = height.max(1);
    let mut count = 1u32;
    while w > 1 || h > 1 {
        w = (w / 2).max(1);
        h = (h / 2).max(1);
        count += 1;
        if count >= 16 {
            break;
        }
    }
    count
}

fn address_mode(wrap: crate::scene3d::TexWrap) -> wgpu::AddressMode {
    match wrap {
        crate::scene3d::TexWrap::Clamp => wgpu::AddressMode::ClampToEdge,
        crate::scene3d::TexWrap::Repeat => wgpu::AddressMode::Repeat,
        crate::scene3d::TexWrap::Mirror => wgpu::AddressMode::MirrorRepeat,
    }
}

fn mesh_sampler(device: &wgpu::Device, desc: &crate::scene3d::TexSampler) -> wgpu::Sampler {
    // Anisotropic filtering requires every filter to be linear. wgpu 30 has no
    // sampler-anisotropy limit; backends may ignore the clamp.
    let aniso = if desc.mag_nearest || desc.min_nearest || desc.mip_nearest {
        1
    } else {
        (desc.anisotropy as u16).max(1)
    };
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("kagra-shared mesh sampler"),
        address_mode_u: address_mode(desc.wrap_u),
        address_mode_v: address_mode(desc.wrap_v),
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: if desc.mag_nearest {
            wgpu::FilterMode::Nearest
        } else {
            wgpu::FilterMode::Linear
        },
        min_filter: if desc.min_nearest {
            wgpu::FilterMode::Nearest
        } else {
            wgpu::FilterMode::Linear
        },
        mipmap_filter: if desc.mip_nearest {
            wgpu::MipmapFilterMode::Nearest
        } else {
            wgpu::MipmapFilterMode::Linear
        },
        anisotropy_clamp: aniso,
        ..Default::default()
    })
}

/// WebGPU `GPUQueue.writeTexture` rejects a `bytesPerRow` that is not a
/// multiple of 256. Native wgpu pads internally; the browser backend does not.
fn webgpu_row_pitch(row_bytes: u32) -> u32 {
    const ALIGN: u32 = 256;
    row_bytes.div_ceil(ALIGN) * ALIGN
}

fn pad_rows_to_256(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let width = width.max(1);
    let height = height.max(1);
    let row = (4 * width) as usize;
    let pitch = webgpu_row_pitch(row as u32) as usize;
    if pitch == row && rgba.len() >= row * height as usize {
        return rgba[..row * height as usize].to_vec();
    }
    let mut staged = vec![0u8; pitch * height as usize];
    for y in 0..height as usize {
        let src = y * row;
        let dst = y * pitch;
        let n = row.min(rgba.len().saturating_sub(src));
        if n > 0 {
            staged[dst..dst + n].copy_from_slice(&rgba[src..src + n]);
        }
    }
    staged
}

fn upload_albedo(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mesh: &crate::scene3d::MeshData,
) -> Option<wgpu::Texture> {
    if let Some(ktx) = &mesh.albedo_ktx {
        if let Some(tex) = create_ktx_texture(device, queue, ktx) {
            return Some(tex);
        }
    }
    let alb = mesh.albedo.as_ref()?;
    if alb.width == 0 || alb.height == 0 || alb.rgba.is_empty() {
        return None;
    }
    Some(create_albedo_texture(
        device, queue, alb.width, alb.height, &alb.rgba,
    ))
}

fn wgpu_color_format(vk_format: u32) -> Option<wgpu::TextureFormat> {
    let format = ktx2::Format::new(vk_format)?;
    use ktx2::Format as F;
    use wgpu::AstcBlock::*;
    use wgpu::AstcChannel::{Unorm, UnormSrgb};
    use wgpu::TextureFormat as W;
    let astc = |block, channel| W::Astc { block, channel };
    Some(match format {
        F::BC1_RGB_UNORM_BLOCK | F::BC1_RGBA_UNORM_BLOCK => W::Bc1RgbaUnorm,
        F::BC1_RGB_SRGB_BLOCK | F::BC1_RGBA_SRGB_BLOCK => W::Bc1RgbaUnormSrgb,
        F::BC2_UNORM_BLOCK => W::Bc2RgbaUnorm,
        F::BC2_SRGB_BLOCK => W::Bc2RgbaUnormSrgb,
        F::BC3_UNORM_BLOCK => W::Bc3RgbaUnorm,
        F::BC3_SRGB_BLOCK => W::Bc3RgbaUnormSrgb,
        F::BC7_UNORM_BLOCK => W::Bc7RgbaUnorm,
        F::BC7_SRGB_BLOCK => W::Bc7RgbaUnormSrgb,
        F::ETC2_R8G8B8_UNORM_BLOCK => W::Etc2Rgb8Unorm,
        F::ETC2_R8G8B8_SRGB_BLOCK => W::Etc2Rgb8UnormSrgb,
        F::ETC2_R8G8B8A1_UNORM_BLOCK => W::Etc2Rgb8A1Unorm,
        F::ETC2_R8G8B8A1_SRGB_BLOCK => W::Etc2Rgb8A1UnormSrgb,
        F::ETC2_R8G8B8A8_UNORM_BLOCK => W::Etc2Rgba8Unorm,
        F::ETC2_R8G8B8A8_SRGB_BLOCK => W::Etc2Rgba8UnormSrgb,
        F::ASTC_4x4_UNORM_BLOCK => astc(B4x4, Unorm),
        F::ASTC_4x4_SRGB_BLOCK => astc(B4x4, UnormSrgb),
        F::ASTC_5x4_UNORM_BLOCK => astc(B5x4, Unorm),
        F::ASTC_5x4_SRGB_BLOCK => astc(B5x4, UnormSrgb),
        F::ASTC_5x5_UNORM_BLOCK => astc(B5x5, Unorm),
        F::ASTC_5x5_SRGB_BLOCK => astc(B5x5, UnormSrgb),
        F::ASTC_6x5_UNORM_BLOCK => astc(B6x5, Unorm),
        F::ASTC_6x5_SRGB_BLOCK => astc(B6x5, UnormSrgb),
        F::ASTC_6x6_UNORM_BLOCK => astc(B6x6, Unorm),
        F::ASTC_6x6_SRGB_BLOCK => astc(B6x6, UnormSrgb),
        F::ASTC_8x5_UNORM_BLOCK => astc(B8x5, Unorm),
        F::ASTC_8x5_SRGB_BLOCK => astc(B8x5, UnormSrgb),
        F::ASTC_8x6_UNORM_BLOCK => astc(B8x6, Unorm),
        F::ASTC_8x6_SRGB_BLOCK => astc(B8x6, UnormSrgb),
        F::ASTC_8x8_UNORM_BLOCK => astc(B8x8, Unorm),
        F::ASTC_8x8_SRGB_BLOCK => astc(B8x8, UnormSrgb),
        F::ASTC_10x5_UNORM_BLOCK => astc(B10x5, Unorm),
        F::ASTC_10x5_SRGB_BLOCK => astc(B10x5, UnormSrgb),
        F::ASTC_10x6_UNORM_BLOCK => astc(B10x6, Unorm),
        F::ASTC_10x6_SRGB_BLOCK => astc(B10x6, UnormSrgb),
        F::ASTC_10x8_UNORM_BLOCK => astc(B10x8, Unorm),
        F::ASTC_10x8_SRGB_BLOCK => astc(B10x8, UnormSrgb),
        F::ASTC_10x10_UNORM_BLOCK => astc(B10x10, Unorm),
        F::ASTC_10x10_SRGB_BLOCK => astc(B10x10, UnormSrgb),
        F::ASTC_12x10_UNORM_BLOCK => astc(B12x10, Unorm),
        F::ASTC_12x10_SRGB_BLOCK => astc(B12x10, UnormSrgb),
        F::ASTC_12x12_UNORM_BLOCK => astc(B12x12, Unorm),
        F::ASTC_12x12_SRGB_BLOCK => astc(B12x12, UnormSrgb),
        _ => return None,
    })
}

fn create_ktx_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    ktx: &crate::ktx_image::KtxImage,
) -> Option<wgpu::Texture> {
    let format = wgpu_color_format(ktx.vk_format)?;
    if !device.features().contains(format.required_features()) {
        return None;
    }
    let (bw, bh) = format.block_dimensions();
    let block = format.block_copy_size(None)?;
    if !ktx.width.is_multiple_of(bw) || !ktx.height.is_multiple_of(bh) {
        return None;
    }
    let blocks_x = ktx.width / bw;
    let blocks_y = ktx.height / bh;
    let row = blocks_x * block;
    let pitch = webgpu_row_pitch(row);
    let need = (row as usize) * (blocks_y as usize);
    if ktx.level0.len() < need {
        return None;
    }
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kagra-shared ktx2"),
        size: wgpu::Extent3d {
            width: ktx.width,
            height: ktx.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut staged = vec![0u8; pitch as usize * blocks_y as usize];
    for y in 0..blocks_y as usize {
        let src = y * row as usize;
        let dst = y * pitch as usize;
        staged[dst..dst + row as usize].copy_from_slice(&ktx.level0[src..src + row as usize]);
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &staged,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(pitch),
            rows_per_image: Some(blocks_y),
        },
        wgpu::Extent3d {
            width: ktx.width,
            height: ktx.height,
            depth_or_array_layers: 1,
        },
    );
    Some(tex)
}

fn create_albedo_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> wgpu::Texture {
    create_rgba_texture(
        device,
        queue,
        width,
        height,
        rgba,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    )
}

fn create_linear_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> wgpu::Texture {
    create_rgba_texture(
        device,
        queue,
        width,
        height,
        rgba,
        wgpu::TextureFormat::Rgba8Unorm,
    )
}

fn create_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    width: u32,
    height: u32,
    rgba: &[u8],
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    let width = width.max(1);
    let height = height.max(1);
    let expected = (width as usize) * (height as usize) * 4;
    let mip_level_count = if rgba.len() >= expected {
        complete_mip_count(width, height)
    } else {
        1
    };
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kagra-shared albedo"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let data = if rgba.len() >= expected {
        &rgba[..expected]
    } else {
        rgba
    };
    // WebGPU writeTexture requires bytesPerRow to be a multiple of 256.
    // wgpu-core pads this itself; the wasm backend forwards the number
    // straight through, so a 4×1 Kenney palette (16 bytes/row) is rejected
    // and the sample comes back opaque white. Pad every row.
    let staged = pad_rows_to_256(data, width, height);
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &staged,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(webgpu_row_pitch(4 * width)),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    if mip_level_count > 1 {
        let srgb = format.is_srgb();
        let mut prev = data.to_vec();
        let mut pw = width;
        let mut ph = height;
        for level in 1..mip_level_count {
            let nw = (pw / 2).max(1);
            let nh = (ph / 2).max(1);
            let next = color_space::box_downsample(&prev, pw, ph, srgb);
            let staged = pad_rows_to_256(&next, nw, nh);
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: level,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &staged,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(webgpu_row_pitch(4 * nw)),
                    rows_per_image: Some(nh),
                },
                wgpu::Extent3d {
                    width: nw,
                    height: nh,
                    depth_or_array_layers: 1,
                },
            );
            prev = next;
            pw = nw;
            ph = nh;
        }
    }
    tex
}

fn create_offscreen_texture(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kagra-shared offscreen"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn create_vertex_buffer(device: &wgpu::Device, vertices: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("kagra-shared quads"),
        size: (vertices * std::mem::size_of::<Vertex>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_instance_buffer(device: &wgpu::Device, instances: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("kagra-shared instances"),
        size: (instances.max(1) * std::mem::size_of::<InstanceRaw>()) as wgpu::BufferAddress,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_shadow_map(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let size = crate::scene3d::SHADOW_MAP_SIZE.max(1);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kagra-shared shadow map"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("kagra-shared shadow view"),
        format: Some(DEPTH_FORMAT),
        dimension: Some(wgpu::TextureViewDimension::D2),
        usage: None,
        aspect: wgpu::TextureAspect::DepthOnly,
        base_mip_level: 0,
        mip_level_count: Some(1),
        base_array_layer: 0,
        array_layer_count: Some(1),
    });
    (texture, view)
}

/// Device z for the PCSS blocker search. Always allocated so the shadow bind
/// group stays valid when the dump leaves PCSS off. The lighting shader only
/// samples it while `shadow.w` is on.
fn create_shadow_raw(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let size = crate::scene3d::SHADOW_MAP_SIZE.max(1);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kagra-shared shadow raw"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    (texture, view)
}

fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kagra-shared depth"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    texture.create_view(&Default::default())
}

/// 中身入りのバッファを作る。`wgpu::util` に頼らずに済ませる。
///
/// `mapped_at_creation` is avoided on purpose. Chrome's WebGPU (Pixel / Dawn)
/// rejects even a 768-byte mesh with "size is too large when mappedAtCreation
/// == true", and the unwrap in wgpu then kills the frame. `write_buffer` is
/// the path those devices accept.
fn create_init_buffer(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    data: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    let usage = usage | wgpu::BufferUsages::COPY_DST;
    // サイズ 0 のバッファは作れないので、空なら最小サイズを空のまま置く。
    if data.is_empty() {
        return device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: 4,
            usage,
            mapped_at_creation: false,
        });
    }
    // WebGPU requires the size (and the write) to be a multiple of 4.
    let size = (data.len() as u64).div_ceil(4) * 4;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage,
        mapped_at_creation: false,
    });
    if data.len() as u64 == size {
        queue.write_buffer(&buffer, 0, data);
    } else {
        let mut padded = vec![0u8; size as usize];
        padded[..data.len()].copy_from_slice(data);
        queue.write_buffer(&buffer, 0, &padded);
    }
    buffer
}

/// 1 つの Quad → 三角形 2 つ分の頂点（左上原点、y 下向き）。
fn quad_vertices(q: &crate::scene::Quad, format: wgpu::TextureFormat) -> [Vertex; 6] {
    let color = color_space::display_u8(q.color, format.is_srgb());
    let (l, t, r, b) = (q.x, q.y, q.x + q.w, q.y + q.h);
    let corners = [[l, t], [r, t], [r, b], [l, t], [r, b], [l, b]];
    std::array::from_fn(|i| Vertex {
        pos: corners[i],
        color,
    })
}

/// `{tier} {backend} fxaa={0|1} bloom={0|1} ssao={n:.0} c{cascades} {post|swap|hdr}`
#[allow(clippy::too_many_arguments)]
fn picture_line(
    tier: &str,
    backend: &str,
    fxaa: bool,
    bloom: bool,
    ssao: f32,
    cascades: u32,
    direct: bool,
    direct_post: bool,
) -> String {
    let path = if direct_post {
        "post"
    } else if direct {
        "swap"
    } else {
        "hdr"
    };
    format!(
        "{tier} {backend} fxaa={} bloom={} ssao={:.0} c{cascades} {path}",
        u8::from(fxaa),
        u8::from(bloom),
        ssao,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_implicit_sample(src: &str) -> bool {
        src.match_indices("textureSample").any(|(i, _)| {
            let rest = &src[i..];
            !rest.starts_with("textureSampleLevel") && !rest.starts_with("textureSampleCompare")
        })
    }

    #[test]
    fn webgpu_texture_rows_are_256_aligned() {
        assert_eq!(webgpu_row_pitch(4), 256);
        assert_eq!(webgpu_row_pitch(16), 256);
        assert_eq!(webgpu_row_pitch(256), 256);
        assert_eq!(webgpu_row_pitch(260), 512);
        let tight = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let padded = pad_rows_to_256(&tight, 1, 2);
        assert_eq!(padded.len(), 512);
        assert_eq!(&padded[0..4], &[1, 2, 3, 4]);
        assert_eq!(&padded[256..260], &[5, 6, 7, 8]);
        assert!(padded[4..256].iter().all(|b| *b == 0));
    }

    #[test]
    fn surface_format_skips_float_canvas() {
        let webgpu = [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba16Float,
        ];
        assert_eq!(
            choose_surface_format(&webgpu),
            Some(wgpu::TextureFormat::Bgra8Unorm)
        );
        let gl = [
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ];
        assert_eq!(
            choose_surface_format(&gl),
            Some(wgpu::TextureFormat::Rgba8UnormSrgb)
        );
        assert_eq!(parse_graphics_backend("webgl2"), GraphicsBackend::Gl);
    }

    #[test]
    fn srgb_conversion_endpoints() {
        let black = color_space::srgb_u8_to_linear([0, 0, 0, 255]);
        assert_eq!(black[0], 0.0);
        let white = color_space::srgb_u8_to_linear([255, 255, 255, 255]);
        assert!((white[0] - 1.0).abs() < 1e-5);
        // HUD on an unorm target keeps the code. Lighting always decodes.
        let plain = color_space::display_u8([128, 0, 0, 255], false);
        assert!((plain[0] - 128.0 / 255.0).abs() < 1e-6);
        let lit = color_space::srgb_u8_to_linear([128, 0, 0, 255]);
        assert!(lit[0] < 0.25, "author 128 is linear ~0.216, got {}", lit[0]);
    }

    #[test]
    fn shader3d_has_ibl_and_aces() {
        let src = include_str!("shader3d.wgsl");
        // ACES は HDR+bloom コミット以降 composite（bloom.wgsl）で適用される。
        let composite = include_str!("bloom.wgsl");
        assert!(
            composite.contains("aces_tonemap"),
            "ACES in bloom composite"
        );
        assert!(composite.contains("fn sample_lut"), "strip LUT");
        assert!(
            composite.contains("params.v.w"),
            "vignette strength is a uniform"
        );
        assert!(src.contains("env_irradiance"), "diffuse IBL / tiny SH");
        assert!(src.contains("env: vec4<f32>"), "Globals.env");
        assert!(
            src.contains("mat_id == 6"),
            "water material on shared family"
        );
        assert!(src.contains("water_normal"), "scrolling procedural waves");
        assert!(src.contains("fn shadow_factor"), "directional PCF");
        assert!(src.contains("textureSampleCompare"), "depth compare PCF");
        assert!(
            src.contains("textureSampleCompareLevel"),
            "PCF must be explicit-LOD so a loop is valid WebGPU"
        );
        assert!(
            src.contains("in.mtoon4.z > 0.30"),
            "emissive ribbons skip the shadow map and get a clip bias"
        );
        assert!(src.contains("fn vs_shadow"), "shadow pass");
        assert!(src.contains("2048.0"), "PCF texel matches SHADOW_MAP_SIZE");
        assert!(src.contains("orm_tex"), "roughness / metal map");
        assert!(src.contains("emissive_rgb"), "authored emissive");
        assert!(src.contains("radius <= 1.01"), "legacy 3x3 shadow kernel");
        assert!(src.contains("fn env_specular"), "prefiltered env");
        assert!(src.contains("textureSampleLevel"), "explicit lod, WebGL2");
        let fs = src.split("fn fs_main").nth(1).expect("fs_main");
        let sky = fs.find("mat_id == 3").expect("sky branch");
        assert!(
            !has_implicit_sample(&fs[sky..]),
            "textureSample after a non-uniform branch is rejected by WebGPU"
        );
        let shadow_call = fs.find("shadow_factor(").expect("shadow call");
        assert!(
            shadow_call < sky,
            "textureSampleCompare must run before the sky return"
        );
        assert!(
            src.contains("lit == lit"),
            "NaN shadow compare must not blank the fragment"
        );
        assert!(
            composite.contains("linear_to_srgb"),
            "unorm swapchains encode sRGB in the shader"
        );
        assert!(
            composite.contains("params.v.x > 1e-4"),
            "bloom intensity 0 must not add an undefined sample"
        );
        assert!(
            src.contains("fn present_rgb"),
            "direct unorm path grades in the fragment shader"
        );
        assert!(src.contains("g.shadow.y"), "present flag rides shadow.y");
        assert!(src.contains("fn vs_shadow_1"), "cascade 1 entry");
        assert!(src.contains("shadow_raw"), "PCSS raw depth");
        assert!(src.contains("g.cascade.x"), "cascade count uniform");
        let fxaa = include_str!("fxaa.wgsl");
        let fx = fxaa.split("fn fs_main").nth(1).expect("fxaa fs");
        let early = fx.find("l_range <").expect("fxaa early out");
        assert!(
            !has_implicit_sample(&fx[early..]),
            "FXAA textureSample must stay above the contrast return"
        );
        assert!(src.contains("hemi_sky"), "authorable hemisphere");
        assert!(!src.contains("var<storage"), "WebGL2: no storage buffers");
        // MToon 完全移植でインスタンススロットは 2..=12（mtoon2/3/4）まで使う。
        assert!(
            src.contains("@location(12) mtoon4"),
            "MToon instance slots wired up to 12"
        );
        assert!(src.contains("@location(13) vcolor"), "vertex color");
        assert!(src.contains("@location(14) tangent"), "tangent");
        assert!(src.contains("@location(15) alpha_cut"), "alpha mode");
        assert!(src.contains("fn normal_from_map"), "tangent-space normal");
    }

    #[test]
    fn hdr_frame_requires_filter_and_blend() {
        let need = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let ok = wgpu::TextureFormatFeatureFlags::FILTERABLE
            | wgpu::TextureFormatFeatureFlags::BLENDABLE;
        assert!(bloom::format_supports_hdr_frame(need, ok));
        assert!(!bloom::format_supports_hdr_frame(
            wgpu::TextureUsages::TEXTURE_BINDING,
            ok
        ));
        assert!(!bloom::format_supports_hdr_frame(
            need,
            wgpu::TextureFormatFeatureFlags::FILTERABLE
        ));
        assert!(!bloom::format_supports_hdr_frame(
            need,
            wgpu::TextureFormatFeatureFlags::BLENDABLE
        ));
    }

    #[test]
    fn globals_env_is_16_aligned() {
        assert_eq!(std::mem::size_of::<Globals>() % 16, 0);
        // 448 + two cascade matrices + the cascade vec4
        assert_eq!(std::mem::size_of::<Globals>(), 592);
        assert_eq!(crate::scene3d::SHADOW_MAP_SIZE, 2048);
    }

    fn pixel_of(rgba: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * w + x) * 4) as usize;
        [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
    }

    fn assert_rgb_near(got: [u8; 4], expect: [u8; 4], tol: i32) {
        for i in 0..3 {
            let d = (got[i] as i32 - expect[i] as i32).abs();
            assert!(
                d <= tol,
                "channel {i} got {got:?} expect {expect:?} (tol {tol})"
            );
        }
    }

    fn write_shot(name: &str, w: u32, h: u32, rgba: &[u8]) {
        let Ok(dir) = std::env::var("KAGRA_COLOR_SHOT") else {
            return;
        };
        let path = std::path::Path::new(&dir).join(name);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let Ok(file) = std::fs::File::create(&path) else {
            return;
        };
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        if let Ok(mut writer) = encoder.write_header() {
            let _ = writer.write_image_data(rgba);
        }
    }

    /// Empty scene, bloom off, FXAA off, vignette off. The center pixel is the
    /// clear color after one grade.
    fn grade_clear(format: wgpu::TextureFormat, tonemap: bool) -> Option<Vec<u8>> {
        grade_clear_curve(format, tonemap, 0.0)
    }

    fn grade_clear_curve(
        format: wgpu::TextureFormat,
        tonemap: bool,
        curve: f32,
    ) -> Option<Vec<u8>> {
        let mut renderer =
            pollster::block_on(Renderer::new_offscreen_format(64, 64, format)).ok()?;
        renderer.set_fxaa(false);
        renderer.bloom.set_grade(0.85, 0.0, 0.0, false, "off", 0.0);
        let scene = crate::scene3d::Scene3D {
            clear: [128, 64, 32, 255],
            tonemap,
            tonemap_code: curve,
            exposure: 1.0,
            ibl: 0.0,
            batches: Vec::new(),
            ..Default::default()
        };
        renderer
            .render_frame(Some(&scene), &crate::scene::DrawList::default())
            .ok()?;
        renderer.read_rgba().ok()
    }

    fn hud_ramp(format: wgpu::TextureFormat) -> Option<Vec<u8>> {
        let mut renderer =
            pollster::block_on(Renderer::new_offscreen_format(80, 16, format)).ok()?;
        renderer.set_fxaa(false);
        renderer.bloom.set_grade(0.85, 0.0, 0.0, false, "off", 0.0);
        let mut hud = crate::scene::DrawList::default();
        for (i, byte) in [0u8, 64, 128, 192, 255].iter().enumerate() {
            hud.quads.push(crate::scene::Quad::new(
                i as f32 * 16.0,
                0.0,
                16.0,
                16.0,
                [*byte, *byte, *byte, 255],
            ));
        }
        renderer.render_frame(None, &hud).ok()?;
        renderer.read_rgba().ok()
    }

    #[test]
    fn offscreen_clear_matches_reference_on_srgb_and_unorm() {
        let author = [128u8, 64, 32, 255];
        let plain = color_space::reference_byte(author, 1.0, false);
        let graded = color_space::reference_byte(author, 1.0, true);
        let cases = [
            (
                wgpu::TextureFormat::Rgba8UnormSrgb,
                false,
                plain,
                "srgb-plain",
            ),
            (
                wgpu::TextureFormat::Rgba8UnormSrgb,
                true,
                graded,
                "srgb-aces",
            ),
            (wgpu::TextureFormat::Rgba8Unorm, false, plain, "unorm-plain"),
            (wgpu::TextureFormat::Rgba8Unorm, true, graded, "unorm-aces"),
        ];
        for (format, tonemap, expect, name) in cases {
            let Some(rgba) = grade_clear(format, tonemap) else {
                eprintln!("no GPU adapter; skipping linear reference {name}");
                return;
            };
            let got = pixel_of(&rgba, 64, 32, 32);
            write_shot(&format!("clear-{name}.png"), 64, 64, &rgba);
            assert_rgb_near(got, expect, 3);
            assert_eq!(got[3], 255, "{name} alpha");
            if !tonemap {
                assert!(
                    (got[0] as i32 - 188).abs() > 20,
                    "{name} looks like a missing sRGB decode: {got:?}"
                );
            }
        }
    }

    #[test]
    fn offscreen_agx_and_neutral_match_the_cpu_curve() {
        let author = [128u8, 64, 32, 255];
        for (curve, name) in [(1.0f32, "agx"), (2.0, "neutral")] {
            let expect = color_space::reference_curve(author, 1.0, true, curve);
            for format in [
                wgpu::TextureFormat::Rgba8UnormSrgb,
                wgpu::TextureFormat::Rgba8Unorm,
            ] {
                let Some(rgba) = grade_clear_curve(format, true, curve) else {
                    eprintln!("no GPU adapter; skipping {name}");
                    return;
                };
                let got = pixel_of(&rgba, 64, 32, 32);
                write_shot(&format!("clear-{name}.png"), 64, 64, &rgba);
                assert_rgb_near(got, expect, 3);
            }
        }
    }

    fn mean_channel_delta(a: &[u8], b: &[u8]) -> f32 {
        let n = (a.len().min(b.len()) / 4).max(1);
        let mut acc = 0i64;
        for i in 0..n {
            for c in 0..3 {
                acc += (a[i * 4 + c] as i32 - b[i * 4 + c] as i32).abs() as i64;
            }
        }
        acc as f32 / (n as f32 * 3.0)
    }

    #[test]
    fn offscreen_clearcoat_and_atmosphere_change_the_picture() {
        let mut renderer = match pollster::block_on(Renderer::new_offscreen_format(
            96,
            96,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )) {
            Ok(r) => r,
            Err(_) => {
                eprintln!("no GPU adapter; skipping coat and sky");
                return;
            }
        };
        renderer.set_fxaa(false);
        renderer.bloom.set_grade(0.85, 0.0, 0.0, false, "off", 0.0);
        let sphere = crate::scene3d::primitives::sphere_mesh(1.0, 12);
        let id = renderer.upload_mesh(&sphere);
        let cam = crate::scene3d::Camera {
            eye: glam::Vec3::new(0.0, 0.15, 3.1),
            target: glam::Vec3::ZERO,
            ..Default::default()
        };
        let bare = crate::scene3d::Instance::with_pbr(
            glam::Mat4::IDENTITY,
            [36, 42, 56, 255],
            crate::scene3d::Material::Solid,
            0.0,
            1.0,
        );
        let mut coat = bare;
        coat.clearcoat = 1.0;
        coat.clearcoat_roughness = 0.05;
        let paint = |renderer: &mut Renderer, inst: crate::scene3d::Instance| {
            let scene = crate::scene3d::Scene3D {
                camera: cam,
                clear: [8, 10, 16, 255],
                light_dir: glam::Vec3::new(0.1, 0.25, 1.0).normalize(),
                ambient: 0.05,
                ibl: 1.0,
                tonemap: true,
                exposure: 1.15,
                fog_start: 40.0,
                fog_end: 120.0,
                batches: vec![crate::scene3d::Batch {
                    mesh: id,
                    instances: vec![inst],
                }],
                ..Default::default()
            };
            renderer
                .render_frame(Some(&scene), &crate::scene::DrawList::default())
                .expect("coat frame");
            renderer.read_rgba().expect("coat read")
        };
        let dull = paint(&mut renderer, bare);
        let shiny = paint(&mut renderer, coat);
        write_shot("clearcoat-off.png", 96, 96, &dull);
        write_shot("clearcoat-on.png", 96, 96, &shiny);
        let coat_delta = mean_channel_delta(&dull, &shiny);
        assert!(
            coat_delta > 1.5,
            "clearcoat should move the sphere, mean delta {coat_delta}"
        );

        let sky_cam = crate::scene3d::Camera {
            eye: glam::Vec3::ZERO,
            target: glam::Vec3::new(0.15, 0.02, 1.0),
            up: glam::Vec3::Y,
            ..Default::default()
        };
        let sky_model = glam::Mat4::from_scale_rotation_translation(
            glam::Vec3::splat(40.0),
            glam::Quat::IDENTITY,
            sky_cam.eye,
        );
        let sky_inst = crate::scene3d::Instance::with_material(
            sky_model,
            [130, 165, 205, 255],
            crate::scene3d::Material::Sky,
        );
        let sky = |renderer: &mut Renderer, atmosphere: bool| {
            let scene = crate::scene3d::Scene3D {
                camera: sky_cam,
                clear: [130, 165, 205, 255],
                light_dir: glam::Vec3::new(0.2, 0.45, 0.15).normalize(),
                ambient: 0.4,
                ibl: 0.0,
                tonemap: true,
                exposure: 1.0,
                fog_start: 500.0,
                fog_end: 800.0,
                sky_atmosphere: atmosphere,
                batches: vec![crate::scene3d::Batch {
                    mesh: id,
                    instances: vec![sky_inst],
                }],
                ..Default::default()
            };
            renderer
                .render_frame(Some(&scene), &crate::scene::DrawList::default())
                .expect("sky frame");
            renderer.read_rgba().expect("sky read")
        };
        let gradient = sky(&mut renderer, false);
        let air = sky(&mut renderer, true);
        write_shot("sky-gradient.png", 96, 96, &gradient);
        write_shot("sky-atmosphere.png", 96, 96, &air);
        let sky_delta = mean_channel_delta(&gradient, &air);
        assert!(
            sky_delta > 2.0,
            "atmosphere should replace the studio gradient, mean delta {sky_delta}"
        );
    }

    #[test]
    fn offscreen_hud_ramp_is_one_encode() {
        for (format, name) in [
            (wgpu::TextureFormat::Rgba8UnormSrgb, "srgb"),
            (wgpu::TextureFormat::Rgba8Unorm, "unorm"),
        ] {
            let Some(rgba) = hud_ramp(format) else {
                eprintln!("no GPU adapter; skipping HUD ramp {name}");
                return;
            };
            write_shot(&format!("hud-ramp-{name}.png"), 80, 16, &rgba);
            for (i, byte) in [0u8, 64, 128, 192, 255].iter().enumerate() {
                let got = pixel_of(&rgba, 80, i as u32 * 16 + 8, 8);
                let expect = [*byte, *byte, *byte, 255];
                assert_rgb_near(got, expect, 1);
            }
        }
    }

    fn solid_box(pos: [f32; 3], scale: [f32; 3], color: [u8; 4]) -> crate::scene3d::Instance {
        let model = glam::Mat4::from_scale_rotation_translation(
            glam::Vec3::from(scale),
            glam::Quat::IDENTITY,
            glam::Vec3::from(pos),
        );
        crate::scene3d::Instance::with_pbr(model, color, crate::scene3d::Material::Solid, 0.0, 1.0)
    }

    fn graded(renderer: &mut Renderer) {
        renderer.set_fxaa(false);
        renderer.bloom.set_grade(0.85, 0.0, 0.0, false, "off", 0.0);
    }

    fn draw_boxes(renderer: &mut Renderer, instances: Vec<crate::scene3d::Instance>) -> Vec<u8> {
        let mesh = crate::scene3d::primitives::box_mesh(glam::Vec3::ONE);
        let id = renderer.upload_mesh(&mesh);
        let scene = crate::scene3d::Scene3D {
            clear: [40, 80, 160, 255],
            ambient: 1.0,
            ibl: 0.0,
            tonemap: false,
            exposure: 1.0,
            fog_start: 500.0,
            fog_end: 800.0,
            batches: vec![crate::scene3d::Batch {
                mesh: id,
                instances,
            }],
            ..Default::default()
        };
        renderer
            .render_frame(Some(&scene), &crate::scene::DrawList::default())
            .expect("render");
        renderer.read_rgba().expect("readback")
    }

    fn channel_sum(px: [u8; 4]) -> i32 {
        px[0] as i32 + px[1] as i32 + px[2] as i32
    }

    fn changed_pixels(a: &[u8], b: &[u8]) -> u32 {
        let mut changed = 0u32;
        for (px, qy) in a.chunks(4).zip(b.chunks(4)) {
            let da = (px[0] as i32 - qy[0] as i32).abs()
                + (px[1] as i32 - qy[1] as i32).abs()
                + (px[2] as i32 - qy[2] as i32).abs();
            if da >= 12 {
                changed += 1;
            }
        }
        changed
    }

    #[test]
    fn offscreen_ssao_darkens_contact_and_keeps_the_clear() {
        let Some(mut renderer) = pollster::block_on(Renderer::new_offscreen(192, 192)).ok() else {
            eprintln!("no GPU adapter; skipping ssao");
            return;
        };
        graded(&mut renderer);
        let boxes = vec![
            solid_box([0.0, 0.5, 0.0], [1.0, 1.0, 1.0], [255, 255, 255, 255]),
            solid_box([0.0, -0.05, 0.0], [8.0, 0.1, 8.0], [220, 220, 220, 255]),
        ];
        renderer.set_post(0.0, false, None, 0.0);
        let off = draw_boxes(&mut renderer, boxes.clone());
        renderer.set_post(24.0, false, None, 0.0);
        let on = draw_boxes(&mut renderer, boxes);
        write_shot("post-ssao-off.png", 192, 192, &off);
        write_shot("post-ssao-on.png", 192, 192, &on);
        assert_rgb_near(pixel_of(&on, 192, 2, 2), pixel_of(&off, 192, 2, 2), 6);
        let mut dropped = 0u32;
        let mut held = 0u32;
        for y in 0..192 {
            for x in 0..192 {
                let a = channel_sum(pixel_of(&off, 192, x, y));
                let b = channel_sum(pixel_of(&on, 192, x, y));
                if a - b >= 18 {
                    dropped += 1;
                }
                if a > 600 && (a - b).abs() <= 15 {
                    held += 1;
                }
            }
        }
        assert!(
            dropped > 30,
            "ssao should darken the cube/floor contact, dropped {dropped}"
        );
        assert!(
            held > 20,
            "open surfaces should stay bright, held {held} dropped {dropped}"
        );
    }

    #[test]
    fn unorm_ssao_darkens_contact_without_a_float_target() {
        let Some(mut renderer) = pollster::block_on(Renderer::new_offscreen_format(
            192,
            192,
            wgpu::TextureFormat::Rgba8Unorm,
        ))
        .ok() else {
            eprintln!("no GPU adapter; skipping unorm ssao");
            return;
        };
        graded(&mut renderer);
        let boxes = vec![
            solid_box([0.0, 0.5, 0.0], [1.0, 1.0, 1.0], [255, 255, 255, 255]),
            solid_box([0.0, -0.05, 0.0], [8.0, 0.1, 8.0], [220, 220, 220, 255]),
        ];
        renderer.set_post(0.5, false, None, 0.0);
        let soft = draw_boxes(&mut renderer, boxes.clone());
        renderer.set_post(24.0, false, None, 0.0);
        let hard = draw_boxes(&mut renderer, boxes);
        write_shot("post-ssao-unorm-soft.png", 192, 192, &soft);
        write_shot("post-ssao-unorm-hard.png", 192, 192, &hard);
        let clear = pixel_of(&hard, 192, 2, 2);
        assert!(
            channel_sum(clear) < 500,
            "unorm SSAO clear clipped {clear:?}"
        );
        let mut dropped = 0u32;
        for y in 0..192 {
            for x in 0..192 {
                let a = channel_sum(pixel_of(&soft, 192, x, y));
                let b = channel_sum(pixel_of(&hard, 192, x, y));
                if a - b >= 12 {
                    dropped += 1;
                }
            }
        }
        assert!(
            dropped > 20,
            "unorm SSAO should darken contact, dropped {dropped}"
        );
    }

    #[test]
    fn neon_mobile_tier_changes_the_lavapipe_picture() {
        let Some(mut renderer) = pollster::block_on(Renderer::new_offscreen(480, 270)).ok() else {
            eprintln!("no GPU adapter; skipping neon quality shots");
            return;
        };
        let low = crate::neon_run::NeonRun::from_json(
            r#"{"version":1,"genre":"neon_run","quality":"low"}"#,
        )
        .expect("low");
        let mobile = crate::neon_run::NeonRun::from_json(
            r#"{"version":1,"genre":"neon_run","quality":"mobile"}"#,
        )
        .expect("mobile");
        let before = renderer
            .render_world_doc(low.document())
            .expect("render low");
        let after = renderer
            .render_world_doc(mobile.document())
            .expect("render mobile");
        write_shot("neon-quality-low.png", 480, 270, &before);
        write_shot("neon-quality-mobile.png", 480, 270, &after);
        if !crate::world_doc::heavy_post_allowed() {
            return;
        }
        assert_eq!(low.document().shadow_cascade_count(), 1);
        assert_eq!(mobile.document().ssao_radius(), 4.0);
        assert_eq!(mobile.document().shadow_cascade_count(), 2);
        let mut changed = 0u32;
        for y in 0..270 {
            for x in 0..480 {
                let a = channel_sum(pixel_of(&before, 480, x, y));
                let b = channel_sum(pixel_of(&after, 480, x, y));
                if (a - b).abs() >= 8 {
                    changed += 1;
                }
            }
        }
        assert!(
            changed > 40,
            "mobile SSAO and cascades should move the neon frame, changed {changed}"
        );
    }

    #[test]
    fn stroke_shader_expands_quads_without_storage() {
        let src = include_str!("strokes.wgsl");
        assert!(src.contains("fn vs_point"));
        assert!(src.contains("fn vs_line"));
        assert!(!src.contains("var<storage"));
    }

    #[test]
    fn fat_line_and_points_change_the_frame() {
        let Some(mut renderer) = pollster::block_on(Renderer::new_offscreen(160, 90)).ok() else {
            eprintln!("no GPU adapter; skipping strokes");
            return;
        };
        let mut doc = crate::world_doc::WorldDoc {
            version: 1,
            ground: Some(false),
            sky: Some([8, 8, 16, 255]),
            bloom: Some(0.0),
            tonemap: Some(false),
            exposure: Some(1.0),
            ..Default::default()
        };
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "cam".into(),
            kind: "camera".into(),
            name: "main".into(),
            position: [0.0, 0.2, 3.2],
            target: [0.0, 0.0, 0.0],
            fov: 50.0,
            roll: 0.0,
        });
        doc.lines.push(crate::strokes::FatLine {
            id: "bar".into(),
            points: vec![[-1.4, 0.0, 0.0], [1.4, 0.0, 0.0]],
            color: [255, 255, 255, 255],
            width: 10.0,
        });
        doc.points.push(crate::strokes::PointCloud {
            id: "dot".into(),
            positions: vec![[0.0, 0.45, 0.0]],
            color: [255, 40, 180, 255],
            size: 0.22,
        });
        let on = renderer.render_world_doc(&doc).expect("strokes");
        write_shot("strokes-on.png", 160, 90, &on);
        let mut quiet = doc.clone();
        quiet.lines.clear();
        quiet.points.clear();
        let off = renderer.render_world_doc(&quiet).expect("quiet");
        write_shot("strokes-off.png", 160, 90, &off);
        let changed = changed_pixels(&on, &off);
        assert!(
            changed > 20,
            "strokes should move the frame, changed {changed}"
        );
        let Some(mut phone) = pollster::block_on(Renderer::new_ldr_unorm(160, 90)).ok() else {
            eprintln!("no unorm adapter; skipping phone strokes");
            return;
        };
        let phone_on = phone.render_world_doc(&doc).expect("phone strokes");
        let status = phone.picture_status();
        assert!(status.ends_with(" post"), "{status}");
        let phone_off = phone.render_world_doc(&quiet).expect("phone quiet");
        write_shot("strokes-phone.png", 160, 90, &phone_on);
        let phone_changed = changed_pixels(&phone_on, &phone_off);
        assert!(
            phone_changed > 20,
            "8-bit path should still draw strokes, changed {phone_changed} {status}"
        );
    }

    #[test]
    fn neon_ribbon_draws_on_the_phone_path() {
        let Some(mut renderer) = pollster::block_on(Renderer::new_ldr_unorm(320, 180)).ok() else {
            eprintln!("no unorm adapter; skipping neon ribbon");
            return;
        };
        let mut run = crate::neon_run::NeonRun::from_json(r#"{"version":1,"genre":"neon_run"}"#)
            .expect("neon");
        run.confirm();
        for _ in 0..90 {
            run.tick(1.0 / 60.0);
        }
        assert!(run.document().props.iter().all(|p| p.name != "trail"));
        assert!(run
            .document()
            .lines
            .iter()
            .any(|line| line.id == "ribbon" && line.points.len() >= 4));
        let on = renderer
            .render_world_doc(run.document())
            .expect("ribbon frame");
        write_shot("neon-ribbon.png", 320, 180, &on);
        let status = renderer.picture_status();
        assert!(status.ends_with(" post"), "{status}");
        let mut quiet = run.document().clone();
        quiet.lines.clear();
        quiet.points.clear();
        let off = renderer.render_world_doc(&quiet).expect("no ribbon");
        let changed = changed_pixels(&on, &off);
        assert!(
            changed > 8,
            "the fat-line ribbon should move the phone frame, changed {changed}"
        );
    }

    #[test]
    fn neon_phone_unorm_runs_post_and_bloom() {
        let Some(mut renderer) = pollster::block_on(Renderer::new_ldr_unorm(320, 180)).ok() else {
            eprintln!("no GPU adapter; skipping phone unorm neon");
            return;
        };
        let mobile = crate::neon_run::NeonRun::from_json(
            r#"{"version":1,"genre":"neon_run","quality":"mobile"}"#,
        )
        .expect("mobile");
        let frame = renderer
            .render_world_doc(mobile.document())
            .expect("render phone");
        let status = renderer.picture_status();
        assert!(status.contains(" fxaa=1 "), "{status}");
        assert!(status.contains(" bloom=1 "), "{status}");
        assert!(status.ends_with(" post"), "{status}");
        if crate::world_doc::heavy_post_allowed() {
            assert!(status.contains(" ssao=4 "), "{status}");
            assert!(status.contains(" c2 "), "{status}");
        }
        let pixels = (frame.len() / 4) as u32;
        let mut white = 0u32;
        for px in frame.chunks(4) {
            let s = px[0] as u32 + px[1] as u32 + px[2] as u32;
            if s > 740 {
                white += 1;
            }
        }
        assert!(
            white * 10 < pixels,
            "phone frame clipped white {white}/{pixels}"
        );
        write_shot("neon-phone-unorm.png", 320, 180, &frame);
        let mut quiet = mobile.document().clone();
        quiet.bloom = Some(0.0);
        let off = renderer.render_world_doc(&quiet).expect("bloom off");
        write_shot("neon-phone-unorm-nobloom.png", 320, 180, &off);
        let mut changed = 0u32;
        for (a, b) in frame.chunks(4).zip(off.chunks(4)) {
            let da = (a[0] as i32 - b[0] as i32).abs()
                + (a[1] as i32 - b[1] as i32).abs()
                + (a[2] as i32 - b[2] as i32).abs();
            if da >= 12 {
                changed += 1;
            }
        }
        assert!(
            changed > 8,
            "8-bit bloom should move the neon frame, changed {changed} status {status}"
        );
    }

    #[test]
    fn picture_line_names_the_phone_path() {
        assert_eq!(
            super::picture_line("mobile", "webgpu", true, true, 4.0, 2, true, true),
            "mobile webgpu fxaa=1 bloom=1 ssao=4 c2 post"
        );
        assert_eq!(
            super::picture_line("legacy", "gl", false, false, 0.0, 1, true, false),
            "legacy gl fxaa=0 bloom=0 ssao=0 c1 swap"
        );
        assert_eq!(
            super::picture_line("high", "vulkan", true, true, 8.2, 3, false, false),
            "high vulkan fxaa=1 bloom=1 ssao=8 c3 hdr"
        );
    }

    #[test]
    fn offscreen_dof_blurs_the_far_box() {
        let Some(mut renderer) = pollster::block_on(Renderer::new_offscreen(160, 160)).ok() else {
            eprintln!("no GPU adapter; skipping dof");
            return;
        };
        graded(&mut renderer);
        let boxes = vec![
            solid_box([-0.7, 0.45, 2.4], [0.7, 0.9, 0.7], [220, 30, 30, 255]),
            solid_box([1.3, 0.45, -4.0], [1.2, 1.2, 1.2], [30, 40, 220, 255]),
        ];
        renderer.set_post(0.0, false, None, 0.0);
        let sharp = draw_boxes(&mut renderer, boxes.clone());
        renderer.set_post(0.0, false, Some((4.2, 2.2)), 0.0);
        let blurred = draw_boxes(&mut renderer, boxes);
        write_shot("post-dof-sharp.png", 160, 160, &sharp);
        write_shot("post-dof-blur.png", 160, 160, &blurred);
        let mut near_diff = 0i64;
        let mut near_n = 0i64;
        let mut far_diff = 0i64;
        let mut far_n = 0i64;
        for i in (0..sharp.len()).step_by(4) {
            let s = [sharp[i], sharp[i + 1], sharp[i + 2], sharp[i + 3]];
            let b = [blurred[i], blurred[i + 1], blurred[i + 2], blurred[i + 3]];
            let d = (s[0] as i64 - b[0] as i64).abs()
                + (s[1] as i64 - b[1] as i64).abs()
                + (s[2] as i64 - b[2] as i64).abs();
            // Clear is (40, 80, 160). That blue is not the far box (30, 40, 220).
            if s[0] > 180 && s[1] < 50 && s[2] < 50 {
                near_diff += d;
                near_n += 1;
            }
            if s[2] > 180 && s[0] < 50 && s[1] < 70 {
                far_diff += d;
                far_n += 1;
            }
        }
        assert!(near_n > 8, "near red box missing ({near_n})");
        assert!(far_n > 8, "far blue box missing ({far_n})");
        let near_m = near_diff as f32 / near_n as f32;
        let far_m = far_diff as f32 / far_n as f32;
        assert!(
            far_m > near_m + 4.0 && far_m > 8.0,
            "far box should blur more than the focus plane: far {far_m} near {near_m}"
        );
    }

    #[test]
    fn offscreen_motion_blur_is_quiet_until_the_camera_moves() {
        let Some(mut renderer) = pollster::block_on(Renderer::new_offscreen(128, 128)).ok() else {
            eprintln!("no GPU adapter; skipping motion");
            return;
        };
        graded(&mut renderer);
        let boxes = vec![solid_box(
            [0.0, 0.5, 0.0],
            [1.0, 1.0, 1.0],
            [240, 220, 40, 255],
        )];
        renderer.set_post(0.0, false, None, 0.0);
        let still = draw_boxes(&mut renderer, boxes.clone());
        renderer.set_post(0.0, false, None, 1.0);
        let held = draw_boxes(&mut renderer, boxes.clone());
        let mut worst = 0i32;
        for i in (0..still.len()).step_by(4) {
            for c in 0..3 {
                worst = worst.max((still[i + c] as i32 - held[i + c] as i32).abs());
            }
        }
        assert!(
            worst <= 6,
            "a still camera should not smear, max channel delta {worst}"
        );
        let mesh = crate::scene3d::primitives::box_mesh(glam::Vec3::ONE);
        let id = renderer.upload_mesh(&mesh);
        let mut moved = crate::scene3d::Scene3D {
            clear: [40, 80, 160, 255],
            ambient: 1.0,
            ibl: 0.0,
            tonemap: false,
            exposure: 1.0,
            fog_start: 500.0,
            fog_end: 800.0,
            camera: crate::scene3d::Camera {
                eye: glam::Vec3::new(1.6, 2.0, 6.0),
                ..Default::default()
            },
            batches: vec![crate::scene3d::Batch {
                mesh: id,
                instances: boxes.clone(),
            }],
            ..Default::default()
        };
        renderer
            .render_frame(Some(&moved), &crate::scene::DrawList::default())
            .unwrap();
        let blur = renderer.read_rgba().unwrap();
        renderer.set_post(0.0, false, None, 0.0);
        moved.camera.eye = glam::Vec3::new(1.6, 2.0, 6.0);
        renderer
            .render_frame(Some(&moved), &crate::scene::DrawList::default())
            .unwrap();
        let sharp = renderer.read_rgba().unwrap();
        write_shot("post-motion-blur.png", 128, 128, &blur);
        write_shot("post-motion-sharp.png", 128, 128, &sharp);
        let mut changed = 0u32;
        for i in (0..blur.len()).step_by(4) {
            let d = (blur[i] as i32 - sharp[i] as i32).abs()
                + (blur[i + 1] as i32 - sharp[i + 1] as i32).abs()
                + (blur[i + 2] as i32 - sharp[i + 2] as i32).abs();
            if d > 24 {
                changed += 1;
            }
        }
        assert!(
            changed > 20,
            "moving the camera should smear, changed {changed}"
        );
    }

    #[test]
    fn offscreen_taa_keeps_a_flat_center() {
        let Some(mut renderer) = pollster::block_on(Renderer::new_offscreen(96, 96)).ok() else {
            eprintln!("no GPU adapter; skipping taa");
            return;
        };
        graded(&mut renderer);
        let boxes = vec![solid_box(
            [0.0, 0.0, 0.0],
            [6.0, 6.0, 6.0],
            [200, 180, 40, 255],
        )];
        renderer.set_post(0.0, false, None, 0.0);
        let plain = draw_boxes(&mut renderer, boxes.clone());
        let center = pixel_of(&plain, 96, 48, 48);
        renderer.set_post(0.0, true, None, 0.0);
        for frame in 0..4 {
            let rgba = draw_boxes(&mut renderer, boxes.clone());
            if frame == 3 {
                write_shot("post-taa.png", 96, 96, &rgba);
            }
            let got = pixel_of(&rgba, 96, 48, 48);
            assert_rgb_near(got, center, 10);
        }
    }

    /// A factor-only red glTF cube. The center pixel has to be the material,
    /// not the blue clear, so the bake reached the albedo.
    #[test]
    fn offscreen_gltf_base_color_factor_beats_the_clear() {
        let Some(rgba) = render_factor_cube() else {
            eprintln!("no GPU adapter; skipping glTF factor cube");
            return;
        };
        write_shot("gltf-factor-cube.png", 64, 64, &rgba);
        let got = pixel_of(&rgba, 64, 32, 32);
        assert!(
            got[0] > got[2] + 20 && got[0] > got[1] + 20,
            "center should be the red baseColorFactor, got {got:?}"
        );
    }

    fn render_factor_cube() -> Option<Vec<u8>> {
        let cube = crate::gltf_load::unit_cube_gltf();
        let with_mat = cube.replacen("\"mode\": 4", "\"mode\": 4, \"material\": 0", 1);
        let trimmed = with_mat.trim_end().trim_end_matches('}');
        let json = format!(
            "{trimmed}, \"materials\": [{{\"alphaMode\": \"OPAQUE\", \"doubleSided\": false, \"pbrMetallicRoughness\": {{\"baseColorFactor\": [1.0, 0.0, 0.0, 1.0], \"metallicFactor\": 0.0, \"roughnessFactor\": 1.0}}}}]}}"
        );
        let mesh = crate::gltf_load::mesh_from_embedded_gltf(&json).ok()?;
        assert!((mesh.alpha_cut + 1.0).abs() < 1e-3);
        let mut renderer = pollster::block_on(Renderer::new_offscreen_format(
            64,
            64,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ))
        .ok()?;
        renderer.set_fxaa(false);
        renderer.bloom.set_grade(0.85, 0.0, 0.0, false, "off", 0.0);
        let id = renderer.upload_mesh(&mesh);
        let scene = crate::scene3d::Scene3D {
            clear: [40, 80, 160, 255],
            tonemap: false,
            exposure: 1.0,
            ibl: 0.0,
            ambient: 1.0,
            batches: vec![crate::scene3d::Batch {
                mesh: id,
                instances: vec![crate::scene3d::Instance::with_pbr(
                    glam::Mat4::IDENTITY,
                    [255, 255, 255, 255],
                    crate::scene3d::Material::Solid,
                    0.0,
                    1.0,
                )],
            }],
            ..Default::default()
        };
        renderer
            .render_frame(Some(&scene), &crate::scene::DrawList::default())
            .ok()?;
        renderer.read_rgba().ok()
    }

    /// `KAGRA_GLTF_SAMPLE=/path/model.glb` renders a Khronos sample. The file
    /// stays out of the tree (DamagedHelmet is CC-BY-NC). Unset skips.
    #[test]
    fn offscreen_gltf_sample_model_when_present() {
        let Ok(path) = std::env::var("KAGRA_GLTF_SAMPLE") else {
            return;
        };
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let mesh =
            crate::gltf_load::mesh_from_glb(&bytes).unwrap_or_else(|e| panic!("{path}: {e}"));
        let bounds = mesh.bounds();
        let center = (bounds.min + bounds.max) * 0.5;
        let extent = (bounds.max - bounds.min).max_element().max(0.25);
        let mut renderer = pollster::block_on(Renderer::new_offscreen(384, 384))
            .unwrap_or_else(|e| panic!("offscreen: {e}"));
        renderer.set_fxaa(false);
        let id = renderer.upload_mesh(&mesh);
        let eye = center + glam::Vec3::new(extent * 0.35, extent * 0.45, extent * 1.8);
        let scene = crate::scene3d::Scene3D {
            clear: [18, 22, 32, 255],
            camera: crate::scene3d::Camera {
                eye,
                target: center,
                ..Default::default()
            },
            tonemap: true,
            tonemap_code: 0.0,
            sky_atmosphere: false,
            exposure: 1.15,
            ibl: 0.55,
            batches: vec![crate::scene3d::Batch {
                mesh: id,
                instances: vec![crate::scene3d::Instance::with_pbr(
                    glam::Mat4::IDENTITY,
                    [255, 255, 255, 255],
                    crate::scene3d::Material::Solid,
                    0.0,
                    0.5,
                )],
            }],
            ..Default::default()
        };
        renderer
            .render_frame(Some(&scene), &crate::scene::DrawList::default())
            .expect("render");
        let rgba = renderer.read_rgba().expect("readback");
        let name = std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("gltf-sample");
        write_shot(&format!("{name}.png"), 384, 384, &rgba);
        let clear = [18u8, 22, 32];
        let mut hit = 0u32;
        for px in rgba.as_chunks::<4>().0 {
            if (px[0] as i32 - clear[0] as i32).abs()
                + (px[1] as i32 - clear[1] as i32).abs()
                + (px[2] as i32 - clear[2] as i32).abs()
                > 40
            {
                hit += 1;
            }
        }
        assert!(hit > 400, "{name} did not cover the frame ({hit} px)");
    }

    #[test]
    fn offscreen_frame_profile_counts_the_box() {
        let Some(mut renderer) = pollster::block_on(Renderer::new_offscreen(64, 64)).ok() else {
            eprintln!("no GPU adapter; skipping frame profile");
            return;
        };
        let doc = crate::world_doc::WorldDoc::from_json(
            r#"{"version":1,"ground":false,"props":[{"id":"b","type":"prop","model":"box","position":[0.0,1.0,0.0]}]}"#,
        )
        .unwrap();
        if renderer.render_world_doc(&doc).is_err() {
            eprintln!("frame profile draw failed");
            return;
        }
        let profile = renderer.frame_profile();
        assert!(profile.draws >= 1, "draws {}", profile.draws);
        assert!(profile.triangles >= 12, "tris {}", profile.triangles);
        assert!(profile.cpu_ms.is_some());
        assert!(profile.texture_bytes > 1024);
        let dump = doc.to_json().unwrap();
        let spliced = crate::profile::splice_profile(&dump, &profile);
        assert!(spliced.contains("\"cpu_ms\""));
        assert!(spliced.contains("\"warnings\""));
    }

    /// A caster outside the historical 24 m map stays lit at cascade count 1
    /// and darkens the floor once three rings are packed into the same map.
    #[test]
    fn offscreen_cascades_shadow_a_far_caster() {
        let Some(mut renderer) = pollster::block_on(Renderer::new_offscreen(192, 108)).ok() else {
            eprintln!("no GPU adapter; skipping cascade shadow");
            return;
        };
        renderer.set_fxaa(false);
        renderer.bloom.set_grade(0.85, 0.0, 0.0, false, "off", 0.0);
        let mesh = renderer.upload_mesh(&crate::scene3d::primitives::box_mesh(glam::Vec3::ONE));
        let paint = |renderer: &mut Renderer, cascades: u32, pcss: bool| -> Option<Vec<u8>> {
            let cam = crate::scene3d::Camera {
                eye: glam::Vec3::new(0.0, 8.0, 16.0),
                target: glam::Vec3::ZERO,
                fov_y: 60f32.to_radians(),
                ..Default::default()
            };
            let mut b = crate::scene3d::SceneBuilder::new(&cam, 192.0 / 108.0);
            let floor = glam::Mat4::from_scale_rotation_translation(
                glam::Vec3::new(50.0, 0.15, 90.0),
                glam::Quat::IDENTITY,
                glam::Vec3::new(0.0, 0.0, -28.0),
            );
            b.push(mesh, floor, [220, 220, 220, 255]);
            let caster = glam::Mat4::from_scale_rotation_translation(
                glam::Vec3::new(10.0, 12.0, 10.0),
                glam::Quat::IDENTITY,
                glam::Vec3::new(0.0, 6.0, -36.0),
            );
            b.push(mesh, caster, [180, 40, 40, 255]);
            let scene = crate::scene3d::Scene3D {
                camera: cam,
                clear: [40, 80, 160, 255],
                ambient: 0.15,
                ibl: 0.0,
                tonemap: false,
                exposure: 1.0,
                fog_start: 400.0,
                fog_end: 800.0,
                shadow_cascades: cascades,
                shadow_pcss: pcss,
                batches: b.finish(),
                ..Default::default()
            };
            renderer
                .render_frame(Some(&scene), &crate::scene::DrawList::default())
                .ok()?;
            renderer.read_rgba().ok()
        };
        let Some(one) = paint(&mut renderer, 1, false) else {
            eprintln!("cascade frame failed");
            return;
        };
        let Some(three) = paint(&mut renderer, 3, false) else {
            eprintln!("cascade frame failed");
            return;
        };
        let Some(pcss) = paint(&mut renderer, 3, true) else {
            eprintln!("cascade frame failed");
            return;
        };
        write_shot("cascade-one.png", 192, 108, &one);
        write_shot("cascade-three.png", 192, 108, &three);
        write_shot("cascade-pcss.png", 192, 108, &pcss);
        let lum = |px: [u8; 4]| px[0] as i32 + px[1] as i32 + px[2] as i32;
        let mut darkened = 0u32;
        let mut stable = 0u32;
        let mut sum = 0i64;
        let n = (192 * 108) as i64;
        for i in 0..n as usize {
            let a = [one[i * 4], one[i * 4 + 1], one[i * 4 + 2], one[i * 4 + 3]];
            let b = [
                three[i * 4],
                three[i * 4 + 1],
                three[i * 4 + 2],
                three[i * 4 + 3],
            ];
            let da = lum(a);
            let db = lum(b);
            sum += db as i64;
            if da - db > 30 {
                darkened += 1;
            }
            if da > 280 && (da - db).abs() < 12 {
                stable += 1;
            }
        }
        let mean = sum / n;
        assert!(
            darkened > 8,
            "three cascades should shade the far floor, darkened {darkened} mean {mean}"
        );
        assert!(
            stable > 20,
            "the near floor should stay the single-map picture, stable {stable}"
        );
        assert!(
            mean > 40 && mean < 700,
            "cascade frame looks blank, mean {mean}"
        );
        let mut pcss_sum = 0i64;
        let mut pcss_dark = 0u32;
        for i in 0..n as usize {
            let a = [one[i * 4], one[i * 4 + 1], one[i * 4 + 2], one[i * 4 + 3]];
            let c = [
                pcss[i * 4],
                pcss[i * 4 + 1],
                pcss[i * 4 + 2],
                pcss[i * 4 + 3],
            ];
            pcss_sum += lum(c) as i64;
            if lum(a) - lum(c) > 20 {
                pcss_dark += 1;
            }
        }
        let pcss_mean = pcss_sum / n;
        assert!(
            pcss_mean > 40 && pcss_mean < 700,
            "PCSS frame looks blank, mean {pcss_mean}"
        );
        assert!(pcss_dark > 4, "PCSS should still shade, dark {pcss_dark}");
    }

    /// A 4×4 BC1 sRGB block stays red when the adapter advertised BC.
    /// Devices without the feature keep the white fallback and skip the color check.
    #[test]
    fn offscreen_bc1_ktx2_is_red_when_the_adapter_has_bc() {
        let Some(mut renderer) = pollster::block_on(Renderer::new_offscreen(64, 64)).ok() else {
            eprintln!("no GPU adapter; skipping ktx2");
            return;
        };
        let bc = wgpu::TextureFormat::Bc1RgbaUnormSrgb.required_features();
        if !renderer.device.features().contains(bc) {
            eprintln!("adapter has no BC; skipping ktx2 color check");
            return;
        }
        let bytes = crate::ktx_image::write_uncompressed_ktx2(
            ktx2::Format::BC1_RGBA_SRGB_BLOCK,
            4,
            4,
            &[0x00, 0xF8, 0x00, 0x00, 0, 0, 0, 0],
        )
        .unwrap();
        let ktx = crate::ktx_image::parse_ktx2(&bytes).unwrap();
        let mut mesh = crate::scene3d::primitives::box_mesh(glam::Vec3::ONE);
        mesh.albedo_ktx = Some(ktx);
        let id = renderer.upload_mesh(&mesh);
        graded(&mut renderer);
        let scene = crate::scene3d::Scene3D {
            clear: [8, 8, 16, 255],
            camera: crate::scene3d::Camera {
                eye: glam::Vec3::new(0.0, 0.4, 2.4),
                target: glam::Vec3::ZERO,
                ..Default::default()
            },
            ambient: 1.0,
            ibl: 0.0,
            tonemap: false,
            exposure: 1.0,
            fog_start: 400.0,
            fog_end: 800.0,
            batches: vec![crate::scene3d::Batch {
                mesh: id,
                instances: vec![solid_box(
                    [0.0, 0.0, 0.0],
                    [1.0, 1.0, 1.0],
                    [255, 255, 255, 255],
                )],
            }],
            ..Default::default()
        };
        let rgba = renderer
            .render_frame(Some(&scene), &crate::scene::DrawList::default())
            .and_then(|_| renderer.read_rgba())
            .expect("ktx2 frame");
        write_shot("ktx2-bc1.png", 64, 64, &rgba);
        let mut red = 0u32;
        for px in rgba.as_chunks::<4>().0 {
            if px[0] > 140 && px[0] > px[1] + 40 && px[0] > px[2] + 40 {
                red += 1;
            }
        }
        assert!(red > 20, "BC1 red block did not cover the box ({red} px)");
    }
}
