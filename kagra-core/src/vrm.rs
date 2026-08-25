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
use crate::vrm_expression::{effective_expression_weights, meta_from_expr_json, ExpressionMeta};
use crate::vrm_constraint::{
    apply_roll, apply_rotation, has_aim, parse_node_constraints, ConstraintKind, NodeConstraint,
};
use crate::vrm_first_person::{
    collect_head_nodes, erase_head_triangles, parse_mesh_annotations, MeshAnnotation,
};
use crate::mtoon::{parse_mtoon, parse_vrm0_material_properties, MtoonMaterial};
use crate::vrm_spring::{
    has_sleeve_coverage, init_rest, parse_spring_bones, push_simple_chain, radius_to_axis,
    snap_to_world, step as step_springs, sleeve_follow, transfer_sleeve_weights,
    unbound_sleeve_nodes, used_spring_nodes, SpringBoneState, SLEEVE_DRAG, SLEEVE_GRAVITY,
    SLEEVE_HIT_RADIUS, SLEEVE_STIFFNESS, SLEEVE_TRANSFER, VIRTUAL_TAIL_LEN,
};

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
    pub fp_flag: MeshAnnotation,
    pub fp_index_buf: Option<Arc<wgpu::Buffer>>,
    pub fp_num_indices: u32,
    /// バインド姿勢。キーはスキンパレット添字（JOINTS_0）。
    pub bone_bind_aabbs: Vec<(u16, crate::frustum::Aabb)>,
    /// モーフ変位の最大長。カリングパッドに足す。
    pub morph_pad: f32,
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
    pub expression_meta: HashMap<String, ExpressionMeta>,
    /// VRM humanoid 標準名 → ノード index（hips, head, …）
    pub human_bones: HashMap<String, usize>,
    /// VRM LookAt メタ（VRM1 lookAt / VRM0 firstPerson）
    pub look_at: Option<VrmLookAtMeta>,
    pub constraints: Vec<NodeConstraint>,
    /// true のとき firstPerson 注釈に従い頭を消す。
    pub first_person: bool,
    pub spring: SpringBoneState,
}



// ── 表情パース（VRM 1.0 + VRM 0.x）────────────────────────────

fn insert_expression(
    expression_targets: &mut HashMap<String, Vec<MorphTarget>>,
    blend_index: &mut HashMap<String, usize>,
    expression_meta: &mut HashMap<String, ExpressionMeta>,
    name: String,
    targets: Vec<MorphTarget>,
    meta: ExpressionMeta,
) {
    if targets.is_empty() {
        return;
    }
    let first_index = targets.first().map(|t| t.index);
    expression_targets.insert(name.clone(), targets);
    expression_meta.insert(name.clone(), meta);
    if let Some(idx) = first_index {
        blend_index.insert(name, idx);
    }
}

