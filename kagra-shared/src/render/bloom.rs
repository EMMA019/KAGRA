//! Threshold bloom post-process, ported from kagra-core (wgpu 0.19) bloom.rs.
//! The 3D pass draws into `frame_tex` as **linear HDR** (Rgba16Float, no
//! tonemap); bright pixels are extracted at half resolution, blurred H+V, and
//! `fs_composite` adds them back while applying exposure + ACES + sRGB encode
//! to the final target. HUD is drawn after composite, so UI colors are
//! unaffected. Inactive → composite still runs with intensity 0 (plain frame).
//! No full-frame blur: toon outlines stay crisp (same rule as V2).

pub(super) const BLOOM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Soft-knee extraction weight (shared with V2 for unit-test parity).
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn extract_weight(lum: f32, threshold: f32, knee: f32) -> f32 {
    let knee = knee.max(1e-4);
    let soft = lum - threshold + knee;
    let soft_c = soft.clamp(0.0, 2.0 * knee);
    let soft_t = (soft_c * soft_c) / (4.0 * knee);
    let contrib = (lum - threshold).max(soft_t);
    contrib / lum.max(1e-5)
}

#[derive(Debug)]
pub(super) struct BloomPass {
    width: u32,
    height: u32,
    threshold: f32,
    intensity: f32,
    frame_tex: wgpu::Texture,
    frame_view: wgpu::TextureView,
    extract_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    tex_bgl: wgpu::BindGroupLayout,
    /// pass ごとに別バッファ。1 本を使い回すと Queue::write_buffer が
    /// submit 前に全部走り、最後の値（intensity）で抽出してしまう。
    extract_params: (wgpu::Buffer, wgpu::BindGroup),
    blur_h_params: (wgpu::Buffer, wgpu::BindGroup),
    blur_v_params: (wgpu::Buffer, wgpu::BindGroup),
    composite_params: (wgpu::Buffer, wgpu::BindGroup),
    sampler: wgpu::Sampler,
    half_a: wgpu::Texture,
    half_a_view: wgpu::TextureView,
    half_b: wgpu::Texture,
    half_b_view: wgpu::TextureView,
    half_w: u32,
    half_h: u32,
    /// Composite output (sRGB, full res). FXAA reads this and writes the final
    /// target; disabled FXAA copies it over.
    composite_tex: wgpu::Texture,
    composite_view: wgpu::TextureView,
    fxaa_pipeline: wgpu::RenderPipeline,
    fxaa_params: (wgpu::Buffer, wgpu::BindGroup),
    fxaa_enabled: bool,
}

