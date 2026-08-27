//! wgpu による描画（feature = "render"）。
//!
//! 3D パス（`scene3d::Scene3D`）を深度付きで描いてから、2D パス
//! （`scene::DrawList`）を HUD として上に重ねる。両方とも同じレンダーパスで、
//! 2D 側は深度テストを常に通して書き込まない。Android / iOS / Web /
//! オフスクリーンで同じコードを通す。
//!
//! WebGL2 でも動く範囲に収めてある。ストレージバッファを使わず、インスタンスは
//! 頂点バッファで渡し、base instance には頼らない（GLES に無いため）。

mod target;

pub use target::SurfaceSource;

use crate::scene::DrawList;
use crate::scene3d::{MeshData, MeshId, Scene3D};

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
    light_pos: [[f32; 4]; 4],
    light_col: [[f32; 4]; 4],
    light_dir: [[f32; 4]; 4],
}

/// インスタンスごとの頂点属性（location 2..7）。
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
    model: [[f32; 4]; 4],
    color: [f32; 4],
    material: f32,
    _pad: [f32; 3],
}

#[derive(Debug)]
struct GpuMesh {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    index_count: u32,
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
    /// 天球用。深度は書いて読まず、裏面カリングもしない。
    pipeline_sky: wgpu::RenderPipeline,
    globals_buf: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    inst_buf: wgpu::Buffer,
    inst_capacity: usize,
    meshes: Vec<GpuMesh>,
}

impl Renderer {
    /// 画面付き。`source` のハンドルはレンダラより長生きさせること。
    pub async fn new_for_surface(
        source: SurfaceSource,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let instance = new_instance();
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
        config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let caps = surface.get_capabilities(&adapter);
        if let Some(srgb) = caps.formats.iter().copied().find(|f| f.is_srgb()) {
            config.format = srgb;
        }
        surface.configure(&device, &config);

        let format = config.format;
        let (width, height) = (config.width, config.height);
        Ok(Self::assemble(
            device,
            queue,
            format,
            width,
            height,
            Target::Surface { surface, config },
        ))
    }

    /// 画面なし。テストやスクリーンショット用。
    pub async fn new_offscreen(width: u32, height: u32) -> Result<Self, String> {
        let instance = new_instance();
        let (_adapter, device, queue) = request_device(&instance, None).await?;
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let (width, height) = (
            width.clamp(1, MAX_TEXTURE_SIDE),
            height.clamp(1, MAX_TEXTURE_SIDE),
        );
        let texture = create_offscreen_texture(&device, format, width, height);
        Ok(Self::assemble(
            device,
            queue,
            format,
            width,
            height,
            Target::Offscreen { texture },
        ))
    }

    fn assemble(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        target: Target,
    ) -> Self {
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

        let layout3d = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kagra-shared 3d layout"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });

        // vertex_attr_array! の一時配列を名前付きに固定しないと、layout の
        // attributes 参照が文の終わりで死ぬ。
        let mesh_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        let inst_attrs = wgpu::vertex_attr_array![
            2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
            6 => Float32x4, 7 => Float32
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

        let color_target = [Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];

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
                targets: &color_target,
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
                targets: &color_target,
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

        let inst_capacity = 256;
        let inst_buf = create_instance_buffer(&device, inst_capacity);
        let depth = create_depth_view(&device, width, height);

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
            pipeline_sky,
            globals_buf,
            globals_bind,
            inst_buf,
            inst_capacity,
            meshes: Vec::new(),
        };
        me.upload_screen();
        me
    }

    /// メッシュを GPU に載せる。`MeshId` は登録順の連番。
    pub fn upload_mesh(&mut self, mesh: &MeshData) -> MeshId {
        let vbuf = create_init_buffer(
            &self.device,
            "kagra-shared mesh vertices",
            bytemuck::cast_slice(&mesh.vertices),
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        let ibuf = create_init_buffer(
            &self.device,
            "kagra-shared mesh indices",
            bytemuck::cast_slice(&mesh.indices),
            wgpu::BufferUsages::INDEX,
        );
        self.meshes.push(GpuMesh {
            vbuf,
            ibuf,
            index_count: mesh.indices.len() as u32,
        });
        MeshId(self.meshes.len() as u32 - 1)
    }

    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
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
        self.upload_screen();
    }

    /// HUD だけを描く。3D を持たない呼び出し側のための薄い入口。
    pub fn render(&mut self, list: &DrawList) -> Result<(), String> {
        self.render_frame(None, list)
    }

