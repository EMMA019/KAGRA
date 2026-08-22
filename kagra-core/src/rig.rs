// src/rig.rs
use std::sync::Arc;
use nalgebra::{Matrix4, Point3, Vector3, UnitQuaternion, Unit};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use wgpu::util::DeviceExt;
use crate::renderer::{SkinnedVertex, SkinnedMeshCommand};
use crate::error::{KaguraError, KaguraResult};

// ── データ構造 ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Rig {
    pub version: String,
    #[serde(rename = "modelName")]
    pub model_name: String,
    pub texture: String,
    #[serde(rename = "depth_map")]
    pub depth_map: Option<String>,
    pub bones: Vec<Bone>,
    #[serde(rename = "physicsChains")]
    pub physics_chains: Vec<PhysicsChain>,
    pub parts: HashMap<String, Part>,

    #[serde(skip)]
    pub gpu_cache: HashMap<String, PartGpuCache>,

    // IK 関連
    #[serde(skip)]
    pub ik_targets: HashMap<String, Point3<f32>>,
    #[serde(skip)]
    pub ik_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Bone {
    pub name: String,
    pub parent: i32,
    pub pos: [f32; 2],
    #[serde(rename = "type")]
    pub bone_type: String,
    pub subtype: String,
    pub role: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct PhysicsChain {
    #[serde(rename = "type")]
    pub chain_type: String,
    pub indices: Vec<usize>,
    pub stiffness: f32,
    pub damping: f32,
    pub gravity: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Part {
    pub texture: String,
    pub mesh: Mesh,
    pub weights: Vec<Vec<Weight>>,
    #[serde(rename = "pmf_baked")]
    pub pmf_baked: HashMap<String, Vec<Pmf>>,
    #[serde(rename = "deformMode")]
    pub deform_mode: String,
    pub z: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Mesh {
    pub vertices: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub triangles: Vec<[usize; 3]>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Weight {
    pub idx: usize,
    pub val: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Pmf {
    pub vid: usize,
    pub vec: [f32; 2],
    pub w: f32,
}

#[derive(Debug, Clone)]
pub struct PartGpuCache {
    pub vertex_buffer: Arc<wgpu::Buffer>,
    pub index_buffer: Arc<wgpu::Buffer>,
    pub num_indices: u32,
    pub blend_weights_buffer: Arc<wgpu::Buffer>,
    pub morph_delta_buffer: Arc<wgpu::Buffer>,
}

// ── ロード（KaguraResult 版）──────────────────────────────────

pub fn load_rig<P: AsRef<Path>>(path: P) -> KaguraResult<Rig> {
    let file = File::open(path).map_err(|e| KaguraError::Io(e))?;
    let reader = BufReader::new(file);
    let mut rig: Rig = serde_json::from_reader(reader)
        .map_err(|e| KaguraError::Other(format!("JSON parse: {}", e)))?;
    rig.gpu_cache = HashMap::new();
    rig.ik_targets = HashMap::new();
    rig.ik_enabled = false;
    Ok(rig)
}

pub fn build_gpu_cache(rig: &mut Rig, device: &wgpu::Device) -> KaguraResult<()> {
    for (part_name, part) in &rig.parts {
        let cache = build_part_cache(device, part_name, part)
            .ok_or_else(|| KaguraError::Other(format!("Failed to build cache for part '{}'", part_name)))?;
        log::debug!("GPU cache built: part='{}' indices={}", part_name, cache.num_indices);
        rig.gpu_cache.insert(part_name.clone(), cache);
    }
    log::info!("Rig '{}': {} parts cached", rig.model_name, rig.gpu_cache.len());
    Ok(())
}

fn build_part_cache(
    device: &wgpu::Device,
    part_name: &str,
    part: &Part,
) -> Option<PartGpuCache> {
    if part.mesh.vertices.is_empty() || part.mesh.triangles.is_empty() {
        return None;
    }

    let vertices: Vec<SkinnedVertex> = part.mesh.vertices.iter().enumerate().map(|(i, v)| {
        let weights = &part.weights[i];
        let mut joints      = [0u32; 4];
        let mut weight_vals = [0.0f32; 4];
        for (j, w) in weights.iter().take(4).enumerate() {
            joints[j]      = w.idx as u32;
            weight_vals[j] = w.val;
        }
        SkinnedVertex {
            position: *v,
            uv: part.mesh.uvs[i],
            joints,
            weights: weight_vals,
            // 2D スキニングシェーダは法線を参照しない
            normal: [0.0, 0.0, 1.0],
        }
    }).collect();

    let vertex_buffer = Arc::new(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label:    Some(&format!("{} VB", part_name)),
        contents: bytemuck::cast_slice(&vertices),
        usage:    wgpu::BufferUsages::VERTEX,
    }));

    let indices: Vec<u32> = part.mesh.triangles
        .iter()
        .flat_map(|tri| tri.iter().map(|&i| i as u32))
        .collect();

    let index_buffer = Arc::new(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label:    Some(&format!("{} IB", part_name)),
        contents: bytemuck::cast_slice(&indices),
        usage:    wgpu::BufferUsages::INDEX,
    }));

    let blend_weights_buffer = Arc::new(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label:    Some(&format!("{} Dummy BlendWeights", part_name)),
        contents: bytemuck::cast_slice(&[0.0f32; 32]),
        usage:    wgpu::BufferUsages::UNIFORM,
    }));
    let morph_delta_buffer = Arc::new(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label:    Some(&format!("{} Dummy MorphDelta", part_name)),
        contents: &[0u8],
        usage:    wgpu::BufferUsages::STORAGE,
    }));

    Some(PartGpuCache {
        vertex_buffer,
        index_buffer,
        num_indices: indices.len() as u32,
        blend_weights_buffer,
        morph_delta_buffer,
    })
}

