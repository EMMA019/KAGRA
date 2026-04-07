// kagra-core/src/vrm.rs
//
// VRM (glTF Binary) ローダー + GPU スキニング
//
// 設計:
//   - glTF バイナリを Rust でパースし、GPU バッファを構築
//   - 既存の skinning_pipeline / update_skin_uniforms を活用
//   - ボーン数が64を超える場合は skin ごとに分割描画
//   - Python API: load_vrm / draw_vrm / set_vrm_bone_rot / update_vrm_pose

use std::collections::HashMap;
use std::sync::Arc;
use nalgebra::Matrix4;
use wgpu::util::DeviceExt;

use crate::renderer::{SkinnedVertex, SkinnedMeshCommand};

// ── データ構造 ────────────────────────────────────────────────

/// 1プリミティブ（描画単位）
pub struct VrmPrimitive {
    pub texture_id:  u32,
    pub vertex_buf:  Arc<wgpu::Buffer>,
    pub index_buf:   Arc<wgpu::Buffer>,
    pub num_indices: u32,
    pub skin_idx:    usize,         // どのスキンを使うか
    pub joint_remap: Vec<usize>,    // local joint index → global bone index
}

/// 1ボーン
pub struct VrmBone {
    pub name:        String,
    pub parent:      Option<usize>,
    /// バインドポーズの local TRS（列優先4x4）
    pub bind_local:  Matrix4<f32>,
    /// 現在のローカル回転（クォータニオン xyzw）
    pub local_rot:   [f32; 4],
    /// バインドポーズの回転（リセット用）
    pub bind_rot:    [f32; 4],
    /// 現在のローカル並進（動的に変更可能）
    pub local_trans: [f32; 3],
    /// バインドポーズの並進（リセット用）
    pub bind_trans:  [f32; 3],
    /// 現在のローカルスケール
    pub local_scale: [f32; 3],
    /// バインドポーズのスケール（リセット用）
    pub bind_scale:  [f32; 3],
    /// ワールド行列（毎フレーム再計算）
    pub world_mat:   Matrix4<f32>,
}

/// スキン情報
pub struct VrmSkin {
    pub joint_node_indices: Vec<usize>,        // joint ごとの node index
    pub inv_bind_matrices:  Vec<Matrix4<f32>>, // joint ごとの逆バインド行列
}

/// VRM モデル全体
pub struct VrmModel {
    pub bones:       Vec<VrmBone>,
    pub bone_index:  HashMap<String, usize>,
    pub skins:       Vec<VrmSkin>,
    pub primitives:  Vec<VrmPrimitive>,
    pub dirty:       bool,
    pub root_offset: [f32; 3],
}

// ── glTF バイナリパーサー ────────────────────────────────────

fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]])
}

fn read_f32_le(data: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]])
}

fn parse_accessor_f32(
    gltf:      &serde_json::Value,
    bin_data:  &[u8],
    acc_idx:   usize,
    components: usize,
) -> Vec<Vec<f32>> {
    let acc      = &gltf["accessors"][acc_idx];
    let bv_idx   = acc["bufferView"].as_u64().unwrap_or(0) as usize;
    let bv       = &gltf["bufferViews"][bv_idx];
    let bv_off   = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
    let acc_off  = acc["byteOffset"].as_u64().unwrap_or(0) as usize;
    let count    = acc["count"].as_u64().unwrap_or(0) as usize;
    let stride   = bv["byteStride"].as_u64().unwrap_or((components * 4) as u64) as usize;
    let off      = bv_off + acc_off;
    let comp_type = acc["componentType"].as_u64().unwrap_or(5126);

    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let mut row = Vec::with_capacity(components);
        for j in 0..components {
            let byte_off = off + i * stride + j * 4;
            if byte_off + 4 > bin_data.len() {
                row.push(0.0);
                continue;
            }
            let val = match comp_type {
                5126 => read_f32_le(bin_data, byte_off),
                5123 => read_u32_le(bin_data, byte_off) as f32 / 65535.0, // USHORT
                5121 => bin_data[byte_off] as f32 / 255.0,                 // UBYTE
                _ => read_f32_le(bin_data, byte_off),
            };
            row.push(val);
        }
        result.push(row);
    }
    result
}

