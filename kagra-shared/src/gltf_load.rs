//! Minimal glTF 2.0 loader. Static meshes (POSITION + NORMAL + indices) and
//! one skinned mesh (nodes, skins, JOINTS_0, WEIGHTS_0, inverseBindMatrices,
//! first Walk/walk clip). CPU-skins into `Vertex3` so the wgpu 30 shader can
//! stay put (WebGL2-friendly: no storage buffers, no joint palette).
//!
//! Does not pull the heavy `gltf` crate. External .bin uses `resolve_buffer`.

use crate::scene3d::{MeshData, Vertex3};
use glam::{Mat4, Quat, Vec3};
use serde::Deserialize;

const WALK_SKINNED_GLTF: &str = include_str!("../tests/fixtures/walk_skinned.gltf");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GltfFile {
    accessors: Vec<Accessor>,
    buffer_views: Vec<BufferView>,
    buffers: Vec<Buffer>,
    meshes: Vec<Mesh>,
    #[serde(default)]
    materials: Vec<serde_json::Value>,
    #[serde(default)]
    nodes: Vec<GltfNode>,
    #[serde(default)]
    skins: Vec<GltfSkin>,
    #[serde(default)]
    animations: Vec<GltfAnimation>,
    #[serde(default)]
    #[allow(dead_code)]
    scenes: Vec<GltfScene>,
    #[serde(default)]
    #[allow(dead_code)]
    scene: Option<usize>,
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
    #[serde(rename = "JOINTS_0")]
    #[serde(default)]
    joints: Option<usize>,
    #[serde(rename = "WEIGHTS_0")]
    #[serde(default)]
    weights: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GltfNode {
    #[serde(default)]
    children: Vec<usize>,
    #[serde(default)]
    translation: Option<[f32; 3]>,
    #[serde(default)]
    rotation: Option<[f32; 4]>,
    #[serde(default)]
    scale: Option<[f32; 3]>,
    #[serde(default)]
    matrix: Option<[f32; 16]>,
    #[serde(default)]
    #[allow(dead_code)]
    mesh: Option<usize>,
    #[serde(default)]
    #[allow(dead_code)]
    skin: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GltfSkin {
    joints: Vec<usize>,
    #[serde(default)]
    inverse_bind_matrices: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct GltfAnimation {
    #[serde(default)]
    name: String,
    channels: Vec<GltfChannel>,
    samplers: Vec<GltfSampler>,
}

#[derive(Debug, Deserialize)]
struct GltfChannel {
    sampler: usize,
    target: GltfTarget,
}

#[derive(Debug, Deserialize)]
struct GltfTarget {
    node: usize,
    path: String,
}

#[derive(Debug, Deserialize)]
struct GltfSampler {
    input: usize,
    output: usize,
    #[serde(default)]
    interpolation: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct GltfScene {
    #[serde(default)]
    #[allow(dead_code)]
    nodes: Vec<usize>,
}

/// One local TRS node used while sampling.
#[derive(Clone, Copy, Debug)]
struct NodeLocal {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
}

/// CPU-skinnable mesh: rest `Vertex3` plus 4 influences, IBM, nodes, one clip.
#[derive(Clone, Debug)]
pub struct SkinnedMesh {
    pub rest: MeshData,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub inverse_bind: Vec<Mat4>,
    pub nodes: Vec<NodeRest>,
    pub skin_joints: Vec<usize>,
    pub clip: Option<AnimClip>,
}

/// Rest-pose node (children + TRS). `matrix` is baked into TRS when present.
#[derive(Clone, Debug)]
pub struct NodeRest {
    pub children: Vec<usize>,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

/// One animation clip (Walk / first clip). LINEAR / STEP only.
#[derive(Clone, Debug)]
pub struct AnimClip {
    pub name: String,
    pub duration: f32,
    pub channels: Vec<AnimChannel>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelPath {
    Translation,
    Rotation,
    Scale,
}

#[derive(Clone, Debug)]
pub struct AnimChannel {
    pub node: usize,
    pub path: ChannelPath,
    pub times: Vec<f32>,
    pub values: Vec<f32>,
    pub step: bool,
}

/// `uri` is data: or a relative path; bytes come back from this callback.
pub type BufferResolver<'a> = dyn FnMut(&str) -> Result<Vec<u8>, String> + 'a;

/// First mesh primitive as a static `MeshData` (bind / rest pose, no skin).
pub fn mesh_from_gltf_json(
    json: &str,
    resolve: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<MeshData, String> {
    let (doc, blobs) = parse_gltf(json, resolve)?;
    static_mesh_from_doc(&doc, &blobs)
}

/// data URI only. Unit tests and offscreen.
pub fn mesh_from_embedded_gltf(json: &str) -> Result<MeshData, String> {
    mesh_from_gltf_json(json, |_| {
        Err("external buffers not allowed in embedded mode".into())
    })
}

/// First skinned (or static) mesh plus optional Walk clip.
pub fn skinned_from_gltf_json(
    json: &str,
    resolve: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<SkinnedMesh, String> {
    let (doc, blobs) = parse_gltf(json, resolve)?;
    skinned_from_doc(&doc, &blobs)
}

/// data URI only.
pub fn skinned_from_embedded_gltf(json: &str) -> Result<SkinnedMesh, String> {
    skinned_from_gltf_json(json, |_| {
        Err("external buffers not allowed in embedded mode".into())
    })
}

/// Hand-authored 2-joint walk clip (glTF 2.0, skin + Walk). Tiny fixture.
pub fn walk_skinned_gltf() -> String {
    WALK_SKINNED_GLTF.to_string()
}

/// True when `spec` names the bundled walk fixture (path or stem).
pub fn is_walk_skinned_spec(spec: &str) -> bool {
    let lower = spec.trim().to_ascii_lowercase();
    let stem = std::path::Path::new(&lower)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(lower.as_str());
    matches!(
        stem,
        "walk_skinned.gltf" | "walk_skinned.glb" | "walk.gltf" | "walk.glb"
    )
}

/// CPU-skin `rest` vertices into `Vertex3` at time `t` (seconds, looped).
/// `t = 0` is the first key / T-pose for the bundled Walk clip.
pub fn sample_skinned(skin: &SkinnedMesh, t: f32) -> MeshData {
    if skin.skin_joints.is_empty()
        || skin.joints.len() != skin.rest.vertices.len()
        || skin.weights.len() != skin.rest.vertices.len()
    {
        return skin.rest.clone();
    }
    let locals = sample_locals(skin, t);
    let globals = global_pose(&skin.nodes, &locals);
    let mut joint_mats = vec![Mat4::IDENTITY; skin.skin_joints.len()];
    for (j, &node) in skin.skin_joints.iter().enumerate() {
        let g = globals.get(node).copied().unwrap_or(Mat4::IDENTITY);
        let ibm = skin.inverse_bind.get(j).copied().unwrap_or(Mat4::IDENTITY);
        joint_mats[j] = g * ibm;
    }
    let mut out = MeshData {
        vertices: Vec::with_capacity(skin.rest.vertices.len()),
        indices: skin.rest.indices.clone(),
    };
    for (i, v) in skin.rest.vertices.iter().enumerate() {
        let p = Vec3::from_array(v.pos);
        let n = Vec3::from_array(v.normal);
        let js = skin.joints[i];
        let ws = skin.weights[i];
        let mut sp = Vec3::ZERO;
        let mut sn = Vec3::ZERO;
        let mut wsum = 0.0f32;
        for k in 0..4 {
            let w = ws[k];
            if w.abs() < 1e-8 {
                continue;
            }
            let jm = joint_mats
                .get(js[k] as usize)
                .copied()
                .unwrap_or(Mat4::IDENTITY);
            sp += w * jm.transform_point3(p);
            sn += w * jm.transform_vector3(n);
            wsum += w;
        }
        if wsum < 1e-8 {
            sp = p;
            sn = n;
        }
        out.vertices
            .push(Vertex3::new(sp, sn.normalize_or(Vec3::Y)));
    }
    out
}

fn parse_gltf(
    json: &str,
    mut resolve: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<(GltfFile, Vec<Vec<u8>>), String> {
    let doc: GltfFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
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
    Ok((doc, blobs))
}

fn static_mesh_from_doc(doc: &GltfFile, blobs: &[Vec<u8>]) -> Result<MeshData, String> {
    if doc.meshes.is_empty() || doc.meshes[0].primitives.is_empty() {
        return Err("gltf has no mesh primitives".into());
    }
    let prim = &doc.meshes[0].primitives[0];
    if let Some(mode) = prim.mode {
        if mode != 4 {
            return Err(format!("only TRIANGLES (mode=4) supported, got {mode}"));
        }
    }
    let positions = read_f32x3(doc, blobs, prim.attributes.position)?;
    let normals = match prim.attributes.normal {
        Some(n) => read_f32x3(doc, blobs, n)?,
        None => positions.iter().map(|_| Vec3::Y).collect(),
    };
    if positions.len() != normals.len() {
        return Err("POSITION/NORMAL count mismatch".into());
    }
    let indices = match prim.indices {
        Some(i) => read_indices(doc, blobs, i)?,
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
    let _ = &doc.materials;
    Ok(mesh)
}

fn skinned_from_doc(doc: &GltfFile, blobs: &[Vec<u8>]) -> Result<SkinnedMesh, String> {
    let rest = static_mesh_from_doc(doc, blobs)?;
    let prim = &doc.meshes[0].primitives[0];
    let nverts = rest.vertices.len();
    let mut joints = vec![[0u16; 4]; nverts];
    let mut weights = vec![[1.0, 0.0, 0.0, 0.0]; nverts];
    if let (Some(j_acc), Some(w_acc)) = (prim.attributes.joints, prim.attributes.weights) {
        joints = read_joints(doc, blobs, j_acc)?;
        weights = read_weights(doc, blobs, w_acc)?;
        if joints.len() != nverts || weights.len() != nverts {
            return Err("JOINTS_0/WEIGHTS_0 count mismatch".into());
        }
    }
    let (skin_joints, inverse_bind) = if let Some(skin) = doc.skins.first() {
        let ibm = if let Some(acc) = skin.inverse_bind_matrices {
            read_mat4(doc, blobs, acc)?
        } else {
            vec![Mat4::IDENTITY; skin.joints.len()]
        };
        if ibm.len() != skin.joints.len() {
            return Err("inverseBindMatrices count mismatch".into());
        }
        (skin.joints.clone(), ibm)
    } else {
        (Vec::new(), Vec::new())
    };
    let nodes = doc.nodes.iter().map(node_rest).collect::<Vec<_>>();
    let clip = pick_clip(doc, blobs)?;
    Ok(SkinnedMesh {
        rest,
        joints,
        weights,
        inverse_bind,
        nodes,
        skin_joints,
        clip,
    })
}

fn node_rest(n: &GltfNode) -> NodeRest {
    if let Some(m) = n.matrix {
        let mat = Mat4::from_cols_array(&m);
        let (scale, rotation, translation) = mat.to_scale_rotation_translation();
        return NodeRest {
            children: n.children.clone(),
            translation,
            rotation,
            scale,
        };
    }
    NodeRest {
        children: n.children.clone(),
        translation: n.translation.map(Vec3::from_array).unwrap_or(Vec3::ZERO),
        rotation: n
            .rotation
            .map(|q| Quat::from_xyzw(q[0], q[1], q[2], q[3]))
            .unwrap_or(Quat::IDENTITY),
        scale: n.scale.map(Vec3::from_array).unwrap_or(Vec3::ONE),
    }
}

fn pick_clip(doc: &GltfFile, blobs: &[Vec<u8>]) -> Result<Option<AnimClip>, String> {
    if doc.animations.is_empty() {
        return Ok(None);
    }
    let anim = doc
        .animations
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("walk"))
        .unwrap_or(&doc.animations[0]);
    let mut channels = Vec::new();
    let mut duration = 0.0f32;
    for ch in &anim.channels {
        let sampler = anim
            .samplers
            .get(ch.sampler)
            .ok_or_else(|| format!("missing sampler {}", ch.sampler))?;
        let path = match ch.target.path.as_str() {
            "translation" => ChannelPath::Translation,
            "rotation" => ChannelPath::Rotation,
            "scale" => ChannelPath::Scale,
            _ => continue,
        };
        let interp = sampler
            .interpolation
            .as_deref()
            .unwrap_or("LINEAR")
            .to_ascii_uppercase();
        if interp == "CUBICSPLINE" {
            continue;
        }
        let times = read_scalars(doc, blobs, sampler.input)?;
        let ncomp = match path {
            ChannelPath::Rotation => 4,
            ChannelPath::Translation | ChannelPath::Scale => 3,
        };
        let values = read_f32_components(doc, blobs, sampler.output, ncomp)?;
        if let Some(&last) = times.last() {
            duration = duration.max(last);
        }
        channels.push(AnimChannel {
            node: ch.target.node,
            path,
            times,
            values,
            step: interp == "STEP",
        });
    }
    if channels.is_empty() {
        return Ok(None);
    }
    Ok(Some(AnimClip {
        name: if anim.name.is_empty() {
            "Walk".into()
        } else {
            anim.name.clone()
        },
        duration,
        channels,
    }))
}

fn sample_locals(skin: &SkinnedMesh, t: f32) -> Vec<NodeLocal> {
    let mut locals: Vec<NodeLocal> = skin
        .nodes
        .iter()
        .map(|n| NodeLocal {
            translation: n.translation,
            rotation: n.rotation,
            scale: n.scale,
        })
        .collect();
    let Some(clip) = &skin.clip else {
        return locals;
    };
    let t = if clip.duration > 1e-6 {
        t.rem_euclid(clip.duration)
    } else {
        0.0
    };
    for ch in &clip.channels {
        if ch.node >= locals.len() || ch.times.is_empty() {
            continue;
        }
        match ch.path {
            ChannelPath::Translation => {
                if let Some(v) = sample_vec3(ch, t) {
                    locals[ch.node].translation = v;
                }
            }
            ChannelPath::Scale => {
                if let Some(v) = sample_vec3(ch, t) {
                    locals[ch.node].scale = v;
                }
            }
            ChannelPath::Rotation => {
                if let Some(q) = sample_quat(ch, t) {
                    locals[ch.node].rotation = q;
                }
            }
        }
    }
    locals
}

fn sample_vec3(ch: &AnimChannel, t: f32) -> Option<Vec3> {
    let (i, j, a) = key_span(&ch.times, t)?;
    let a = if ch.step { 0.0 } else { a };
    let va = vec3_at(&ch.values, i)?;
    let vb = vec3_at(&ch.values, j)?;
    Some(va.lerp(vb, a))
}

fn sample_quat(ch: &AnimChannel, t: f32) -> Option<Quat> {
    let (i, j, a) = key_span(&ch.times, t)?;
    let a = if ch.step { 0.0 } else { a };
    let mut qa = quat_at(&ch.values, i)?;
    let mut qb = quat_at(&ch.values, j)?;
    if qa.dot(qb) < 0.0 {
        qb = -qb;
    }
    qa = qa.normalize();
    qb = qb.normalize();
    Some(qa.slerp(qb, a))
}

fn vec3_at(values: &[f32], i: usize) -> Option<Vec3> {
    let o = i * 3;
    Some(Vec3::new(
        *values.get(o)?,
        *values.get(o + 1)?,
        *values.get(o + 2)?,
    ))
}

fn quat_at(values: &[f32], i: usize) -> Option<Quat> {
    let o = i * 4;
    Some(Quat::from_xyzw(
        *values.get(o)?,
        *values.get(o + 1)?,
        *values.get(o + 2)?,
        *values.get(o + 3)?,
    ))
}

fn key_span(times: &[f32], t: f32) -> Option<(usize, usize, f32)> {
    if times.is_empty() {
        return None;
    }
    if times.len() == 1 || t <= times[0] {
        return Some((0, 0, 0.0));
    }
    if t >= *times.last()? {
        let last = times.len() - 1;
        return Some((last, last, 0.0));
    }
    for i in 0..times.len() - 1 {
        if t >= times[i] && t < times[i + 1] {
            let span = times[i + 1] - times[i];
            let a = if span > 1e-8 {
                (t - times[i]) / span
            } else {
                0.0
            };
            return Some((i, i + 1, a));
        }
    }
    let last = times.len() - 1;
    Some((last, last, 0.0))
}

fn global_pose(nodes: &[NodeRest], locals: &[NodeLocal]) -> Vec<Mat4> {
    let n = nodes.len();
    let mut parent = vec![None; n];
    for (i, node) in nodes.iter().enumerate() {
        for &c in &node.children {
            if c < n {
                parent[c] = Some(i);
            }
        }
    }
    let mut local_mat = vec![Mat4::IDENTITY; n];
    for (i, loc) in locals.iter().enumerate() {
        local_mat[i] =
            Mat4::from_scale_rotation_translation(loc.scale, loc.rotation, loc.translation);
    }
    let mut global = vec![Mat4::IDENTITY; n];
    let mut done = vec![false; n];
    fn compute(
        i: usize,
        parent: &[Option<usize>],
        local_mat: &[Mat4],
        global: &mut [Mat4],
        done: &mut [bool],
    ) {
        if done[i] {
            return;
        }
        if let Some(p) = parent[i] {
            compute(p, parent, local_mat, global, done);
            global[i] = global[p] * local_mat[i];
        } else {
            global[i] = local_mat[i];
        }
        done[i] = true;
    }
    for i in 0..n {
        compute(i, &parent, &local_mat, &mut global, &mut done);
    }
    global
}

fn decode_data_uri(uri: &str) -> Result<Vec<u8>, String> {
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
    let floats = read_f32_components(doc, blobs, accessor, 3)?;
    Ok(floats
        .chunks_exact(3)
        .map(|c| Vec3::new(c[0], c[1], c[2]))
        .collect())
}

fn read_mat4(doc: &GltfFile, blobs: &[Vec<u8>], accessor: usize) -> Result<Vec<Mat4>, String> {
    let acc = doc
        .accessors
        .get(accessor)
        .ok_or_else(|| format!("missing accessor {accessor}"))?;
    if acc.type_name != "MAT4" || acc.component_type != 5126 {
        return Err("inverseBindMatrices must be FLOAT MAT4".into());
    }
    let floats = read_f32_components(doc, blobs, accessor, 16)?;
    Ok(floats
        .chunks_exact(16)
        .map(|c| {
            let mut a = [0.0f32; 16];
            a.copy_from_slice(c);
            Mat4::from_cols_array(&a)
        })
        .collect())
}

fn read_scalars(doc: &GltfFile, blobs: &[Vec<u8>], accessor: usize) -> Result<Vec<f32>, String> {
    read_f32_components(doc, blobs, accessor, 1)
}

fn read_f32_components(
    doc: &GltfFile,
    blobs: &[Vec<u8>],
    accessor: usize,
    ncomp: usize,
) -> Result<Vec<f32>, String> {
    let acc = doc
        .accessors
        .get(accessor)
        .ok_or_else(|| format!("missing accessor {accessor}"))?;
    if acc.component_type != 5126 {
        return Err(format!(
            "accessor {accessor} must be FLOAT, got {}",
            acc.component_type
        ));
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
    let elem = ncomp * 4;
    let stride = view.byte_stride.unwrap_or(elem);
    let start = view.byte_offset + acc.byte_offset;
    let mut out = Vec::with_capacity(acc.count * ncomp);
    for i in 0..acc.count {
        let off = start + i * stride;
        let slice = blob
            .get(off..off + elem)
            .ok_or_else(|| "accessor out of range".to_string())?;
        for k in 0..ncomp {
            let o = k * 4;
            out.push(f32::from_le_bytes(slice[o..o + 4].try_into().unwrap()));
        }
    }
    Ok(out)
}

fn read_joints(
    doc: &GltfFile,
    blobs: &[Vec<u8>],
    accessor: usize,
) -> Result<Vec<[u16; 4]>, String> {
    let acc = doc
        .accessors
        .get(accessor)
        .ok_or_else(|| format!("missing accessor {accessor}"))?;
    if acc.type_name != "VEC4" {
        return Err("JOINTS_0 must be VEC4".into());
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
    let start = view.byte_offset + acc.byte_offset;
    let mut out = Vec::with_capacity(acc.count);
    match acc.component_type {
        5121 => {
            let stride = view.byte_stride.unwrap_or(4);
            for i in 0..acc.count {
                let off = start + i * stride;
                let s = blob
                    .get(off..off + 4)
                    .ok_or_else(|| "JOINTS_0 out of range".to_string())?;
                out.push([s[0] as u16, s[1] as u16, s[2] as u16, s[3] as u16]);
            }
        }
        5123 => {
            let stride = view.byte_stride.unwrap_or(8);
            for i in 0..acc.count {
                let off = start + i * stride;
                let s = blob
                    .get(off..off + 8)
                    .ok_or_else(|| "JOINTS_0 out of range".to_string())?;
                out.push([
                    u16::from_le_bytes(s[0..2].try_into().unwrap()),
                    u16::from_le_bytes(s[2..4].try_into().unwrap()),
                    u16::from_le_bytes(s[4..6].try_into().unwrap()),
                    u16::from_le_bytes(s[6..8].try_into().unwrap()),
                ]);
            }
        }
        other => return Err(format!("unsupported JOINTS_0 componentType {other}")),
    }
    Ok(out)
}

fn read_weights(
    doc: &GltfFile,
    blobs: &[Vec<u8>],
    accessor: usize,
) -> Result<Vec<[f32; 4]>, String> {
    let acc = doc
        .accessors
        .get(accessor)
        .ok_or_else(|| format!("missing accessor {accessor}"))?;
    if acc.type_name != "VEC4" {
        return Err("WEIGHTS_0 must be VEC4".into());
    }
    match acc.component_type {
        5126 => {
            let f = read_f32_components(doc, blobs, accessor, 4)?;
            Ok(f.chunks_exact(4)
                .map(|c| [c[0], c[1], c[2], c[3]])
                .collect())
        }
        5121 => {
            let view_idx = acc
                .buffer_view
                .ok_or_else(|| "accessor has no bufferView".to_string())?;
            let view = &doc.buffer_views[view_idx];
            let blob = &blobs[view.buffer];
            let start = view.byte_offset + acc.byte_offset;
            let stride = view.byte_stride.unwrap_or(4);
            let mut out = Vec::with_capacity(acc.count);
            for i in 0..acc.count {
                let off = start + i * stride;
                let s = blob
                    .get(off..off + 4)
                    .ok_or_else(|| "WEIGHTS_0 out of range".to_string())?;
                out.push([
                    s[0] as f32 / 255.0,
                    s[1] as f32 / 255.0,
                    s[2] as f32 / 255.0,
                    s[3] as f32 / 255.0,
                ]);
            }
            Ok(out)
        }
        other => Err(format!("unsupported WEIGHTS_0 componentType {other}")),
    }
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
            for i in 0..acc.count {
                let off = start + i * 2;
                let v = u16::from_le_bytes(blob[off..off + 2].try_into().unwrap());
                out.push(v as u32);
            }
        }
        5125 => {
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

/// 1x1x1 box. POSITION + NORMAL + indices as a data URI.
pub fn unit_cube_gltf() -> String {
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

    #[test]
    fn walk_fixture_has_skin_joints_weights_ibm_and_walk_clip() {
        let skin = skinned_from_embedded_gltf(&walk_skinned_gltf()).expect("skin");
        assert_eq!(skin.rest.vertices.len(), 8);
        assert_eq!(skin.rest.indices.len(), 36);
        assert_eq!(skin.joints.len(), 8);
        assert_eq!(skin.weights.len(), 8);
        assert_eq!(skin.skin_joints.len(), 2);
        assert_eq!(skin.inverse_bind.len(), 2);
        assert_eq!(skin.nodes.len(), 2);
        let clip = skin.clip.as_ref().expect("Walk clip");
        assert!(clip.name.eq_ignore_ascii_case("walk"));
        assert!(clip.duration > 0.9);
        assert!(!clip.channels.is_empty());
        assert!(skin.joints.iter().any(|j| j[0] == 1));
        assert!(skin.weights.iter().all(|w| (w[0] - 1.0).abs() < 1e-5));
    }

    #[test]
    fn walk_sample_moves_vertices_off_tpose() {
        let skin = skinned_from_embedded_gltf(&walk_skinned_gltf()).expect("skin");
        let rest = sample_skinned(&skin, 0.0);
        let walk = sample_skinned(&skin, 0.25);
        assert_eq!(rest.vertices.len(), walk.vertices.len());
        let mut moved = 0u32;
        let mut max_d = 0.0f32;
        for (a, b) in rest.vertices.iter().zip(walk.vertices.iter()) {
            let d = (Vec3::from_array(a.pos) - Vec3::from_array(b.pos)).length();
            if d > 1e-4 {
                moved += 1;
            }
            max_d = max_d.max(d);
        }
        assert!(
            moved > 0 && max_d > 0.05,
            "walk at t=0.25 must move verts off T-pose (moved={moved} max_d={max_d})"
        );
        let back = sample_skinned(&skin, 0.0);
        for (a, b) in rest.vertices.iter().zip(back.vertices.iter()) {
            let d = (Vec3::from_array(a.pos) - Vec3::from_array(b.pos)).length();
            assert!(d < 1e-4, "t=0 must return to rest");
        }
    }

    #[test]
    fn static_loader_still_reads_skinned_rest_pose() {
        let mesh = mesh_from_embedded_gltf(&walk_skinned_gltf()).expect("static");
        assert_eq!(mesh.vertices.len(), 8);
        assert_eq!(mesh.indices.len(), 36);
    }
}
