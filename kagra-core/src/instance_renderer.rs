// src/instance_renderer.rs
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceData {
    pub x:       f32,
    pub y:       f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation: f32,
    pub alpha:   f32,
    pub _pad:    [f32; 2],
}

impl InstanceData {
    const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        2 => Float32x4,
        3 => Float32x2,
        4 => Float32x2,
        5 => Float32x2,
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceData>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 16,
                    shader_location: 3,
                },
            ],
        }
    }
}

pub struct InstanceBatch {
    pub texture_id:      u32,
    pub instance_buffer: Arc<wgpu::Buffer>,
    pub capacity:        u32,
    pub count:           u32,
    pub sprite_w:        f32,
    pub sprite_h:        f32,
}

impl InstanceBatch {
    pub fn new(
        device: &wgpu::Device,
        texture_id: u32,
        capacity: u32,
        sprite_w: f32,
        sprite_h: f32,
    ) -> Self {
        let data = vec![InstanceData {
            x: 0.0, y: 0.0,
            scale_x: 1.0, scale_y: 1.0,
            rotation: 0.0, alpha: 1.0,
            _pad: [0.0; 2],
        }; capacity as usize];

        let instance_buffer = Arc::new(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("InstanceBatch Buffer"),
            contents: bytemuck::cast_slice(&data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        }));

        Self {
            texture_id,
            instance_buffer,
            capacity,
            count: 0,
            sprite_w,
            sprite_h,
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, data: &[[f32; 6]]) {
        let count = data.len().min(self.capacity as usize);
        self.count = count as u32;

        let instances: Vec<InstanceData> = data[..count].iter().map(|d| InstanceData {
            x: d[0], y: d[1],
            scale_x: d[2], scale_y: d[3],
            rotation: d[4], alpha: d[5],
            _pad: [0.0; 2],
        }).collect();

        queue.write_buffer(&*self.instance_buffer, 0, bytemuck::cast_slice(&instances));
    }
}

pub struct InstanceRenderer {
    pipeline:    wgpu::RenderPipeline,
    quad_vb:     wgpu::Buffer,
    quad_ib:     wgpu::Buffer,
    screen_size_buffer: wgpu::Buffer,
    screen_size_bg:     wgpu::BindGroup,
    screen_size_bgl:    wgpu::BindGroupLayout,
    pub batches: HashMap<u32, InstanceBatch>,
    next_id:     u32,
    external_buffer: Option<(Arc<wgpu::Buffer>, u32)>,
}

