// src/vrm.rs
// VRM (glTF Binary) ローダー + GPU スキニング + ブレンドシェイプ対応
// 修正: blend_weights 削除、set_blend_shape で dirty=true を設定

use std::collections::HashMap;
use std::sync::Arc;
use nalgebra::Matrix4;
use wgpu::util::DeviceExt;
use wgpu::Device;

use crate::renderer::SkinnedVertex;
use crate::error::{KaguraError, KaguraResult};
use crate::gltf_common::*;
use crate::vrm_humanoid::{apply_humanoid_aliases, parse_human_bones};
use crate::vrm_lookat_meta::parse_look_at;
use crate::mtoon::{parse_mtoon, parse_vrm0_material_properties, MtoonMaterial};

pub use crate::vrm_lookat_meta::VrmLookAtMeta;

// ── 公開構造体 ──────────────────────────────────────────────

pub struct VrmPrimitive {
    pub texture_id: u32,
    pub vertex_buf: Arc<wgpu::Buffer>,
    pub index_buf: Arc<wgpu::Buffer>,
    pub num_indices: u32,
    pub skin_idx: usize,
    pub morph_delta_buf: Arc<wgpu::Buffer>,
    pub num_morph_targets: u32,
    pub node_idx: usize,
    pub cached_weights: Option<([f32; 256], Arc<wgpu::Buffer>)>,
    pub mtoon: MtoonMaterial,
}

pub struct VrmBone {
    pub parent: Option<usize>,
    pub local_rot: [f32; 4],
    pub bind_rot: [f32; 4],
    pub local_trans: [f32; 3],
    pub bind_trans: [f32; 3],
    pub local_scale: [f32; 3],
    pub bind_scale: [f32; 3],
    pub world_mat: Matrix4<f32>,
}

pub struct VrmSkin {
    pub joint_node_indices: Vec<usize>,
    pub inv_bind_matrices: Vec<Matrix4<f32>>,
}

#[derive(Clone)]
pub struct MorphTarget {
    pub node_idx: usize,
    pub index: usize,
    pub weight: f32,
}

pub struct VrmModel {
    pub bones: Vec<VrmBone>,
    pub bone_index: HashMap<String, usize>,
    /// 親が必ず子より先に来るボーン走査順（ノード配列の並び順は当てにできない）
    pub hierarchy_order: Vec<usize>,
    pub skins: Vec<VrmSkin>,
    pub primitives: Vec<VrmPrimitive>,
    pub dirty: bool,
    pub root_offset: [f32; 3],
    // blend_weights を削除
    pub active_expressions: HashMap<String, f32>,
    pub expression_targets: HashMap<String, Vec<MorphTarget>>,
    pub blend_index: HashMap<String, usize>,
    /// VRM humanoid 標準名 → ノード index（hips, head, …）
    pub human_bones: HashMap<String, usize>,
    /// VRM LookAt メタ（VRM1 lookAt / VRM0 firstPerson）
    pub look_at: Option<VrmLookAtMeta>,
}



// ── 表情パース（VRM 1.0 + VRM 0.x）────────────────────────────

fn insert_expression(
    expression_targets: &mut HashMap<String, Vec<MorphTarget>>,
    blend_index: &mut HashMap<String, usize>,
    name: String,
    targets: Vec<MorphTarget>,
) {
    if targets.is_empty() {
        return;
    }
    let first_index = targets.first().map(|t| t.index);
    expression_targets.insert(name.clone(), targets);
    if let Some(idx) = first_index {
        blend_index.insert(name, idx);
    }
}

