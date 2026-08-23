//! 最小の glTF 2.0 ローダ。静的メッシュ（POSITION + NORMAL + indices）だけ読む。
//!
//! 重い `gltf` クレートは使わない。依存を増やさず、エージェントが吐いた
//! 単純な .gltf を共有コアに載せられるようにする。外部 .bin は `resolve_buffer`
//! コールバックで渡す。

use crate::scene3d::{MeshData, Vertex3};
use glam::Vec3;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GltfFile {
    accessors: Vec<Accessor>,
    buffer_views: Vec<BufferView>,
    buffers: Vec<Buffer>,
    meshes: Vec<Mesh>,
    #[serde(default)]
    materials: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Buffer {
    #[serde(default)]
    uri: Option<String>,
    byte_length: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BufferView {
    buffer: usize,
    #[serde(default)]
    byte_offset: usize,
    #[allow(dead_code)]
    byte_length: usize,
    #[serde(default)]
    byte_stride: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Accessor {
    buffer_view: Option<usize>,
    #[serde(default)]
    byte_offset: usize,
    component_type: u32,
    count: usize,
    #[serde(rename = "type")]
    type_name: String,
}

#[derive(Debug, Deserialize)]
struct Mesh {
    primitives: Vec<Primitive>,
}

#[derive(Debug, Deserialize)]
struct Primitive {
    attributes: Attributes,
    indices: Option<usize>,
    #[serde(default)]
    mode: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct Attributes {
    #[serde(rename = "POSITION")]
    position: usize,
    #[serde(rename = "NORMAL")]
    #[serde(default)]
    normal: Option<usize>,
}

/// `uri` が data: でも相対パスでも、バイト列を返す。
pub type BufferResolver<'a> = dyn FnMut(&str) -> Result<Vec<u8>, String> + 'a;

/// glTF JSON から最初のメッシュを `MeshData` にする。
pub fn mesh_from_gltf_json(
    json: &str,
    mut resolve: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<MeshData, String> {
    let doc: GltfFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    if doc.meshes.is_empty() || doc.meshes[0].primitives.is_empty() {
        return Err("gltf has no mesh primitives".into());
    }
    let prim = &doc.meshes[0].primitives[0];
    if let Some(mode) = prim.mode {
        if mode != 4 {
            return Err(format!("only TRIANGLES (mode=4) supported, got {mode}"));
        }
    }

    let mut blobs = Vec::with_capacity(doc.buffers.len());
    for (i, buf) in doc.buffers.iter().enumerate() {
        let bytes = match &buf.uri {
            Some(uri) if uri.starts_with("data:") => decode_data_uri(uri)?,
            Some(uri) => resolve(uri)?,
            None => {
                return Err(format!(
                    "buffer {i} has no uri; pass embedded data: URIs for headless loads"
                ));
            }
        };
        if bytes.len() < buf.byte_length {
            return Err(format!(
                "buffer {i} too short: {} < {}",
                bytes.len(),
                buf.byte_length
            ));
        }
        blobs.push(bytes);
    }

    let positions = read_f32x3(&doc, &blobs, prim.attributes.position)?;
    let normals = match prim.attributes.normal {
        Some(n) => read_f32x3(&doc, &blobs, n)?,
        None => positions.iter().map(|_| Vec3::Y).collect(),
    };
    if positions.len() != normals.len() {
        return Err("POSITION/NORMAL count mismatch".into());
    }

    let indices = match prim.indices {
        Some(i) => read_indices(&doc, &blobs, i)?,
        None => (0..positions.len() as u32).collect(),
    };

    let mesh = MeshData {
        vertices: positions
            .into_iter()
            .zip(normals)
            .map(|(p, n)| Vertex3::new(p, n.normalize_or(Vec3::Y)))
            .collect(),
        indices,
    };
    if mesh.vertices.is_empty() {
        return Err("empty mesh".into());
    }
    let _ = doc.materials;
    Ok(mesh)
}

/// data URI だけを読む簡易入口。単体テストとオフスクリーン向け。
pub fn mesh_from_embedded_gltf(json: &str) -> Result<MeshData, String> {
    mesh_from_gltf_json(json, |_| {
        Err("external buffers not allowed in embedded mode".into())
    })
}

fn decode_data_uri(uri: &str) -> Result<Vec<u8>, String> {
    // data:application/octet-stream;base64,XXXX
    let comma = uri
        .find(',')
        .ok_or_else(|| "invalid data uri".to_string())?;
    let meta = &uri[..comma];
    let data = &uri[comma + 1..];
    if !meta.contains(";base64") {
        return Err("only base64 data URIs are supported".into());
    }
    base64_decode(data)
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // 依存を増やさないための最小デコーダ。パディングありの標準 base64 のみ。
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut table = [255u8; 256];
    for (i, &c) in T.iter().enumerate() {
        table[c as usize] = i as u8;
    }
    let cleaned: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !cleaned.len().is_multiple_of(4) {
        return Err("base64 length not multiple of 4".into());
    }
    let mut out = Vec::with_capacity(cleaned.len() / 4 * 3);
    for chunk in cleaned.as_chunks::<4>().0 {
        let mut n = [0u8; 4];
        for i in 0..4 {
            if chunk[i] == b'=' {
                n[i] = 0;
            } else {
                let v = table[chunk[i] as usize];
                if v == 255 {
                    return Err("invalid base64 char".into());
                }
                n[i] = v;
            }
        }
        out.push((n[0] << 2) | (n[1] >> 4));
        if chunk[2] != b'=' {
            out.push((n[1] << 4) | (n[2] >> 2));
        }
        if chunk[3] != b'=' {
            out.push((n[2] << 6) | n[3]);
        }
    }
    Ok(out)
}

fn read_f32x3(doc: &GltfFile, blobs: &[Vec<u8>], accessor: usize) -> Result<Vec<Vec3>, String> {
    let acc = doc
        .accessors
        .get(accessor)
        .ok_or_else(|| format!("missing accessor {accessor}"))?;
    if acc.type_name != "VEC3" || acc.component_type != 5126 {
        return Err("POSITION/NORMAL must be FLOAT VEC3".into());
    }
    let view_idx = acc
        .buffer_view
        .ok_or_else(|| "accessor has no bufferView".to_string())?;
    let view = doc
        .buffer_views
        .get(view_idx)
        .ok_or_else(|| format!("missing bufferView {view_idx}"))?;
    let blob = blobs
        .get(view.buffer)
        .ok_or_else(|| format!("missing buffer {}", view.buffer))?;
    let stride = view.byte_stride.unwrap_or(12);
    let start = view.byte_offset + acc.byte_offset;
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let off = start + i * stride;
        let slice = blob
            .get(off..off + 12)
            .ok_or_else(|| "accessor out of range".to_string())?;
        let x = f32::from_le_bytes(slice[0..4].try_into().unwrap());
        let y = f32::from_le_bytes(slice[4..8].try_into().unwrap());
        let z = f32::from_le_bytes(slice[8..12].try_into().unwrap());
        out.push(Vec3::new(x, y, z));
    }
    Ok(out)
}

fn read_indices(doc: &GltfFile, blobs: &[Vec<u8>], accessor: usize) -> Result<Vec<u32>, String> {
    let acc = doc
        .accessors
        .get(accessor)
        .ok_or_else(|| format!("missing accessor {accessor}"))?;
    if acc.type_name != "SCALAR" {
        return Err("indices must be SCALAR".into());
    }
    let view_idx = acc
        .buffer_view
        .ok_or_else(|| "accessor has no bufferView".to_string())?;
    let view = &doc.buffer_views[view_idx];
    let blob = &blobs[view.buffer];
    let start = view.byte_offset + acc.byte_offset;
    let mut out = Vec::with_capacity(acc.count);
    match acc.component_type {
        5123 => {
            // UNSIGNED_SHORT
            for i in 0..acc.count {
                let off = start + i * 2;
                let v = u16::from_le_bytes(blob[off..off + 2].try_into().unwrap());
                out.push(v as u32);
            }
        }
        5125 => {
            // UNSIGNED_INT
            for i in 0..acc.count {
                let off = start + i * 4;
                let v = u32::from_le_bytes(blob[off..off + 4].try_into().unwrap());
                out.push(v);
            }
        }
        other => return Err(format!("unsupported index componentType {other}")),
    }
    Ok(out)
}

/// 単体テスト用の 1x1x1 箱。POSITION + NORMAL + indices を data URI で持つ。
pub fn unit_cube_gltf() -> String {
    // 8 corners × 3 = 24 floats for a non-indexed unique-vertex box would be long;
    // use the same 24-vertex face layout as primitives::box_mesh but compact.
    let mesh = crate::scene3d::primitives::box_mesh(Vec3::ONE);
    embed_mesh_as_gltf(&mesh)
}

fn embed_mesh_as_gltf(mesh: &MeshData) -> String {
    let mut pos = Vec::with_capacity(mesh.vertices.len() * 12);
    let mut nrm = Vec::with_capacity(mesh.vertices.len() * 12);
    for v in &mesh.vertices {
        pos.extend_from_slice(&v.pos[0].to_le_bytes());
        pos.extend_from_slice(&v.pos[1].to_le_bytes());
        pos.extend_from_slice(&v.pos[2].to_le_bytes());
        nrm.extend_from_slice(&v.normal[0].to_le_bytes());
        nrm.extend_from_slice(&v.normal[1].to_le_bytes());
        nrm.extend_from_slice(&v.normal[2].to_le_bytes());
    }
    let mut idx = Vec::with_capacity(mesh.indices.len() * 2);
    for i in &mesh.indices {
        idx.extend_from_slice(&(*i as u16).to_le_bytes());
    }
    let mut bin = Vec::new();
    bin.extend_from_slice(&pos);
    let nrm_off = bin.len();
    bin.extend_from_slice(&nrm);
    let idx_off = bin.len();
    bin.extend_from_slice(&idx);
    let b64 = base64_encode(&bin);
    let nverts = mesh.vertices.len();
    let nidx = mesh.indices.len();
    format!(
        r#"{{
  "asset": {{"version": "2.0"}},
  "buffers": [{{"byteLength": {blen}, "uri": "data:application/octet-stream;base64,{b64}"}}],
  "bufferViews": [
    {{"buffer": 0, "byteOffset": 0, "byteLength": {plen}}},
    {{"buffer": 0, "byteOffset": {nrm_off}, "byteLength": {plen}}},
    {{"buffer": 0, "byteOffset": {idx_off}, "byteLength": {ilen}}}
  ],
  "accessors": [
    {{"bufferView": 0, "componentType": 5126, "count": {nverts}, "type": "VEC3"}},
    {{"bufferView": 1, "componentType": 5126, "count": {nverts}, "type": "VEC3"}},
    {{"bufferView": 2, "componentType": 5123, "count": {nidx}, "type": "SCALAR"}}
  ],
  "meshes": [{{"primitives": [{{"attributes": {{"POSITION": 0, "NORMAL": 1}}, "indices": 2, "mode": 4}}]}}]
}}"#,
        blen = bin.len(),
        plen = pos.len(),
        ilen = idx.len(),
    )
}

fn base64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let a = chunk[0] as u32;
        let b = chunk.get(1).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (a << 16) | (b << 8) | c;
        out.push(T[((triple >> 18) & 63) as usize] as char);
        out.push(T[((triple >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(T[((triple >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(T[(triple & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_cube_roundtrips() {
        let json = unit_cube_gltf();
        let mesh = mesh_from_embedded_gltf(&json).expect("load");
        assert_eq!(mesh.vertices.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
        let b = mesh.bounds();
        assert!((b.min + Vec3::splat(0.5)).length() < 1e-3);
        assert!((b.max - Vec3::splat(0.5)).length() < 1e-3);
    }

    #[test]
    fn rejects_non_triangle_mode() {
        let json = r#"{"accessors":[],"bufferViews":[],"buffers":[],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"mode":1}]}]}"#;
        assert!(mesh_from_embedded_gltf(json).is_err());
    }
}
