//! 閾値ブルーム。高輝度画素だけを半解像度でぼかしてシャープなフレームへ加算する。
//! 画面全体のぼかしはしない（トゥーン輪郭が濁る）。

use super::shaders::BLOOM_SHADER;

const BLOOM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn extract_weight(lum: f32, threshold: f32, knee: f32) -> f32 {
    let knee = knee.max(1e-4);
    let soft = lum - threshold + knee;
    let soft_c = soft.clamp(0.0, 2.0 * knee);
    let soft_t = (soft_c * soft_c) / (4.0 * knee);
    let contrib = (lum - threshold).max(soft_t);
    contrib / lum.max(1e-5)
}

pub(super) struct BloomPass {
    threshold: f32,
    intensity: f32,
    extract_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    tex_bgl: wgpu::BindGroupLayout,
    params_buf: wgpu::Buffer,
    params_bg: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    half_a: wgpu::Texture,
    half_a_view: wgpu::TextureView,
    half_b: wgpu::Texture,
    half_b_view: wgpu::TextureView,
    out: wgpu::Texture,
    out_view: wgpu::TextureView,
    half_w: u32,
    half_h: u32,
    last_applied: bool,
}

impl BloomPass {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bloom Tex BGL"),
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
        });
        let params_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bloom Params BGL"),
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
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bloom"),
            source: wgpu::ShaderSource::Wgsl(BLOOM_SHADER.into()),
        });
        let extract_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bloom Extract Layout"),
            bind_group_layouts: &[&tex_bgl, &params_bgl],
            push_constant_ranges: &[],
        });
        let composite_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bloom Composite Layout"),
            bind_group_layouts: &[&tex_bgl, &params_bgl, &tex_bgl],
            push_constant_ranges: &[],
        });
        let extract_pipeline = make_fs_pipeline(
            device,
            &shader,
            "fs_extract",
            &extract_layout,
            BLOOM_FORMAT,
            "Bloom Extract",
        );
        let blur_pipeline = make_fs_pipeline(
            device,
            &shader,
            "fs_blur",
            &extract_layout,
            BLOOM_FORMAT,
            "Bloom Blur",
        );
        let composite_pipeline = make_fs_pipeline(
            device,
            &shader,
            "fs_composite",
            &composite_layout,
            surface_format,
            "Bloom Composite",
        );
        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Bloom Params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom Params BG"),
            layout: &params_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buf.as_entire_binding(),
            }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bloom Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let (half_w, half_h) = half_size(width, height);
        let (half_a, half_a_view) = make_bloom_tex(device, half_w, half_h, "Bloom A");
        let (half_b, half_b_view) = make_bloom_tex(device, half_w, half_h, "Bloom B");
        let (out, out_view) = make_out_tex(device, width, height, surface_format);

        Self {
            threshold: 0.85,
            intensity: 0.0,
            extract_pipeline,
            blur_pipeline,
            composite_pipeline,
            tex_bgl,
            params_buf,
            params_bg,
            sampler,
            half_a,
            half_a_view,
            half_b,
            half_b_view,
            out,
            out_view,
            half_w,
            half_h,
            last_applied: false,
        }
    }

    pub fn set_params(&mut self, threshold: f32, intensity: f32) {
        self.threshold = threshold.clamp(0.0, 2.0);
        self.intensity = intensity.max(0.0);
    }

    pub fn is_active(&self) -> bool {
        self.intensity > 1e-4
    }

    pub fn last_applied(&self) -> bool {
        self.last_applied
    }

    pub fn output_texture(&self) -> &wgpu::Texture {
        &self.out
    }

    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        surface_format: wgpu::TextureFormat,
    ) {
        let (half_w, half_h) = half_size(width, height);
        let (half_a, half_a_view) = make_bloom_tex(device, half_w, half_h, "Bloom A");
        let (half_b, half_b_view) = make_bloom_tex(device, half_w, half_h, "Bloom B");
        let (out, out_view) = make_out_tex(device, width, height, surface_format);
        self.half_a = half_a;
        self.half_a_view = half_a_view;
        self.half_b = half_b;
        self.half_b_view = half_b_view;
        self.out = out;
        self.out_view = out_view;
        self.half_w = half_w;
        self.half_h = half_h;
        self.last_applied = false;
    }

    /// `frame_view` は resolve 済みのシャープなフレーム。成功したら out に合成結果。
    pub fn apply(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame_view: &wgpu::TextureView,
    ) -> bool {
        self.last_applied = false;
        if !self.is_active() {
            return false;
        }
        let src_bg = bind_tex(device, &self.tex_bgl, frame_view, &self.sampler, "Bloom Src");
        let a_bg = bind_tex(device, &self.tex_bgl, &self.half_a_view, &self.sampler, "Bloom A");
        let b_bg = bind_tex(device, &self.tex_bgl, &self.half_b_view, &self.sampler, "Bloom B");

        let knee = (self.threshold * 0.12).clamp(0.04, 0.2);
        write_params(queue, &self.params_buf, [self.threshold, knee, 0.0, 0.0]);
        fullscreen(
            encoder,
            &self.extract_pipeline,
            &self.half_a_view,
            &[&src_bg, &self.params_bg],
            "Bloom Extract",
        );

        let inv_w = 1.0 / self.half_w.max(1) as f32;
        let inv_h = 1.0 / self.half_h.max(1) as f32;
        write_params(queue, &self.params_buf, [inv_w, 0.0, 0.0, 0.0]);
        fullscreen(
            encoder,
            &self.blur_pipeline,
            &self.half_b_view,
            &[&a_bg, &self.params_bg],
            "Bloom Blur H",
        );
        write_params(queue, &self.params_buf, [0.0, inv_h, 0.0, 0.0]);
        fullscreen(
            encoder,
            &self.blur_pipeline,
            &self.half_a_view,
            &[&b_bg, &self.params_bg],
            "Bloom Blur V",
        );

        write_params(queue, &self.params_buf, [self.intensity, 0.0, 0.0, 0.0]);
        fullscreen(
            encoder,
            &self.composite_pipeline,
            &self.out_view,
            &[&src_bg, &self.params_bg, &a_bg],
            "Bloom Composite",
        );
        self.last_applied = true;
        true
    }
}