fn parse_expressions(
    gltf: &serde_json::Value,
    mesh_to_node: &HashMap<usize, usize>,
) -> (HashMap<String, Vec<MorphTarget>>, HashMap<String, usize>) {
    let mut expression_targets = HashMap::new();
    let mut blend_index = HashMap::new();

    if let Some(vrmc_vrm) = gltf.get("extensions").and_then(|e| e.get("VRMC_vrm")) {
        if let Some(expressions_obj) = vrmc_vrm.get("expressions").and_then(|e| e.as_object()) {
            for section in ["preset", "custom"] {
                if let Some(map) = expressions_obj.get(section).and_then(|p| p.as_object()) {
                    for (expr_name, expr) in map {
                        let mut targets = Vec::new();
                        let binds = expr
                            .get("binds")
                            .or_else(|| expr.get("morphTargetBinds"))
                            .and_then(|b| b.as_array());
                        let Some(binds) = binds else { continue };
                        for bind in binds {
                            let node_i = bind.get("node").and_then(|n| n.as_u64()).unwrap_or(0) as usize;
                            if let (Some(idx), Some(w)) = (
                                bind.get("index").and_then(|i| i.as_u64()),
                                bind.get("weight").and_then(|w| w.as_f64()),
                            ) {
                                targets.push(MorphTarget {
                                    node_idx: node_i,
                                    index: idx as usize,
                                    weight: w as f32,
                                });
                            }
                        }
                        insert_expression(
                            &mut expression_targets,
                            &mut blend_index,
                            expr_name.to_string(),
                            targets,
                        );
                    }
                }
            }
        }
    }

    // VRM 0.x blendShapeMaster（mesh index → node に変換）
    if let Some(groups) = gltf
        .pointer("/extensions/VRM/blendShapeMaster/blendShapeGroups")
        .and_then(|g| g.as_array())
    {
        for group in groups {
            let preset = group
                .get("presetName")
                .and_then(|p| p.as_str())
                .unwrap_or("");
            let name = group.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let key = if !preset.is_empty() && preset != "unknown" {
                preset.to_string()
            } else if !name.is_empty() {
                name.to_string()
            } else {
                continue;
            };
            // 既に VRM1 で入っていれば上書きしない
            if expression_targets.contains_key(&key) {
                continue;
            }
            let mut targets = Vec::new();
            if let Some(binds) = group.get("binds").and_then(|b| b.as_array()) {
                for bind in binds {
                    let mesh_i = bind.get("mesh").and_then(|m| m.as_u64()).unwrap_or(0) as usize;
                    let node_i = mesh_to_node.get(&mesh_i).copied().unwrap_or(mesh_i);
                    if let (Some(idx), Some(w)) = (
                        bind.get("index").and_then(|i| i.as_u64()),
                        bind.get("weight").and_then(|w| w.as_f64()),
                    ) {
                        // VRM0 weight は 0–100
                        let weight = if w > 1.0 { (w as f32) / 100.0 } else { w as f32 };
                        targets.push(MorphTarget {
                            node_idx: node_i,
                            index: idx as usize,
                            weight,
                        });
                    }
                }
            }
            insert_expression(&mut expression_targets, &mut blend_index, key, targets);
        }
    }

    (expression_targets, blend_index)
}

// ── メインロード関数（公開）───────────────────────────────────

