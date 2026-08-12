// src/gltf_common.rs
// glTF / VRM 共通パーサー関数群 (アクセサ, 行列変換, テクスチャ抽出など)

use std::fs;
use nalgebra::Matrix4;
use serde_json::Value;

use crate::error::{KaguraError, KaguraResult};

// ----- バイナリ読み込み補助 -----
pub fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]])
}

pub fn read_f32_le(data: &[u8], offset: usize) -> f32 {
    f32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]])
}

// ----- アクセサパーサ -----
pub fn parse_accessor_f32(
    gltf: &Value,
    bin_data: &[u8],
    acc_idx: usize,
    components: usize,
) -> KaguraResult<Vec<Vec<f32>>> {
    let acc = gltf["accessors"].get(acc_idx)
        .ok_or_else(|| KaguraError::VrmParse(format!("accessor {} not found", acc_idx)))?;
    let bv_idx = acc["bufferView"].as_u64()
        .ok_or_else(|| KaguraError::VrmParse("bufferView missing".to_string()))? as usize;
    let bv = gltf["bufferViews"].get(bv_idx)
        .ok_or_else(|| KaguraError::VrmParse(format!("bufferView {} not found", bv_idx)))?;
    let bv_off = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
    let acc_off = acc["byteOffset"].as_u64().unwrap_or(0) as usize;
    let count = acc["count"].as_u64()
        .ok_or_else(|| KaguraError::VrmParse("count missing".to_string()))? as usize;
    let stride = bv["byteStride"].as_u64().unwrap_or((components * 4) as u64) as usize;
    let off = bv_off + acc_off;
    let comp_type = acc["componentType"].as_u64().unwrap_or(5126); // 5126 = FLOAT

    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let mut row = Vec::with_capacity(components);
        for j in 0..components {
            let byte_off = off + i * stride + j * 4;
            if byte_off + 4 > bin_data.len() {
                return Err(KaguraError::VrmParse("accessor out of bounds".to_string()));
            }
            let val = match comp_type {
                5126 => read_f32_le(bin_data, byte_off),
                5123 => read_u32_le(bin_data, byte_off) as f32 / 65535.0,
                5121 => bin_data[byte_off] as f32 / 255.0,
                _ => read_f32_le(bin_data, byte_off),
            };
            row.push(val);
        }
        result.push(row);
    }
    Ok(result)
}

pub fn parse_accessor_u32(
    gltf: &Value,
    bin_data: &[u8],
    acc_idx: usize,
) -> KaguraResult<Vec<u32>> {
    let acc = gltf["accessors"].get(acc_idx)
        .ok_or_else(|| KaguraError::VrmParse(format!("accessor {} not found", acc_idx)))?;
    let bv_idx = acc["bufferView"].as_u64()
        .ok_or_else(|| KaguraError::VrmParse("bufferView missing".to_string()))? as usize;
    let bv = gltf["bufferViews"].get(bv_idx)
        .ok_or_else(|| KaguraError::VrmParse(format!("bufferView {} not found", bv_idx)))?;
    let bv_off = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
    let acc_off = acc["byteOffset"].as_u64().unwrap_or(0) as usize;
    let count = acc["count"].as_u64()
        .ok_or_else(|| KaguraError::VrmParse("count missing".to_string()))? as usize;
    let comp_type = acc["componentType"].as_u64().unwrap_or(5125); // 5125 = UNSIGNED_INT
    let byte_size = match comp_type { 5123 => 2, _ => 4 };
    let stride = bv["byteStride"].as_u64().unwrap_or(byte_size as u64) as usize;
    let off = bv_off + acc_off;

    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let byte_off = off + i * stride;
        if byte_off + byte_size > bin_data.len() {
            return Err(KaguraError::VrmParse("index accessor out of bounds".to_string()));
        }
        let val = match comp_type {
            5123 => u16::from_le_bytes([bin_data[byte_off], bin_data[byte_off+1]]) as u32,
            5121 => bin_data[byte_off] as u32,
            _    => read_u32_le(bin_data, byte_off),
        };
        result.push(val);
    }
    Ok(result)
}