impl BloomPass {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
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
            source: wgpu::ShaderSource::Wgsl(include_str!("bloom.wgsl").into()),
        });
        let extract_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bloom Extract Layout"),
            bind_group_layouts: &[Some(&tex_bgl), Some(&params_bgl)],
            immediate_size: 0,
        });
        let composite_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bloom Composite Layout"),
            bind_group_layouts: &[Some(&tex_bgl), Some(&params_bgl), Some(&tex_bgl)],
            immediate_size: 0,
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
            format,
            "Bloom Composite",
        );
        let extract_params = make_params(device, &params_bgl, "Bloom Extract Params");
        let blur_h_params = make_params(device, &params_bgl, "Bloom BlurH Params");
        let blur_v_params = make_params(device, &params_bgl, "Bloom BlurV Params");
        let composite_params = make_params(device, &params_bgl, "Bloom Composite Params");
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Bloom Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let (half_w, half_h) = half_size(width, height);
        let (half_a, half_a_view) = make_bloom_tex(device, half_w, half_h, "Bloom A");
        let (half_b, half_b_view) = make_bloom_tex(device, half_w, half_h, "Bloom B");
        let frame_tex = make_frame_tex(device, width, height);
        let frame_view = frame_tex.create_view(&Default::default());

        // FXAA: composite（sRGB）を読んで最終ターゲットへ。HUD はその後。
        let fxaa_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("FXAA"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fxaa.wgsl").into()),
        });
        let fxaa_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("FXAA Layout"),
            bind_group_layouts: &[Some(&tex_bgl), Some(&params_bgl)],
            immediate_size: 0,
        });
        let fxaa_pipeline = make_fs_pipeline(
            device,
            &fxaa_shader,
            "fs_main",
            &fxaa_layout,
            format,
            "FXAA",
        );
        let fxaa_params = make_params(device, &params_bgl, "FXAA Params");
        let composite_tex = make_composite_tex(device, format, width, height);
        let composite_view = composite_tex.create_view(&Default::default());

        Self {
            width,
            height,
            threshold: 0.85,
            intensity: 0.0,
            frame_tex,
            frame_view,
            extract_pipeline,
            blur_pipeline,
            composite_pipeline,
            tex_bgl,
            extract_params,
            blur_h_params,
            blur_v_params,
            composite_params,
            sampler,
            half_a,
            half_a_view,
            half_b,
            half_b_view,
            half_w,
            half_h,
            composite_tex,
            composite_view,
            fxaa_pipeline,
            fxaa_params,
            fxaa_enabled: true,
        }
    }

    /// 3D+HUD の描き込み先。`apply` はこのフレームを読み、最終ターゲットへ出す。
    pub fn frame_view(&self) -> &wgpu::TextureView {
        &self.frame_view
    }

    pub fn set_params(&mut self, threshold: f32, intensity: f32) {
        self.threshold = threshold.clamp(0.0, 2.0);
        self.intensity = intensity.max(0.0);
    }

    pub fn is_active(&self) -> bool {
        self.intensity > 1e-4
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.frame_tex = make_frame_tex(device, width, height);
        self.frame_view = self.frame_tex.create_view(&Default::default());
        let (half_w, half_h) = half_size(width, height);
        let (half_a, half_a_view) = make_bloom_tex(device, half_w, half_h, "Bloom A");
        let (half_b, half_b_view) = make_bloom_tex(device, half_w, half_h, "Bloom B");
        self.half_a = half_a;
        self.half_a_view = half_a_view;
        self.half_b = half_b;
        self.half_b_view = half_b_view;
        self.half_w = half_w;
        self.half_h = half_h;
        let format = self.composite_tex.format();
        self.composite_tex = make_composite_tex(device, format, width, height);
        self.composite_view = self.composite_tex.create_view(&Default::default());
        self.width = width;
        self.height = height;
    }

    /// Edge smoothing on the composite output. Default on.
    pub fn set_fxaa(&mut self, enabled: bool) {
        self.fxaa_enabled = enabled;
    }

    /// `frame_tex`(linear HDR 3D)→ 最終 sRGB ターゲットへ。アクティブなら
    /// extract/blur を走らせ、composite(sharp + bloom*intensity → exposure →
    /// ACES → sRGB)を composite_tex に書く。その後 FXAA が composite_tex を
    /// 最終ターゲットへ（無効時はコピー）。HUD はこの後に別パスで重ねる。
    pub fn apply(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: (&wgpu::Texture, &wgpu::TextureView),
        exposure: f32,
        tonemap: bool,
    ) {
        let (target_texture, target_view) = target;
        let src_bg = bind_tex(
            device,
            &self.tex_bgl,
            &self.frame_view,
            &self.sampler,
            "Bloom Src",
        );
        if self.is_active() {
            let a_bg = bind_tex(
                device,
                &self.tex_bgl,
                &self.half_a_view,
                &self.sampler,
                "Bloom A",
            );
            let b_bg = bind_tex(
                device,
                &self.tex_bgl,
                &self.half_b_view,
                &self.sampler,
                "Bloom B",
            );

            let knee = (self.threshold * 0.12).clamp(0.04, 0.2);
            write_params(
                queue,
                &self.extract_params.0,
                [self.threshold, knee, 0.0, 0.0],
            );
            fullscreen(
                encoder,
                &self.extract_pipeline,
                &self.half_a_view,
                &[&src_bg, &self.extract_params.1],
                "Bloom Extract",
            );

            let inv_w = 1.0 / self.half_w.max(1) as f32;
            let inv_h = 1.0 / self.half_h.max(1) as f32;
            write_params(queue, &self.blur_h_params.0, [inv_w, 0.0, 0.0, 0.0]);
            fullscreen(
                encoder,
                &self.blur_pipeline,
                &self.half_b_view,
                &[&a_bg, &self.blur_h_params.1],
                "Bloom Blur H",
            );
            write_params(queue, &self.blur_v_params.0, [0.0, inv_h, 0.0, 0.0]);
            fullscreen(
                encoder,
                &self.blur_pipeline,
                &self.half_a_view,
                &[&b_bg, &self.blur_v_params.1],
                "Bloom Blur V",
            );
        }

        // composite → composite_tex（sRGB）。
        let a_bg = bind_tex(
            device,
            &self.tex_bgl,
            &self.half_a_view,
            &self.sampler,
            "Bloom A",
        );
        write_params(
            queue,
            &self.composite_params.0,
            [
                self.intensity,
                exposure.max(0.0),
                if tonemap { 1.0 } else { 0.0 },
                0.0,
            ],
        );
        fullscreen(
            encoder,
            &self.composite_pipeline,
            &self.composite_view,
            &[&src_bg, &self.composite_params.1, &a_bg],
            "Bloom Composite",
        );

        // FXAA or plain copy → 最終ターゲット。
        let inv_w = 1.0 / self.width.max(1) as f32;
        let inv_h = 1.0 / self.height.max(1) as f32;
        write_params(queue, &self.fxaa_params.0, [inv_w, inv_h, 0.0, 0.0]);
        if self.fxaa_enabled {
            let fxaa_bg = bind_tex(
                device,
                &self.tex_bgl,
                &self.composite_view,
                &self.sampler,
                "FXAA Src",
            );
            fullscreen(
                encoder,
                &self.fxaa_pipeline,
                target_view,
                &[&fxaa_bg, &self.fxaa_params.1],
                "FXAA",
            );
        } else {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.composite_tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: target_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: self.width,
                    height: self.height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
}

fn half_size(w: u32, h: u32) -> (u32, u32) {
    ((w / 2).max(1), (h / 2).max(1))
}

/// Linear HDR frame the 3D pass draws into. Tonemap happens in composite.
fn make_frame_tex(device: &wgpu::Device, w: u32, h: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Bloom Frame"),
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: BLOOM_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

/// Composite output（sRGB、フル解像度）。FXAA が読み、最終ターゲットへ出す。
fn make_composite_tex(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    w: u32,
    h: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Composite Frame"),
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
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

fn make_params(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    label: &str,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buf.as_entire_binding(),
        }],
    });
    (buf, bg)
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
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
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
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fs),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
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