fn parse_accessor_u32(
    gltf:     &serde_json::Value,
    bin_data: &[u8],
    acc_idx:  usize,
) -> Vec<u32> {
    let acc      = &gltf["accessors"][acc_idx];
    let bv_idx   = acc["bufferView"].as_u64().unwrap_or(0) as usize;
    let bv       = &gltf["bufferViews"][bv_idx];
    let bv_off   = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
    let acc_off  = acc["byteOffset"].as_u64().unwrap_or(0) as usize;
    let count    = acc["count"].as_u64().unwrap_or(0) as usize;
    let comp_type = acc["componentType"].as_u64().unwrap_or(5125);
    let byte_size = match comp_type { 5123 => 2, _ => 4 };
    let stride   = bv["byteStride"].as_u64().unwrap_or(byte_size as u64) as usize;
    let off      = bv_off + acc_off;

    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let byte_off = off + i * stride;
        if byte_off + byte_size > bin_data.len() {
            result.push(0);
            continue;
        }
        let val = match comp_type {
            5123 => u16::from_le_bytes([bin_data[byte_off], bin_data[byte_off+1]]) as u32,
            5121 => bin_data[byte_off] as u32,
            _    => read_u32_le(bin_data, byte_off),
        };
        result.push(val);
    }
    result
}

fn parse_accessor_u8x4(
    gltf:     &serde_json::Value,
    bin_data: &[u8],
    acc_idx:  usize,
) -> Vec<[u32; 4]> {
    let acc      = &gltf["accessors"][acc_idx];
    let bv_idx   = acc["bufferView"].as_u64().unwrap_or(0) as usize;
    let bv       = &gltf["bufferViews"][bv_idx];
    let bv_off   = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
    let acc_off  = acc["byteOffset"].as_u64().unwrap_or(0) as usize;
    let count    = acc["count"].as_u64().unwrap_or(0) as usize;
    let comp_type = acc["componentType"].as_u64().unwrap_or(5121);
    let comp_size = match comp_type { 5123 => 2usize, _ => 1 };
    let stride   = bv["byteStride"].as_u64().unwrap_or((4 * comp_size) as u64) as usize;
    let off      = bv_off + acc_off;

    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let mut row = [0u32; 4];
        for j in 0..4 {
            let byte_off = off + i * stride + j * comp_size;
            if byte_off + comp_size > bin_data.len() { continue; }
            row[j] = match comp_type {
                5123 => u16::from_le_bytes([bin_data[byte_off], bin_data[byte_off+1]]) as u32,
                _ => bin_data[byte_off] as u32,
            };
        }
        result.push(row);
    }
    result
}

fn parse_mat4(arr: &[f32]) -> Matrix4<f32> {
    if arr.len() < 16 {
        return Matrix4::identity();
    }
    // glTF は列優先
    Matrix4::new(
        arr[0], arr[4], arr[8],  arr[12],
        arr[1], arr[5], arr[9],  arr[13],
        arr[2], arr[6], arr[10], arr[14],
        arr[3], arr[7], arr[11], arr[15],
    )
}

fn trs_to_mat4(t: [f32;3], r: [f32;4], s: [f32;3]) -> Matrix4<f32> {
    // クォータニオン → 回転行列
    let [rx, ry, rz, rw] = r;
    let x2 = rx*2.0; let y2 = ry*2.0; let z2 = rz*2.0;
    let xx = rx*x2; let yy = ry*y2; let zz = rz*z2;
    let xy = rx*y2; let xz = rx*z2; let yz = ry*z2;
    let wx = rw*x2; let wy = rw*y2; let wz = rw*z2;
    let [sx, sy, sz] = s;
    Matrix4::new(
        (1.0-(yy+zz))*sx, (xy-wz)*sy,       (xz+wy)*sz,       t[0],
        (xy+wz)*sx,       (1.0-(xx+zz))*sy, (yz-wx)*sz,       t[1],
        (xz-wy)*sx,       (yz+wx)*sy,       (1.0-(xx+yy))*sz, t[2],
        0.0,              0.0,              0.0,              1.0,
    )
}

// ── ロード ────────────────────────────────────────────────────