pub fn parse_accessor_u8x4(
    gltf: &Value,
    bin_data: &[u8],
    acc_idx: usize,
) -> KaguraResult<Vec<[u32; 4]>> {
    let acc = gltf["accessors"].get(acc_idx)
        .ok_or_else(|| KaguraError::VrmParse(format!("accessor {} not found", acc_idx)))?;
    let bv_idx = acc["bufferView"].as_u64()
        .ok_or_else(|| KaguraError::VrmParse("bufferView missing".to_string()))? as usize;
    let bv = gltf["bufferViews"].get(bv_idx)
        .ok_or_else(|| KaguraError::VrmParse(format!("bufferView {} not found", bv_idx)))?;
    let bv_off = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
    let acc_off = acc["byteOffset"].as_u64().unwrap_or(0) as usize;
    let count = acc["count"].as_u64()
        .ok_or_else(|| KaguraError::VrmParse("count missing".to_string()))? as usize;
    let comp_type = acc["componentType"].as_u64().unwrap_or(5121); // 5121 = UNSIGNED_BYTE
    let comp_size = match comp_type { 5123 => 2, _ => 1 };
    let stride = bv["byteStride"].as_u64().unwrap_or((4 * comp_size) as u64) as usize;
    let off = bv_off + acc_off;

    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let mut row = [0u32; 4];
        for j in 0..4 {
            let byte_off = off + i * stride + j * comp_size;
            if byte_off + comp_size > bin_data.len() {
                return Err(KaguraError::VrmParse("joints accessor out of bounds".to_string()));
            }
            row[j] = match comp_type {
                5123 => u16::from_le_bytes([bin_data[byte_off], bin_data[byte_off+1]]) as u32,
                _ => bin_data[byte_off] as u32,
            };
        }
        result.push(row);
    }
    Ok(result)
}

// ----- 法線 -----
/// インデックス三角形から頂点法線を面法線の平均として求める。
///
/// glTF の NORMAL アクセサを持たないメッシュ用のフォールバック。
/// 縮退三角形は外積が長さ 0 になるため自然に寄与しない。
pub fn compute_smooth_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0f32; 3]; positions.len()];
    for tri in indices.chunks_exact(3) {
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if i0 >= positions.len() || i1 >= positions.len() || i2 >= positions.len() {
            continue;
        }
        let (p0, p1, p2) = (positions[i0], positions[i1], positions[i2]);
        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        // 面積で重み付けしたいので、外積は正規化せずそのまま加算する
        let fnrm = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        for &i in &[i0, i1, i2] {
            normals[i][0] += fnrm[0];
            normals[i][1] += fnrm[1];
            normals[i][2] += fnrm[2];
        }
    }
    for nrm in normals.iter_mut() {
        let len_sq = nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2];
        if len_sq > 1e-16 {
            let inv = 1.0 / len_sq.sqrt();
            nrm[0] *= inv;
            nrm[1] *= inv;
            nrm[2] *= inv;
        } else {
            *nrm = [0.0, 1.0, 0.0];
        }
    }
    normals
}