fn parse_expressions(
    gltf: &serde_json::Value,
    mesh_to_node: &HashMap<usize, usize>,
) -> (
    HashMap<String, Vec<MorphTarget>>,
    HashMap<String, usize>,
    HashMap<String, ExpressionMeta>,
) {
    let mut expression_targets = HashMap::new();
    let mut blend_index = HashMap::new();
    let mut expression_meta = HashMap::new();

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
                            &mut expression_meta,
                            expr_name.to_string(),
                            targets,
                            meta_from_expr_json(expr_name, expr),
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
            insert_expression(
                &mut expression_targets,
                &mut blend_index,
                &mut expression_meta,
                key.clone(),
                targets,
                meta_from_expr_json(&key, group),
            );
        }
    }

    (expression_targets, blend_index, expression_meta)
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
    let (expression_targets, blend_index, expression_meta) = parse_expressions(&gltf, &mesh_to_node);

    // LookAt メタ
    let look_at = parse_look_at(&gltf);
    let constraints = parse_node_constraints(&gltf);
    let (fp_by_mesh, fp_by_node) = parse_mesh_annotations(&gltf);
    let head_nodes = collect_head_nodes(
        &bones.iter().map(|b| b.parent).collect::<Vec<_>>(),
        &human_bones,
    );

    let mut spring = parse_spring_bones(&gltf);
    let sleeve_remaps = ensure_sleeve_cloth(
        &mut bones,
        &mut bone_index,
        &mut skins,
        &human_bones,
        &mut spring,
    );

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

            // JOINTS_0 は skin.joints への添字。袖ヘルパーがあるスキンだけ外側ウェイトを移す。
            let vertices: Vec<SkinnedVertex> = (0..n).map(|i| {
                let u = uvs.get(i).map(|v| [v[0], v[1]]).unwrap_or([0.0;2]);
                let mut j = joints.get(i).copied().unwrap_or([0;4]);
                let mut w = weights.get(i).map(|v| [v[0], v[1], v[2], v[3]]).unwrap_or([1.0,0.0,0.0,0.0]);
                for remap in sleeve_remaps.iter().filter(|r| r.skin_idx == skin_idx) {
                    let rad = radius_to_axis(pos_xyz[i], remap.origin, remap.axis);
                    let follow = sleeve_follow(rad) * SLEEVE_TRANSFER;
                    let (nj, nw) = transfer_sleeve_weights(
                        j, w, remap.arm_palette, remap.helper_palette, follow,
                    );
                    j = nj;
                    w = nw;
                }
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
            let fp_flag = fp_by_mesh
                .get(&mi)
                .copied()
                .or_else(|| fp_by_node.get(&prim_node_idx).copied())
                .unwrap_or(MeshAnnotation::Auto);

            let mut fp_index_buf = None;
            let mut fp_num_indices = 0u32;
            if fp_flag == MeshAnnotation::Auto {
                let weight_rows: Vec<[f32; 4]> = (0..n)
                    .map(|i| {
                        weights
                            .get(i)
                            .map(|v| {
                                [
                                    *v.first().unwrap_or(&1.0),
                                    *v.get(1).unwrap_or(&0.0),
                                    *v.get(2).unwrap_or(&0.0),
                                    *v.get(3).unwrap_or(&0.0),
                                ]
                            })
                            .unwrap_or([1.0, 0.0, 0.0, 0.0])
                    })
                    .collect();
                let joint_rows: Vec<[u32; 4]> = (0..n)
                    .map(|i| joints.get(i).copied().unwrap_or([0; 4]))
                    .collect();
                let skin_joints = skins
                    .get(skin_idx)
                    .map(|s| s.joint_node_indices.as_slice())
                    .unwrap_or(&[]);
                let fp_idx = erase_head_triangles(
                    &indices,
                    &joint_rows,
                    &weight_rows,
                    skin_joints,
                    &head_nodes,
                    0.2,
                );
                fp_num_indices = fp_idx.len() as u32;
                if !fp_idx.is_empty() {
                    let fp_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("VRM FP IB"),
                        contents: bytemuck::cast_slice(&fp_idx),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    fp_index_buf = Some(Arc::new(fp_ib));
                }
            }

            let joint_rows: Vec<[u32; 4]> = vertices.iter().map(|v| v.joints).collect();
            let weight_rows: Vec<[f32; 4]> = vertices.iter().map(|v| v.weights).collect();
            let bone_bind_aabbs =
                crate::frustum::bone_bind_aabbs(&pos_xyz, &joint_rows, &weight_rows);
            let morph_pad = morph_deltas
                .chunks(3)
                .map(|d| {
                    if d.len() < 3 {
                        0.0
                    } else {
                        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
                    }
                })
                .fold(0.0f32, f32::max);

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
                fp_flag,
                fp_index_buf,
                fp_num_indices,
                bone_bind_aabbs,
                morph_pad,
            });
        }
    }

    let hierarchy_order = build_hierarchy_order(
        &bones.iter().map(|b| b.parent).collect::<Vec<_>>(),
    );
    if !spring.chains.is_empty() {
        let bind_mats = bind_world_mats(&bones, &hierarchy_order);
        init_rest(&mut spring, &bind_mats);
    }

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
        expression_meta,
        human_bones,
        look_at,
        constraints,
        first_person: false,
        spring,
    })
}

