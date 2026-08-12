// src/boids_gpu.rs
// GPU Compute Shader によるボイドシミュレーション（本物のボイド：分離・整列・結合）
// 注意: 近傍探索は全探索（O(N²)）で実装しています。パフォーマンスが必要な場合はボイド数を少なくしてください。

use wgpu::util::DeviceExt;
use std::sync::Arc;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuBoid {
    x: f32, y: f32, vx: f32, vy: f32,
    _pad: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BoidParams {
    pub dt: f32,
    pub time: f32,
    pub width: f32,
    pub height: f32,
    pub max_speed: f32,
    pub count: u32,
    pub separation_weight: f32,
    pub alignment_weight: f32,
    pub cohesion_weight: f32,
    pub perception_radius: f32,
    pub max_force: f32,
    pub view_angle_cos: f32,
    pub _pad: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceData {
    x: f32, y: f32,
    scale_x: f32, scale_y: f32,
    rotation: f32, alpha: f32,
    _pad: [f32; 2],
}

pub struct BoidSystemGpu {
    pub count: u32,
    pub active_count: u32,
    pub time: f32,
    pub pending_dt: Option<f32>,
    pub cached_batch_id: Option<u32>,
    boid_buffer: wgpu::Buffer,
    pub instance_buffer: Arc<wgpu::Buffer>,
    params_buffer: wgpu::Buffer,
    compute_pipeline: wgpu::ComputePipeline,
    compute_bg: wgpu::BindGroup,
}

impl BoidSystemGpu {
    pub fn new(device: &wgpu::Device, _queue: &wgpu::Queue,
               count: u32, width: f32, height: f32) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let boids: Vec<GpuBoid> = (0..count as usize).map(|i| {
            let mut h = DefaultHasher::new();
            i.hash(&mut h); let r1 = (h.finish() as f32) / (u64::MAX as f32);
            (i*2+1).hash(&mut h); let r2 = (h.finish() as f32) / (u64::MAX as f32);
            (i*3+7).hash(&mut h); let r3 = (h.finish() as f32) / (u64::MAX as f32) * 4.0 - 2.0;
            (i*5+3).hash(&mut h); let r4 = (h.finish() as f32) / (u64::MAX as f32) * 4.0 - 2.0;
            GpuBoid { x: r1*width, y: r2*height, vx: r3, vy: r4, _pad: [0.0;4] }
        }).collect();

        let boid_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Boid Buffer"),
            contents: bytemuck::cast_slice(&boids),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let initial_instances: Vec<InstanceData> = boids.iter().map(|b| {
            let rot = b.vy.atan2(b.vx);
            InstanceData { x: b.x, y: b.y, scale_x: 4.0, scale_y: 2.0,
                          rotation: rot, alpha: 1.0, _pad: [0.0;2] }
        }).collect();

        let instance_buffer = Arc::new(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Boid Instance Buffer"),
            contents: bytemuck::cast_slice(&initial_instances),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
        }));

        let params = BoidParams {
            dt: 0.016, time: 0.0, width, height,
            max_speed: 3.0, count,
            separation_weight: 1.5,
            alignment_weight: 1.0,
            cohesion_weight: 1.0,
            perception_radius: 50.0,
            max_force: 1.0,
            view_angle_cos: 0.9,
            _pad: [0.0; 2],
        };
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Boid Params"),
            contents: bytemuck::cast_slice(&[params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Boid Compute"),
            source: wgpu::ShaderSource::Wgsl(BOID_COMPUTE_SHADER.into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Boid BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });

        let compute_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Boid BG"), layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: boid_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: instance_buffer.as_entire_binding() },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Boid Compute Layout"),
            bind_group_layouts: &[&bgl], push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Boid Compute Pipeline"), layout: Some(&pipeline_layout),
            module: &shader, entry_point: "main",
        });

        Self {
            count, active_count: count, time: 0.0, pending_dt: None, cached_batch_id: None,
            boid_buffer, instance_buffer, params_buffer,
            compute_pipeline, compute_bg,
        }
    }

    pub fn record_compute_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        dt: f32,
        width: f32,
        height: f32,
        time: f32,
        active_count: u32,
    ) {
        let params = BoidParams {
            dt, time, width, height,
            max_speed: 3.0, count: active_count,
            separation_weight: 1.5,
            alignment_weight: 1.0,
            cohesion_weight: 1.0,
            perception_radius: 50.0,
            max_force: 1.0,
            view_angle_cos: 0.9,
            _pad: [0.0; 2],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[params]));

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Boid Compute Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.compute_pipeline);
        pass.set_bind_group(0, &self.compute_bg, &[]);
        pass.dispatch_workgroups((active_count + 255) / 256, 1, 1);
    }
}