/// NORMAL アクセサの行があればそれを使い、無ければ形状から計算する。
pub fn resolve_normals(
    normal_rows: Option<&Vec<Vec<f32>>>,
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Vec<[f32; 3]> {
    if let Some(rows) = normal_rows {
        if rows.len() >= positions.len() {
            return (0..positions.len())
                .map(|i| {
                    let r = &rows[i];
                    if r.len() >= 3 { [r[0], r[1], r[2]] } else { [0.0, 1.0, 0.0] }
                })
                .collect();
        }
    }
    compute_smooth_normals(positions, indices)
}

// ----- 階層走査順 -----
/// 親が必ず子より先に来るノード走査順を返す。
///
/// glTF はノード配列の並び順を規定しておらず、VRoid 製 VRM は葉から先に並ぶ。
/// 昇順ループでワールド行列を解くと親の値が 1 フレーム古いまま使われるため、
/// ロード時にこの順序を作って使う。循環参照があっても全ノードを必ず含める。
pub fn build_hierarchy_order(parents: &[Option<usize>]) -> Vec<usize> {
    let n = parents.len();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut stack: Vec<usize> = Vec::new();

    for i in 0..n {
        match parents[i] {
            Some(p) if p < n && p != i => children[p].push(i),
            _ => stack.push(i),
        }
    }
    stack.reverse();

    let mut order = Vec::with_capacity(n);
    let mut visited = vec![false; n];
    while let Some(i) = stack.pop() {
        if visited[i] {
            continue;
        }
        visited[i] = true;
        order.push(i);
        for &c in children[i].iter().rev() {
            if !visited[c] {
                stack.push(c);
            }
        }
    }
    for i in 0..n {
        if !visited[i] {
            order.push(i);
        }
    }
    order
}

// ----- 行列・変換ユーティリティ -----
pub fn parse_mat4(arr: &[f32]) -> Matrix4<f32> {
    if arr.len() < 16 {
        return Matrix4::identity();
    }
    Matrix4::new(
        arr[0], arr[4], arr[8],  arr[12],
        arr[1], arr[5], arr[9],  arr[13],
        arr[2], arr[6], arr[10], arr[14],
        arr[3], arr[7], arr[11], arr[15],
    )
}

pub fn trs_to_mat4(t: [f32;3], r: [f32;4], s: [f32;3]) -> Matrix4<f32> {
    let norm_sq = r[0]*r[0] + r[1]*r[1] + r[2]*r[2] + r[3]*r[3];
    let (rx, ry, rz, rw) = if norm_sq < 1e-8 {
        (0.0, 0.0, 0.0, 1.0)
    } else {
        let inv_len = 1.0 / norm_sq.sqrt();
        (r[0]*inv_len, r[1]*inv_len, r[2]*inv_len, r[3]*inv_len)
    };
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

// ----- glTF/VRM 共通のテクスチャ抽出（GLB形式）-----
pub fn extract_texture_data_from_glb(path: &str) -> KaguraResult<Vec<(usize, Vec<u8>, String)>> {
    let data = fs::read(path)?;
    if &data[0..4] != b"glTF" {
        return Err(KaguraError::VrmParse("Not a glTF/GLB file".to_string()));
    }
    let mut offset = 12usize;
    let mut json_bytes: Option<&[u8]> = None;
    let mut bin_data: &[u8] = &[];
    while offset + 8 <= data.len() {
        let chunk_len = read_u32_le(&data, offset) as usize;
        let chunk_type = read_u32_le(&data, offset + 4);
        let chunk_data = &data[offset + 8 .. (offset + 8 + chunk_len).min(data.len())];
        match chunk_type {
            0x4E4F534A => json_bytes = Some(chunk_data), // JSON chunk
            0x004E4942 => bin_data = chunk_data,         // BIN chunk
            _ => {}
        }
        offset += 8 + chunk_len;
    }
    let json_bytes = json_bytes.ok_or_else(|| KaguraError::VrmParse("JSON chunk not found".to_string()))?;
    let json_str = std::str::from_utf8(json_bytes)
        .map_err(|e| KaguraError::VrmParse(format!("UTF-8 error: {}", e)))?
        .trim_end_matches('\0');
    let gltf: Value = serde_json::from_str(json_str)
        .map_err(|e| KaguraError::VrmParse(format!("JSON parse: {}", e)))?;

    let empty_vec = vec![];
    let images = gltf["images"].as_array().unwrap_or(&empty_vec);
    let textures = gltf["textures"].as_array().unwrap_or(&empty_vec);
    let bvs = gltf["bufferViews"].as_array().unwrap_or(&empty_vec);

    let mut result = Vec::new();
    for (ti, tex) in textures.iter().enumerate() {
        let src_idx = tex["source"].as_u64().unwrap_or(0) as usize;
        if src_idx >= images.len() { continue; }
        let img = &images[src_idx];
        let bv_idx = match img["bufferView"].as_u64() { Some(v) => v as usize, None => continue };
        if bv_idx >= bvs.len() { continue; }
        let bv = &bvs[bv_idx];
        let off = bv["byteOffset"].as_u64().unwrap_or(0) as usize;
        let size = bv["byteLength"].as_u64().unwrap_or(0) as usize;
        if size < 16 || off + size > bin_data.len() { continue; }
        let mime = img["mimeType"].as_str().unwrap_or("image/png");
        let ext = if mime.contains("jpeg") { "jpg".to_string() } else { "png".to_string() };
        result.push((ti, bin_data[off..off+size].to_vec(), ext));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn read_u32_le_endian() {
        assert_eq!(read_u32_le(&[0x78, 0x56, 0x34, 0x12], 0), 0x1234_5678);
    }

    #[test]
    fn read_f32_le_roundtrip() {
        let bytes = 1.5f32.to_le_bytes();
        assert!((read_f32_le(&bytes, 0) - 1.5).abs() < 1e-6);
    }

    #[test]
    fn parse_accessor_f32_vec3() {
        let mut bin = Vec::new();
        for v in [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0] {
            bin.extend_from_slice(&v.to_le_bytes());
        }
        let gltf = json!({
            "accessors": [{
                "bufferView": 0,
                "componentType": 5126,
                "count": 2,
                "type": "VEC3"
            }],
            "bufferViews": [{ "byteOffset": 0 }]
        });
        let rows = parse_accessor_f32(&gltf, &bin, 0, 3).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec![1.0, 2.0, 3.0]);
        assert_eq!(rows[1], vec![4.0, 5.0, 6.0]);
    }

    #[test]
    fn parse_accessor_u32_from_u16() {
        let bin = [1u8, 0, 2, 0, 3, 0];
        let gltf = json!({
            "accessors": [{
                "bufferView": 0,
                "componentType": 5123,
                "count": 3
            }],
            "bufferViews": [{ "byteOffset": 0 }]
        });
        assert_eq!(parse_accessor_u32(&gltf, &bin, 0).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn parse_accessor_f32_out_of_bounds() {
        let gltf = json!({
            "accessors": [{
                "bufferView": 0,
                "componentType": 5126,
                "count": 2,
                "type": "VEC3"
            }],
            "bufferViews": [{ "byteOffset": 0 }]
        });
        let err = parse_accessor_f32(&gltf, &[0u8; 4], 0, 3).unwrap_err();
        assert!(err.to_string().contains("out of bounds"));
    }

    #[test]
    fn parse_mat4_short_is_identity() {
        assert_eq!(parse_mat4(&[1.0; 8]), Matrix4::identity());
    }

    #[test]
    fn smooth_normals_of_xz_quad_point_up() {
        // y=0 平面の 2 三角形。反時計回り(CCW)なので法線は +Y。
        let positions = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, -1.0],
            [0.0, 0.0, -1.0],
        ];
        let indices = [0, 1, 2, 0, 2, 3];
        let normals = compute_smooth_normals(&positions, &indices);
        assert_eq!(normals.len(), 4);
        for n in &normals {
            assert!((n[0]).abs() < 1e-5, "x should be 0, got {:?}", n);
            assert!((n[1] - 1.0).abs() < 1e-5, "y should be 1, got {:?}", n);
            assert!((n[2]).abs() < 1e-5, "z should be 0, got {:?}", n);
        }
    }

    #[test]
    fn smooth_normals_are_unit_length() {
        let positions = [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 3.0, 4.0],
        ];
        let normals = compute_smooth_normals(&positions, &[0, 1, 2]);
        for n in &normals {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-5, "not unit: {:?}", n);
        }
    }

    #[test]
    fn smooth_normals_fallback_without_indices() {
        // インデックスが無いと寄与ゼロ。NaN ではなく既定値になること。
        let normals = compute_smooth_normals(&[[0.0, 0.0, 0.0]], &[]);
        assert_eq!(normals, vec![[0.0, 1.0, 0.0]]);
    }

    #[test]
    fn smooth_normals_ignore_out_of_range_indices() {
        let normals = compute_smooth_normals(&[[0.0, 0.0, 0.0]], &[0, 5, 9]);
        assert_eq!(normals, vec![[0.0, 1.0, 0.0]]);
    }

    #[test]
    fn resolve_normals_prefers_accessor_rows() {
        let rows = vec![vec![1.0, 0.0, 0.0], vec![0.0, 0.0, 1.0]];
        let positions = [[0.0; 3], [1.0, 0.0, 0.0]];
        let got = resolve_normals(Some(&rows), &positions, &[]);
        assert_eq!(got, vec![[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
    }

    #[test]
    fn resolve_normals_falls_back_when_rows_too_short() {
        let rows = vec![vec![1.0, 0.0, 0.0]];
        let positions = [[0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]];
        let got = resolve_normals(Some(&rows), &positions, &[0, 1, 2]);
        assert_eq!(got.len(), 3);
        // アクセサ由来の +X ではなく、形状から求めた値になっているはず
        assert!(got[0] != [1.0, 0.0, 0.0]);
    }

    /// 親が常に子より後ろにある並び（VRoid 製 VRM と同じ形）でも、
    /// 走査順では親が先に来ること。
    fn assert_parents_first(parents: &[Option<usize>], order: &[usize]) {
        let mut seen = vec![false; parents.len()];
        for &i in order {
            if let Some(p) = parents[i] {
                assert!(seen[p], "node {} visited before its parent {}", i, p);
            }
            seen[i] = true;
        }
        assert_eq!(order.len(), parents.len());
    }

    #[test]
    fn hierarchy_order_handles_leaf_first_layout() {
        // 0 <- 1 <- 2 <- 3(root): 子ほど index が小さい
        let parents = vec![Some(1), Some(2), Some(3), None];
        let order = build_hierarchy_order(&parents);
        assert_eq!(order, vec![3, 2, 1, 0]);
        assert_parents_first(&parents, &order);
    }

    #[test]
    fn hierarchy_order_handles_parent_first_layout() {
        let parents = vec![None, Some(0), Some(1), Some(0)];
        let order = build_hierarchy_order(&parents);
        assert_parents_first(&parents, &order);
    }

    #[test]
    fn hierarchy_order_survives_cycle() {
        // 1 <-> 2 の循環。到達不能でも全ノードを取りこぼさない。
        let parents = vec![None, Some(2), Some(1)];
        let order = build_hierarchy_order(&parents);
        assert_eq!(order.len(), 3);
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2]);
    }

    #[test]
    fn hierarchy_order_ignores_self_parent() {
        let parents = vec![Some(0), Some(0)];
        let order = build_hierarchy_order(&parents);
        assert_eq!(order.len(), 2);
        assert_eq!(order[0], 0);
    }

    #[test]
    fn trs_to_mat4_translation_and_zero_quat() {
        let m = trs_to_mat4([1.0, 2.0, 3.0], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]);
        assert!((m[(0, 3)] - 1.0).abs() < 1e-5);
        assert!((m[(1, 3)] - 2.0).abs() < 1e-5);
        assert!((m[(2, 3)] - 3.0).abs() < 1e-5);

        let m0 = trs_to_mat4([0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        assert!((m0[(0, 0)] - 1.0).abs() < 1e-5);
        assert!((m0[(1, 1)] - 1.0).abs() < 1e-5);
        assert!((m0[(2, 2)] - 1.0).abs() < 1e-5);
    }
}