fn bind_world_mats(bones: &[VrmBone], order: &[usize]) -> Vec<Matrix4<f32>> {
    let mut mats = vec![Matrix4::identity(); bones.len()];
    for &i in order {
        let local = trs_to_mat4(bones[i].bind_trans, bones[i].bind_rot, bones[i].bind_scale);
        mats[i] = match bones[i].parent {
            Some(pi) if pi < bones.len() => mats[pi] * local,
            _ => local,
        };
    }
    mats
}

struct SleeveRemap {
    skin_idx: usize,
    arm_palette: u32,
    helper_palette: u32,
    origin: [f32; 3],
    axis: [f32; 3],
}

const ARM_LINKS: [(&str, &str, &str); 4] = [
    ("leftUpperArm", "leftLowerArm", "_kagraSleeveLU"),
    ("leftLowerArm", "leftHand", "_kagraSleeveLL"),
    ("rightUpperArm", "rightLowerArm", "_kagraSleeveRU"),
    ("rightLowerArm", "rightHand", "_kagraSleeveRL"),
];

fn bone_names_vec(len: usize, bone_index: &HashMap<String, usize>) -> Vec<String> {
    let mut names = vec![String::new(); len];
    for (n, &i) in bone_index {
        if i < names.len() && names[i].is_empty() {
            names[i] = n.clone();
        }
    }
    names
}

fn mat_world_pos(m: &Matrix4<f32>) -> [f32; 3] {
    [m[(0, 3)], m[(1, 3)], m[(2, 3)]]
}