impl InstanceRenderer {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Instance Shader"),
            source: wgpu::ShaderSource::Wgsl(INSTANCE_SHADER.into()),
        });

        let screen_data = [width as f32, height as f32, 0.0, 0.0_f32];
        let screen_size_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Screen Size"),
            contents: bytemuck::cast_slice(&screen_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let screen_size_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Screen BGL"),
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
        let screen_size_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Screen BG"),
            layout: &screen_size_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_size_buffer.as_entire_binding(),
            }],
        });

        let quad_verts: &[[f32; 4]] = &[
            [-0.5,  0.5,  0.0, 0.0],
            [ 0.5,  0.5,  1.0, 0.0],
            [ 0.5, -0.5,  1.0, 1.0],
            [-0.5, -0.5,  0.0, 1.0],
        ];
        let quad_indices: &[u16] = &[0, 1, 2, 0, 2, 3];

        let quad_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad VB"),
            contents: bytemuck::cast_slice(quad_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let quad_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad IB"),
            contents: bytemuck::cast_slice(quad_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Instance Pipeline Layout"),
            bind_group_layouts: &[&screen_size_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Instance Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: 16,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<InstanceData>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 0,
                                shader_location: 2,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 16,
                                shader_location: 3,
                            },
                        ],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: crate::renderer::msaa_state(),
            multiview: None,
        });

        Self {
            pipeline,
            quad_vb,
            quad_ib,
            screen_size_buffer,
            screen_size_bg,
            screen_size_bgl,
            batches: HashMap::new(),
            next_id: 1,
            external_buffer: None,
        }
    }

    pub fn resize(&self, queue: &wgpu::Queue, width: u32, height: u32) {
        let data = [width as f32, height as f32, 0.0, 0.0_f32];
        queue.write_buffer(&self.screen_size_buffer, 0, bytemuck::cast_slice(&data));
    }

    pub fn create_batch(
        &mut self,
        device: &wgpu::Device,
        texture_id: u32,
        capacity: u32,
        sprite_w: f32,
        sprite_h: f32,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let batch = InstanceBatch::new(device, texture_id, capacity, sprite_w, sprite_h);
        self.batches.insert(id, batch);
        id
    }

    pub fn set_external_buffer_arc(&mut self, buffer: Arc<wgpu::Buffer>, count: u32) {
        self.external_buffer = Some((buffer, count));
    }

    pub fn register_gpu_buffer(&mut self, buffer: Arc<wgpu::Buffer>, count: u32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.batches.insert(id, InstanceBatch {
            texture_id: 0,
            instance_buffer: buffer,
            capacity: count,
            count,
            sprite_w: 4.0,
            sprite_h: 2.0,
        });
        id
    }

    pub fn draw_batch<'a>(&'a self, rp: &mut wgpu::RenderPass<'a>, batch_id: u32) {
        rp.set_pipeline(&self.pipeline);
        rp.set_bind_group(0, &self.screen_size_bg, &[]);
        rp.set_vertex_buffer(0, self.quad_vb.slice(..));
        rp.set_index_buffer(self.quad_ib.slice(..), wgpu::IndexFormat::Uint16);

        if batch_id == u32::MAX {
            if let Some((ref buf, count)) = self.external_buffer {
                rp.set_vertex_buffer(1, buf.slice(..));
                rp.draw_indexed(0..6, 0, 0..count);
            }
        } else if let Some(batch) = self.batches.get(&batch_id) {
            if batch.count > 0 {
                rp.set_vertex_buffer(1, batch.instance_buffer.slice(..));
                rp.draw_indexed(0..6, 0, 0..batch.count);
            }
        }
    }

    pub fn update_batch(&mut self, queue: &wgpu::Queue, batch_id: u32, data: &[[f32; 6]]) {
        if let Some(batch) = self.batches.get_mut(&batch_id) {
            batch.update(queue, data);
        }
    }

    pub fn draw<'a>(&'a self, rp: &mut wgpu::RenderPass<'a>) {
        rp.set_pipeline(&self.pipeline);
        rp.set_bind_group(0, &self.screen_size_bg, &[]);
        rp.set_vertex_buffer(0, self.quad_vb.slice(..));
        rp.set_index_buffer(self.quad_ib.slice(..), wgpu::IndexFormat::Uint16);
        for batch in self.batches.values() {
            if batch.count == 0 { continue; }
            rp.set_vertex_buffer(1, batch.instance_buffer.slice(..));
            rp.draw_indexed(0..6, 0, 0..batch.count);
        }
    }
}

const INSTANCE_SHADER: &str = r#"
struct ScreenSize {
    size: vec2<f32>,
    _pad: vec2<f32>,
};
@group(0) @binding(0) var<uniform> screen: ScreenSize;

struct VertexIn {
    @location(0) pos:   vec2<f32>,
    @location(1) uv:    vec2<f32>,
    @location(2) inst:  vec4<f32>,
    @location(3) inst2: vec2<f32>,
};

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv:   vec2<f32>,
    @location(1) alpha: f32,
};

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    let ix = in.inst.x;
    let iy = in.inst.y;
    let sx = in.inst.z;
    let sy = in.inst.w;
    let rot = in.inst2.x;

    let c = cos(rot);
    let s = sin(rot);
    let rx = in.pos.x * c - in.pos.y * s;
    let ry = in.pos.x * s + in.pos.y * c;

    let px = ix + rx * sx;
    let py = iy + ry * sy;

    let ndcx =  (px / screen.size.x) * 2.0 - 1.0;
    let ndcy = -(py / screen.size.y) * 2.0 + 1.0;

    var out: VertexOut;
    out.clip_pos = vec4<f32>(ndcx, ndcy, 0.0, 1.0);
    out.uv       = in.uv;
    out.alpha    = in.inst2.y;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.9, 0.3, in.alpha);
}
"#;