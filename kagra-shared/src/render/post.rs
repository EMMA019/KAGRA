//! Post-process pass graph.
//!
//! Modeled on three.js EffectComposer / ShaderPass (MIT). Each [`Pass`] is one
//! fullscreen WGSL entry, a uniform buffer, and a color target. Scene-space
//! passes ping-pong the HDR frame with `scratch`. Display passes (TAA,
//! vignette) run after bloom's grade.
//!
//! Order when a pass is on: half-res SSAO → DOF → camera motion blur → bloom
//! → exposure / ACES / LUT → TAA (or FXAA) → vignette. Omitted dump fields
//! leave every one of these off, and the historical bloom composite is the
//! picture that already shipped. Dump `quality` and `KAGRA_QUALITY` fill or
//! force the passes in `WorldDoc`. The direct unorm path runs SSAO plus the
//! grade when SSAO is on, and skips TAA, DOF, motion, and PCSS.

use glam::Mat4;

const AO_BIAS: f32 = 0.05;
const AO_INTENSITY: f32 = 1.85;
const AO_RANGE: f32 = 2.0;
/// Unorm depth stores `eye / span` in 0..1. 32 m covers contact occlusion;
/// an 8-bit step is about 0.13 m, inside the AO range. HDR passes 0 and
/// stores meters.
pub(super) const AO_DEPTH_SPAN: f32 = 32.0;
const DOF_MAX_PX: f32 = 14.0;
const TAA_BLEND: f32 = 0.85;

/// One switchable fullscreen step (three.js `ShaderPass`).
trait Pass {
    fn name(&self) -> &'static str;
    fn enabled(&self) -> bool;
    fn pipeline(&self) -> &wgpu::RenderPipeline;
    fn params(&self) -> &wgpu::Buffer;
    fn params_bind(&self) -> &wgpu::BindGroup;
}

#[derive(Debug)]
struct FsPass {
    name: &'static str,
    pipeline: wgpu::RenderPipeline,
    params_buf: wgpu::Buffer,
    params_bg: wgpu::BindGroup,
    enabled: bool,
}