fn vsub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn vlen3(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// 袖ボーンが無い VRM（Alicia のセーラー等）にヘルパーを足し、外側の筒ウェイトを移す。
fn ensure_sleeve_cloth(
    bones: &mut Vec<VrmBone>,
    bone_index: &mut HashMap<String, usize>,
    skins: &mut [VrmSkin],
    human_bones: &HashMap<String, usize>,
    spring: &mut SpringBoneState,
) -> Vec<SleeveRemap> {
    let names = bone_names_vec(bones.len(), bone_index);
    let arm_nodes: Vec<usize> = [
        "leftUpperArm",
        "leftLowerArm",
        "rightUpperArm",
        "rightLowerArm",
    ]
    .iter()
    .filter_map(|k| human_bones.get(*k).copied())
    .collect();

    let used = used_spring_nodes(spring);
    for node in unbound_sleeve_nodes(&names, &used) {
        push_simple_chain(
            spring,
            node,
            None,
            SLEEVE_STIFFNESS,
            SLEEVE_DRAG,
            SLEEVE_GRAVITY,
            SLEEVE_HIT_RADIUS,
            [0.0, 1.0, 0.0],
            VIRTUAL_TAIL_LEN,
        );
    }

    let names = bone_names_vec(bones.len(), bone_index);
    let parents: Vec<Option<usize>> = bones.iter().map(|b| b.parent).collect();
    if has_sleeve_coverage(spring, &names, &arm_nodes, &parents) || arm_nodes.is_empty() {
        return Vec::new();
    }

    let order = build_hierarchy_order(&parents);
    let bind = bind_world_mats(bones, &order);
    let mut remaps = Vec::new();

    for &(arm_key, next_key, helper_name) in &ARM_LINKS {
        let Some(&arm) = human_bones.get(arm_key) else { continue };
        let Some(&nxt) = human_bones.get(next_key) else { continue };
        if arm >= bones.len() || nxt >= bones.len() || bone_index.contains_key(helper_name) {
            continue;
        }
        let origin = mat_world_pos(&bind[arm]);
        let next_p = mat_world_pos(&bind[nxt]);
        let delta = vsub3(next_p, origin);
        let arm_len = vlen3(delta);
        if arm_len < 0.02 {
            continue;
        }
        let axis_w = [delta[0] / arm_len, delta[1] / arm_len, delta[2] / arm_len];
        let lt = bones[nxt].bind_trans;
        let llen = vlen3(lt).max(1e-8);
        let axis_local = [lt[0] / llen, lt[1] / llen, lt[2] / llen];
        let helper_trans = [
            axis_local[0] * arm_len * 0.45,
            axis_local[1] * arm_len * 0.45,
            axis_local[2] * arm_len * 0.45,
        ];

        let helper_idx = bones.len();
        bones.push(VrmBone {
            parent: Some(arm),
            local_rot: [0.0, 0.0, 0.0, 1.0],
            bind_rot: [0.0, 0.0, 0.0, 1.0],
            local_trans: helper_trans,
            bind_trans: helper_trans,
            local_scale: [1.0, 1.0, 1.0],
            bind_scale: [1.0, 1.0, 1.0],
            world_mat: Matrix4::identity(),
        });
        bone_index.insert(helper_name.to_string(), helper_idx);

        push_simple_chain(
            spring,
            helper_idx,
            None,
            SLEEVE_STIFFNESS,
            SLEEVE_DRAG,
            SLEEVE_GRAVITY,
            SLEEVE_HIT_RADIUS,
            axis_local,
            (arm_len * 0.40).max(0.05),
        );

        for (si, skin) in skins.iter_mut().enumerate() {
            if skin.joint_node_indices.len() >= 256 {
                continue;
            }
            let Some(arm_pal) = skin
                .joint_node_indices
                .iter()
                .position(|&n| n == arm)
            else {
                continue;
            };
            if skin.joint_node_indices.iter().any(|&n| n == helper_idx) {
                continue;
            }
            let helper_pal = skin.joint_node_indices.len() as u32;
            skin.joint_node_indices.push(helper_idx);
            remaps.push(SleeveRemap {
                skin_idx: si,
                arm_palette: arm_pal as u32,
                helper_palette: helper_pal,
                origin,
                axis: axis_w,
            });
        }
    }

    let parents: Vec<Option<usize>> = bones.iter().map(|b| b.parent).collect();
    let order = build_hierarchy_order(&parents);
    let bind = bind_world_mats(bones, &order);
    for skin in skins.iter_mut() {
        while skin.inv_bind_matrices.len() < skin.joint_node_indices.len() {
            let node = skin.joint_node_indices[skin.inv_bind_matrices.len()];
            let inv = bind
                .get(node)
                .and_then(|m| m.try_inverse())
                .unwrap_or_else(Matrix4::identity);
            skin.inv_bind_matrices.push(inv);
        }
    }
    remaps
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

    pub fn set_first_person(&mut self, enabled: bool) {
        self.first_person = enabled;
        self.dirty = true;
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

    fn apply_local_constraints(&mut self) {
        let n = self.bones.len();
        for i in 0..self.constraints.len() {
            let c = self.constraints[i];
            if c.dest >= n {
                continue;
            }
            match c.kind {
                ConstraintKind::Rotation { source, weight } if source < n => {
                    let src_local = self.bones[source].local_rot;
                    let src_rest = self.bones[source].bind_rot;
                    let dst_rest = self.bones[c.dest].bind_rot;
                    self.bones[c.dest].local_rot =
                        apply_rotation(src_local, src_rest, dst_rest, weight);
                }
                ConstraintKind::Roll {
                    source,
                    weight,
                    axis,
                } if source < n => {
                    let src_local = self.bones[source].local_rot;
                    let src_rest = self.bones[source].bind_rot;
                    let dst_rest = self.bones[c.dest].bind_rot;
                    self.bones[c.dest].local_rot =
                        apply_roll(src_local, src_rest, dst_rest, axis, weight);
                }
                _ => {}
            }
        }
    }

    fn apply_aim_constraints(&mut self) {
        let n = self.bones.len();
        for c in &self.constraints {
            let ConstraintKind::Aim {
                source,
                weight,
                aim_axis,
            } = c.kind
            else {
                continue;
            };
            if c.dest >= n || source >= n {
                continue;
            }
            let src_pos = {
                let w = &self.bones[source].world_mat;
                [w[(0, 3)], w[(1, 3)], w[(2, 3)]]
            };
            let dest_pos = {
                let w = &self.bones[c.dest].world_mat;
                [w[(0, 3)], w[(1, 3)], w[(2, 3)]]
            };
            let dir = [
                src_pos[0] - dest_pos[0],
                src_pos[1] - dest_pos[1],
                src_pos[2] - dest_pos[2],
            ];
            let rest = self.bones[c.dest].bind_rot;
            let from = crate::vrm_constraint::qrotate(rest, aim_axis);
            let rot = crate::vrm_constraint::q_from_to(from, dir);
            let target = crate::vrm_constraint::qmul(rot, rest);
            self.bones[c.dest].local_rot = crate::vrm_constraint::qslerp(rest, target, weight);
        }
    }

    fn fk_world(&mut self) {
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

    pub fn recompute_world(&mut self) {
        self.apply_local_constraints();
        self.fk_world();
        if has_aim(&self.constraints) {
            self.apply_aim_constraints();
            self.fk_world();
        }
    }

    pub fn step_spring(&mut self, dt: f32) {
        if !self.spring.enabled || self.spring.chains.is_empty() {
            return;
        }
        self.recompute_world();
        let mats: Vec<Matrix4<f32>> = self.bones.iter().map(|b| b.world_mat).collect();
        let parents: Vec<Option<usize>> = self.bones.iter().map(|b| b.parent).collect();
        let updates = step_springs(&mut self.spring, &mats, &parents, dt);
        for (idx, rot) in updates {
            if idx < self.bones.len() {
                self.bones[idx].local_rot = rot;
            }
        }
        self.dirty = true;
    }

    pub fn reset_spring(&mut self) {
        self.recompute_world();
        let mats: Vec<Matrix4<f32>> = self.bones.iter().map(|b| b.world_mat).collect();
        snap_to_world(&mut self.spring, &mats);
        self.spring.initialized = true;
    }

    pub fn set_spring_wind(&mut self, x: f32, y: f32, z: f32) {
        self.spring.wind = [x, y, z];
    }

    pub fn set_spring_enabled(&mut self, enabled: bool) {
        self.spring.enabled = enabled;
    }

    pub fn build_draw_commands(&mut self, device: &Device) -> Vec<(Vec<Matrix4<f32>>, crate::renderer::SkinnedMeshCommand)> {
        if self.dirty {
            self.recompute_world();
            self.dirty = false;
        }

        let effective = effective_expression_weights(&self.active_expressions, &self.expression_meta);
        let mut result = Vec::new();
        for prim in &mut self.primitives {
            match (self.first_person, prim.fp_flag) {
                (true, MeshAnnotation::ThirdPersonOnly) => continue,
                (false, MeshAnnotation::FirstPersonOnly) => continue,
                (true, MeshAnnotation::Auto) if prim.fp_num_indices == 0 => continue,
                _ => {}
            }
            let (index_buffer, num_indices) = if self.first_person
                && prim.fp_flag == MeshAnnotation::Auto
                && prim.fp_index_buf.is_some()
            {
                (prim.fp_index_buf.as_ref().unwrap().clone(), prim.fp_num_indices)
            } else {
                (Arc::clone(&prim.index_buf), prim.num_indices)
            };

            let mut current_weights = [0.0f32; 256];
            for (expr_name, &user_w) in &effective {
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

            let aabb = crate::frustum::skinned_aabb(
                &prim.bone_bind_aabbs,
                &matrices,
                prim.morph_pad,
            );

            result.push((
                matrices,
                crate::renderer::SkinnedMeshCommand {
                    texture_id: prim.texture_id,
                    shade_texture_id: prim.mtoon.shade_texture_id,
                    matcap_texture_id: prim.mtoon.matcap_texture_id,
                    normal_texture_id: prim.mtoon.normal_texture_id,
                    uv_mask_texture_id: prim.mtoon.uv_mask_texture_id,
                    mtoon_buffer: Some(Arc::clone(&prim.mtoon.buffer)),
                    outline_width: prim.mtoon.gpu.params[2],
                    vertex_buffer: Arc::clone(&prim.vertex_buf),
                    index_buffer,
                    num_indices,
                    blend_weights_buffer: blend_weights_buf,
                    morph_delta_buffer: Arc::clone(&prim.morph_delta_buf),
                    num_morph_targets: prim.num_morph_targets,
                    skin_slot: None,
                    aabb,
                    double_sided: prim.mtoon.double_sided,
                },
            ));
        }
        result
    }
}