const BOID_COMPUTE_SHADER: &str = r#"
struct Params {
    dt: f32,
    time: f32,
    width: f32,
    height: f32,
    max_speed: f32,
    count: u32,
    separation_weight: f32,
    alignment_weight: f32,
    cohesion_weight: f32,
    perception_radius: f32,
    max_force: f32,
    view_angle_cos: f32,
    _pad: vec2<f32>,
}

struct Boid {
    x: f32, y: f32,
    vx: f32, vy: f32,
    _pad: vec4<f32>,
}

struct Instance {
    x: f32, y: f32,
    scale_x: f32, scale_y: f32,
    rotation: f32, alpha: f32,
    _pad: vec2<f32>,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> boids: array<Boid>;
@group(0) @binding(2) var<storage, read_write> instances: array<Instance>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.count { return; }

    var b = boids[i];
    let pos = vec2<f32>(b.x, b.y);
    let vel = vec2<f32>(b.vx, b.vy);

    var sep = vec2<f32>(0.0);
    var ali = vec2<f32>(0.0);
    var coh = vec2<f32>(0.0);
    var neighbor_count = 0u;

    // 全探索による近傍検出（O(N²)）
    // パフォーマンスが必要な場合はボイド数を減らしてください（set_boid_active_count）
    for (var j = 0u; j < params.count; j++) {
        if i == j { continue; }
        let other = boids[j];
        let other_pos = vec2<f32>(other.x, other.y);
        let delta = pos - other_pos;
        let dist_sq = dot(delta, delta);
        if dist_sq > 0.0 && dist_sq <= params.perception_radius * params.perception_radius {
            let dist = sqrt(dist_sq);
            // 視野角チェック（前方のみ）
            let to_other = normalize(-delta);
            let forward = normalize(vel);
            let dot_fwd = dot(forward, to_other);
            if dot_fwd >= params.view_angle_cos {
                // 分離（近いほど強く離れる）
                let dir = normalize(delta);
                sep += dir * (params.perception_radius / dist);
                // 整列
                ali += vec2<f32>(other.vx, other.vy);
                // 結合
                coh += other_pos;
                neighbor_count++;
            }
        }
    }

    if neighbor_count > 0u {
        let inv_cnt = 1.0 / f32(neighbor_count);
        ali = ali * inv_cnt;
        coh = coh * inv_cnt - pos;

        let accel = sep * params.separation_weight + ali * params.alignment_weight + coh * params.cohesion_weight;
        let acc_len = length(accel);
        var new_vel = vel;
        if acc_len > params.max_force {
            new_vel += accel / acc_len * params.max_force * params.dt;
        } else {
            new_vel += accel * params.dt;
        }

        let spd = length(new_vel);
        if spd > params.max_speed {
            new_vel = new_vel / spd * params.max_speed;
        }

        var new_pos = pos + new_vel * params.dt * 60.0;
        if new_pos.x < 0.0 { new_pos.x += params.width; }
        if new_pos.x >= params.width { new_pos.x -= params.width; }
        if new_pos.y < 0.0 { new_pos.y += params.height; }
        if new_pos.y >= params.height { new_pos.y -= params.height; }

        let ui_top = 68.0;
        let ui_bottom = params.height - 38.0;
        if new_pos.y < ui_top {
            new_pos.y = ui_top;
            new_vel.y = abs(new_vel.y);
        }
        if new_pos.y > ui_bottom {
            new_pos.y = ui_bottom;
            new_vel.y = -abs(new_vel.y);
        }

        b.x = new_pos.x;
        b.y = new_pos.y;
        b.vx = new_vel.x;
        b.vy = new_vel.y;
    } else {
        // 近傍がない場合：ランダムウォーク
        let angle = params.time * 0.5 + f32(i) * 0.00001;
        b.vx += cos(angle) * 0.1;
        b.vy += sin(angle) * 0.1;
        let spd = sqrt(b.vx*b.vx + b.vy*b.vy);
        if spd > params.max_speed {
            b.vx = b.vx / spd * params.max_speed;
            b.vy = b.vy / spd * params.max_speed;
        }
        b.x = (b.x + b.vx * params.dt * 60.0) % params.width;
        b.y = (b.y + b.vy * params.dt * 60.0) % params.height;
        if b.x < 0.0 { b.x += params.width; }
        if b.y < 0.0 { b.y += params.height; }

        let ui_top = 68.0;
        let ui_bottom = params.height - 38.0;
        if b.y < ui_top { b.y = ui_top; b.vy = abs(b.vy); }
        if b.y > ui_bottom { b.y = ui_bottom; b.vy = -abs(b.vy); }
    }

    boids[i] = b;
    instances[i] = Instance(b.x, b.y, 4.0, 2.0, atan2(b.vy, b.vx), 1.0, vec2<f32>(0.0,0.0));
}
"#;