// ── IK ソルバー（2ボーン）───────────────────────────────────

pub fn solve_two_bone_ik(
    shoulder_world: &Matrix4<f32>,
    elbow_world: &Matrix4<f32>,
    _wrist_world: &Matrix4<f32>,
    target_wrist: &Point3<f32>,
    upper_len: f32,
    lower_len: f32,
) -> (UnitQuaternion<f32>, UnitQuaternion<f32>) {
    let shoulder_pos = shoulder_world.transform_point(&Point3::origin());
    let elbow_pos    = elbow_world.transform_point(&Point3::origin());

    let dir_shoulder_to_target = (target_wrist - shoulder_pos).normalize();
    let dir_shoulder_to_elbow  = (elbow_pos - shoulder_pos).normalize();

    let rot_axis = dir_shoulder_to_elbow.cross(&dir_shoulder_to_target);
    let rot_axis_norm = rot_axis.normalize();
    let rot_angle = dir_shoulder_to_elbow.dot(&dir_shoulder_to_target).acos();
    let shoulder_rot = if rot_angle > 1e-6 {
        UnitQuaternion::from_axis_angle(&Unit::new_normalize(rot_axis_norm), rot_angle)
    } else {
        UnitQuaternion::identity()
    };

    let rotated_elbow_dir = shoulder_rot * (elbow_pos - shoulder_pos);
    let rotated_elbow_pos = shoulder_pos + rotated_elbow_dir;

    let dx = target_wrist - rotated_elbow_pos;
    let target_len = dx.magnitude();
    let cos_theta = (upper_len*upper_len + lower_len*lower_len - target_len*target_len)
                    / (2.0 * upper_len * lower_len);
    let theta = cos_theta.clamp(-1.0, 1.0).acos();

    let elbow_axis = Vector3::z_axis();
    let elbow_rot = UnitQuaternion::from_axis_angle(&elbow_axis, theta);

    (shoulder_rot, elbow_rot)
}

// ── Rig メソッド ─────────────────────────────────────────────