pub fn load_vrm(
    path: &str,
    device: &Device,
    tex_id_map: &HashMap<usize, u32>,
) -> KaguraResult<VrmModel> {
    let data = std::fs::read(path)?;
    if &data[0..4] != b"glTF" {
        return Err(KaguraError::VrmParse("glTFではありません".to_string()));
    }

    let mut offset = 12usize;
    let mut json_bytes: Option<&[u8]> = None;
    let mut bin_data: &[u8] = &[];
    while offset + 8 <= data.len() {
        let chunk_len = read_u32_le(&data, offset) as usize;
        let chunk_type = read_u32_le(&data, offset + 4);
        let chunk_data = &data[offset + 8 .. (offset + 8 + chunk_len).min(data.len())];
        match chunk_type {
            0x4E4F534A => json_bytes = Some(chunk_data),
            0x004E4942 => bin_data = chunk_data,
            _ => {}
        }
        offset += 8 + chunk_len;
    }

    let json_bytes = json_bytes.ok_or_else(|| KaguraError::VrmParse("JSONチャンクが見つかりません".to_string()))?;
    let json_str = std::str::from_utf8(json_bytes).map_err(|e| KaguraError::VrmParse(format!("UTF-8: {}", e)))?.trim_end_matches('\0');
    let gltf: serde_json::Value = serde_json::from_str(json_str).map_err(|e| KaguraError::VrmParse(format!("JSON: {}", e)))?;

    let nodes = gltf["nodes"].as_array().ok_or_else(|| KaguraError::VrmParse("nodes配列がありません".to_string()))?;
    let skins_arr = gltf["skins"].as_array().ok_or_else(|| KaguraError::VrmParse("skins配列がありません".to_string()))?;
    let meshes = gltf["meshes"].as_array().ok_or_else(|| KaguraError::VrmParse("meshes配列がありません".to_string()))?;
    let empty_materials = vec![];
    let materials = gltf["materials"].as_array().unwrap_or(&empty_materials);
    let mtoon_mats = parse_vrm0_material_properties(device, &gltf, materials, tex_id_map);

    // ── ボーン構築 ──────────────────────────────────────────
    let mut bones: Vec<VrmBone> = Vec::new();
    let mut bone_index: HashMap<String, usize> = HashMap::new();

    for (ni, node) in nodes.iter().enumerate() {
        let name = node["name"].as_str().unwrap_or("").to_string();
        let t = if let Some(arr) = node["translation"].as_array() {
            [
                arr[0].as_f64().unwrap_or(0.0) as f32,
                arr[1].as_f64().unwrap_or(0.0) as f32,
                arr[2].as_f64().unwrap_or(0.0) as f32,
            ]
        } else { [0.0,0.0,0.0] };
        let r = if let Some(arr) = node["rotation"].as_array() {
            [
                arr[0].as_f64().unwrap_or(0.0) as f32,
                arr[1].as_f64().unwrap_or(0.0) as f32,
                arr[2].as_f64().unwrap_or(0.0) as f32,
                arr[3].as_f64().unwrap_or(1.0) as f32,
            ]
        } else { [0.0,0.0,0.0,1.0] };
        let s = if let Some(arr) = node["scale"].as_array() {
            [
                arr[0].as_f64().unwrap_or(1.0) as f32,
                arr[1].as_f64().unwrap_or(1.0) as f32,
                arr[2].as_f64().unwrap_or(1.0) as f32,
            ]
        } else { [1.0,1.0,1.0] };
        if !name.is_empty() {
            bone_index.insert(name.clone(), ni);
        }
        bones.push(VrmBone {
            parent: None,
            local_rot: r,
            bind_rot: r,
            local_trans: t,
            bind_trans: t,
            local_scale: s,
            bind_scale: s,
            world_mat: Matrix4::identity(),
        });
    }

    // humanoid.humanBones → 標準名 / J_Bip_* エイリアス
    let human_bones = parse_human_bones(&gltf);
    apply_humanoid_aliases(&mut bone_index, &human_bones);

    // 親子関係
    for (ni, node) in nodes.iter().enumerate() {
        if let Some(children) = node["children"].as_array() {
            for child_val in children {
                let ci = child_val.as_u64().ok_or_else(|| KaguraError::VrmParse("child not u64".to_string()))? as usize;
                if ci < bones.len() {
                    bones[ci].parent = Some(ni);
                }
            }
        }
    }

    // ── スキン構築 ──────────────────────────────────────────
    let mut skins: Vec<VrmSkin> = Vec::new();
    for skin in skins_arr {
        let joints: Vec<usize> = skin["joints"].as_array()
            .ok_or_else(|| KaguraError::VrmParse("skin missing joints".to_string()))?
            .iter()
            .map(|v| v.as_u64().unwrap_or(0) as usize)
            .collect();
        let ibm_acc = skin["inverseBindMatrices"].as_u64().map(|v| v as usize);
        let inv_bind = if let Some(acc_idx) = ibm_acc {
            let flat = parse_accessor_f32(&gltf, bin_data, acc_idx, 16)?;
            flat.iter().map(|m| parse_mat4(m)).collect()
        } else {
            vec![Matrix4::identity(); joints.len()]
        };
        skins.push(VrmSkin { joint_node_indices: joints, inv_bind_matrices: inv_bind });
    }

    // mesh → skin / node マッピング
    let mut mesh_to_skin: HashMap<usize, usize> = HashMap::new();
    let mut mesh_to_node: HashMap<usize, usize> = HashMap::new();
    for (ni, node) in nodes.iter().enumerate() {
        if let Some(mi) = node["mesh"].as_u64().map(|v| v as usize) {
            mesh_to_node.insert(mi, ni);
            if let Some(si) = node["skin"].as_u64().map(|v| v as usize) {
                mesh_to_skin.insert(mi, si);
            }
        }
    }

    // 表情パース
    let (expression_targets, blend_index) = parse_expressions(&gltf, &mesh_to_node);

    // LookAt メタ
    let look_at = parse_look_at(&gltf);

    // ── プリミティブ構築（キャッシュ付き）────────────────────
    let mut primitives = Vec::new();
    for (mi, mesh) in meshes.iter().enumerate() {
        let skin_idx = mesh_to_skin.get(&mi).copied().unwrap_or(0);
        let primitives_arr = mesh["primitives"].as_array().ok_or_else(|| KaguraError::VrmParse("mesh missing primitives".to_string()))?;

        for prim in primitives_arr {
            let attrs = &prim["attributes"];
            let mat_idx = prim["material"].as_u64().map(|v| v as usize);
            let idx_acc = prim["indices"].as_u64().map(|v| v as usize);

            let kagra_tex_id = mat_idx
                .and_then(|mi| materials.get(mi))
                .and_then(|mat| {
                    mat["pbrMetallicRoughness"]["baseColorTexture"]["index"]
                        .as_u64()
                        .map(|ti| tex_id_map.get(&(ti as usize)).copied().unwrap_or(0))
                })
                .unwrap_or(0);

            let mtoon = mat_idx
                .and_then(|mi| mtoon_mats.get(mi).cloned())
                .unwrap_or_else(|| {
                    mat_idx
                        .and_then(|mi| materials.get(mi))
                        .map(|mat| parse_mtoon(device, mat, tex_id_map))
                        .unwrap_or_else(|| crate::mtoon::MtoonMaterial::default_mat(device))
                });

            let pos_acc = attrs["POSITION"].as_u64().map(|v| v as usize);
            let uv_acc = attrs["TEXCOORD_0"].as_u64().map(|v| v as usize);
            let jnt_acc = attrs["JOINTS_0"].as_u64().map(|v| v as usize);
            let wgt_acc = attrs["WEIGHTS_0"].as_u64().map(|v| v as usize);
            let nrm_acc = attrs["NORMAL"].as_u64().map(|v| v as usize);

            let positions = pos_acc.map(|a| parse_accessor_f32(&gltf, bin_data, a, 3)).transpose()?.unwrap_or_default();
            let uvs = uv_acc.map(|a| parse_accessor_f32(&gltf, bin_data, a, 2)).transpose()?.unwrap_or_default();
            let joints = jnt_acc.map(|a| parse_accessor_u8x4(&gltf, bin_data, a)).transpose()?.unwrap_or_default();
            let weights = wgt_acc.map(|a| parse_accessor_f32(&gltf, bin_data, a, 4)).transpose()?.unwrap_or_default();
            let normal_rows = nrm_acc.map(|a| parse_accessor_f32(&gltf, bin_data, a, 3)).transpose()?;

            if positions.is_empty() { continue; }
            let n = positions.len();

            let indices = idx_acc.map(|a| parse_accessor_u32(&gltf, bin_data, a)).transpose()?.unwrap_or_default();
            let pos_xyz: Vec<[f32; 3]> = positions.iter()
                .map(|v| [v[0], v[1], v[2]])
                .collect();
            let normals = resolve_normals(normal_rows.as_ref(), &pos_xyz, &indices);

            // JOINTS_0 は skin.joints への添字であり、行列パレットも同じ添字で
            // 構築するため（build_draw_commands 参照）ここでの再マップは不要。
            let vertices: Vec<SkinnedVertex> = (0..n).map(|i| {
                let u = uvs.get(i).map(|v| [v[0], v[1]]).unwrap_or([0.0;2]);
                let j = joints.get(i).copied().unwrap_or([0;4]);
                let w = weights.get(i).map(|v| [v[0], v[1], v[2], v[3]]).unwrap_or([1.0,0.0,0.0,0.0]);
                SkinnedVertex {
                    position: pos_xyz[i],
                    uv: u,
                    joints: j,
                    weights: w,
                    normal: normals[i],
                }
            }).collect();
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("VRM VB"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("VRM IB"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            let targets = prim["targets"].as_array();
            let num_targets = targets.map(|t| t.len()).unwrap_or(0);
            let vertices_len = n;
            let mut morph_deltas = Vec::new();
            if let Some(targets_arr) = targets {
                for target in targets_arr {
                    if let Some(pos_acc_idx) = target["POSITION"].as_u64().map(|v| v as usize) {
                        let deltas = parse_accessor_f32(&gltf, bin_data, pos_acc_idx, 3)?;
                        for d in deltas {
                            morph_deltas.push(d[0]); morph_deltas.push(d[1]); morph_deltas.push(d[2]);
                        }
                    } else {
                        for _ in 0..vertices_len {
                            morph_deltas.push(0.0); morph_deltas.push(0.0); morph_deltas.push(0.0);
                        }
                    }
                }
            }

            let morph_delta_buf = if morph_deltas.is_empty() {
                let dummy_data = [0.0f32; 4];
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("VRM Morph Delta (dummy)"),
                    contents: bytemuck::cast_slice(&dummy_data),
                    usage: wgpu::BufferUsages::STORAGE,
                })
            } else {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("VRM Morph Delta"),
                    contents: bytemuck::cast_slice(&morph_deltas),
                    usage: wgpu::BufferUsages::STORAGE,
                })
            };

            let prim_node_idx = mesh_to_node.get(&mi).copied().unwrap_or(0);
            primitives.push(VrmPrimitive {
                texture_id: kagra_tex_id,
                vertex_buf: Arc::new(vb),
                index_buf: Arc::new(ib),
                num_indices: indices.len() as u32,
                skin_idx,
                morph_delta_buf: Arc::new(morph_delta_buf),
                num_morph_targets: num_targets as u32,
                node_idx: prim_node_idx,
                cached_weights: None,
                mtoon,
            });
        }
    }

    let hierarchy_order = build_hierarchy_order(
        &bones.iter().map(|b| b.parent).collect::<Vec<_>>(),
    );

    // blend_weights は削除
    Ok(VrmModel {
        bones,
        bone_index,
        hierarchy_order,
        skins,
        primitives,
        dirty: true,
        root_offset: [0.0; 3],
        active_expressions: HashMap::new(),
        expression_targets,
        blend_index,
        human_bones,
        look_at,
    })
}