fn half_size(w: u32, h: u32) -> (u32, u32) {
    ((w / 2).max(1), (h / 2).max(1))
}

fn make_bloom_tex(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: BLOOM_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn make_out_tex(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Bloom Out"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
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
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn bind_tex(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn write_params(queue: &wgpu::Queue, buf: &wgpu::Buffer, v: [f32; 4]) {
    queue.write_buffer(buf, 0, bytemuck::cast_slice(&v));
}

fn fullscreen(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    target: &wgpu::TextureView,
    groups: &[&wgpu::BindGroup],
    label: &str,
) {
    let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    rp.set_pipeline(pipeline);
    for (i, g) in groups.iter().enumerate() {
        rp.set_bind_group(i as u32, *g, &[]);
    }
    rp.draw(0..3, 0..1);
}

fn make_fs_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    fs: &str,
    layout: &wgpu::PipelineLayout,
    format: wgpu::TextureFormat,
    label: &str,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: fs,
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

#[cfg(test)]
mod tests {
    use super::extract_weight;

    #[test]
    fn midtone_is_zero() {
        assert!(extract_weight(0.4, 0.85, 0.08) < 1e-5);
    }

    #[test]
    fn highlight_is_kept() {
        let w = extract_weight(1.0, 0.85, 0.08);
        assert!(w > 0.1);
        assert!(w <= 1.0);
    }

    #[test]
    fn just_under_threshold_is_soft_not_full() {
        let w = extract_weight(0.82, 0.85, 0.08);
        assert!(w < 0.15);
    }
}