impl Rig {
    pub fn compute_bone_matrices(&self, bone_rotations: &[f32]) -> Vec<Matrix4<f32>> {
        let mut world = vec![Matrix4::identity(); self.bones.len()];

        // 1. FK 計算（全ボーン）
        for (i, bone) in self.bones.iter().enumerate() {
            let rot_deg = bone_rotations[i];
            let rot_rad = rot_deg.to_radians();
            let (sin, cos) = rot_rad.sin_cos();
            let local = Matrix4::new(
                cos, -sin, 0.0, bone.pos[0],
                sin,  cos, 0.0, bone.pos[1],
                0.0,  0.0, 1.0, 0.0,
                0.0,  0.0, 0.0, 1.0,
            );
            if bone.parent >= 0 {
                world[i] = world[bone.parent as usize] * local;
            } else {
                world[i] = local;
            }
        }

        // 2. IK を適用（肩・肘の行列を変更）
        let mut modified = Vec::new();
        if self.ik_enabled {
            for (target_name, target) in &self.ik_targets {
                let wrist_idx = self.bones.iter().position(|b| &b.name == target_name);
                if let Some(w) = wrist_idx {
                    let elbow_idx = if self.bones[w].parent >= 0 { Some(self.bones[w].parent as usize) } else { None };
                    if let Some(e) = elbow_idx {
                        let shoulder_idx = if self.bones[e].parent >= 0 { Some(self.bones[e].parent as usize) } else { None };
                        if let Some(s) = shoulder_idx {
                            // 長さを計算
                            let upper_len = (world[e].transform_point(&Point3::origin()) -
                                             world[s].transform_point(&Point3::origin())).magnitude();
                            let lower_len = (world[w].transform_point(&Point3::origin()) -
                                             world[e].transform_point(&Point3::origin())).magnitude();

                            let (shoulder_rot, elbow_rot) = solve_two_bone_ik(
                                &world[s], &world[e], &world[w], target,
                                upper_len, lower_len,
                            );

                            // 肩の行列を更新
                            let rot3_shoulder = world[s].fixed_view::<3,3>(0,0).into_owned();
                            let shoulder_quat = UnitQuaternion::from_matrix(&rot3_shoulder);
                            let new_shoulder_quat = shoulder_rot * shoulder_quat;
                            let mut new_shoulder = new_shoulder_quat.to_homogeneous();
                            new_shoulder[(0,3)] = world[s][(0,3)];
                            new_shoulder[(1,3)] = world[s][(1,3)];
                            new_shoulder[(2,3)] = world[s][(2,3)];
                            world[s] = new_shoulder;
                            modified.push(s);

                            // 肘の行列を更新
                            let rot3_elbow = world[e].fixed_view::<3,3>(0,0).into_owned();
                            let elbow_quat = UnitQuaternion::from_matrix(&rot3_elbow);
                            let new_elbow_quat = elbow_rot * elbow_quat;
                            let mut new_elbow = new_elbow_quat.to_homogeneous();
                            new_elbow[(0,3)] = world[e][(0,3)];
                            new_elbow[(1,3)] = world[e][(1,3)];
                            new_elbow[(2,3)] = world[e][(2,3)];
                            world[e] = new_elbow;
                            modified.push(e);
                        }
                    }
                }
            }
        }

        // 3. 変更されたボーンの子孫を再計算（IK の伝搬）
        for &idx in &modified {
            let mut stack = vec![idx];
            while let Some(parent_idx) = stack.pop() {
                for (child_idx, bone) in self.bones.iter().enumerate() {
                    if bone.parent == parent_idx as i32 {
                        let rot_deg = bone_rotations[child_idx];
                        let rot_rad = rot_deg.to_radians();
                        let (sin, cos) = rot_rad.sin_cos();
                        let local = Matrix4::new(
                            cos, -sin, 0.0, bone.pos[0],
                            sin,  cos, 0.0, bone.pos[1],
                            0.0,  0.0, 1.0, 0.0,
                            0.0,  0.0, 0.0, 1.0,
                        );
                        world[child_idx] = world[parent_idx] * local;
                        stack.push(child_idx);
                    }
                }
            }
        }

        world
    }

    pub fn create_draw_command(
        &self,
        part_name: &str,
        texture_id: u32,
    ) -> Option<SkinnedMeshCommand> {
        let cache = self.gpu_cache.get(part_name)?;
        Some(SkinnedMeshCommand {
            texture_id,
            vertex_buffer:        Arc::clone(&cache.vertex_buffer),
            index_buffer:         Arc::clone(&cache.index_buffer),
            num_indices:          cache.num_indices,
            blend_weights_buffer: Arc::clone(&cache.blend_weights_buffer),
            morph_delta_buffer:   Arc::clone(&cache.morph_delta_buffer),
            num_morph_targets:    0,
            mtoon_buffer: None,
            shade_texture_id: None,
            outline_width: 0.0,
            skin_slot: None,
        })
    }

    pub fn sorted_parts_by_z(&self) -> Vec<(String, String)> {
        let mut entries: Vec<(&str, &str, i32)> = self.parts.iter()
            .map(|(name, part)| (name.as_str(), part.texture.as_str(), part.z))
            .collect();
        entries.sort_by_key(|&(_, _, z)| z);
        entries.into_iter()
            .map(|(name, tex, _)| (name.to_string(), tex.to_string()))
            .collect()
    }

    pub fn get_texture_path(&self) -> Option<String> {
        self.parts.values().next().map(|p| p.texture.clone())
    }
}