/// テクスチャバイト列だけを抽出する（renderer 不要）
pub fn extract_texture_data(path: &str)
    -> Result<Vec<(usize, Vec<u8>, String)>, String>
{
    let data = std::fs::read(path)
        .map_err(|e| format!("VRM 読み込み失敗: {} ({})", path, e))?;

    if &data[0..4] != b"glTF" {
        return Err(format!("glTF ではありません: {}", path));
    }

    let mut offset = 12usize;
    let mut json_bytes: Option<&[u8]> = None;
    let mut bin_data: &[u8] = &[];
    while offset + 8 <= data.len() {
        let chunk_len  = read_u32_le(&data, offset) as usize;
        let chunk_type = read_u32_le(&data, offset + 4);
        let chunk_data = &data[offset + 8 .. (offset + 8 + chunk_len).min(data.len())];
        match chunk_type {
            0x4E4F534A => json_bytes = Some(chunk_data),
            0x004E4942 => bin_data   = chunk_data,
            _ => {}
        }
        offset += 8 + chunk_len;
    }

    let json_bytes = json_bytes.ok_or("JSON チャンクが見つかりません")?;
    let json_str   = std::str::from_utf8(json_bytes)
        .map_err(|e| format!("UTF-8: {}", e))?
        .trim_end_matches('\0');
    let gltf: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("JSON: {}", e))?;

    let images    = gltf["images"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    let textures  = gltf["textures"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    let bvs       = gltf["bufferViews"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);

    let mut result = Vec::new();
    for (ti, tex) in textures.iter().enumerate() {
        let src_idx = tex["source"].as_u64().unwrap_or(0) as usize;
        if src_idx >= images.len() { continue; }
        let img    = &images[src_idx];
        let bv_idx = match img["bufferView"].as_u64() { Some(v) => v as usize, None => continue };
        if bv_idx >= bvs.len() { continue; }
        let bv   = &bvs[bv_idx];
        let off  = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
        let size = bv["byteLength"].as_u64().unwrap_or(0) as usize;
        if size < 16 || off + size > bin_data.len() { continue; }
        let mime = img["mimeType"].as_str().unwrap_or("image/png");
        let ext  = if mime.contains("jpeg") { "jpg".to_string() } else { "png".to_string() };
        result.push((ti, bin_data[off..off+size].to_vec(), ext));
    }
    Ok(result)
}

/// VRM を読み込む（テクスチャ ID マップは呼び出し側が用意する）
pub fn load_vrm(
    path:       &str,
    device:     &wgpu::Device,
    tex_id_map: &std::collections::HashMap<usize, u32>,
) -> Result<VrmModel, String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("VRM 読み込み失敗: {} ({})", path, e))?;

    // glTF バイナリヘッダー
    if &data[0..4] != b"glTF" {
        return Err(format!("glTF ではありません: {}", path));
    }

    // チャンク解析
    let mut offset = 12usize;
    let mut json_bytes: Option<&[u8]> = None;
    let mut bin_data:   &[u8] = &[];

    while offset + 8 <= data.len() {
        let chunk_len  = read_u32_le(&data, offset) as usize;
        let chunk_type = read_u32_le(&data, offset + 4);
        let chunk_data = &data[offset + 8 .. (offset + 8 + chunk_len).min(data.len())];
        match chunk_type {
            0x4E4F534A => json_bytes = Some(chunk_data), // JSON
            0x004E4942 => bin_data   = chunk_data,        // BIN
            _ => {}
        }
        offset += 8 + chunk_len;
    }

    let json_bytes = json_bytes.ok_or("JSON チャンクが見つかりません")?;
    let json_str   = std::str::from_utf8(json_bytes)
        .map_err(|e| format!("JSON UTF-8 エラー: {}", e))?
        .trim_end_matches('\0');
    let gltf: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("JSON パース失敗: {}", e))?;

    let nodes     = gltf["nodes"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    let skins_arr = gltf["skins"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    let meshes    = gltf["meshes"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    let materials = gltf["materials"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    let textures  = gltf["textures"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    let images    = gltf["images"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
    let bvs       = gltf["bufferViews"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);

    // ── ボーン構築 ──────────────────────────────────────────
    let mut bones: Vec<VrmBone> = Vec::new();
    let mut bone_index: HashMap<String, usize> = HashMap::new();

    for (ni, node) in nodes.iter().enumerate() {
        let name = node["name"].as_str().unwrap_or("").to_string();
        let t = if let Some(arr) = node["translation"].as_array() {
            [arr[0].as_f64().unwrap_or(0.0) as f32,
             arr[1].as_f64().unwrap_or(0.0) as f32,
             arr[2].as_f64().unwrap_or(0.0) as f32]
        } else { [0.0,0.0,0.0] };
        let r = if let Some(arr) = node["rotation"].as_array() {
            [arr[0].as_f64().unwrap_or(0.0) as f32,
             arr[1].as_f64().unwrap_or(0.0) as f32,
             arr[2].as_f64().unwrap_or(0.0) as f32,
             arr[3].as_f64().unwrap_or(1.0) as f32]
        } else { [0.0,0.0,0.0,1.0] };
        let s = if let Some(arr) = node["scale"].as_array() {
            [arr[0].as_f64().unwrap_or(1.0) as f32,
             arr[1].as_f64().unwrap_or(1.0) as f32,
             arr[2].as_f64().unwrap_or(1.0) as f32]
        } else { [1.0,1.0,1.0] };

        let local_mat = trs_to_mat4(t, r, s);
        if !name.is_empty() {
            bone_index.insert(name.clone(), ni);
        }
        bones.push(VrmBone {
            name,
            parent: None,
            bind_local: local_mat,
            local_rot:   r,
            bind_rot:    r,
            local_trans: t,
            bind_trans:  t,
            local_scale: s,
            bind_scale:  s,
            world_mat: Matrix4::identity(),
        });
    }

    // 親子関係を設定
    for (ni, node) in nodes.iter().enumerate() {
        if let Some(children) = node["children"].as_array() {
            for child in children {
                let ci = child.as_u64().unwrap_or(0) as usize;
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
            .map(|a| a.iter().map(|v| v.as_u64().unwrap_or(0) as usize).collect())
            .unwrap_or_default();
        let ibm_acc = skin["inverseBindMatrices"].as_u64().map(|v| v as usize);
        let inv_bind = if let Some(acc_idx) = ibm_acc {
            let flat = parse_accessor_f32(&gltf, bin_data, acc_idx, 16);
            flat.iter().map(|m| parse_mat4(m)).collect()
        } else {
            vec![Matrix4::identity(); joints.len()]
        };
        skins.push(VrmSkin { joint_node_indices: joints, inv_bind_matrices: inv_bind });
    }

    // テクスチャ ID マップは呼び出し側から受け取る

    // ── mesh → skin のマッピング ────────────────────────────
    let mut mesh_to_skin: HashMap<usize, usize> = HashMap::new();
    for node in nodes {
        if let (Some(mi), Some(si)) = (
            node["mesh"].as_u64().map(|v| v as usize),
            node["skin"].as_u64().map(|v| v as usize),
        ) {
            mesh_to_skin.insert(mi, si);
        }
    }

    // ── プリミティブ GPU バッファ構築 ───────────────────────
    let mut primitives: Vec<VrmPrimitive> = Vec::new();

    for (mi, mesh) in meshes.iter().enumerate() {
        let skin_idx = mesh_to_skin.get(&mi).copied().unwrap_or(0);
        let skin     = skins.get(skin_idx);

        for prim in mesh["primitives"].as_array().unwrap_or(&vec![]) {
            let attrs    = &prim["attributes"];
            let mat_idx  = prim["material"].as_u64().map(|v| v as usize);
            let idx_acc  = prim["indices"].as_u64().map(|v| v as usize);

            // テクスチャ ID
            let kagra_tex_id = mat_idx
                .and_then(|mi| materials.get(mi))
                .and_then(|mat| {
                    mat["pbrMetallicRoughness"]["baseColorTexture"]["index"]
                        .as_u64()
                        .map(|ti| tex_id_map.get(&(ti as usize)).copied().unwrap_or(0))
                })
                .unwrap_or(0);

            // Accessor 読み込み
            let pos_acc  = attrs["POSITION"].as_u64().map(|v| v as usize);
            let nrm_acc  = attrs["NORMAL"].as_u64().map(|v| v as usize);
            let uv_acc   = attrs["TEXCOORD_0"].as_u64().map(|v| v as usize);
            let jnt_acc  = attrs["JOINTS_0"].as_u64().map(|v| v as usize);
            let wgt_acc  = attrs["WEIGHTS_0"].as_u64().map(|v| v as usize);

            let positions = pos_acc.map(|a| parse_accessor_f32(&gltf, bin_data, a, 3))
                                   .unwrap_or_default();
            let _normals  = nrm_acc.map(|a| parse_accessor_f32(&gltf, bin_data, a, 3))
                                   .unwrap_or_default();
            let uvs       = uv_acc.map(|a| parse_accessor_f32(&gltf, bin_data, a, 2))
                                  .unwrap_or_default();
            let joints    = jnt_acc.map(|a| parse_accessor_u8x4(&gltf, bin_data, a))
                                   .unwrap_or_default();
            let weights   = wgt_acc.map(|a| parse_accessor_f32(&gltf, bin_data, a, 4))
                                   .unwrap_or_default();

            if positions.is_empty() { continue; }
            let n = positions.len();

            // joint remap（skin のジョイントリスト → global bone index）
            let joint_remap: Vec<usize> = skin
                .map(|s| s.joint_node_indices.clone())
                .unwrap_or_default();

            // SkinnedVertex 構築
            let vertices: Vec<SkinnedVertex> = (0..n).map(|i| {
                let p = positions.get(i).map(|v| [v[0], v[1], v[2]]).unwrap_or([0.0;3]);
                let u = uvs.get(i).map(|v| [v[0], v[1]]).unwrap_or([0.0;2]);
                let j = joints.get(i).copied().unwrap_or([0;4]);
                let w = weights.get(i).map(|v| {
                    let arr: [f32;4] = [v[0], v[1], v[2], v[3]];
                    arr
                }).unwrap_or([1.0, 0.0, 0.0, 0.0]);
                SkinnedVertex { position: p, uv: u, joints: j, weights: w }
            }).collect();

            // インデックス
            let indices: Vec<u32> = idx_acc
                .map(|a| parse_accessor_u32(&gltf, bin_data, a))
                .unwrap_or_default();

            if vertices.is_empty() || indices.is_empty() { continue; }

            let vb = Arc::new(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label:    Some("VRM VB"),
                contents: bytemuck::cast_slice(&vertices),
                usage:    wgpu::BufferUsages::VERTEX,
            }));
            let ib = Arc::new(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label:    Some("VRM IB"),
                contents: bytemuck::cast_slice(&indices),
                usage:    wgpu::BufferUsages::INDEX,
            }));

            primitives.push(VrmPrimitive {
                texture_id:  kagra_tex_id,
                vertex_buf:  vb,
                index_buf:   ib,
                num_indices: indices.len() as u32,
                skin_idx,
                joint_remap,
            });
        }
    }

    // 初期ポーズ計算
    let mut model = VrmModel { bones, bone_index, skins, primitives, dirty: true, root_offset: [0.0, 0.0, 0.0] };
    model.recompute_world();
    model.dirty = false;

    log::info!(
        "VrmModel loaded: {} bones, {} skins, {} primitives",
        model.bones.len(), model.skins.len(), model.primitives.len()
    );
    Ok(model)
}

// ── ポーズ制御 ────────────────────────────────────────────────

impl VrmModel {
    /// ワールド行列を再計算する（dirty=true のときだけ呼ぶ）
    pub fn recompute_world(&mut self) {
        for i in 0..self.bones.len() {
            // 動的な並進・回転・スケールを使用
            let local = trs_to_mat4(
                self.bones[i].local_trans,
                self.bones[i].local_rot,
                self.bones[i].local_scale,
            );
            let world = if let Some(pi) = self.bones[i].parent {
                self.bones[pi].world_mat * local
            } else {
                let off = Matrix4::new_translation(&nalgebra::Vector3::new(
                    self.root_offset[0], self.root_offset[1], self.root_offset[2],
                ));
                off * local
            };
            self.bones[i].world_mat = world;
        }
    }

    /// ボーンをクォータニオンで回転させる
    pub fn set_bone_rot_quat(&mut self, name: &str, qx: f32, qy: f32, qz: f32, qw: f32) {
        if let Some(&idx) = self.bone_index.get(name) {
            self.bones[idx].local_rot = [qx, qy, qz, qw];
            self.dirty = true;
        }
    }

    /// 全ボーンをバインドポーズに戻す
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
            bone.local_rot   = bone.bind_rot;
            bone.local_trans = bone.bind_trans;
            bone.local_scale = bone.bind_scale;
        }
        self.dirty = true;
    }

    /// スキン行列を計算して SkinnedMeshCommand のリストを返す
    /// （skinning_pipeline 用: 64ボーン以内のバッチに分割）
    pub fn build_draw_commands(&mut self) -> Vec<(Vec<nalgebra::Matrix4<f32>>, SkinnedMeshCommand)> {
        if self.dirty {
            self.recompute_world();
            self.dirty = false;
        }

        let mut result = Vec::new();

        for prim in &self.primitives {
            let skin = match self.skins.get(prim.skin_idx) {
                Some(s) => s,
                None => continue,
            };

            // スキニング行列を計算（world * inv_bind）
            // 64本ずつに分割
            let joints = &skin.joint_node_indices;
            let ibms   = &skin.inv_bind_matrices;
            let n      = joints.len().min(ibms.len());

            let mut matrices: Vec<Matrix4<f32>> = (0..n.min(256)).map(|ji| {
                let node_idx = joints[ji];
                let world = self.bones.get(node_idx)
                    .map(|b| b.world_mat)
                    .unwrap_or(Matrix4::identity());
                world * ibms[ji]
            }).collect();

            // 256本に満たない場合は identity で埋める
            while matrices.len() < 256 {
                matrices.push(Matrix4::identity());
            }

            result.push((
                matrices,
                SkinnedMeshCommand {
                    texture_id:        prim.texture_id,
                    vertex_buffer:     Arc::clone(&prim.vertex_buf),
                    index_buffer:      Arc::clone(&prim.index_buf),
                    num_indices:       prim.num_indices,
                    morph_bind_group:  None,
                    morph_weights:     [0.0f32; 8],
                },
            ));
        }

        result
    }
}
