// src/gltf.rs
// glTF 2.0 ローダー（標準形式）- スキニング、モーフィング対応
// 修正: reset_pose 実装、バインド姿勢保持

use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use nalgebra::Matrix4;
use wgpu::util::DeviceExt;
use wgpu::Device;

use crate::renderer::SkinnedVertex;
use crate::error::{KaguraError, KaguraResult};
use crate::gltf_common::*;

// ----- 公開構造体 -----
pub struct GltfPrimitive {
    pub texture_id: u32,
    pub vertex_buf: Arc<wgpu::Buffer>,
    pub index_buf: Arc<wgpu::Buffer>,
    pub num_indices: u32,
    pub skin_idx: usize,
    pub morph_delta_buf: Arc<wgpu::Buffer>,
    pub num_morph_targets: u32,
    pub num_vertices: u32,
    pub cached_weights: Option<([f32; 256], Arc<wgpu::Buffer>)>,
}

pub struct GltfBone {
    pub name: String,
    pub parent: Option<usize>,
    pub bind_local: Matrix4<f32>,
    // バインド姿勢（リセット用）
    pub bind_trans: [f32; 3],
    pub bind_rot: [f32; 4],
    pub bind_scale: [f32; 3],
    // 現在の姿勢
    pub local_trans: [f32; 3],
    pub local_rot: [f32; 4],
    pub local_scale: [f32; 3],
    pub world_mat: Matrix4<f32>,
}

pub struct GltfSkin {
    pub joint_node_indices: Vec<usize>,
    pub inv_bind_matrices: Vec<Matrix4<f32>>,
}

pub struct GltfModel {
    pub nodes: Vec<GltfBone>,
    pub node_index: HashMap<String, usize>,
    /// 親が必ず子より先に来るノード走査順（ノード配列の並び順は当てにできない）
    pub hierarchy_order: Vec<usize>,
    pub skins: Vec<GltfSkin>,
    pub primitives: Vec<GltfPrimitive>,
    pub dirty: bool,
    pub root_offset: [f32; 3],
}

// ----- 内部構造体（パース用）-----
struct GltfNodeInfo {
    name: String,
    parent: Option<usize>,
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
    children: Vec<usize>,
    mesh: Option<usize>,
    skin: Option<usize>,
}