// ── VrmModel のメソッド ──────────────────────────────────────

impl VrmModel {
    pub fn set_blend_shape(&mut self, name: &str, weight: f32) {
        let w = weight.clamp(0.0, 1.0);
        if self.expression_targets.contains_key(name) {
            if w > 0.0 {
                self.active_expressions.insert(name.to_string(), w);
            } else {
                self.active_expressions.remove(name);
            }
            self.dirty = true;  // ← 追加: 表情変更時にキャッシュを無効化
        } else {
            let flag_key = format!("__warned_{}", name);
            if !self.active_expressions.contains_key(&flag_key) {
                self.active_expressions.insert(flag_key, -1.0);
            }
        }
    }

    // set_blend_shape_by_index は削除

    pub fn reset_blend_shapes(&mut self) {
        self.active_expressions.retain(|k, _| k.starts_with("__warned_"));
        self.dirty = true;  // ← 追加: リセット時もキャッシュを無効化
    }

    pub fn list_blend_shapes(&self) -> Vec<String> {
        self.expression_targets.keys().cloned().collect()
    }

    /// humanoid 標準名の一覧（hips, head, …）
    pub fn list_human_bones(&self) -> Vec<String> {
        let mut names: Vec<_> = self.human_bones.keys().cloned().collect();
        names.sort();
        names
    }

