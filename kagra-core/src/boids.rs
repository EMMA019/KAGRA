// src/boids.rs
use wgpu;
use rayon::prelude::*;
use crate::instance_renderer::InstanceData;

#[derive(Clone)]
pub struct Boid {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
}

pub struct BoidSystem {
    pub boids:      Vec<Boid>,
    pub width:      f32,
    pub height:     f32,
    pub max_speed:  f32,
    pub max_force:  f32,
    pub perception: f32,      // 視野距離
    pub view_angle_cos: f32,  // 視野角のcos値
    pub separation_weight: f32,
    pub alignment_weight: f32,
    pub cohesion_weight: f32,
    pub time:       f32,
    pub instance_data: Vec<InstanceData>,
}

impl BoidSystem {
    pub fn new(count: usize, width: f32, height: f32) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let boids: Vec<Boid> = (0..count).map(|i| {
            let mut h = DefaultHasher::new();
            i.hash(&mut h);
            let r1 = (h.finish() as f32) / (u64::MAX as f32);
            (i * 2 + 1).hash(&mut h);
            let r2 = (h.finish() as f32) / (u64::MAX as f32);
            (i * 3 + 7).hash(&mut h);
            let r3 = (h.finish() as f32) / (u64::MAX as f32) * 4.0 - 2.0;
            (i * 5 + 3).hash(&mut h);
            let r4 = (h.finish() as f32) / (u64::MAX as f32) * 4.0 - 2.0;

            Boid {
                x: r1 * width,
                y: r2 * height,
                vx: r3,
                vy: r4,
            }
        }).collect();

        let instance_data = boids.iter().map(|b| {
            let rot = b.vy.atan2(b.vx);
            InstanceData {
                x: b.x,
                y: b.y,
                scale_x: 6.0,
                scale_y: 3.0,
                rotation: rot,
                alpha: 1.0,
                _pad: [0.0; 2],
            }
        }).collect();

        Self {
            boids,
            width,
            height,
            max_speed: 3.0,
            max_force: 1.0,
            perception: 50.0,
            view_angle_cos: 0.9,   // 約25度前方
            separation_weight: 1.5,
            alignment_weight: 1.0,
            cohesion_weight: 1.0,
            time: 0.0,
            instance_data,
        }
    }

    /// ボイドの加速度を計算（分離・整列・結合）
    fn compute_acceleration(&self, i: usize) -> (f32, f32) {
        let b = &self.boids[i];
        let mut separation = (0.0, 0.0);
        let mut alignment = (0.0, 0.0);
        let mut cohesion = (0.0, 0.0);
        let mut neighbor_count = 0;

        let perception_sq = self.perception * self.perception;
        let (px, py) = (b.x, b.y);
        let (vx, vy) = (b.vx, b.vy);

        for (j, other) in self.boids.iter().enumerate() {
            if i == j { continue; }
            let dx = px - other.x;
            let dy = py - other.y;
            let dist_sq = dx*dx + dy*dy;
            if dist_sq > 0.0 && dist_sq < perception_sq {
                let dist = dist_sq.sqrt();
                // 視野角チェック（前方のみ）
                let to_other_x = -dx / dist;
                let to_other_y = -dy / dist;
                let speed = (vx*vx + vy*vy).sqrt();
                let forward_x = if speed > 0.0 { vx / speed } else { 1.0 };
                let forward_y = if speed > 0.0 { vy / speed } else { 0.0 };
                let dot = forward_x * to_other_x + forward_y * to_other_y;
                if dot >= self.view_angle_cos {
                    // 分離（近いほど強く離れる）
                    let inv_dist = self.perception / dist;
                    separation.0 += dx / dist * inv_dist;
                    separation.1 += dy / dist * inv_dist;
                    // 整列
                    alignment.0 += other.vx;
                    alignment.1 += other.vy;
                    // 結合
                    cohesion.0 += other.x;
                    cohesion.1 += other.y;
                    neighbor_count += 1;
                }
            }
        }

        if neighbor_count > 0 {
            let inv_count = 1.0 / neighbor_count as f32;
            alignment.0 *= inv_count;
            alignment.1 *= inv_count;
            cohesion.0 = cohesion.0 * inv_count - px;
            cohesion.1 = cohesion.1 * inv_count - py;

            let sep_x = separation.0 * self.separation_weight;
            let sep_y = separation.1 * self.separation_weight;
            let ali_x = alignment.0 * self.alignment_weight;
            let ali_y = alignment.1 * self.alignment_weight;
            let coh_x = cohesion.0 * self.cohesion_weight;
            let coh_y = cohesion.1 * self.cohesion_weight;

            let mut acc_x = sep_x + ali_x + coh_x;
            let mut acc_y = sep_y + ali_y + coh_y;
            let acc_len = (acc_x*acc_x + acc_y*acc_y).sqrt();
            if acc_len > self.max_force {
                acc_x = acc_x / acc_len * self.max_force;
                acc_y = acc_y / acc_len * self.max_force;
            }
            (acc_x, acc_y)
        } else {
            (0.0, 0.0)
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        let w = self.width;
        let h = self.height;
        let max_spd = self.max_speed;
        let dt_factor = dt * 60.0;  // フレームレート補正

        // 加速度を並列計算
        let accelerations: Vec<(f32, f32)> = (0..self.boids.len())
            .into_par_iter()
            .map(|i| self.compute_acceleration(i))
            .collect();

        // 速度と位置を更新（並列）
        self.boids.par_iter_mut().enumerate().for_each(|(i, boid)| {
            let (acc_x, acc_y) = accelerations[i];
            let mut new_vx = boid.vx + acc_x * dt;
            let mut new_vy = boid.vy + acc_y * dt;
            let spd = (new_vx*new_vx + new_vy*new_vy).sqrt();
            if spd > max_spd {
                new_vx = new_vx / spd * max_spd;
                new_vy = new_vy / spd * max_spd;
            }
            let mut new_x = boid.x + new_vx * dt_factor;
            let mut new_y = boid.y + new_vy * dt_factor;
            // 境界処理（トーラス）
            if new_x < 0.0 { new_x += w; }
            if new_x >= w { new_x -= w; }
            if new_y < 0.0 { new_y += h; }
            if new_y >= h { new_y -= h; }
            // UIエリアの反発（上部68px、下部38px）
            let ui_top = 68.0;
            let ui_bottom = h - 38.0;
            if new_y < ui_top {
                new_y = ui_top;
                new_vy = new_vy.abs();
            }
            if new_y > ui_bottom {
                new_y = ui_bottom;
                new_vy = -new_vy.abs();
            }
            boid.x = new_x;
            boid.y = new_y;
            boid.vx = new_vx;
            boid.vy = new_vy;
        });

        // インスタンスデータ更新
        self.instance_data.par_iter_mut().zip(&self.boids).for_each(|(inst, boid)| {
            inst.x = boid.x;
            inst.y = boid.y;
            inst.rotation = boid.vy.atan2(boid.vx);
        });
    }

    pub fn write_to_buffer(
        &self,
        queue: &wgpu::Queue,
        buffer: &wgpu::Buffer,
    ) {
        let count = self.boids.len().min(buffer.size() as usize / std::mem::size_of::<InstanceData>());
        if count == 0 { return; }
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&self.instance_data[..count]));
    }
}