// ----- メインロード関数 -----
pub fn load_gltf(
    path: &str,
    device: &Device,
    tex_id_map: &HashMap<usize, u32>,
) -> KaguraResult<GltfModel> {
    let data = fs::read(path)?;
    if &data[0..4] != b"glTF" {
        return Err(KaguraError::Other("Not a glTF file".into()));
    }

    // GLB から JSON と BIN を抽出
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

    let json_bytes = json_bytes.ok_or_else(|| KaguraError::VrmParse("JSON chunk not found".to_string()))?;
    let json_str = std::str::from_utf8(json_bytes)
        .map_err(|e| KaguraError::VrmParse(format!("UTF-8 error: {}", e)))?
        .trim_end_matches('\0');
    let gltf: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| KaguraError::VrmParse(format!("JSON parse: {}", e)))?;

    let empty_vec = vec![];
    let empty_bv = vec![];
    let empty_buf = vec![];
    let empty_nodes = vec![];
    let empty_skins = vec![];
    let empty_mats = vec![];
    let empty_tex = vec![];
    let empty_img = vec![];

    let _accessors = gltf["accessors"].as_array().unwrap_or(&empty_vec);
    let _buffer_views = gltf["bufferViews"].as_array().unwrap_or(&empty_bv);
    let _buffers = gltf["buffers"].as_array().unwrap_or(&empty_buf);
    let meshes = gltf["meshes"].as_array().unwrap_or(&empty_vec);
    let nodes_arr = gltf["nodes"].as_array().unwrap_or(&empty_nodes);
    let skins_arr = gltf["skins"].as_array().unwrap_or(&empty_skins);
    let materials = gltf["materials"].as_array().unwrap_or(&empty_mats);
    let _textures = gltf["textures"].as_array().unwrap_or(&empty_tex);
    let _images = gltf["images"].as_array().unwrap_or(&empty_img);

    // ---- ボーン（ノード）の情報収集 ----
    let mut node_infos: Vec<GltfNodeInfo> = Vec::new();
    let mut node_name_to_index: HashMap<String, usize> = HashMap::new();
    for (ni, node) in nodes_arr.iter().enumerate() {
        let name = node["name"].as_str().unwrap_or("").to_string();
        let translation = if let Some(arr) = node["translation"].as_array() {
            [arr[0].as_f64().unwrap_or(0.0) as f32,
             arr[1].as_f64().unwrap_or(0.0) as f32,
             arr[2].as_f64().unwrap_or(0.0) as f32]
        } else { [0.0,0.0,0.0] };
        let rotation = if let Some(arr) = node["rotation"].as_array() {
            [arr[0].as_f64().unwrap_or(0.0) as f32,
             arr[1].as_f64().unwrap_or(0.0) as f32,
             arr[2].as_f64().unwrap_or(0.0) as f32,
             arr[3].as_f64().unwrap_or(1.0) as f32]
        } else { [0.0,0.0,0.0,1.0] };
        let scale = if let Some(arr) = node["scale"].as_array() {
            [arr[0].as_f64().unwrap_or(1.0) as f32,
             arr[1].as_f64().unwrap_or(1.0) as f32,
             arr[2].as_f64().unwrap_or(1.0) as f32]
        } else { [1.0,1.0,1.0] };
        let mesh = node["mesh"].as_u64().map(|v| v as usize);
        let skin = node["skin"].as_u64().map(|v| v as usize);
        let children = node["children"].as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_u64().map(|i| i as usize)).collect())
            .unwrap_or_default();
        node_infos.push(GltfNodeInfo {
            name: name.clone(),
            parent: None,
            translation,
            rotation,
            scale,
            children,
            mesh,
            skin,
        });
        if !name.is_empty() {
            node_name_to_index.insert(name, ni);
        }
    }

    // 親子関係の設定
    for i in 0..node_infos.len() {
        let children = node_infos[i].children.clone();
        for child_idx in children {
            if child_idx < node_infos.len() {
                node_infos[child_idx].parent = Some(i);
            }
        }
    }

    // ボーン構造体の生成（バインド姿勢を保存）
    let mut bones: Vec<GltfBone> = Vec::new();
    for info in node_infos {
        let local_mat = trs_to_mat4(info.translation, info.rotation, info.scale);
        bones.push(GltfBone {
            name: info.name,
            parent: info.parent,
            bind_local: local_mat,
            bind_trans: info.translation,
            bind_rot: info.rotation,
            bind_scale: info.scale,
            local_trans: info.translation,
            local_rot: info.rotation,
            local_scale: info.scale,
            world_mat: Matrix4::identity(),
        });
    }

    // ---- スキン構築 ----
    let mut skins: Vec<GltfSkin> = Vec::new();
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
        skins.push(GltfSkin { joint_node_indices: joints, inv_bind_matrices: inv_bind });
    }

    // mesh → skin / node マッピング
    let mut mesh_to_skin: HashMap<usize, usize> = HashMap::new();
    let mut mesh_to_node: HashMap<usize, usize> = HashMap::new();
    for (ni, node) in nodes_arr.iter().enumerate() {
        if let Some(mi) = node["mesh"].as_u64().map(|v| v as usize) {
            mesh_to_node.insert(mi, ni);
            if let Some(si) = node["skin"].as_u64().map(|v| v as usize) {
                mesh_to_skin.insert(mi, si);
            }
        }
    }

    // ---- プリミティブ（メッシュ）の構築 ----
    let mut primitives = Vec::new();
    for (mi, mesh) in meshes.iter().enumerate() {
        let skin_idx = mesh_to_skin.get(&mi).copied().unwrap_or(0);
        let primitives_arr = mesh["primitives"].as_array()
            .ok_or_else(|| KaguraError::VrmParse("mesh missing primitives".to_string()))?;

        for prim in primitives_arr {
            let attrs = &prim["attributes"];
            let mat_idx = prim["material"].as_u64().map(|v| v as usize);
            let idx_acc = prim["indices"].as_u64().map(|v| v as usize);

            // テクスチャID
            let kagra_tex_id = mat_idx
                .and_then(|mi| materials.get(mi))
                .and_then(|mat| {
                    mat["pbrMetallicRoughness"]["baseColorTexture"]["index"]
                        .as_u64()
                        .map(|ti| tex_id_map.get(&(ti as usize)).copied().unwrap_or(0))
                })
                .unwrap_or(0);

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
                label: Some("glTF VB"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("glTF IB"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            // モーフターゲット
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
                    label: Some("glTF Morph Delta (dummy)"),
                    contents: bytemuck::cast_slice(&dummy_data),
                    usage: wgpu::BufferUsages::STORAGE,
                })
            } else {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("glTF Morph Delta"),
                    contents: bytemuck::cast_slice(&morph_deltas),
                    usage: wgpu::BufferUsages::STORAGE,
                })
            };

            primitives.push(GltfPrimitive {
                texture_id: kagra_tex_id,
                vertex_buf: Arc::new(vb),
                index_buf: Arc::new(ib),
                num_indices: indices.len() as u32,
                skin_idx,
                morph_delta_buf: Arc::new(morph_delta_buf),
                num_morph_targets: num_targets as u32,
                num_vertices: vertices_len as u32,
                cached_weights: None,
            });
        }
    }

    let hierarchy_order = build_hierarchy_order(
        &bones.iter().map(|b| b.parent).collect::<Vec<_>>(),
    );

    Ok(GltfModel {
        nodes: bones,
        node_index: node_name_to_index,
        hierarchy_order,
        skins,
        primitives,
        dirty: true,
        root_offset: [0.0; 3],
    })
}

