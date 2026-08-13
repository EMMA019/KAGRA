//! wgpu による 2D 描画（feature = "render"）。
//!
//! `scene::DrawList` の矩形を 1 パスで描くだけの薄い層。Android / iOS / Web /
//! オフスクリーンで同じコードを通す。オフスクリーンがあるので GPU のある
//! デスクトップで絵を確認できる。

mod target;

pub use target::SurfaceSource;

use crate::scene::DrawList;

const MAX_TEXTURE_SIDE: u32 = 8192;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    color: [f32; 4],
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
    pipeline: wgpu::RenderPipeline,
    screen_buf: wgpu::Buffer,
    screen_bind: wgpu::BindGroup,
    vbuf: wgpu::Buffer,
    vbuf_capacity: usize,
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
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let vbuf_capacity = 6 * 256;
        let vbuf = create_vertex_buffer(&device, vbuf_capacity);

        let me = Self {
            device,
            queue,
            format,
            width,
            height,
            target,
            pipeline,
            screen_buf,
            screen_bind,
            vbuf,
            vbuf_capacity,
        };
        me.upload_screen();
        me
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
        self.upload_screen();
    }

    /// `DrawList` を 1 枚描いて present する。
    pub fn render(&mut self, list: &DrawList) -> Result<(), String> {
        let vertices = self.build_vertices(list);
        self.ensure_vertex_capacity(vertices.len());
        if !vertices.is_empty() {
            self.queue
                .write_buffer(&self.vbuf, 0, bytemuck::cast_slice(&vertices));
        }

        let clear = to_linear(list.clear, self.format);
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
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
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