    /// ボーン名を解決してノード index を返す。
    /// 実ノード名 / 標準名 / J_Bip_* エイリアスのいずれでも可。
    pub fn resolve_bone(&self, name: &str) -> Option<usize> {
        self.bone_index.get(name).copied()
    }

    pub fn set_bone_rot_quat(&mut self, name: &str, qx: f32, qy: f32, qz: f32, qw: f32) {
        if let Some(&idx) = self.bone_index.get(name) {
            self.bones[idx].local_rot = [qx, qy, qz, qw];
            self.dirty = true;
        }
    }

    pub fn set_bone_trans(&mut self, name: &str, tx: f32, ty: f32, tz: f32) {
        if let Some(&idx) = self.bone_index.get(name) {
            self.bones[idx].local_trans = [tx, ty, tz];
            self.dirty = true;
        }
    }

    pub fn set_bone_scale(&mut self, name: &str, sx: f32, sy: f32, sz: f32) {
        if let Some(&idx) = self.bone_index.get(name) {
            self.bones[idx].local_scale = [sx, sy, sz];
            self.dirty = true;
        }
    }

    pub fn reset_pose(&mut self) {
        for bone in &mut self.bones {
            bone.local_rot = bone.bind_rot;
            bone.local_trans = bone.bind_trans;
            bone.local_scale = bone.bind_scale;
        }
        self.dirty = true;
    }