    /// 3D を描いてから HUD を重ね、1 枚として present する。
    pub fn render_frame(&mut self, world: Option<&Scene3D>, hud: &DrawList) -> Result<(), String> {
        let vertices = self.build_vertices(hud);
        self.ensure_vertex_capacity(vertices.len());
        if !vertices.is_empty() {
            self.queue
                .write_buffer(&self.vbuf, 0, bytemuck::cast_slice(&vertices));
        }

        // 3D のインスタンスを 1 本のバッファに連結し、バッチごとの開始位置を覚える。
        // base instance は GLES に無いので、描画時は毎回 0.. で数え、バッファの
        // スライス位置でずらす。
        // (mesh, byte_offset, instance_count, is_sky)
        let mut draws: Vec<(usize, u64, u32, bool)> = Vec::new();
        if let Some(scene) = world {
            let mut raw: Vec<InstanceRaw> = Vec::with_capacity(scene.instance_count());
            for batch in &scene.batches {
                let mesh = batch.mesh.0 as usize;
                if mesh >= self.meshes.len() || batch.instances.is_empty() {
                    continue;
                }
                let offset =
                    (raw.len() * std::mem::size_of::<InstanceRaw>()) as wgpu::BufferAddress;
                let is_sky = batch.instances[0].material == crate::scene3d::Material::Sky;
                for inst in &batch.instances {
                    raw.push(InstanceRaw {
                        model: inst.model.to_cols_array_2d(),
                        color: to_linear(inst.color, self.format),
                        material: inst.material as u8 as f32,
                        _pad: [0.0; 3],
                    });
                }
                draws.push((mesh, offset, batch.instances.len() as u32, is_sky));
            }
            if !raw.is_empty() {
                self.ensure_instance_capacity(raw.len());
                self.queue
                    .write_buffer(&self.inst_buf, 0, bytemuck::cast_slice(&raw));
            }
            self.upload_globals(scene);
        }

        let clear = to_linear(world.map(|s| s.clear).unwrap_or(hud.clear), self.format);
        let frame = match &self.target {
            Target::Surface { surface, .. } => {
                use wgpu::CurrentSurfaceTexture as Cst;
                match surface.get_current_texture() {
                    // Suboptimal でもそのフレームは描ける。次フレームで作り直す。
                    Cst::Success(f) | Cst::Suboptimal(f) => Some(f),
                    Cst::Outdated | Cst::Lost => {
                        // 回転・リサイズ直後。再構成して次フレームに任せる。
                        self.reconfigure();
                        return Ok(());
                    }
                    Cst::Timeout | Cst::Occluded => return Ok(()),
                    Cst::Validation => return Err("surface validation error".into()),
                }
            }
            Target::Offscreen { .. } => None,
        };

        let view = match (&frame, &self.target) {
            (Some(f), _) => f.texture.create_view(&Default::default()),
            (None, Target::Offscreen { texture }) => texture.create_view(&Default::default()),
            _ => return Err("no render target".into()),
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kagra-shared frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kagra-shared 2d pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
                })],
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
                // 空を先に。深度を書かないので、このあと地面が上に乗る。
                pass.set_pipeline(&self.pipeline_sky);
                for (mesh, offset, count, is_sky) in &draws {
                    if !*is_sky {
                        continue;
                    }
                    let m = &self.meshes[*mesh];
                    pass.set_vertex_buffer(0, m.vbuf.slice(..));
                    pass.set_vertex_buffer(1, self.inst_buf.slice(*offset..));
                    pass.set_index_buffer(m.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.index_count, 0, 0..*count);
                }
                pass.set_pipeline(&self.pipeline3d);
                for (mesh, offset, count, is_sky) in &draws {
                    if *is_sky {
                        continue;
                    }
                    let m = &self.meshes[*mesh];
                    pass.set_vertex_buffer(0, m.vbuf.slice(..));
                    pass.set_vertex_buffer(1, self.inst_buf.slice(*offset..));
                    pass.set_index_buffer(m.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..m.index_count, 0, 0..*count);
                }
            }

            if !vertices.is_empty() {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.screen_bind, &[]);
                pass.set_vertex_buffer(0, self.vbuf.slice(..));
                pass.draw(0..vertices.len() as u32, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        if let Some(f) = frame {
            self.queue.present(f);
        }
        Ok(())
    }

    fn reconfigure(&mut self) {
        if let Target::Surface { surface, config } = &self.target {
            surface.configure(&self.device, config);
        }
    }

    fn build_vertices(&self, list: &DrawList) -> Vec<Vertex> {
        let mut out = Vec::with_capacity(list.quads.len() * 6);
        for q in &list.quads {
            let color = to_linear(q.color, self.format);
            let (l, t, r, b) = (q.x, q.y, q.x + q.w, q.y + q.h);
            let corners = [[l, t], [r, t], [r, b], [l, t], [r, b], [l, b]];
            for pos in corners {
                out.push(Vertex { pos, color });
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

    fn upload_globals(&self, scene: &Scene3D) {
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
        let g = Globals {
            view_proj: cam.view_projection(self.aspect()).to_cols_array_2d(),
            light: [
                scene.light_dir.x,
                scene.light_dir.y,
                scene.light_dir.z,
                scene.ambient.clamp(0.0, 1.0),
            ],
            fog_color: to_linear(scene.fog_color, self.format),
            fog_range: [scene.fog_start, scene.fog_end, 0.0, 0.0],
            camera_pos: [cam.eye.x, cam.eye.y, cam.eye.z, 0.0],
            light_pos,
            light_col,
            light_dir,
        };
        self.queue
            .write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&g));
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
    pub fn update_world_gltf(&mut self, doc: &crate::world_doc::WorldDoc) -> Result<(), String> {
        for (id, mesh) in doc.compile_meshes() {
            if id.0 >= crate::world_doc::MESH_GLTF_BASE {
                self.update_mesh(id, &mesh)?;
            }
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
        if self.meshes.is_empty() {
            self.upload_world_meshes(doc)?;
        }
        let scene = doc.compile_scene(self.aspect());
        self.render_frame(Some(&scene), &DrawList::default())
    }

    /// Draw a compiled `WorldDoc` into the offscreen target and read RGBA8.
    /// No kagra-core window. Capsules / boxes are the primitives.
    pub fn render_world_doc(
        &mut self,
        doc: &crate::world_doc::WorldDoc,
    ) -> Result<Vec<u8>, String> {
        if !self.is_offscreen() {
            return Err("render_world_doc requires an offscreen renderer".into());
        }
        self.draw_world_doc(doc)?;
        self.read_rgba()
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
    let mut renderer = pollster::block_on(Renderer::new_offscreen(width, height))?;
    renderer.render_world_doc(doc)
}

fn new_instance() -> wgpu::Instance {
    // サーフェスごとに display handle を渡すので、インスタンス側には持たせない。
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::all();
    wgpu::Instance::new(desc)
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

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("kagra-shared"),
            required_limits: limits.using_resolution(adapter.limits()),
            ..Default::default()
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok((adapter, device, queue))
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
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
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
fn create_init_buffer(
    device: &wgpu::Device,
    label: &str,
    data: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    // サイズ 0 のバッファは作れないので、空なら最小サイズを空のまま置く。
    if data.is_empty() {
        return device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: 4,
            usage,
            mapped_at_creation: false,
        });
    }
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: data.len() as wgpu::BufferAddress,
        usage,
        mapped_at_creation: true,
    });
    buffer
        .get_mapped_range_mut(..)
        .expect("freshly mapped buffer")
        .copy_from_slice(data);
    buffer.unmap();
    buffer
}

/// sRGB のターゲットへはリニア値を書く必要がある。シーンの色は sRGB の
/// u8 なので、フォーマットに応じて変換する。
fn to_linear(color: [u8; 4], format: wgpu::TextureFormat) -> [f32; 4] {
    let f = |v: u8| v as f32 / 255.0;
    let a = f(color[3]);
    if format.is_srgb() {
        let srgb_to_linear = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        [
            srgb_to_linear(f(color[0])),
            srgb_to_linear(f(color[1])),
            srgb_to_linear(f(color[2])),
            a,
        ]
    } else {
        [f(color[0]), f(color[1]), f(color[2]), a]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_conversion_endpoints() {
        let black = to_linear([0, 0, 0, 255], wgpu::TextureFormat::Rgba8UnormSrgb);
        assert_eq!(black[0], 0.0);
        let white = to_linear([255, 255, 255, 255], wgpu::TextureFormat::Rgba8UnormSrgb);
        assert!((white[0] - 1.0).abs() < 1e-5);
        let plain = to_linear([128, 0, 0, 255], wgpu::TextureFormat::Rgba8Unorm);
        assert!((plain[0] - 128.0 / 255.0).abs() < 1e-6);
    }
}