// ----- GltfModel のメソッド -----
impl GltfModel {
    pub fn set_bone_rot_quat(&mut self, name: &str, qx: f32, qy: f32, qz: f32, qw: f32) {
        if let Some(&idx) = self.node_index.get(name) {
            self.nodes[idx].local_rot = [qx, qy, qz, qw];
            self.dirty = true;
        }
    }

    pub fn set_bone_trans(&mut self, name: &str, tx: f32, ty: f32, tz: f32) {
        if let Some(&idx) = self.node_index.get(name) {
            self.nodes[idx].local_trans = [tx, ty, tz];
            self.dirty = true;
        }
    }

    pub fn set_bone_scale(&mut self, name: &str, sx: f32, sy: f32, sz: f32) {
        if let Some(&idx) = self.node_index.get(name) {
            self.nodes[idx].local_scale = [sx, sy, sz];
            self.dirty = true;
        }
    }

    /// 全ボーンの姿勢をバインド姿勢（初期状態）にリセットする
    pub fn reset_pose(&mut self) {
        for node in &mut self.nodes {
            node.local_trans = node.bind_trans;
            node.local_rot = node.bind_rot;
            node.local_scale = node.bind_scale;
        }
        self.dirty = true;
    }

    pub fn recompute_world(&mut self) {
        let off = Matrix4::new_translation(&nalgebra::Vector3::new(
            self.root_offset[0], self.root_offset[1], self.root_offset[2],
        ));
        for oi in 0..self.hierarchy_order.len() {
            let i = self.hierarchy_order[oi];
            let local = trs_to_mat4(self.nodes[i].local_trans, self.nodes[i].local_rot, self.nodes[i].local_scale);
            let world = match self.nodes[i].parent {
                Some(pi) if pi < self.nodes.len() => self.nodes[pi].world_mat * local,
                _ => off * local,
            };
            self.nodes[i].world_mat = world;
        }
    }

    pub fn build_draw_commands(&mut self, device: &Device) -> Vec<(Vec<Matrix4<f32>>, crate::renderer::SkinnedMeshCommand)> {
        if self.dirty {
            self.recompute_world();
            self.dirty = false;
        }

        let mut result = Vec::new();
        for prim in &mut self.primitives {
            // モーフウェイトはダミー
            let blend_weights_buf = match &prim.cached_weights {
                Some((_cached, buf)) => buf.clone(),
                None => {
                    let dummy_weights = [0.0f32; 256];
                    let new_buf = Arc::new(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glTF Blend Weights (dummy)"),
                        contents: bytemuck::cast_slice(&dummy_weights),
                        usage: wgpu::BufferUsages::UNIFORM,
                    }));
                    prim.cached_weights = Some((dummy_weights, new_buf.clone()));
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
                let world = self.nodes.get(node_idx).map(|b| b.world_mat).unwrap_or(Matrix4::identity());
                world * ibms[ji]
            }).collect();
            while matrices.len() < 256 {
                matrices.push(Matrix4::identity());
            }

            result.push((
                matrices,
                crate::renderer::SkinnedMeshCommand {
                    texture_id: prim.texture_id,
                    vertex_buffer: Arc::clone(&prim.vertex_buf),
                    index_buffer: Arc::clone(&prim.index_buf),
                    num_indices: prim.num_indices,
                    blend_weights_buffer: blend_weights_buf,
                    morph_delta_buffer: Arc::clone(&prim.morph_delta_buf),
                    num_morph_targets: prim.num_morph_targets,
                    mtoon_buffer: None,
                    shade_texture_id: None,
                    outline_width: 0.0,
                },
            ));
        }
        result
    }
}