impl Pass for FsPass {
    fn name(&self) -> &'static str {
        self.name
    }
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }
    fn params(&self) -> &wgpu::Buffer {
        &self.params_buf
    }
    fn params_bind(&self) -> &wgpu::BindGroup {
        &self.params_bg
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PostParams {
    v: [f32; 4],
    extra: [f32; 4],
    m0: [f32; 4],
    m1: [f32; 4],
    m2: [f32; 4],
    m3: [f32; 4],
    p0: [f32; 4],
    p1: [f32; 4],
    p2: [f32; 4],
    p3: [f32; 4],
}

impl Default for PostParams {
    fn default() -> Self {
        Self::zeroed()
    }
}

impl PostParams {
    fn zeroed() -> Self {
        bytemuck::Zeroable::zeroed()
    }
}

#[derive(Debug)]
pub(super) struct PostGraph {
    ssao: FsPass,
    ao_blur_h: FsPass,
    ao_blur_v: FsPass,
    ao_apply: FsPass,
    dof: FsPass,
    motion: FsPass,
    taa: FsPass,
    vignette: FsPass,
    copy: FsPass,
    tex_bgl: wgpu::BindGroupLayout,
    extra_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    scratch: wgpu::Texture,
    scratch_view: wgpu::TextureView,
    half_a: wgpu::Texture,
    half_a_view: wgpu::TextureView,
    half_b: wgpu::Texture,
    half_b_view: wgpu::TextureView,
    /// Linear eye depth in meters (frame format). Owned so `depth_view` stays valid.
    _depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    hist: [wgpu::Texture; 2],
    hist_view: [wgpu::TextureView; 2],
    read_hist: usize,
    width: u32,
    height: u32,
    half_w: u32,
    half_h: u32,
    frame_format: wgpu::TextureFormat,
    swap_format: wgpu::TextureFormat,
    ssao_radius: f32,
    taa_on: bool,
    dof_focus: f32,
    dof_aperture: f32,
    motion_amt: f32,
    frame_i: u32,
    prev_vp: Mat4,
    has_prev: bool,
}

impl PostGraph {
    pub fn new(
        device: &wgpu::Device,
        swap_format: wgpu::TextureFormat,
        frame_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let tex_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Post Tex"),
            entries: &[tex_entry(0), samp_entry(1)],
        });
        let extra_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Post Extra"),
            entries: &[tex_entry(0), samp_entry(1)],
        });
        let params_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Post Params"),
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
            label: Some("Post"),
            source: wgpu::ShaderSource::Wgsl(include_str!("post.wgsl").into()),
        });
        let layout_one = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Post Layout"),
            bind_group_layouts: &[Some(&tex_bgl), Some(&params_bgl)],
            immediate_size: 0,
        });
        let layout_two = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Post Layout Extra"),
            bind_group_layouts: &[Some(&tex_bgl), Some(&params_bgl), Some(&extra_bgl)],
            immediate_size: 0,
        });
        let ssao = pass(
            device,
            &params_bgl,
            &shader,
            &layout_one,
            "fs_ssao",
            frame_format,
            "SSAO",
        );
        let ao_blur_h = pass(
            device,
            &params_bgl,
            &shader,
            &layout_one,
            "fs_ao_blur",
            frame_format,
            "AO Blur H",
        );
        let ao_blur_v = pass(
            device,
            &params_bgl,
            &shader,
            &layout_one,
            "fs_ao_blur",
            frame_format,
            "AO Blur V",
        );
        let ao_apply = pass(
            device,
            &params_bgl,
            &shader,
            &layout_two,
            "fs_ao_apply",
            frame_format,
            "AO Apply",
        );
        let dof = pass(
            device,
            &params_bgl,
            &shader,
            &layout_two,
            "fs_dof",
            frame_format,
            "DOF",
        );
        let motion = pass(
            device,
            &params_bgl,
            &shader,
            &layout_two,
            "fs_motion",
            frame_format,
            "Motion",
        );
        let taa = pass(
            device,
            &params_bgl,
            &shader,
            &layout_two,
            "fs_taa",
            swap_format,
            "TAA",
        );
        let vignette = pass(
            device,
            &params_bgl,
            &shader,
            &layout_one,
            "fs_vignette",
            swap_format,
            "Vignette",
        );
        let copy = pass(
            device,
            &params_bgl,
            &shader,
            &layout_one,
            "fs_copy",
            frame_format,
            "Post Copy",
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Post Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let (scratch, scratch_view) =
            color_tex(device, frame_format, width, height, "Post Scratch");
        let (half_w, half_h) = half_size(width, height);
        let (half_a, half_a_view) = color_tex(device, frame_format, half_w, half_h, "Post AO A");
        let (half_b, half_b_view) = color_tex(device, frame_format, half_w, half_h, "Post AO B");
        let (depth, depth_view) = color_tex(device, frame_format, width, height, "Post Depth");
        let (h0, h0v) = color_tex(device, swap_format, width, height, "Post History 0");
        let (h1, h1v) = color_tex(device, swap_format, width, height, "Post History 1");
        Self {
            ssao,
            ao_blur_h,
            ao_blur_v,
            ao_apply,
            dof,
            motion,
            taa,
            vignette,
            copy,
            tex_bgl,
            extra_bgl,
            sampler,
            scratch,
            scratch_view,
            half_a,
            half_a_view,
            half_b,
            half_b_view,
            _depth: depth,
            depth_view,
            hist: [h0, h1],
            hist_view: [h0v, h1v],
            read_hist: 0,
            width: width.max(1),
            height: height.max(1),
            half_w,
            half_h,
            frame_format,
            swap_format,
            ssao_radius: 0.0,
            taa_on: false,
            dof_focus: 0.0,
            dof_aperture: 0.0,
            motion_amt: 0.0,
            frame_i: 0,
            prev_vp: Mat4::IDENTITY,
            has_prev: false,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        let (scratch, scratch_view) = color_tex(
            device,
            self.frame_format,
            self.width,
            self.height,
            "Post Scratch",
        );
        self.scratch = scratch;
        self.scratch_view = scratch_view;
        let (half_w, half_h) = half_size(self.width, self.height);
        self.half_w = half_w;
        self.half_h = half_h;
        let (half_a, half_a_view) =
            color_tex(device, self.frame_format, half_w, half_h, "Post AO A");
        let (half_b, half_b_view) =
            color_tex(device, self.frame_format, half_w, half_h, "Post AO B");
        self.half_a = half_a;
        self.half_a_view = half_a_view;
        self.half_b = half_b;
        self.half_b_view = half_b_view;
        let (depth, depth_view) = color_tex(
            device,
            self.frame_format,
            self.width,
            self.height,
            "Post Depth",
        );
        self._depth = depth;
        self.depth_view = depth_view;
        let (h0, h0v) = color_tex(
            device,
            self.swap_format,
            self.width,
            self.height,
            "Post History 0",
        );
        let (h1, h1v) = color_tex(
            device,
            self.swap_format,
            self.width,
            self.height,
            "Post History 1",
        );
        self.hist = [h0, h1];
        self.hist_view = [h0v, h1v];
        self.read_hist = 0;
        self.frame_i = 0;
        self.has_prev = false;
    }

    /// Radius in full-res pixels. 0 disables SSAO. TAA replaces FXAA.
    /// DOF is `(focus distance, aperture)`. Motion is a 0..1 shutter.
    pub fn set(&mut self, ssao: f32, taa: bool, dof: Option<(f32, f32)>, motion: f32) {
        let was_taa = self.taa_on;
        self.ssao_radius = if ssao > 1e-3 { ssao.min(64.0) } else { 0.0 };
        self.taa_on = taa;
        if let Some((focus, aperture)) = dof {
            self.dof_focus = focus.max(0.05);
            self.dof_aperture = aperture.max(0.0);
        } else {
            self.dof_focus = 0.0;
            self.dof_aperture = 0.0;
        }
        self.motion_amt = motion.clamp(0.0, 1.0);
        self.ssao.enabled = self.ssao_radius > 0.0;
        self.ao_blur_h.enabled = self.ssao.enabled;
        self.ao_blur_v.enabled = self.ssao.enabled;
        self.ao_apply.enabled = self.ssao.enabled;
        self.dof.enabled = self.dof_aperture > 1e-4;
        self.motion.enabled = self.motion_amt > 1e-4;
        self.taa.enabled = self.taa_on;
        self.vignette.enabled = true;
        self.copy.enabled = true;
        if self.taa_on && !was_taa {
            self.frame_i = 0;
        }
    }

    pub fn ssao_enabled(&self) -> bool {
        self.ssao.enabled
    }

    pub fn needs_depth(&self) -> bool {
        self.ssao.enabled || self.dof.enabled || self.motion.enabled
    }

    /// Linear eye depth written by the depth prepass. Valid after that pass.
    pub fn scene_depth_view(&self) -> &wgpu::TextureView {
        &self.depth_view
    }

    pub fn scene_active(&self) -> bool {
        self.needs_depth()
    }

    pub fn taa_enabled(&self) -> bool {
        self.taa_on
    }

    pub fn sample_index(&self) -> u32 {
        self.frame_i
    }

    pub fn store_view(&mut self, unjittered_vp: Mat4) {
        self.prev_vp = unjittered_vp;
        self.has_prev = true;
    }

    /// SSAO, DOF, and camera motion on the HDR frame. Bloom reads `frame` after this.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_scene(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::TextureView,
        unjittered_vp: Mat4,
        near: f32,
        far: f32,
    ) {
        let mut in_scratch = false;
        if self.ssao.enabled {
            self.run_ssao(device, queue, encoder, frame, near, far, 0.0);
            in_scratch = true;
        }
        if self.dof.enabled {
            let scratch = self.scratch_view.clone();
            if in_scratch {
                self.run_dof(device, queue, encoder, &scratch, frame, near, far, true);
            } else {
                self.run_dof(device, queue, encoder, frame, &scratch, near, far, true);
            }
            in_scratch = !in_scratch;
        }
        if self.motion.enabled {
            self.run_motion(
                device,
                queue,
                encoder,
                frame,
                in_scratch,
                unjittered_vp,
                near,
                far,
            );
            in_scratch = !in_scratch;
        }
        if in_scratch {
            self.run_copy(device, queue, encoder, frame);
        }
    }

    /// SSAO only, then copy the darkened color back onto `frame`. DOF, motion,
    /// and TAA stay off. `depth_span` > 1 reads packed unorm depth.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_ssao_only(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::TextureView,
        near: f32,
        far: f32,
        depth_span: f32,
    ) {
        if !self.ssao.enabled {
            return;
        }
        self.run_ssao(device, queue, encoder, frame, near, far, depth_span);
        self.run_copy(device, queue, encoder, frame);
    }

    /// TAA then vignette. `src` is bloom's graded composite. `target` is the swapchain.
    pub fn apply_display(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        src: &wgpu::TextureView,
        target: &wgpu::TextureView,
        vignette: f32,
    ) {
        let blend = if self.frame_i == 0 { 0.0 } else { TAA_BLEND };
        let mut p = PostParams::zeroed();
        p.v[0] = blend;
        p.extra[0] = 1.0 / self.width as f32;
        p.extra[1] = 1.0 / self.height as f32;
        write_params(queue, self.taa.params(), &p);
        let read = self.read_hist;
        let write = 1 - read;
        let src_bg = bind_tex(device, &self.tex_bgl, src, &self.sampler, "TAA Src");
        let hist_bg = bind_tex(
            device,
            &self.extra_bgl,
            &self.hist_view[read],
            &self.sampler,
            "TAA History",
        );
        draw_pass(
            encoder,
            &self.taa,
            &self.hist_view[write],
            &src_bg,
            Some(&hist_bg),
        );
        self.read_hist = write;
        self.frame_i = self.frame_i.saturating_add(1);

        let mut v = PostParams::zeroed();
        v.v[3] = vignette;
        write_params(queue, self.vignette.params(), &v);
        let vig_src = bind_tex(
            device,
            &self.tex_bgl,
            &self.hist_view[self.read_hist],
            &self.sampler,
            "Vignette Src",
        );
        draw_pass(encoder, &self.vignette, target, &vig_src, None);
    }

    #[allow(clippy::too_many_arguments)]
    fn run_ssao(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::TextureView,
        near: f32,
        far: f32,
        depth_span: f32,
    ) {
        let mut p = PostParams::zeroed();
        p.v = [self.ssao_radius, AO_BIAS, AO_INTENSITY, AO_RANGE];
        p.extra = [
            near.max(1e-3),
            far.max(near + 0.01),
            1.0 / self.width as f32,
            1.0 / self.height as f32,
        ];
        p.m0[0] = depth_span;
        write_params(queue, self.ssao.params(), &p);
        let depth = self.depth_view.clone();
        let src = bind_tex(device, &self.tex_bgl, &depth, &self.sampler, "SSAO Depth");
        draw_pass(encoder, &self.ssao, &self.half_a_view, &src, None);

        let mut bh = PostParams::zeroed();
        bh.v[0] = 1.0 / self.half_w as f32;
        bh.v[2] = depth_span;
        write_params(queue, self.ao_blur_h.params(), &bh);
        let a = bind_tex(
            device,
            &self.tex_bgl,
            &self.half_a_view,
            &self.sampler,
            "AO A",
        );
        draw_pass(encoder, &self.ao_blur_h, &self.half_b_view, &a, None);

        let mut bv = PostParams::zeroed();
        bv.v[1] = 1.0 / self.half_h as f32;
        bv.v[2] = depth_span;
        write_params(queue, self.ao_blur_v.params(), &bv);
        let b = bind_tex(
            device,
            &self.tex_bgl,
            &self.half_b_view,
            &self.sampler,
            "AO B",
        );
        draw_pass(encoder, &self.ao_blur_v, &self.half_a_view, &b, None);

        let mut apply = PostParams::zeroed();
        apply.v[0] = 1.0;
        write_params(queue, self.ao_apply.params(), &apply);
        let color = bind_tex(device, &self.tex_bgl, frame, &self.sampler, "AO Color");
        let ao = bind_tex(
            device,
            &self.extra_bgl,
            &self.half_a_view,
            &self.sampler,
            "AO Tex",
        );
        draw_pass(
            encoder,
            &self.ao_apply,
            &self.scratch_view,
            &color,
            Some(&ao),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn run_dof(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        src_view: &wgpu::TextureView,
        dst_view: &wgpu::TextureView,
        near: f32,
        far: f32,
        keep_depth: bool,
    ) {
        let mut p = PostParams::zeroed();
        p.v = [
            self.dof_focus,
            self.dof_aperture,
            DOF_MAX_PX,
            if keep_depth { 1.0 } else { 0.0 },
        ];
        p.extra = [
            near.max(1e-3),
            far.max(near + 0.01),
            1.0 / self.width as f32,
            1.0 / self.height as f32,
        ];
        write_params(queue, self.dof.params(), &p);
        let src = bind_tex(device, &self.tex_bgl, src_view, &self.sampler, "DOF Src");
        let depth = self.depth_view.clone();
        let extra = bind_tex(device, &self.extra_bgl, &depth, &self.sampler, "DOF Depth");
        draw_pass(encoder, &self.dof, dst_view, &src, Some(&extra));
    }

    #[allow(clippy::too_many_arguments)]
    fn run_motion(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::TextureView,
        src_is_scratch: bool,
        current_vp: Mat4,
        near: f32,
        far: f32,
    ) {
        let prev = if self.has_prev {
            self.prev_vp
        } else {
            current_vp
        };
        let inv = current_vp.inverse();
        let mut p = PostParams::zeroed();
        p.v[0] = self.motion_amt;
        p.v[1] = 1.0;
        p.extra[0] = near.max(1e-3);
        p.extra[1] = far.max(near + 0.01);
        write_pair(&mut p, inv, prev);
        write_params(queue, self.motion.params(), &p);
        let scratch = self.scratch_view.clone();
        let (src_view, dst_view): (&wgpu::TextureView, &wgpu::TextureView) = if src_is_scratch {
            (&scratch, frame)
        } else {
            (frame, &scratch)
        };
        let src = bind_tex(device, &self.tex_bgl, src_view, &self.sampler, "Motion Src");
        let depth = self.depth_view.clone();
        let extra = bind_tex(
            device,
            &self.extra_bgl,
            &depth,
            &self.sampler,
            "Motion Depth",
        );
        draw_pass(encoder, &self.motion, dst_view, &src, Some(&extra));
    }

    fn run_copy(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::TextureView,
    ) {
        let mut p = PostParams::zeroed();
        p.v[0] = 1.0;
        write_params(queue, self.copy.params(), &p);
        let scratch = self.scratch_view.clone();
        let src = bind_tex(
            device,
            &self.tex_bgl,
            &scratch,
            &self.sampler,
            "Post Copy Src",
        );
        draw_pass(encoder, &self.copy, frame, &src, None);
    }
}

/// Halton sequence in (0, 1). Index 0 is 0. Bases 2 and 3 are the TAA pair.
pub fn halton(mut index: u32, base: u32) -> f32 {
    let mut f = 1.0f32;
    let mut r = 0.0f32;
    let b = base.max(2);
    while index > 0 {
        f /= b as f32;
        r += f * (index % b) as f32;
        index /= b;
    }
    r
}

/// Sub-pixel offset in NDC. One pixel is `2 / resolution`; the sample stays
/// inside half a pixel so a flat surface's center does not change.
pub fn ndc_jitter(frame: u32, width: u32, height: u32) -> [f32; 2] {
    let n = frame + 1;
    let x = halton(n, 2) - 0.5;
    let y = halton(n, 3) - 0.5;
    [
        x * 2.0 / width.max(1) as f32,
        y * 2.0 / height.max(1) as f32,
    ]
}

/// `clip.xy += jitter * clip.w`, so NDC shifts by `jitter` for every point.
pub fn apply_projection_jitter(proj: Mat4, jitter_ndc: [f32; 2]) -> Mat4 {
    let mut p = proj;
    for row in 0..2 {
        let j = jitter_ndc[row];
        for col in 0..4 {
            let w = p.col(col).w;
            let mut c = p.col(col);
            c[row] += j * w;
            *p.col_mut(col) = c;
        }
    }
    p
}

/// Positive eye distance from a wgpu DirectX-RH depth (0 at the near plane).
#[cfg(test)]
fn linear_eye_depth(depth: f32, near: f32, far: f32) -> f32 {
    let z = depth.clamp(0.0, 1.0);
    (near * far) / (far - z * (far - near)).max(1e-4)
}

fn write_pair(dst: &mut PostParams, inv_vp: Mat4, prev_vp: Mat4) {
    let a = inv_vp.to_cols_array_2d();
    dst.m0 = a[0];
    dst.m1 = a[1];
    dst.m2 = a[2];
    dst.m3 = a[3];
    let b = prev_vp.to_cols_array_2d();
    dst.p0 = b[0];
    dst.p1 = b[1];
    dst.p2 = b[2];
    dst.p3 = b[3];
}

fn write_params(queue: &wgpu::Queue, buf: &wgpu::Buffer, params: &PostParams) {
    queue.write_buffer(buf, 0, bytemuck::bytes_of(params));
}

fn draw_pass(
    encoder: &mut wgpu::CommandEncoder,
    pass: &impl Pass,
    target: &wgpu::TextureView,
    src: &wgpu::BindGroup,
    extra: Option<&wgpu::BindGroup>,
) {
    if !pass.enabled() {
        return;
    }
    let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(pass.name()),
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
    rp.set_pipeline(pass.pipeline());
    rp.set_bind_group(0, src, &[]);
    rp.set_bind_group(1, pass.params_bind(), &[]);
    if let Some(extra) = extra {
        rp.set_bind_group(2, extra, &[]);
    }
    rp.draw(0..3, 0..1);
}

fn pass(
    device: &wgpu::Device,
    params_bgl: &wgpu::BindGroupLayout,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    entry: &str,
    format: wgpu::TextureFormat,
    label: &'static str,
) -> FsPass {
    let params_buf = uniform_buffer(device, label);
    let params_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: params_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: params_buf.as_entire_binding(),
        }],
    });
    FsPass {
        name: label,
        pipeline: pipeline(device, shader, entry, layout, format, label),
        params_buf,
        params_bg,
        enabled: false,
    }
}

fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    entry: &str,
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
            entry_point: Some(entry),
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

fn uniform_buffer(device: &wgpu::Device, label: &str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: std::mem::size_of::<PostParams>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn color_tex(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    w: u32,
    h: u32,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w.max(1),
            height: h.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    (tex, view)
}

fn half_size(w: u32, h: u32) -> (u32, u32) {
    ((w / 2).max(1), (h / 2).max(1))
}

fn tex_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            multisampled: false,
            view_dimension: wgpu::TextureViewDimension::D2,
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
        },
        count: None,
    }
}

fn samp_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene3d::Camera;
    use glam::Vec3;

    #[test]
    fn post_params_are_160_bytes() {
        assert_eq!(std::mem::size_of::<PostParams>(), 160);
        assert_eq!(std::mem::size_of::<PostParams>() % 16, 0);
    }

    #[test]
    fn halton_stays_inside_the_unit_interval() {
        assert_eq!(halton(0, 2), 0.0);
        let mut prev = -1.0f32;
        for i in 1..16 {
            let x = halton(i, 2);
            let y = halton(i, 3);
            assert!(x > 0.0 && x < 1.0, "{x}");
            assert!(y > 0.0 && y < 1.0, "{y}");
            assert!((x - prev).abs() > 1e-6 || i == 1);
            prev = x;
        }
    }

    #[test]
    fn jitter_is_half_a_pixel_and_shifts_ndc() {
        let w = 64u32;
        let h = 40u32;
        for frame in 0..8 {
            let j = ndc_jitter(frame, w, h);
            assert!(j[0].abs() <= 1.0 / w as f32 + 1e-6);
            assert!(j[1].abs() <= 1.0 / h as f32 + 1e-6);
        }
        let cam = Camera {
            eye: Vec3::new(0.0, 0.0, 5.0),
            target: Vec3::ZERO,
            ..Camera::default()
        };
        let proj = cam.projection(1.6);
        let view = cam.view();
        let point = Vec3::new(0.2, -0.1, 1.0);
        let j = [0.013f32, -0.007];
        let clip0 = proj * view * point.extend(1.0);
        let ndc0 = clip0.truncate() / clip0.w;
        let clip1 = apply_projection_jitter(proj, j) * view * point.extend(1.0);
        let ndc1 = clip1.truncate() / clip1.w;
        assert!(
            (ndc1.x - ndc0.x - j[0]).abs() < 1e-4,
            "{} vs {}",
            ndc1.x,
            ndc0.x
        );
        assert!(
            (ndc1.y - ndc0.y - j[1]).abs() < 1e-4,
            "{} vs {}",
            ndc1.y,
            ndc0.y
        );
    }

    #[test]
    fn linear_eye_matches_the_shader() {
        let near = 0.1f32;
        let far = 100.0;
        assert!((linear_eye_depth(0.0, near, far) - near).abs() < 1e-4);
        assert!((linear_eye_depth(1.0, near, far) - far).abs() < 1e-2);
        let mid = linear_eye_depth(0.5, near, far);
        assert!(mid > near && mid < far);
    }

    #[test]
    fn linear_eye_matches_a_projected_point() {
        let cam = Camera::default();
        let vp = cam.view_projection(1.0);
        for p in [
            Vec3::new(0.0, 0.5, 2.4),
            Vec3::new(1.3, 0.5, -4.0),
            Vec3::new(0.0, 0.0, 0.0),
        ] {
            let clip = vp * p.extend(1.0);
            let depth = clip.z / clip.w;
            let eye = linear_eye_depth(depth, cam.near, cam.far);
            let dist = (p - cam.eye).length();
            assert!(
                (eye - dist).abs() < 0.05 * dist.max(1.0),
                "point {p:?} depth {depth} eye {eye} dist {dist}"
            );
        }
    }

    #[test]
    fn post_shader_is_webgl2_safe() {
        let src = include_str!("post.wgsl");
        assert!(!src.contains("var<storage"), "no storage buffers");
        assert!(src.contains("fn fs_ssao"));
        assert!(src.contains("fn fs_dof"));
        assert!(src.contains("fn fs_motion"));
        assert!(src.contains("fn fs_taa"));
        assert!(src.contains("fn fs_vignette"));
    }
}