    pub fn recompute_world(&mut self) {
        let off = Matrix4::new_translation(&nalgebra::Vector3::new(
            self.root_offset[0], self.root_offset[1], self.root_offset[2],
        ));
        for oi in 0..self.hierarchy_order.len() {
            let i = self.hierarchy_order[oi];
            let local = trs_to_mat4(
                self.bones[i].local_trans,
                self.bones[i].local_rot,
                self.bones[i].local_scale,
            );
            let world = match self.bones[i].parent {
                Some(pi) if pi < self.bones.len() => self.bones[pi].world_mat * local,
                _ => off * local,
            };
            self.bones[i].world_mat = world;
        }
    }

    pub fn build_draw_commands(&mut self, device: &Device) -> Vec<(Vec<Matrix4<f32>>, crate::renderer::SkinnedMeshCommand)> {
        if self.dirty {
            self.recompute_world();
            self.dirty = false;
        }

        let mut result = Vec::new();
        for prim in &mut self.primitives {
            let mut current_weights = [0.0f32; 256];
            for (expr_name, &user_w) in &self.active_expressions {
                if expr_name.starts_with("__warned_") { continue; }
                if let Some(targets) = self.expression_targets.get(expr_name) {
                    for target in targets {
                        if target.node_idx == prim.node_idx && target.index < 256 {
                            let w = (user_w * target.weight).max(current_weights[target.index]);
                            current_weights[target.index] = w;
                        }
                    }
                }
            }

            let blend_weights_buf = match &prim.cached_weights {
                Some((cached, buf)) if cached == &current_weights => buf.clone(),
                _ => {
                    let new_buf = Arc::new(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("VRM Blend Weights"),
                        contents: bytemuck::cast_slice(&current_weights),
                        usage: wgpu::BufferUsages::UNIFORM,
                    }));
                    prim.cached_weights = Some((current_weights, new_buf.clone()));
                    new_buf
                }
            };

            let skin = match self.skins.get(prim.skin_idx) {
                Some(s) => s,
                None => continue,
            };
            let joints = &skin.joint_node_indices;
            let ibms = &skin.inv_bind_matrices;
            let n = joints.len().min(ibms.len());
            let mut matrices: Vec<Matrix4<f32>> = (0..n.min(256)).map(|ji| {
                let node_idx = joints[ji];
                let world = self.bones.get(node_idx).map(|b| b.world_mat).unwrap_or(Matrix4::identity());
                world * ibms[ji]
            }).collect();
            while matrices.len() < 256 {
                matrices.push(Matrix4::identity());
            }

            result.push((
                matrices,
                crate::renderer::SkinnedMeshCommand {
                    texture_id: prim.texture_id,
                    shade_texture_id: prim.mtoon.shade_texture_id,
                    mtoon_buffer: Some(Arc::clone(&prim.mtoon.buffer)),
                    outline_width: prim.mtoon.gpu.params[2],
                    vertex_buffer: Arc::clone(&prim.vertex_buf),
                    index_buffer: Arc::clone(&prim.index_buf),
                    num_indices: prim.num_indices,
                    blend_weights_buffer: blend_weights_buf,
                    morph_delta_buffer: Arc::clone(&prim.morph_delta_buf),
                    num_morph_targets: prim.num_morph_targets,
                    skin_slot: None,
                },
            ));
        }
        result
    }
}