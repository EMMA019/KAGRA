//! Minimal glTF 2.0 loader. Static meshes (POSITION + NORMAL + TEXCOORD_0 + indices) and
//! one skinned mesh (nodes, skins, JOINTS_0, WEIGHTS_0, inverseBindMatrices,
//! first Walk/walk clip). CPU-skins into `Vertex3` so the wgpu 30 shader can
//! stay put (WebGL2-friendly: no storage buffers, no joint palette).
//!
//! Optional `pbrMetallicRoughness.baseColorTexture` (or VRM0 `_MainTex`) is decoded
//! into `MeshData.albedo`. `metallicRoughnessTexture` becomes linear ORM (G roughness,
//! B metallic), multiplied by the factors. When those factors are authored and
//! there is no texture, a 1×1 ORM carries them so instance roughness does not
//! replace a fully rough metal. Missing keys stay scalar. Dump maps still
//! replace that image later.
//! Toon primitives skip ORM so MToon slots stay the shade record. VRM 0 materialProperties / VRM 1 VRMC_materials_mtoon shadeColor + shadingToony land on MeshData.mtoon. Morph targets (POSITION deltas) plus VRM 0 blendShapeMaster / VRM 1 VRMC_vrm expressions apply one named shape onto CPU-skinned Vertex3. VRM 0 firstPerson / VRM 1 VRMC_vrm.lookAt yaw/pitch the head (eyes if present) with Mixamo rest+roll, not raw bind*delta. Does not pull the heavy `gltf` crate. External .bin uses
//! `resolve_buffer`. `.vrm` is GLB plus VRM 0/1 humanoid extras.

use std::collections::HashMap;
use std::sync::Arc;

use crate::scene3d::{AlbedoRgba, MeshData, MtoonShade, TexSampler, TexWrap, Vertex3};
use glam::{Mat4, Quat, Vec2, Vec3};
use serde::Deserialize;

const WALK_SKINNED_GLTF: &str = include_str!("../tests/fixtures/walk_skinned.gltf");
const WALK_SKINNED_VRM: &[u8] = include_bytes!("../tests/fixtures/walk_skinned.vrm");

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
    textures: Vec<GltfTexture>,
    #[serde(default)]
    samplers: Vec<GltfTexSampler>,
    #[serde(default)]
    images: Vec<GltfImage>,
    #[serde(default)]
    extensions: Option<serde_json::Value>,
    #[serde(default)]
    extras: Option<serde_json::Value>,
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
    #[serde(default)]
    normalized: bool,
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
    #[serde(default)]
    material: Option<usize>,
    #[serde(default)]
    targets: Vec<MorphAttrs>,
}

#[derive(Debug, Deserialize, Default)]
struct MorphAttrs {
    #[serde(rename = "POSITION")]
    #[serde(default)]
    position: Option<usize>,
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
    #[serde(rename = "TEXCOORD_0")]
    #[serde(default)]
    texcoord: Option<usize>,
    #[serde(rename = "COLOR_0")]
    #[serde(default)]
    color: Option<usize>,
    #[serde(rename = "TANGENT")]
    #[serde(default)]
    tangent: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GltfTexture {
    #[serde(default)]
    source: Option<usize>,
    #[serde(default)]
    sampler: Option<usize>,
}

/// Texture sampler. Not the animation `GltfSampler`.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GltfTexSampler {
    #[serde(default)]
    mag_filter: Option<u32>,
    #[serde(default)]
    min_filter: Option<u32>,
    #[serde(default)]
    wrap_s: Option<u32>,
    #[serde(default)]
    wrap_t: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GltfImage {
    #[serde(default)]
    uri: Option<String>,
    #[serde(default)]
    buffer_view: Option<usize>,
    #[serde(default)]
    #[allow(dead_code)]
    mime_type: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GltfNode {
    #[serde(default)]
    name: String,
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
    #[serde(default)]
    #[allow(dead_code)]
    extensions: Option<serde_json::Value>,
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
    /// Every glTF animation in file order (`clip` is the default pick among them).
    pub clips: Vec<AnimClip>,
    /// VRM 0/1 humanoid bone name -> node index. Empty when extras are absent.
    pub humanoid: HashMap<String, usize>,
    /// glTF メッシュインデックス（firstPerson の by_mesh 注釈参照用）。
    pub mesh_index: usize,
    /// VRM 0 secondaryAnimation / VRM 1 VRMC_springBone chains (rest).
    pub springs: crate::spring::SpringState,
    /// glTF morph target POSITION deltas (one vec per target, rest-pose).
    pub morphs: Vec<Vec<Vec3>>,
    /// VRM 0 blendShapeMaster / VRM 1 VRMC_vrm expression name -> binds.
    pub expressions: crate::morph::Expressions,
    /// VRM 0 firstPerson / VRM 1 VRMC_vrm.lookAt maps. None when absent.
    pub look_at: Option<crate::lookat::LookAt>,
    /// VRMC_node_constraint 1.0 (rotation / roll). Aim parses, not applied.
    pub constraints: Vec<crate::constraint::NodeConstraint>,
    /// VRM 0.x / VRM 1.0 firstPerson annotations (parsed; applied with an FPS cam).
    pub first_person: crate::first_person::FirstPerson,
}

/// Rest-pose node (children + TRS). `matrix` is baked into TRS when present.
#[derive(Clone, Debug)]
pub struct NodeRest {
    pub name: String,
    pub children: Vec<usize>,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

/// One glTF animation clip. LINEAR / STEP only.
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
    let (doc, blobs) = parse_gltf(json, resolve, None)?;
    static_mesh_from_doc(&doc, &blobs)
}

/// Extract a single mesh by index (untransformed by scene node translation) from a glTF JSON.
pub fn mesh_index_from_gltf_json(
    json: &str,
    mesh_index: usize,
    resolve: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<MeshData, String> {
    let (doc, blobs) = parse_gltf(json, resolve, None)?;
    let mesh = doc
        .meshes
        .get(mesh_index)
        .ok_or_else(|| format!("mesh index {mesh_index} out of range"))?;
    let prim = mesh
        .primitives
        .first()
        .ok_or_else(|| format!("mesh {mesh_index} has no primitives"))?;
    static_mesh_from_prim(&doc, &blobs, prim)
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
    let (doc, blobs) = parse_gltf(json, resolve, None)?;
    skinned_from_doc(&doc, &blobs)
}

/// data URI only.
pub fn skinned_from_embedded_gltf(json: &str) -> Result<SkinnedMesh, String> {
    skinned_from_gltf_json(json, |_| {
        Err("external buffers not allowed in embedded mode".into())
    })
}

/// Binary glTF / `.vrm` (GLB). Buffer 0 is the BIN chunk when `uri` is omitted.
pub fn mesh_from_glb(bytes: &[u8]) -> Result<MeshData, String> {
    let (json, bin) = split_glb(bytes)?;
    let (doc, blobs) = parse_gltf(
        &json,
        |_| Err("external buffers not allowed in glb mode".into()),
        bin.as_deref(),
    )?;
    static_mesh_from_doc(&doc, &blobs)
}

/// Skinned mesh from a `.glb` / `.vrm` (same CPU-skin path as JSON glTF).
pub fn skinned_from_glb(bytes: &[u8]) -> Result<SkinnedMesh, String> {
    let (json, bin) = split_glb(bytes)?;
    let (doc, blobs) = parse_gltf(
        &json,
        |_| Err("external buffers not allowed in glb mode".into()),
        bin.as_deref(),
    )?;
    skinned_from_doc(&doc, &blobs)
}

/// Every triangle primitive as its own skinned mesh (shared skeleton / Mixamo clip).
/// VRoid / VRM 1 files are Body+Face+Hair with many materials; first-prim-only
/// is a nude T-pose mannequin.
pub fn skinned_parts_from_glb(bytes: &[u8]) -> Result<Vec<SkinnedMesh>, String> {
    let (json, bin) = split_glb(bytes)?;
    let (doc, blobs) = parse_gltf(
        &json,
        |_| Err("external buffers not allowed in glb mode".into()),
        bin.as_deref(),
    )?;
    skinned_parts_from_doc(&doc, &blobs)
}

/// JSON glTF counterpart of `skinned_parts_from_glb`.
pub fn skinned_parts_from_gltf_json(
    json: &str,
    resolve: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<SkinnedMesh>, String> {
    let (doc, blobs) = parse_gltf(json, resolve, None)?;
    skinned_parts_from_doc(&doc, &blobs)
}

/// data URI only.
pub fn skinned_parts_from_embedded_gltf(json: &str) -> Result<Vec<SkinnedMesh>, String> {
    skinned_parts_from_gltf_json(json, |_| {
        Err("external buffers not allowed in embedded mode".into())
    })
}

/// Hand-authored 2-joint walk as a tiny VRM 0 (GLB + humanoid extras).
pub fn walk_skinned_vrm() -> Vec<u8> {
    WALK_SKINNED_VRM.to_vec()
}

/// True when `spec` names the bundled VRM fixture (path or stem).
pub fn is_walk_vrm_spec(spec: &str) -> bool {
    let lower = spec.trim().to_ascii_lowercase();
    let stem = std::path::Path::new(&lower)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(lower.as_str());
    matches!(stem, "walk_skinned.vrm" | "walk.vrm")
}
/// Bundled clip-less humanoid (same bytes as the VRM walk fixture; clip stripped on load).
pub fn tpose_humanoid_vrm() -> Vec<u8> {
    WALK_SKINNED_VRM.to_vec()
}

/// Clip-less bundled humanoid with Mixamo walk retargeted (rest+roll).
pub fn skinned_tpose_humanoid() -> Result<SkinnedMesh, String> {
    let mut skin = skinned_from_glb(&tpose_humanoid_vrm())?;
    skin.clip = None;
    crate::mixamo::bind_locomotion(&mut skin);
    Ok(skin)
}

/// True when `spec` names the clip-less Mixamo target (not Emma.vrm on disk).
pub fn is_tpose_humanoid_spec(spec: &str) -> bool {
    let lower = spec.trim().to_ascii_lowercase();
    let stem = std::path::Path::new(&lower)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(lower.as_str());
    matches!(stem, "tpose_humanoid.vrm" | "tpose_humanoid.gltf")
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

/// ウォーカー描画パラメータ（dump 由来）。ロコモーションブレンドと
/// 上半身ジェスチャー（overlay）を束ねる（Phase 2）。
#[derive(Clone, Debug, Default)]
pub struct WalkerPose {
    /// クリップ時間（秒）。None = rest。
    pub clip: Option<f32>,
    pub hair: f32,
    pub expression: String,
    pub morph: f32,
    pub look_yaw: f32,
    pub look_pitch: f32,
    /// ロコモーションブレンド 0..1（0 = rest、1 = walk クリップ）。
    pub anim_blend: f32,
    /// 上半身ジェスチャー: ノード名（humanoid 名 or node 名）→ 目標ローカル回転。
    pub overlay_bones: std::collections::HashMap<String, [f32; 4]>,
    /// overlay のブレンド係数 0..1。
    pub overlay_weight: f32,
    /// glTF clip to play by name. None / unknown = default (Walk / first) clip.
    pub clip_name: Option<String>,
    /// Crossfade source clip (by name) and its time in seconds.
    pub fade_from: Option<String>,
    pub fade_from_clip: f32,
    /// Weight of `fade_from` (1 = all old clip, 0 = all `clip_name`).
    pub fade_from_weight: f32,
}

impl WalkerPose {
    fn from_parts(
        t: Option<f32>,
        hair: f32,
        expression: &str,
        morph: f32,
        look_yaw: f32,
        look_pitch: f32,
        anim_blend: f32,
    ) -> Self {
        Self {
            clip: t,
            hair,
            expression: expression.to_string(),
            morph,
            look_yaw,
            look_pitch,
            anim_blend,
            ..Self::default()
        }
    }
}

/// CPU-skin `rest` vertices into `Vertex3` at time `t` (seconds, looped).
/// `t = 0` is the first key / T-pose for the bundled Walk clip.
pub fn sample_skinned(skin: &SkinnedMesh, t: f32) -> MeshData {
    sample_skinned_hair(skin, Some(t), 0.0, "blink", 0.0)
}

/// CPU-skin with optional clip time, dump `hair` yaw, named expression, and
/// dump `morph` weight. `t = None` is bind/rest (idle T-pose, not Mixamo key 0).
pub fn sample_skinned_hair(
    skin: &SkinnedMesh,
    t: Option<f32>,
    hair: f32,
    expression: &str,
    morph: f32,
) -> MeshData {
    sample_skinned_look(skin, t, hair, expression, morph, 0.0, 0.0)
}

/// CPU-skin with dump `hair`, named `expression` + `morph` weight, and head
/// look yaw/pitch.
pub fn sample_skinned_look(
    skin: &SkinnedMesh,
    t: Option<f32>,
    hair: f32,
    expression: &str,
    morph: f32,
    look_yaw: f32,
    look_pitch: f32,
) -> MeshData {
    let pose = WalkerPose::from_parts(t, hair, expression, morph, look_yaw, look_pitch, 1.0);
    sample_skinned_inner(skin, &pose, None)
}

/// `sample_skinned_look` + ロコモーションブレンド係数（0 = rest、1 = clip）。
#[allow(clippy::too_many_arguments)]
pub fn sample_skinned_look_blend(
    skin: &SkinnedMesh,
    t: Option<f32>,
    hair: f32,
    expression: &str,
    morph: f32,
    look_yaw: f32,
    look_pitch: f32,
    anim_blend: f32,
) -> MeshData {
    let pose = WalkerPose::from_parts(t, hair, expression, morph, look_yaw, look_pitch, anim_blend);
    sample_skinned_inner(skin, &pose, None)
}

/// `WalkerPose` 直接版（布なし）。overlay（上半身ジェスチャー）込み。
pub fn sample_skinned_pose(skin: &SkinnedMesh, pose: &WalkerPose) -> MeshData {
    sample_skinned_inner(skin, pose, None)
}

/// `sample_skinned_look` + SpringBone の布シミュレーション。
///
/// 一人称視点（"eye" カメラ）でこのパーツを隠すべきか。
///
/// VRM firstPerson 注釈: ThirdPersonOnly / Auto（頭部相当）は一人称で隠す。
/// FirstPersonOnly（手等）は残す。注釈が無ければ Auto 扱い（隠す）。
pub fn part_hidden_in_first_person(part: &SkinnedMesh) -> bool {
    use crate::first_person::MeshAnnotation;
    let by_mesh = part.first_person.by_mesh.get(&part.mesh_index);
    let by_node = part
        .skin_joints
        .iter()
        .find_map(|&n| part.first_person.by_node.get(&n));
    let flag = by_mesh.or(by_node).copied().unwrap_or(MeshAnnotation::Auto);
    matches!(flag, MeshAnnotation::ThirdPersonOnly | MeshAnnotation::Auto)
}

/// ポーズのワールド行列で Verlet を 1 ステップ進め、得られた関節の回転
/// デルタをノードのローカル回転に足してからスキンする。`sim` はフレームを
/// 跨いで保持する（レンダラが walker spec ごとに持つ）。初回は snap で
/// 動かない。
#[allow(clippy::too_many_arguments)]
pub fn sample_skinned_cloth(
    skin: &SkinnedMesh,
    t: Option<f32>,
    hair: f32,
    expression: &str,
    morph: f32,
    look_yaw: f32,
    look_pitch: f32,
    anim_blend: f32,
    sim: &mut crate::spring::SpringState,
    dt: f32,
) -> MeshData {
    let pose = WalkerPose::from_parts(t, hair, expression, morph, look_yaw, look_pitch, anim_blend);
    sample_skinned_inner(skin, &pose, Some((sim, dt)))
}

/// `sample_skinned_cloth` のポーズ直接版（レンダラ用。overlay 込み）。
pub fn sample_skinned_cloth_pose(
    skin: &SkinnedMesh,
    pose: &WalkerPose,
    sim: &mut crate::spring::SpringState,
    dt: f32,
) -> MeshData {
    sample_skinned_inner(skin, pose, Some((sim, dt)))
}

#[allow(clippy::too_many_arguments)]
/// Current clip at `t`, crossfaded from `pose.fade_from` (three.js
/// AnimationMixer crossFadeTo idea: per-bone lerp/slerp by fade weight;
/// three.js, MIT License).
fn sample_pose_clip_locals(skin: &SkinnedMesh, pose: &WalkerPose, t: f32) -> Vec<NodeLocal> {
    let cur = sample_clip_locals(skin, resolve_clip(skin, pose.clip_name.as_deref()), t);
    let w = pose.fade_from_weight.clamp(0.0, 1.0);
    let Some(from) = pose.fade_from.as_deref().and_then(|n| find_clip(skin, n)) else {
        return cur;
    };
    if w <= 1e-5 {
        return cur;
    }
    let old = sample_clip_locals(skin, Some(from), pose.fade_from_clip);
    // blend_locals(a, b, k) = a -> b by k; old weight w means k = 1 - w toward cur.
    blend_locals(&old, &cur, 1.0 - w)
}

/// Joints a GPU-skinned mesh may use. 64 mat4 = 4 KiB of uniform, well under
/// the WebGL2 / phone 16 KiB uniform-block minimum.
pub const MAX_GPU_JOINTS: usize = 64;

/// Per-vertex GPU skin stream: lane k = joint index + weight (weight in
/// [0, 1), stored as the fractional part). One RGBA32F texel per vertex, so it
/// needs no extra vertex attribute (all 16 WebGL2 slots are taken).
/// `None` when the mesh is unskinned or uses more than `MAX_GPU_JOINTS` joints.
pub fn gpu_skin_stream(skin: &SkinnedMesh) -> Option<Vec<[f32; 4]>> {
    if skin.skin_joints.is_empty()
        || skin.skin_joints.len() > MAX_GPU_JOINTS
        || skin.joints.len() != skin.rest.vertices.len()
        || skin.weights.len() != skin.rest.vertices.len()
    {
        return None;
    }
    let mut out = Vec::with_capacity(skin.joints.len());
    for (js, ws) in skin.joints.iter().zip(&skin.weights) {
        let mut lane = [0.0f32; 4];
        for k in 0..4 {
            let j = js[k] as usize;
            let w = ws[k].clamp(0.0, 0.9999);
            if j >= MAX_GPU_JOINTS {
                if w > 1e-8 {
                    return None;
                }
                continue;
            }
            lane[k] = j as f32 + w;
        }
        out.push(lane);
    }
    Some(out)
}

/// Decode one `gpu_skin_stream` lane (mirror of the WGSL `skin_matrix`).
pub fn unpack_skin_lane(v: f32) -> (usize, f32) {
    let j = v.floor();
    (j as usize, v - j)
}

/// True when this pose can be drawn by the GPU skin path: no morph weight and
/// no spring-bone cloth (both stay on the CPU).
pub fn gpu_skin_pose_ok(skin: &SkinnedMesh, pose: &WalkerPose) -> bool {
    pose.morph.abs() < 1e-8 && skin.springs.is_empty()
}

fn posed_locals(skin: &SkinnedMesh, pose: &WalkerPose) -> Vec<NodeLocal> {
    let mut locals = match pose.clip {
        Some(tt) => {
            let clip = sample_pose_clip_locals(skin, pose, tt);
            let blend = pose.anim_blend.clamp(0.0, 1.0);
            if blend >= 1.0 - 1e-5 {
                clip
            } else if blend <= 1e-5 {
                rest_locals(skin)
            } else {
                blend_locals(&rest_locals(skin), &clip, blend)
            }
        }
        None => rest_locals(skin),
    };
    apply_look(skin, &mut locals, pose.look_yaw, pose.look_pitch);
    apply_hair(skin, &mut locals, pose.hair);
    apply_overlay(skin, &mut locals, pose);
    locals
}

fn palette_from_locals(skin: &SkinnedMesh, locals: &[NodeLocal]) -> Vec<Mat4> {
    let globals = global_pose(&skin.nodes, locals);
    skin.skin_joints
        .iter()
        .enumerate()
        .map(|(j, &node)| {
            let g = globals.get(node).copied().unwrap_or(Mat4::IDENTITY);
            let ibm = skin.inverse_bind.get(j).copied().unwrap_or(Mat4::IDENTITY);
            g * ibm
        })
        .collect()
}

/// Joint palette (`global(joint) * inverseBind`) for `pose`, no cloth.
/// This is what the GPU path uploads; the CPU path skins with the same mats.
pub fn joint_palette(skin: &SkinnedMesh, pose: &WalkerPose) -> Vec<Mat4> {
    palette_from_locals(skin, &posed_locals(skin, pose))
}

fn sample_skinned_inner(
    skin: &SkinnedMesh,
    pose: &WalkerPose,
    cloth: Option<(&mut crate::spring::SpringState, f32)>,
) -> MeshData {
    if skin.skin_joints.is_empty()
        || skin.joints.len() != skin.rest.vertices.len()
        || skin.weights.len() != skin.rest.vertices.len()
    {
        return skin.rest.clone();
    }
    let mut locals = posed_locals(skin, pose);
    if let Some((sim, dt)) = cloth {
        let parents = node_parents(&skin.nodes);
        let pose_globals = global_pose(&skin.nodes, &locals);
        let updates = crate::spring::step_with_updates(sim, &pose_globals, &parents, dt);
        for (node, q) in updates {
            if node < locals.len() {
                locals[node].rotation = q;
            }
        }
    }
    let joint_mats = palette_from_locals(skin, &locals);
    let mut out = skin.rest.clone();
    out.vertices.clear();
    out.vertices.reserve(skin.rest.vertices.len());
    let rest_pos = morphed_rest(skin, &pose.expression, pose.morph);
    for (i, v) in skin.rest.vertices.iter().enumerate() {
        let p = rest_pos.get(i).copied().unwrap_or(Vec3::from_array(v.pos));
        let n = Vec3::from_array(v.normal);
        let t = Vec3::new(v.tangent[0], v.tangent[1], v.tangent[2]);
        let js = skin.joints[i];
        let ws = skin.weights[i];
        let mut sp = Vec3::ZERO;
        let mut sn = Vec3::ZERO;
        let mut st = Vec3::ZERO;
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
            st += w * jm.transform_vector3(t);
            wsum += w;
        }
        if wsum < 1e-8 {
            sp = p;
            sn = n;
            st = t;
        }
        let sn = sn.normalize_or(Vec3::Y);
        let tangent = if st.length_squared() > 1e-8 {
            let tn = st.normalize();
            [tn.x, tn.y, tn.z, v.tangent[3]]
        } else {
            [0.0, 0.0, 0.0, v.tangent[3]]
        };
        out.vertices.push(Vertex3 {
            pos: sp.to_array(),
            normal: sn.to_array(),
            uv: v.uv,
            color: v.color,
            tangent,
        });
    }
    out
}

fn parse_gltf(
    json: &str,
    mut resolve: impl FnMut(&str) -> Result<Vec<u8>, String>,
    bin: Option<&[u8]>,
) -> Result<(GltfFile, Vec<Vec<u8>>), String> {
    let doc: GltfFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut blobs = Vec::with_capacity(doc.buffers.len());
    for (i, buf) in doc.buffers.iter().enumerate() {
        let bytes = match &buf.uri {
            Some(uri) if uri.starts_with("data:") => decode_data_uri(uri)?,
            Some(uri) => resolve(uri)?,
            None => match bin {
                Some(bytes) if i == 0 => bytes.to_vec(),
                _ => {
                    return Err(format!(
                        "buffer {i} has no uri; pass embedded data: URIs for headless loads"
                    ));
                }
            },
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
    let nodes: Vec<NodeRest> = doc.nodes.iter().map(node_rest).collect();
    let locals: Vec<NodeLocal> = nodes
        .iter()
        .map(|n| NodeLocal {
            translation: n.translation,
            rotation: n.rotation,
            scale: n.scale,
        })
        .collect();
    let world = global_pose(&nodes, &locals);
    let mut mesh_world = vec![Mat4::IDENTITY; doc.meshes.len()];
    for (ni, n) in doc.nodes.iter().enumerate() {
        if let Some(mi) = n.mesh {
            if mi < mesh_world.len() && ni < world.len() {
                mesh_world[mi] = world[ni];
            }
        }
    }
    let mut parts: Vec<(MeshData, [u8; 4])> = Vec::new();
    let mut last_err = None;
    for (mi, m) in doc.meshes.iter().enumerate() {
        let mw = mesh_world.get(mi).copied().unwrap_or(Mat4::IDENTITY);
        let apply_mw = (mw - Mat4::IDENTITY).abs_diff_eq(Mat4::ZERO, 1e-5);
        for prim in &m.primitives {
            match static_mesh_from_prim(doc, blobs, prim) {
                Ok(mut pm) => {
                    if !apply_mw {
                        let flip = if mw.determinant() < 0.0 { -1.0 } else { 1.0 };
                        for v in &mut pm.vertices {
                            let p = mw.transform_point3(Vec3::from_array(v.pos));
                            let n = mw
                                .transform_vector3(Vec3::from_array(v.normal))
                                .normalize_or(Vec3::Y);
                            v.pos = p.to_array();
                            v.normal = n.to_array();
                            let t = Vec3::new(v.tangent[0], v.tangent[1], v.tangent[2]);
                            if t.length_squared() > 1e-8 {
                                let tn = mw.transform_vector3(t).normalize_or(Vec3::ZERO);
                                v.tangent = [tn.x, tn.y, tn.z, v.tangent[3] * flip];
                            }
                        }
                    }
                    let col = prim_base_color_u8(doc, prim);
                    parts.push((pm, col));
                }
                Err(e) => last_err = Some(e),
            }
        }
    }
    if parts.is_empty() {
        return Err(last_err.unwrap_or_else(|| "gltf has no mesh primitives".into()));
    }
    if parts.len() == 1 {
        let (pm, col) = parts.pop().unwrap();
        // A compressed base color cannot be rebaked into a 1×1 tint strip.
        if pm.albedo_ktx.is_some() {
            return Ok(pm);
        }
        // A factor-only material has ORM and a tint, and no baseColor texture.
        // Keep the tint. An ORM texture with its own albedo is left alone.
        if pm.albedo.is_none() && col != [230, 230, 235, 255] {
            return Ok(merge_colored_parts(vec![(pm, col)]));
        }
        return Ok(pm);
    }
    let any_tex = parts.iter().any(|(m, _)| m.albedo.is_some());
    if !any_tex {
        return Ok(merge_colored_parts(parts));
    }
    Ok(merge_textured_parts(parts))
}

fn merge_textured_parts(parts: Vec<(MeshData, [u8; 4])>) -> MeshData {
    let surface = part_surface(&parts);
    let first_tex = parts.iter().find_map(|(m, _)| m.albedo.as_ref());
    let Some(first_tex) = first_tex else {
        return merge_colored_parts(parts);
    };
    let same_ptr = parts.iter().all(|(m, _)| {
        m.albedo
            .as_ref()
            .is_some_and(|a| Arc::ptr_eq(&a.rgba, &first_tex.rgba))
    });
    if same_ptr {
        let borrowed = parts.iter().find_map(|(m, _)| {
            m.orm
                .as_ref()
                .map(|img| (img.width, img.height, img.rgba.clone()))
        });
        let any_rough = parts.iter().any(|(m, _)| m.orm_rough);
        let any_metal = parts.iter().any(|(m, _)| m.orm_metal);
        let mut iter = parts.into_iter();
        let mut out = iter.next().unwrap().0;
        for (pm, _) in iter {
            let base = out.vertices.len() as u32;
            out.vertices.extend(pm.vertices);
            out.indices.extend(pm.indices.into_iter().map(|i| base + i));
        }
        if out.orm.is_none() {
            if let Some((width, height, rgba)) = borrowed {
                out.orm = Some(AlbedoRgba {
                    width,
                    height,
                    rgba,
                });
                out.orm_rough = any_rough;
                out.orm_metal = any_metal;
            }
        }
        stamp_surface(&mut out, surface);
        return out;
    }
    let any_orm = parts.iter().any(|(m, _)| m.orm.is_some());
    let tile_w = first_tex.width.max(1);
    let tile_h = first_tex.height.max(1);
    let count = parts.len() as u32;
    let atlas_w = tile_w * count;
    let mut atlas_rgba = vec![255u8; (atlas_w * tile_h * 4) as usize];
    let mut orm_atlas = if any_orm {
        let mut px = vec![0u8; (atlas_w * tile_h * 4) as usize];
        for chunk in px.as_chunks_mut::<4>().0 {
            chunk[0] = 255;
            chunk[1] = 255;
            chunk[3] = 255;
        }
        px
    } else {
        Vec::new()
    };
    let mut out = MeshData::default();
    for (idx, (mut pm, col)) in parts.into_iter().enumerate() {
        let x_off = (idx as u32) * tile_w;
        if let Some(alb) = &pm.albedo {
            for y in 0..tile_h {
                let sy = (y * alb.height / tile_h).min(alb.height.saturating_sub(1));
                for x in 0..tile_w {
                    let sx = (x * alb.width / tile_w).min(alb.width.saturating_sub(1));
                    let si = ((sy * alb.width + sx) * 4) as usize;
                    let di = ((y * atlas_w + (x_off + x)) * 4) as usize;
                    if si + 4 <= alb.rgba.len() && di + 4 <= atlas_rgba.len() {
                        atlas_rgba[di..di + 4].copy_from_slice(&alb.rgba[si..si + 4]);
                    }
                }
            }
        } else {
            for y in 0..tile_h {
                for x in 0..tile_w {
                    let di = ((y * atlas_w + (x_off + x)) * 4) as usize;
                    atlas_rgba[di..di + 4].copy_from_slice(&col);
                }
            }
        }
        if any_orm {
            if let Some(orm) = &pm.orm {
                let x_off = (idx as u32) * tile_w;
                for y in 0..tile_h {
                    let sy = (y * orm.height / tile_h).min(orm.height.saturating_sub(1));
                    for x in 0..tile_w {
                        let sx = (x * orm.width / tile_w).min(orm.width.saturating_sub(1));
                        let si = ((sy * orm.width + sx) * 4) as usize;
                        let di = ((y * atlas_w + (x_off + x)) * 4) as usize;
                        if si + 4 <= orm.rgba.len() && di + 4 <= orm_atlas.len() {
                            orm_atlas[di..di + 4].copy_from_slice(&orm.rgba[si..si + 4]);
                        }
                    }
                }
            }
        }
        let inv_n = 1.0 / (count as f32);
        let u_base = (idx as f32) * inv_n;
        for v in &mut pm.vertices {
            let u = v.uv[0].rem_euclid(1.0);
            let vv = v.uv[1].rem_euclid(1.0);
            v.uv = [u_base + u * inv_n, vv];
        }
        let base = out.vertices.len() as u32;
        out.vertices.extend(pm.vertices);
        out.indices.extend(pm.indices.into_iter().map(|i| base + i));
    }
    out.albedo = Some(AlbedoRgba {
        width: atlas_w,
        height: tile_h,
        rgba: Arc::from(atlas_rgba),
    });
    if any_orm {
        out.orm = Some(AlbedoRgba {
            width: atlas_w,
            height: tile_h,
            rgba: Arc::from(orm_atlas),
        });
        out.orm_rough = true;
        out.orm_metal = true;
    }
    stamp_surface(&mut out, surface);
    out
}

fn prim_base_color_u8(doc: &GltfFile, prim: &Primitive) -> [u8; 4] {
    if let Some(mi) = prim.material {
        if let Some(mat) = doc.materials.get(mi) {
            if let Some(arr) = mat
                .pointer("/pbrMetallicRoughness/baseColorFactor")
                .and_then(|v| v.as_array())
            {
                if arr.len() >= 3 {
                    let r = linear_to_srgb_u8(arr[0].as_f64().unwrap_or(1.0) as f32);
                    let g = linear_to_srgb_u8(arr[1].as_f64().unwrap_or(1.0) as f32);
                    let b = linear_to_srgb_u8(arr[2].as_f64().unwrap_or(1.0) as f32);
                    let a = if arr.len() >= 4 {
                        (arr[3].as_f64().unwrap_or(1.0).clamp(0.0, 1.0) * 255.0).round() as u8
                    } else {
                        255
                    };
                    return [r, g, b, a];
                }
            }
        }
    }
    [230, 230, 235, 255]
}

fn merge_colored_parts(parts: Vec<(MeshData, [u8; 4])>) -> MeshData {
    let surface = part_surface(&parts);
    let n = parts.len().max(1) as u32;
    let any_orm = parts.iter().any(|(m, _)| m.orm.is_some());
    let mut rgba = Vec::with_capacity((n as usize) * 4);
    let mut orm_px = Vec::new();
    let mut out = MeshData::default();
    for (idx, (mut pm, col)) in parts.into_iter().enumerate() {
        rgba.extend_from_slice(&col);
        if any_orm {
            let (g, b) = orm_center(&pm);
            orm_px.extend_from_slice(&[255, g, b, 255]);
        }
        let u = (idx as f32 + 0.5) / (n as f32);
        for v in &mut pm.vertices {
            v.uv = [u, 0.5];
        }
        let base = out.vertices.len() as u32;
        out.vertices.extend(pm.vertices);
        out.indices.extend(pm.indices.into_iter().map(|i| base + i));
    }
    out.albedo = Some(AlbedoRgba {
        width: n,
        height: 1,
        rgba: Arc::from(rgba),
    });
    if any_orm {
        out.orm = Some(AlbedoRgba {
            width: n,
            height: 1,
            rgba: Arc::from(orm_px),
        });
        out.orm_rough = true;
        out.orm_metal = true;
    }
    stamp_surface(&mut out, surface);
    out
}

struct PartSurface {
    alpha_cut: f32,
    double_sided: bool,
    sampler: TexSampler,
    emissive: f32,
    emissive_color: [f32; 3],
    emissive_map: Option<AlbedoRgba>,
    clearcoat: f32,
    clearcoat_roughness: f32,
}

fn part_surface(parts: &[(MeshData, [u8; 4])]) -> PartSurface {
    let alpha_cut = parts
        .iter()
        .find(|(m, _)| m.alpha_cut != 0.0)
        .map(|(m, _)| m.alpha_cut)
        .unwrap_or(0.0);
    let double_sided = parts.iter().any(|(m, _)| m.double_sided);
    let sampler = parts
        .iter()
        .find(|(m, _)| m.sampler != TexSampler::default())
        .map(|(m, _)| m.sampler)
        .unwrap_or_default();
    let src = parts
        .iter()
        .find(|(m, _)| m.emissive > 0.0 || m.emissive_map.is_some());
    let (emissive, emissive_color, emissive_map) = src
        .map(|(m, _)| (m.emissive, m.emissive_color, m.emissive_map.clone()))
        .unwrap_or((0.0, [0.0; 3], None));
    let coat = parts.iter().find(|(m, _)| m.clearcoat > 0.0);
    let (clearcoat, clearcoat_roughness) = coat
        .map(|(m, _)| (m.clearcoat, m.clearcoat_roughness))
        .unwrap_or((0.0, 0.0));
    PartSurface {
        alpha_cut,
        double_sided,
        sampler,
        emissive,
        emissive_color,
        emissive_map,
        clearcoat,
        clearcoat_roughness,
    }
}

fn stamp_surface(out: &mut MeshData, surface: PartSurface) {
    if out.alpha_cut == 0.0 {
        out.alpha_cut = surface.alpha_cut;
    }
    out.double_sided = out.double_sided || surface.double_sided;
    if out.sampler == TexSampler::default() {
        out.sampler = surface.sampler;
    }
    if out.emissive <= 0.0 {
        out.emissive = surface.emissive;
        out.emissive_color = surface.emissive_color;
    }
    if out.emissive_map.is_none() {
        out.emissive_map = surface.emissive_map;
    }
    if out.clearcoat <= 0.0 {
        out.clearcoat = surface.clearcoat;
        out.clearcoat_roughness = surface.clearcoat_roughness;
    }
}

fn orm_center(mesh: &MeshData) -> (u8, u8) {
    let Some(img) = &mesh.orm else {
        return (255, 0);
    };
    let x = img.width / 2;
    let y = img.height / 2;
    let x = x.min(img.width.saturating_sub(1));
    let y = y.min(img.height.saturating_sub(1));
    let i = ((y * img.width + x) * 4) as usize;
    (
        img.rgba.get(i + 1).copied().unwrap_or(255),
        img.rgba.get(i + 2).copied().unwrap_or(0),
    )
}

fn static_mesh_from_prim(
    doc: &GltfFile,
    blobs: &[Vec<u8>],
    prim: &Primitive,
) -> Result<MeshData, String> {
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
    let mut uvs = match prim.attributes.texcoord {
        Some(i) => {
            let uv = read_uv(doc, blobs, i)?;
            if uv.len() != positions.len() {
                return Err("POSITION/TEXCOORD_0 count mismatch".into());
            }
            uv
        }
        None => vec![[0.0, 0.0]; positions.len()],
    };
    if let Some(xf) = texture_transform(doc, prim) {
        for uv in &mut uvs {
            *uv = apply_uv_transform(*uv, xf);
        }
    }
    let colors = match prim.attributes.color {
        Some(i) => {
            let c = read_colors(doc, blobs, i)?;
            if c.len() != positions.len() {
                return Err("POSITION/COLOR_0 count mismatch".into());
            }
            c
        }
        None => vec![[1.0, 1.0, 1.0, 1.0]; positions.len()],
    };
    let indices = match prim.indices {
        Some(i) => read_indices(doc, blobs, i)?,
        None => (0..positions.len() as u32).collect(),
    };
    let authored_tangent = if let Some(i) = prim.attributes.tangent {
        let t = read_tangents(doc, blobs, i)?;
        if t.len() != positions.len() {
            return Err("POSITION/TANGENT count mismatch".into());
        }
        Some(t)
    } else {
        None
    };
    let mtoon = load_mtoon(doc, prim);
    // Toon keeps the shade record in the instance slots. ORM flags share that
    // packing on non-toon materials only.
    let orm = if mtoon.is_some() {
        None
    } else {
        load_orm(doc, blobs, prim)
    };
    let has_orm = orm.is_some();
    let normal = normal_tex_index(doc, prim).and_then(|ti| texture_by_index(doc, blobs, ti));
    let normal_scale = normal.as_ref().and_then(|_| normal_tex_scale(doc, prim));
    let (emissive_map, emissive, emissive_color) = if mtoon.is_some() {
        (None, 0.0, [0.0; 3])
    } else {
        load_emissive(doc, blobs, prim)
    };
    let (clearcoat, clearcoat_roughness) = if mtoon.is_some() {
        (0.0, 0.0)
    } else {
        load_clearcoat(doc, prim)
    };
    let mut vertices: Vec<Vertex3> = positions
        .into_iter()
        .zip(normals)
        .zip(uvs)
        .zip(colors)
        .enumerate()
        .map(|(i, (((p, n), uv), color))| {
            let mut v = Vertex3::with_uv(p, n.normalize_or(Vec3::Y), uv);
            v.color = color;
            if let Some(t) = authored_tangent.as_ref() {
                v.tangent = t[i];
            }
            v
        })
        .collect();
    if normal.is_some() && authored_tangent.is_none() {
        generate_tangents(&mut vertices, &indices);
    }
    let (alpha_cut, double_sided, sampler) = material_state(doc, prim);
    let mesh = MeshData {
        vertices,
        indices,
        albedo: load_base_color(doc, blobs, prim),
        albedo_ktx: load_base_color_ktx(doc, blobs, prim),
        matcap: matcap_tex_index(doc, prim).and_then(|ti| texture_by_index(doc, blobs, ti)),
        normal,
        normal_scale,
        mtoon,
        orm,
        orm_rough: has_orm,
        orm_metal: has_orm,
        emissive_map,
        alpha_cut,
        double_sided,
        sampler,
        emissive,
        emissive_color,
        clearcoat,
        clearcoat_roughness,
    };
    if mesh.vertices.is_empty() {
        return Err("empty mesh".into());
    }
    Ok(mesh)
}

#[derive(Clone, Copy)]
struct UvXform {
    offset: [f32; 2],
    rotation: f32,
    scale: [f32; 2],
}

/// KHR_texture_transform: `uv' = R * (uv * scale) + offset`.
fn apply_uv_transform(uv: [f32; 2], xf: UvXform) -> [f32; 2] {
    let x = uv[0] * xf.scale[0];
    let y = uv[1] * xf.scale[1];
    let c = xf.rotation.cos();
    let s = xf.rotation.sin();
    [c * x + s * y + xf.offset[0], -s * x + c * y + xf.offset[1]]
}

fn texture_transform(doc: &GltfFile, prim: &Primitive) -> Option<UvXform> {
    let mi = prim.material?;
    let mat = doc.materials.get(mi)?;
    const PATHS: [&str; 4] = [
        "/pbrMetallicRoughness/baseColorTexture/extensions/KHR_texture_transform",
        "/normalTexture/extensions/KHR_texture_transform",
        "/pbrMetallicRoughness/metallicRoughnessTexture/extensions/KHR_texture_transform",
        "/emissiveTexture/extensions/KHR_texture_transform",
    ];
    let node = PATHS.iter().find_map(|p| mat.pointer(p))?;
    let tex_coord = node
        .pointer("/texCoord")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if tex_coord != 0 {
        return None;
    }
    let offset = json_f32x2(node, "/offset").unwrap_or([0.0, 0.0]);
    let scale = json_f32x2(node, "/scale").unwrap_or([1.0, 1.0]);
    let rotation = node
        .pointer("/rotation")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;
    Some(UvXform {
        offset,
        rotation,
        scale,
    })
}

fn json_f32x2(node: &serde_json::Value, pointer: &str) -> Option<[f32; 2]> {
    let arr = node.pointer(pointer)?.as_array()?;
    if arr.len() < 2 {
        return None;
    }
    Some([
        arr[0].as_f64().unwrap_or(0.0) as f32,
        arr[1].as_f64().unwrap_or(0.0) as f32,
    ])
}

/// Lengyel tangents from the UVs that sample the normal map. Handedness is glTF `w`.
fn generate_tangents(verts: &mut [Vertex3], indices: &[u32]) {
    let mut tan = vec![Vec3::ZERO; verts.len()];
    let mut bit = vec![Vec3::ZERO; verts.len()];
    for tri in indices.as_chunks::<3>().0 {
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if i0 >= verts.len() || i1 >= verts.len() || i2 >= verts.len() {
            continue;
        }
        let p0 = Vec3::from_array(verts[i0].pos);
        let p1 = Vec3::from_array(verts[i1].pos);
        let p2 = Vec3::from_array(verts[i2].pos);
        let w0 = Vec2::from(verts[i0].uv);
        let w1 = Vec2::from(verts[i1].uv);
        let w2 = Vec2::from(verts[i2].uv);
        let e1 = p1 - p0;
        let e2 = p2 - p0;
        let x1 = w1.x - w0.x;
        let x2 = w2.x - w0.x;
        let y1 = w1.y - w0.y;
        let y2 = w2.y - w0.y;
        let det = x1 * y2 - x2 * y1;
        if det.abs() < 1e-12 {
            continue;
        }
        let r = 1.0 / det;
        let t = (e1 * y2 - e2 * y1) * r;
        let b = (e2 * x1 - e1 * x2) * r;
        tan[i0] += t;
        tan[i1] += t;
        tan[i2] += t;
        bit[i0] += b;
        bit[i1] += b;
        bit[i2] += b;
    }
    for (i, v) in verts.iter_mut().enumerate() {
        let n = Vec3::from_array(v.normal).normalize_or(Vec3::Y);
        let mut t = tan[i] - n * n.dot(tan[i]);
        if t.length_squared() < 1e-12 {
            v.tangent = [0.0, 0.0, 0.0, 1.0];
            continue;
        }
        t = t.normalize();
        let w = if n.cross(t).dot(bit[i]) < 0.0 {
            -1.0
        } else {
            1.0
        };
        v.tangent = [t.x, t.y, t.z, w];
    }
}

fn material_state(doc: &GltfFile, prim: &Primitive) -> (f32, bool, TexSampler) {
    let Some(mi) = prim.material else {
        return (0.0, false, TexSampler::default());
    };
    let Some(mat) = doc.materials.get(mi) else {
        return (0.0, false, TexSampler::default());
    };
    let mode = mat
        .pointer("/alphaMode")
        .and_then(|v| v.as_str())
        .unwrap_or("OPAQUE");
    let alpha_cut = match mode {
        "MASK" => {
            let cut = mat
                .pointer("/alphaCutoff")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5) as f32;
            cut.max(1.0e-4)
        }
        "BLEND" => -2.0,
        _ => -1.0,
    };
    let double_sided = mat
        .pointer("/doubleSided")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    (alpha_cut, double_sided, sampler_for_material(doc, prim))
}

fn sampler_for_material(doc: &GltfFile, prim: &Primitive) -> TexSampler {
    let tex = base_color_tex_index(doc, prim)
        .or_else(|| orm_tex_index(doc, prim))
        .or_else(|| normal_tex_index(doc, prim))
        .or_else(|| emissive_tex_index(doc, prim));
    let Some(ti) = tex else {
        return TexSampler::default();
    };
    let Some(texture) = doc.textures.get(ti) else {
        return TexSampler::gltf_default();
    };
    let Some(si) = texture.sampler else {
        return TexSampler::gltf_default();
    };
    let Some(s) = doc.samplers.get(si) else {
        return TexSampler::gltf_default();
    };
    let code = s.min_filter.unwrap_or(9987);
    let (min_nearest, mip_nearest) = match code {
        9728 | 9984 => (true, true),
        9986 => (true, false),
        9729 | 9985 => (false, true),
        _ => (false, false),
    };
    TexSampler {
        wrap_u: wrap_mode(s.wrap_s.unwrap_or(10497)),
        wrap_v: wrap_mode(s.wrap_t.unwrap_or(10497)),
        mag_nearest: s.mag_filter.unwrap_or(9729) == 9728,
        min_nearest,
        mip_nearest,
        anisotropy: 4,
    }
}

fn wrap_mode(code: u32) -> TexWrap {
    match code {
        33648 => TexWrap::Mirror,
        33071 => TexWrap::Clamp,
        _ => TexWrap::Repeat,
    }
}

fn skinned_from_doc(doc: &GltfFile, blobs: &[Vec<u8>]) -> Result<SkinnedMesh, String> {
    if doc.meshes.is_empty() || doc.meshes[0].primitives.is_empty() {
        return Err("gltf has no mesh primitives".into());
    }
    skinned_from_prim(doc, blobs, &doc.meshes[0].primitives[0], 0)
}

fn skinned_parts_from_doc(doc: &GltfFile, blobs: &[Vec<u8>]) -> Result<Vec<SkinnedMesh>, String> {
    let mut parts = Vec::new();
    let mut last_err = None;
    let has_any_joints = doc
        .meshes
        .iter()
        .flat_map(|m| m.primitives.iter())
        .any(|p| p.attributes.joints.is_some());
    for (mesh_idx, mesh) in doc.meshes.iter().enumerate() {
        for prim in &mesh.primitives {
            if has_any_joints && prim.attributes.joints.is_none() {
                continue;
            }
            match skinned_from_prim(doc, blobs, prim, mesh_idx) {
                Ok(s) => parts.push(s),
                Err(e) => last_err = Some(e),
            }
        }
    }
    if parts.is_empty() {
        return Err(last_err.unwrap_or_else(|| "gltf has no mesh primitives".into()));
    }
    let clip = parts.first().and_then(|p| p.clip.clone());
    if let Some(clip) = clip {
        for p in parts.iter_mut().skip(1) {
            if p.clip.is_none() {
                p.clip = Some(clip.clone());
            }
        }
    }
    Ok(parts)
}

fn skinned_from_prim(
    doc: &GltfFile,
    blobs: &[Vec<u8>],
    prim: &Primitive,
    mesh_index: usize,
) -> Result<SkinnedMesh, String> {
    let rest = static_mesh_from_prim(doc, blobs, prim)?;
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
    let clips = all_clips(doc, blobs)?;
    let mut skin = SkinnedMesh {
        rest,
        joints,
        weights,
        inverse_bind,
        nodes,
        skin_joints,
        clip,
        clips,
        mesh_index,
        humanoid: parse_humanoid(doc),
        springs: {
            let children: Vec<Vec<usize>> = doc.nodes.iter().map(|n| n.children.clone()).collect();
            crate::spring::parse_spring_bones(doc.extensions.as_ref(), &children)
        },
        morphs: load_morphs(doc, blobs, prim, nverts)?,
        expressions: {
            let e = crate::morph::parse_expressions(doc.extensions.as_ref());
            crate::morph::with_default_names(e, prim.targets.len())
        },
        look_at: crate::lookat::parse_look_at(doc.extensions.as_ref()),
        constraints: crate::constraint::parse_from_node_extensions(
            &doc.nodes
                .iter()
                .map(|n| n.extensions.clone())
                .collect::<Vec<_>>(),
        ),
        first_person: crate::first_person::parse_mesh_annotations(doc.extensions.as_ref()),
    };
    crate::mixamo::bind_locomotion(&mut skin);
    // 袖ボーンが無い VRM にヘルパーを足す（スキニング・布は既存経路が扱う）
    crate::sleeve::ensure_sleeve_cloth(&mut skin);
    Ok(skin)
}

fn load_morphs(
    doc: &GltfFile,
    blobs: &[Vec<u8>],
    prim: &Primitive,
    nverts: usize,
) -> Result<Vec<Vec<Vec3>>, String> {
    let mut morphs = Vec::with_capacity(prim.targets.len());
    for (i, tgt) in prim.targets.iter().enumerate() {
        let Some(acc) = tgt.position else {
            morphs.push(vec![Vec3::ZERO; nverts]);
            continue;
        };
        let d = read_f32x3(doc, blobs, acc)?;
        if d.len() != nverts {
            return Err(format!(
                "morph target {i} POSITION count mismatch: {} != {nverts}",
                d.len()
            ));
        }
        morphs.push(d);
    }
    Ok(morphs)
}

fn morphed_rest(skin: &SkinnedMesh, expression: &str, morph: f32) -> Vec<Vec3> {
    let mut pos: Vec<Vec3> = skin
        .rest
        .vertices
        .iter()
        .map(|v| Vec3::from_array(v.pos))
        .collect();
    if morph.abs() < 1e-8 {
        return pos;
    }
    // 名前付き表情（smile / angry / ...）か、自動（blink → aa → 最初）。
    let binds: Vec<crate::morph::MorphBind> =
        if !expression.is_empty() && !expression.eq_ignore_ascii_case("blink") {
            skin.expressions
                .get(expression)
                .cloned()
                .unwrap_or_default()
        } else {
            match skin.expressions.pick() {
                Some(name) => skin.expressions.get(name).cloned().unwrap_or_default(),
                None if !skin.morphs.is_empty() => vec![crate::morph::MorphBind {
                    index: 0,
                    weight: 1.0,
                }],
                None => return pos,
            }
        };
    for b in binds {
        let Some(deltas) = skin.morphs.get(b.index) else {
            continue;
        };
        let w = morph * b.weight;
        let n = pos.len().min(deltas.len());
        for i in 0..n {
            pos[i] += w * deltas[i];
        }
    }
    pos
}

fn node_rest(n: &GltfNode) -> NodeRest {
    if let Some(m) = n.matrix {
        let mat = Mat4::from_cols_array(&m);
        let (scale, rotation, translation) = mat.to_scale_rotation_translation();
        return NodeRest {
            name: n.name.clone(),
            children: n.children.clone(),
            translation,
            rotation,
            scale,
        };
    }
    NodeRest {
        name: n.name.clone(),
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
        .or_else(|| {
            doc.animations
                .iter()
                .find(|a| a.name.to_ascii_lowercase().starts_with("walking"))
        })
        .or_else(|| {
            doc.animations
                .iter()
                .find(|a| a.name.to_ascii_lowercase().starts_with("running"))
        })
        .or_else(|| {
            doc.animations
                .iter()
                .find(|a| a.name.eq_ignore_ascii_case("run"))
        })
        .unwrap_or(&doc.animations[0]);
    load_clip(doc, blobs, anim)
}

/// Every animation that has at least one TRS channel, in file order.
fn all_clips(doc: &GltfFile, blobs: &[Vec<u8>]) -> Result<Vec<AnimClip>, String> {
    let mut out = Vec::new();
    for (i, anim) in doc.animations.iter().enumerate() {
        if let Some(mut c) = load_clip(doc, blobs, anim)? {
            if anim.name.is_empty() {
                c.name = format!("clip{i}");
            }
            out.push(c);
        }
    }
    Ok(out)
}

fn load_clip(
    doc: &GltfFile,
    blobs: &[Vec<u8>],
    anim: &GltfAnimation,
) -> Result<Option<AnimClip>, String> {
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

fn rest_locals(skin: &SkinnedMesh) -> Vec<NodeLocal> {
    skin.nodes
        .iter()
        .map(|n| NodeLocal {
            translation: n.translation,
            rotation: n.rotation,
            scale: n.scale,
        })
        .collect()
}

/// rest と clip のローカル TRS を係数でブレンド（回転は slerp）。
fn blend_locals(rest: &[NodeLocal], clip: &[NodeLocal], blend: f32) -> Vec<NodeLocal> {
    let n = rest.len().min(clip.len());
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(NodeLocal {
            translation: rest[i].translation.lerp(clip[i].translation, blend),
            rotation: rest[i].rotation.slerp(clip[i].rotation, blend),
            scale: rest[i].scale.lerp(clip[i].scale, blend),
        });
    }
    out
}

fn apply_look(skin: &SkinnedMesh, locals: &mut [NodeLocal], yaw: f32, pitch: f32) {
    if yaw.abs() < 1e-8 && pitch.abs() < 1e-8 {
        return;
    }
    let Some(head) = crate::lookat::head_node(&skin.humanoid) else {
        return;
    };
    if head >= locals.len() {
        return;
    }
    let worlds = crate::mixamo::rest_world_rotations(&skin.nodes);
    let meta = skin.look_at.clone().unwrap_or_default();
    let (hy, hp) = crate::lookat::clamp_head(&meta, yaw, pitch);
    let mut head_yaw = hy;
    if let Some(neck) = crate::lookat::neck_node(&skin.humanoid) {
        if neck < locals.len() {
            let ny = hy * 0.4;
            head_yaw = hy - ny;
            apply_look_bone(locals, &worlds, neck, Quat::from_rotation_y(ny));
        }
    }
    apply_look_bone(
        locals,
        &worlds,
        head,
        crate::lookat::look_quat(head_yaw, hp),
    );
    let (ey, ep) = crate::lookat::map_eyes(&meta, yaw, pitch);
    if ey.abs() > 1e-8 || ep.abs() > 1e-8 {
        let eq = crate::lookat::look_quat(ey, ep);
        for i in crate::lookat::eye_nodes(&skin.humanoid)
            .into_iter()
            .flatten()
        {
            if i < locals.len() {
                apply_look_bone(locals, &worlds, i, eq);
            }
        }
    }
}

fn apply_look_bone(locals: &mut [NodeLocal], worlds: &[Quat], i: usize, delta_src: Quat) {
    let wd = worlds.get(i).copied().unwrap_or(Quat::IDENTITY);
    locals[i].rotation = crate::lookat::compensated_local(locals[i].rotation, wd, delta_src);
}

/// Rest-pose world translation of the head bone (plus lookAt offset).
pub fn head_world_pos(skin: &SkinnedMesh) -> Vec3 {
    let Some(head) = crate::lookat::head_node(&skin.humanoid) else {
        return Vec3::new(0.0, 1.2, 0.0);
    };
    let n = skin.nodes.len();
    let mut parent = vec![None; n];
    for (i, node) in skin.nodes.iter().enumerate() {
        for &c in &node.children {
            if c < n {
                parent[c] = Some(i);
            }
        }
    }
    let mut chain = Vec::new();
    let mut cur = Some(head);
    while let Some(i) = cur {
        chain.push(i);
        cur = parent[i];
    }
    let mut pos = Vec3::ZERO;
    let mut rot = Quat::IDENTITY;
    for &i in chain.iter().rev() {
        pos += rot * skin.nodes[i].translation;
        rot *= skin.nodes[i].rotation;
    }
    let off = skin
        .look_at
        .as_ref()
        .map(|l| l.offset_from_head_bone)
        .unwrap_or([0.0, 0.06, 0.0]);
    pos + rot * Vec3::from_array(off)
}

fn apply_hair(skin: &SkinnedMesh, locals: &mut [NodeLocal], hair: f32) {
    if hair.abs() < 1e-8 {
        return;
    }
    let Some(node) = crate::spring::hair_node(&skin.springs) else {
        return;
    };
    if node >= locals.len() {
        return;
    }
    locals[node].rotation *= Quat::from_axis_angle(Vec3::Z, hair);
}

/// 上半身ジェスチャー（overlay）: ノード名（humanoid 名 or node 名）→ 目標ローカル
/// 回転を `overlay_weight` でスラープする。腕等が歩きクリップに乗って揺れる
/// （Phase 2: 上半身/下半身レイヤー分離）。
fn apply_overlay(skin: &SkinnedMesh, locals: &mut [NodeLocal], pose: &WalkerPose) {
    let w = pose.overlay_weight.clamp(0.0, 1.0);
    if w < 1e-5 || pose.overlay_bones.is_empty() {
        return;
    }
    for (name, target) in &pose.overlay_bones {
        let node = skin
            .humanoid
            .get(name)
            .copied()
            .or_else(|| skin.nodes.iter().position(|n| n.name == *name));
        let Some(node) = node else {
            continue;
        };
        if node >= locals.len() {
            continue;
        }
        let target_q = Quat::from_array(*target);
        let cur = locals[node].rotation;
        locals[node].rotation = cur.slerp(target_q, w);
    }
}

/// Step Verlet after pose. Returns dump-visible hair yaw.
pub fn step_springs(
    skin: &SkinnedMesh,
    sim: &mut crate::spring::SpringState,
    t: Option<f32>,
    dt: f32,
) -> f32 {
    if sim.chains.is_empty() {
        *sim = skin.springs.clone();
    }
    if sim.chains.is_empty() {
        return 0.0;
    }
    let locals = match t {
        Some(tt) => sample_locals(skin, tt),
        None => rest_locals(skin),
    };
    let world = global_pose(&skin.nodes, &locals);
    crate::spring::step(sim, &world, dt);
    crate::spring::hair_yaw(sim)
}

fn sample_locals(skin: &SkinnedMesh, t: f32) -> Vec<NodeLocal> {
    sample_clip_locals(skin, skin.clip.as_ref(), t)
}

/// Clip by name (case-insensitive) among `clips`, else the default `clip`
/// when the name matches it. `None` when nothing matches.
pub fn find_clip<'a>(skin: &'a SkinnedMesh, name: &str) -> Option<&'a AnimClip> {
    skin.clips
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(name))
        .or_else(|| {
            skin.clip
                .as_ref()
                .filter(|c| c.name.eq_ignore_ascii_case(name))
        })
}

/// Clip names a game can pass to `clip_name` / `AnimMixer::play`.
pub fn clip_names(skin: &SkinnedMesh) -> Vec<String> {
    skin.clips.iter().map(|c| c.name.clone()).collect()
}

/// Named clip, or the default clip when `name` is `None` / unknown.
fn resolve_clip<'a>(skin: &'a SkinnedMesh, name: Option<&str>) -> Option<&'a AnimClip> {
    name.and_then(|n| find_clip(skin, n)).or(skin.clip.as_ref())
}

fn sample_clip_locals(skin: &SkinnedMesh, clip: Option<&AnimClip>, t: f32) -> Vec<NodeLocal> {
    let mut locals: Vec<NodeLocal> = skin
        .nodes
        .iter()
        .map(|n| NodeLocal {
            translation: n.translation,
            rotation: n.rotation,
            scale: n.scale,
        })
        .collect();
    let Some(clip) = clip else {
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
    apply_constraints(skin, &mut locals);
    locals
}

/// VRMC_node_constraint: copy source rotation / roll onto the dest bone.
fn apply_constraints(skin: &SkinnedMesh, locals: &mut [NodeLocal]) {
    if skin.constraints.is_empty() {
        return;
    }
    for c in &skin.constraints {
        let dest = c.dest;
        if dest >= locals.len() {
            continue;
        }
        let dst_rest = skin
            .nodes
            .get(dest)
            .map(|n| n.rotation)
            .unwrap_or(Quat::IDENTITY);
        match c.kind {
            crate::constraint::ConstraintKind::Rotation { source, weight } => {
                if source >= locals.len() {
                    continue;
                }
                let src_rest = skin
                    .nodes
                    .get(source)
                    .map(|n| n.rotation)
                    .unwrap_or(Quat::IDENTITY);
                locals[dest].rotation = crate::constraint::apply_rotation(
                    locals[source].rotation,
                    src_rest,
                    dst_rest,
                    weight,
                );
            }
            crate::constraint::ConstraintKind::Roll {
                source,
                weight,
                axis,
            } => {
                if source >= locals.len() {
                    continue;
                }
                let src_rest = skin
                    .nodes
                    .get(source)
                    .map(|n| n.rotation)
                    .unwrap_or(Quat::IDENTITY);
                locals[dest].rotation = crate::constraint::apply_roll(
                    locals[source].rotation,
                    src_rest,
                    dst_rest,
                    Vec3::from_array(axis),
                    weight,
                );
            }
            crate::constraint::ConstraintKind::Aim { .. } => {}
        }
    }
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

/// Node の親リスト（children から導出）。
pub(crate) fn node_parents(nodes: &[NodeRest]) -> Vec<Option<usize>> {
    let n = nodes.len();
    let mut parent = vec![None; n];
    for (i, node) in nodes.iter().enumerate() {
        for &c in &node.children {
            if c < n {
                parent[c] = Some(i);
            }
        }
    }
    parent
}

fn global_pose(nodes: &[NodeRest], locals: &[NodeLocal]) -> Vec<Mat4> {
    let n = nodes.len();
    let parent = node_parents(nodes);
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

const GLB_MAGIC: u32 = 0x4654_6C67;
const GLB_JSON: u32 = 0x4E4F_534A;
const GLB_BIN: u32 = 0x004E_4942;

fn split_glb(bytes: &[u8]) -> Result<(String, Option<Vec<u8>>), String> {
    if bytes.len() < 12 {
        return Err("glb too short".into());
    }
    let magic = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    if magic != GLB_MAGIC {
        return Err("not a glTF binary (missing glTF magic)".into());
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if version != 2 {
        return Err(format!("unsupported glb version {version}"));
    }
    let declared = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let end = declared.min(bytes.len());
    let mut off = 12;
    let mut json = None;
    let mut bin = None;
    while off + 8 <= end {
        let chunk_len = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) as usize;
        let chunk_type = u32::from_le_bytes(bytes[off + 4..off + 8].try_into().unwrap());
        off += 8;
        let chunk_end = off.saturating_add(chunk_len);
        if chunk_end > bytes.len() {
            return Err("glb chunk truncated".into());
        }
        let data = &bytes[off..chunk_end];
        match chunk_type {
            GLB_JSON => {
                let t = std::str::from_utf8(data).map_err(|e| e.to_string())?;
                json = Some(t.trim_end().to_string());
            }
            GLB_BIN => bin = Some(data.to_vec()),
            _ => {}
        }
        off = chunk_end;
    }
    let json = json.ok_or_else(|| "glb has no JSON chunk".to_string())?;
    Ok((json, bin))
}

fn parse_humanoid(doc: &GltfFile) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    if let Some(ext) = &doc.extensions {
        if let Some(obj) = ext
            .pointer("/VRMC_vrm/humanoid/humanBones")
            .and_then(|v| v.as_object())
        {
            for (name, entry) in obj {
                if let Some(node) = entry.get("node").and_then(|n| n.as_u64()) {
                    map.insert(name.clone(), node as usize);
                }
            }
        }
        if let Some(arr) = ext
            .pointer("/VRM/humanoid/humanBones")
            .and_then(|v| v.as_array())
        {
            for entry in arr {
                let name = entry.get("bone").and_then(|b| b.as_str()).unwrap_or("");
                if name.is_empty() {
                    continue;
                }
                if let Some(node) = entry.get("node").and_then(|n| n.as_u64()) {
                    map.entry(name.to_string()).or_insert(node as usize);
                }
            }
        }
    }
    if let Some(extra) = &doc.extras {
        if let Some(arr) = extra
            .pointer("/VRM/humanoid/humanBones")
            .and_then(|v| v.as_array())
        {
            for entry in arr {
                let name = entry.get("bone").and_then(|b| b.as_str()).unwrap_or("");
                if name.is_empty() {
                    continue;
                }
                if let Some(node) = entry.get("node").and_then(|n| n.as_u64()) {
                    map.entry(name.to_string()).or_insert(node as usize);
                }
            }
        }
    }
    for (i, n) in doc.nodes.iter().enumerate() {
        let name = n.name.trim();
        if name.is_empty() {
            continue;
        }
        map.entry(name.to_string()).or_insert(i);
        let mut chars = name.chars();
        if let Some(c) = chars.next() {
            let camel = format!("{}{}", c.to_ascii_lowercase(), chars.as_str());
            map.entry(camel).or_insert(i);
        }
    }
    map
}

fn f32_arr3(v: Option<&serde_json::Value>, def: [f32; 3]) -> [f32; 3] {
    v.and_then(|a| a.as_array())
        .map(|a| {
            [
                a.first().and_then(|x| x.as_f64()).unwrap_or(def[0] as f64) as f32,
                a.get(1).and_then(|x| x.as_f64()).unwrap_or(def[1] as f64) as f32,
                a.get(2).and_then(|x| x.as_f64()).unwrap_or(def[2] as f64) as f32,
            ]
        })
        .unwrap_or(def)
}

fn f32_val(v: Option<&serde_json::Value>, def: f32) -> f32 {
    v.and_then(|x| x.as_f64()).map(|x| x as f32).unwrap_or(def)
}

/// VRM 1 VRMC_materials_mtoon / VRM 0 materialProperties shade + rim + outline.
/// Port of kagra-core mtoon.rs (no GPU, no matcap/normal/uv-anim textures).
fn load_mtoon(doc: &GltfFile, prim: &Primitive) -> Option<MtoonShade> {
    let mut shade = MtoonShade::default();
    let mut found = false;

    if let Some(mi) = prim.material {
        if let Some(mat) = doc.materials.get(mi) {
            if let Some(mtoon) = mat
                .pointer("/extensions/VRMC_materials_mtoon")
                .or_else(|| mat.pointer("/extensions/VRM/materials_mtoon"))
            {
                found = true;
                shade.shade_color = f32_arr3(mtoon.get("shadeColorFactor"), shade.shade_color);
                shade.shading_toony =
                    f32_val(mtoon.get("shadingToonyFactor"), shade.shading_toony).clamp(0.0, 0.999);
                shade.shading_shift = f32_val(mtoon.get("shadingShiftFactor"), shade.shading_shift);
                shade.rim_color = f32_arr3(mtoon.get("parametricRimColorFactor"), shade.rim_color);
                shade.rim_power = f32_val(
                    mtoon.get("parametricRimFresnelPowerFactor"),
                    shade.rim_power,
                )
                .max(0.1);
                shade.rim_lift = f32_val(mtoon.get("parametricRimLiftFactor"), shade.rim_lift);
                shade.outline_color =
                    f32_arr3(mtoon.get("outlineColorFactor"), shade.outline_color);
                let ow = f32_val(mtoon.get("outlineWidthFactor"), 0.0);
                let mode = mtoon
                    .get("outlineWidthMode")
                    .and_then(|m| m.as_str())
                    .unwrap_or("none");
                shade.outline_width = match mode {
                    "worldCoordinates" => ow,
                    "screenCoordinates" => ow * 0.01,
                    _ => 0.0,
                };
                if let Some(ti) = mtoon
                    .pointer("/matcapTexture/index")
                    .and_then(|v| v.as_u64())
                {
                    shade.has_matcap = (ti as usize) < doc.textures.len();
                }
                if let Some(ti) = mat.pointer("/normalTexture/index").and_then(|v| v.as_u64()) {
                    shade.has_normal = (ti as usize) < doc.textures.len();
                }
            }
        }
    }

    let props = doc
        .extensions
        .as_ref()
        .and_then(|e| e.pointer("/VRM/materialProperties"))
        .and_then(|v| v.as_array());
    if let Some(props) = props {
        let entry = prim
            .material
            .and_then(|i| props.get(i))
            .or_else(|| props.first());
        if let Some(prop) = entry {
            let shader = prop.get("shader").and_then(|s| s.as_str()).unwrap_or("");
            if shader.to_ascii_lowercase().contains("mtoon") {
                found = true;
            }
            if let Some(arr) = prop
                .pointer("/vectorProperties/_ShadeColor")
                .or_else(|| prop.pointer("/vectorProperties/ShadeColor"))
                .and_then(|a| a.as_array())
            {
                found = true;
                shade.shade_color = [
                    arr.first().and_then(|x| x.as_f64()).unwrap_or(0.55) as f32,
                    arr.get(1).and_then(|x| x.as_f64()).unwrap_or(0.50) as f32,
                    arr.get(2).and_then(|x| x.as_f64()).unwrap_or(0.52) as f32,
                ];
            }
            if let Some(v) = prop
                .pointer("/floatProperties/_ShadeToony")
                .or_else(|| prop.pointer("/floatProperties/_ShadingToony"))
                .or_else(|| prop.pointer("/floatProperties/ShadeToony"))
                .and_then(|x| x.as_f64())
            {
                found = true;
                shade.shading_toony = (v as f32).clamp(0.0, 0.999);
            }
            if let Some(v) = prop
                .pointer("/floatProperties/_ShadeShift")
                .or_else(|| prop.pointer("/floatProperties/ShadeShift"))
                .and_then(|x| x.as_f64())
            {
                shade.shading_shift = v as f32;
            }
            if let Some(arr) = prop
                .pointer("/vectorProperties/_RimColor")
                .or_else(|| prop.pointer("/vectorProperties/RimColor"))
                .and_then(|a| a.as_array())
            {
                shade.rim_color = [
                    arr.first().and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
                    arr.get(1).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
                    arr.get(2).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
                ];
            }
            if let Some(v) = prop
                .pointer("/floatProperties/_RimFresnelPower")
                .or_else(|| prop.pointer("/floatProperties/RimFresnelPower"))
                .and_then(|x| x.as_f64())
            {
                shade.rim_power = (v as f32).max(0.1);
            }
            if let Some(v) = prop
                .pointer("/floatProperties/_RimLift")
                .or_else(|| prop.pointer("/floatProperties/RimLift"))
                .and_then(|x| x.as_f64())
            {
                shade.rim_lift = v as f32;
            }
            if let Some(arr) = prop
                .pointer("/vectorProperties/_OutlineColor")
                .or_else(|| prop.pointer("/vectorProperties/OutlineColor"))
                .and_then(|a| a.as_array())
            {
                shade.outline_color = [
                    arr.first().and_then(|x| x.as_f64()).unwrap_or(0.05) as f32,
                    arr.get(1).and_then(|x| x.as_f64()).unwrap_or(0.05) as f32,
                    arr.get(2).and_then(|x| x.as_f64()).unwrap_or(0.08) as f32,
                ];
            }
            if let Some(ow) = prop
                .pointer("/floatProperties/_OutlineWidth")
                .or_else(|| prop.pointer("/floatProperties/OutlineWidth"))
                .and_then(|x| x.as_f64())
            {
                let mode = prop
                    .pointer("/floatProperties/_OutlineWidthMode")
                    .or_else(|| prop.pointer("/floatProperties/OutlineWidthMode"))
                    .and_then(|x| x.as_f64())
                    .unwrap_or(1.0);
                if mode > 0.5 {
                    shade.outline_width = ((ow as f32) * 0.01).clamp(0.002, 0.04);
                }
            }
            if prop
                .pointer("/textureProperties/_SphereAdd")
                .or_else(|| prop.pointer("/textureProperties/_MatcapTexture"))
                .or_else(|| prop.pointer("/textureProperties/SphereAdd"))
                .and_then(|v| v.as_u64())
                .is_some()
            {
                shade.has_matcap = true;
            }
            if prop
                .pointer("/textureProperties/_BumpMap")
                .or_else(|| prop.pointer("/textureProperties/BumpMap"))
                .and_then(|v| v.as_u64())
                .is_some()
            {
                shade.has_normal = true;
            }
        }
    }

    found.then_some(shade)
}

fn orm_tex_index(doc: &GltfFile, prim: &Primitive) -> Option<usize> {
    let mi = prim.material?;
    let mat = doc.materials.get(mi)?;
    mat.pointer("/pbrMetallicRoughness/metallicRoughnessTexture/index")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
}

fn orm_factors(doc: &GltfFile, prim: &Primitive) -> (f32, f32) {
    let Some(mi) = prim.material else {
        return (1.0, 1.0);
    };
    let Some(mat) = doc.materials.get(mi) else {
        return (1.0, 1.0);
    };
    let metal = mat
        .pointer("/pbrMetallicRoughness/metallicFactor")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0) as f32;
    let rough = mat
        .pointer("/pbrMetallicRoughness/roughnessFactor")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0) as f32;
    (metal.clamp(0.0, 1.0), rough.clamp(0.0, 1.0))
}

/// Linear ORM. An authored factor with no texture emits a 1×1 so it beats
/// the dump scalars. Omitted factors stay `None`. The written pair metal 1 /
/// rough 1 also stays scalar: that is the glTF default, and Kenney stores it
/// on every base-color material. Sampling a 1×1 of it replaces the dump's
/// metallic / roughness. A real `metallicRoughnessTexture` is still sampled.
fn load_orm(doc: &GltfFile, blobs: &[Vec<u8>], prim: &Primitive) -> Option<AlbedoRgba> {
    let mi = prim.material?;
    let mat = doc.materials.get(mi)?;
    let (metal, rough) = orm_factors(doc, prim);
    let authored = mat
        .pointer("/pbrMetallicRoughness/metallicFactor")
        .is_some()
        || mat
            .pointer("/pbrMetallicRoughness/roughnessFactor")
            .is_some();
    let Some(tex) = orm_tex_index(doc, prim) else {
        if !authored {
            return None;
        }
        // metal 1 / rough 1 is the spec default, even when both keys are written.
        if (metal - 1.0).abs() < 1e-3 && (rough - 1.0).abs() < 1e-3 {
            return None;
        }
        return Some(solid_rgba(
            1,
            1,
            [255, scale_unorm(255, rough), scale_unorm(255, metal), 255],
        ));
    };
    let mut img = texture_by_index(doc, blobs, tex)?;
    if (metal - 1.0).abs() > 1e-4 || (rough - 1.0).abs() > 1e-4 {
        let mut px = img.rgba.to_vec();
        for chunk in px.as_chunks_mut::<4>().0 {
            chunk[1] = scale_unorm(chunk[1], rough);
            chunk[2] = scale_unorm(chunk[2], metal);
        }
        img.rgba = Arc::from(px);
    }
    Some(img)
}

fn scale_unorm(byte: u8, factor: f32) -> u8 {
    (((byte as f32 / 255.0) * factor).clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Identity `baseColorFactor` plus a KTX2 image. A non-identity factor
/// stays on the RGBA path, because block data cannot be multiplied in place.
fn load_base_color_ktx(
    doc: &GltfFile,
    blobs: &[Vec<u8>],
    prim: &Primitive,
) -> Option<crate::ktx_image::KtxImage> {
    let factor = base_color_factor(doc, prim);
    let identity = (factor[0] - 1.0).abs() < 1e-4
        && (factor[1] - 1.0).abs() < 1e-4
        && (factor[2] - 1.0).abs() < 1e-4
        && (factor[3] - 1.0).abs() < 1e-4;
    if !identity {
        return None;
    }
    let tex = base_color_tex_index(doc, prim)?;
    let src = doc.textures.get(tex)?.source?;
    let img = doc.images.get(src)?;
    let bytes = image_bytes(doc, blobs, img).ok()?;
    if !crate::ktx_image::is_ktx2(&bytes) {
        return None;
    }
    crate::ktx_image::parse_ktx2(&bytes).ok()
}

fn load_base_color(doc: &GltfFile, blobs: &[Vec<u8>], prim: &Primitive) -> Option<AlbedoRgba> {
    let factor = base_color_factor(doc, prim);
    let baked = base_color_tex_index(doc, prim).and_then(|tex| {
        let src = doc.textures.get(tex)?.source?;
        let img = doc.images.get(src)?;
        let bytes = image_bytes(doc, blobs, img).ok()?;
        let mut image = decode_raster(&bytes).ok()?;
        bake_srgb_factor(&mut image, factor);
        Some(image)
    });
    if baked.is_some() {
        return baked;
    }
    let identity = (factor[0] - 1.0).abs() < 1e-4
        && (factor[1] - 1.0).abs() < 1e-4
        && (factor[2] - 1.0).abs() < 1e-4
        && (factor[3] - 1.0).abs() < 1e-4;
    if identity {
        return None;
    }
    Some(solid_rgba(
        1,
        1,
        [
            linear_to_srgb_u8(factor[0]),
            linear_to_srgb_u8(factor[1]),
            linear_to_srgb_u8(factor[2]),
            scale_unorm(255, factor[3]),
        ],
    ))
}

fn base_color_factor(doc: &GltfFile, prim: &Primitive) -> [f32; 4] {
    let Some(mi) = prim.material else {
        return [1.0, 1.0, 1.0, 1.0];
    };
    let Some(mat) = doc.materials.get(mi) else {
        return [1.0, 1.0, 1.0, 1.0];
    };
    let Some(arr) = mat
        .pointer("/pbrMetallicRoughness/baseColorFactor")
        .and_then(|v| v.as_array())
    else {
        return [1.0, 1.0, 1.0, 1.0];
    };
    if arr.len() < 3 {
        return [1.0, 1.0, 1.0, 1.0];
    }
    [
        arr[0].as_f64().unwrap_or(1.0).clamp(0.0, 1.0) as f32,
        arr[1].as_f64().unwrap_or(1.0).clamp(0.0, 1.0) as f32,
        arr[2].as_f64().unwrap_or(1.0).clamp(0.0, 1.0) as f32,
        arr.get(3)
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0)
            .clamp(0.0, 1.0) as f32,
    ]
}

fn bake_srgb_factor(img: &mut AlbedoRgba, factor: [f32; 4]) {
    if (factor[0] - 1.0).abs() < 1e-4
        && (factor[1] - 1.0).abs() < 1e-4
        && (factor[2] - 1.0).abs() < 1e-4
        && (factor[3] - 1.0).abs() < 1e-4
    {
        return;
    }
    let mut px = img.rgba.to_vec();
    for c in px.as_chunks_mut::<4>().0 {
        c[0] = linear_to_srgb_u8(srgb_u8_to_linear(c[0]) * factor[0]);
        c[1] = linear_to_srgb_u8(srgb_u8_to_linear(c[1]) * factor[1]);
        c[2] = linear_to_srgb_u8(srgb_u8_to_linear(c[2]) * factor[2]);
        c[3] = scale_unorm(c[3], factor[3]);
    }
    img.rgba = Arc::from(px);
}

fn load_clearcoat(doc: &GltfFile, prim: &Primitive) -> (f32, f32) {
    let Some(mi) = prim.material else {
        return (0.0, 0.0);
    };
    let Some(mat) = doc.materials.get(mi) else {
        return (0.0, 0.0);
    };
    let Some(node) = mat.pointer("/extensions/KHR_materials_clearcoat") else {
        return (0.0, 0.0);
    };
    let factor = node
        .pointer("/clearcoatFactor")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;
    let rough = node
        .pointer("/clearcoatRoughnessFactor")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;
    (factor.clamp(0.0, 1.0), rough.clamp(0.0, 1.0))
}

fn load_emissive(
    doc: &GltfFile,
    blobs: &[Vec<u8>],
    prim: &Primitive,
) -> (Option<AlbedoRgba>, f32, [f32; 3]) {
    let Some(mi) = prim.material else {
        return (None, 0.0, [0.0; 3]);
    };
    let Some(mat) = doc.materials.get(mi) else {
        return (None, 0.0, [0.0; 3]);
    };
    let factor = mat
        .pointer("/emissiveFactor")
        .and_then(|v| v.as_array())
        .map(|arr| {
            [
                arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0) as f32,
                arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0) as f32,
                arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0) as f32,
            ]
        })
        .unwrap_or([0.0; 3]);
    let strength = mat
        .pointer("/extensions/KHR_materials_emissive_strength/emissiveStrength")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0)
        .max(0.0) as f32;
    let map = emissive_tex_index(doc, prim).and_then(|ti| texture_by_index(doc, blobs, ti));
    if let Some(mut img) = map {
        bake_srgb_factor(&mut img, [factor[0], factor[1], factor[2], 1.0]);
        return (Some(img), strength, [1.0, 1.0, 1.0]);
    }
    if factor == [0.0, 0.0, 0.0] {
        return (None, 0.0, [0.0; 3]);
    }
    (
        Some(solid_rgba(
            1,
            1,
            [
                linear_to_srgb_u8(factor[0].clamp(0.0, 1.0)),
                linear_to_srgb_u8(factor[1].clamp(0.0, 1.0)),
                linear_to_srgb_u8(factor[2].clamp(0.0, 1.0)),
                255,
            ],
        )),
        strength,
        [1.0, 1.0, 1.0],
    )
}

fn emissive_tex_index(doc: &GltfFile, prim: &Primitive) -> Option<usize> {
    let mi = prim.material?;
    let mat = doc.materials.get(mi)?;
    mat.pointer("/emissiveTexture/index")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
}

fn solid_rgba(width: u32, height: u32, px: [u8; 4]) -> AlbedoRgba {
    let n = (width.max(1) * height.max(1)) as usize;
    let mut rgba = Vec::with_capacity(n * 4);
    for _ in 0..n {
        rgba.extend_from_slice(&px);
    }
    AlbedoRgba {
        width: width.max(1),
        height: height.max(1),
        rgba: Arc::from(rgba),
    }
}

fn srgb_u8_to_linear(byte: u8) -> f32 {
    let e = byte as f32 / 255.0;
    if e <= 0.04045 {
        e / 12.92
    } else {
        ((e + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb_u8(linear: f32) -> u8 {
    let c = if linear.is_finite() {
        linear.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let e = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (e * 255.0).round().clamp(0.0, 255.0) as u8
}

fn base_color_tex_index(doc: &GltfFile, prim: &Primitive) -> Option<usize> {
    if let Some(mi) = prim.material {
        if let Some(mat) = doc.materials.get(mi) {
            if let Some(idx) = mat
                .pointer("/pbrMetallicRoughness/baseColorTexture/index")
                .and_then(|v| v.as_u64())
            {
                return Some(idx as usize);
            }
        }
    }
    let mats = doc
        .extensions
        .as_ref()
        .and_then(|e| e.pointer("/VRM/materialProperties"))
        .and_then(|v| v.as_array())?;
    let entry = prim
        .material
        .and_then(|i| mats.get(i))
        .or_else(|| mats.first())?;
    entry
        .pointer("/textureProperties/_MainTex")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
}

/// Decode a texture by glTF texture index (data URI or bufferView).
fn texture_by_index(doc: &GltfFile, blobs: &[Vec<u8>], tex: usize) -> Option<AlbedoRgba> {
    let src = doc.textures.get(tex)?.source?;
    let img = doc.images.get(src)?;
    let bytes = image_bytes(doc, blobs, img).ok()?;
    decode_raster(&bytes).ok()
}

/// PNG stays on the `png` crate. JPEG and WebP go through `image` with
/// default features off (`zune-jpeg` and `image-webp`, no rayon).
fn decode_raster(bytes: &[u8]) -> Result<AlbedoRgba, String> {
    if bytes.starts_with(b"\x89PNG") {
        return decode_png(bytes);
    }
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return decode_jpeg(bytes);
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return decode_webp(bytes);
    }
    decode_png(bytes)
}

fn decode_jpeg(bytes: &[u8]) -> Result<AlbedoRgba, String> {
    decode_image_format(bytes, image::ImageFormat::Jpeg)
}

fn decode_webp(bytes: &[u8]) -> Result<AlbedoRgba, String> {
    decode_image_format(bytes, image::ImageFormat::WebP)
}

fn decode_image_format(bytes: &[u8], format: image::ImageFormat) -> Result<AlbedoRgba, String> {
    let img = image::load_from_memory_with_format(bytes, format).map_err(|e| e.to_string())?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(AlbedoRgba {
        width,
        height,
        rgba: Arc::from(rgba.into_raw()),
    })
}

/// MToon matcap texture index: VRM 1 matcapTexture / VRM 0 _SphereAdd.
fn matcap_tex_index(doc: &GltfFile, prim: &Primitive) -> Option<usize> {
    if let Some(mi) = prim.material {
        if let Some(mat) = doc.materials.get(mi) {
            if let Some(idx) = mat
                .pointer("/extensions/VRMC_materials_mtoon/matcapTexture/index")
                .and_then(|v| v.as_u64())
            {
                return Some(idx as usize);
            }
        }
    }
    let mats = doc
        .extensions
        .as_ref()
        .and_then(|e| e.pointer("/VRM/materialProperties"))
        .and_then(|v| v.as_array())?;
    let entry = prim
        .material
        .and_then(|i| mats.get(i))
        .or_else(|| mats.first())?;
    entry
        .pointer("/textureProperties/_SphereAdd")
        .or_else(|| entry.pointer("/textureProperties/_MatcapTexture"))
        .or_else(|| entry.pointer("/textureProperties/SphereAdd"))
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
}

/// glTF `normalTexture.scale`. Missing or non-finite = `None` (1.0).
fn normal_tex_scale(doc: &GltfFile, prim: &Primitive) -> Option<f32> {
    let mat = doc.materials.get(prim.material?)?;
    let s = mat.pointer("/normalTexture/scale")?.as_f64()? as f32;
    s.is_finite().then_some(s)
}

/// Normal map texture index: glTF normalTexture / VRM 0 _BumpMap.
fn normal_tex_index(doc: &GltfFile, prim: &Primitive) -> Option<usize> {
    if let Some(mi) = prim.material {
        if let Some(mat) = doc.materials.get(mi) {
            if let Some(idx) = mat.pointer("/normalTexture/index").and_then(|v| v.as_u64()) {
                return Some(idx as usize);
            }
        }
    }
    let mats = doc
        .extensions
        .as_ref()
        .and_then(|e| e.pointer("/VRM/materialProperties"))
        .and_then(|v| v.as_array())?;
    let entry = prim
        .material
        .and_then(|i| mats.get(i))
        .or_else(|| mats.first())?;
    entry
        .pointer("/textureProperties/_BumpMap")
        .or_else(|| entry.pointer("/textureProperties/BumpMap"))
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
}

fn image_bytes(doc: &GltfFile, blobs: &[Vec<u8>], img: &GltfImage) -> Result<Vec<u8>, String> {
    if let Some(uri) = img.uri.as_deref() {
        if uri.starts_with("data:") {
            return decode_data_uri(uri);
        }
        return Err("external image uri not supported".into());
    }
    let view_idx = img
        .buffer_view
        .ok_or_else(|| "image has no uri or bufferView".to_string())?;
    let view = doc
        .buffer_views
        .get(view_idx)
        .ok_or_else(|| format!("missing image bufferView {view_idx}"))?;
    let blob = blobs
        .get(view.buffer)
        .ok_or_else(|| format!("missing buffer {}", view.buffer))?;
    blob.get(view.byte_offset..view.byte_offset + view.byte_length)
        .map(|s| s.to_vec())
        .ok_or_else(|| "image bufferView out of range".into())
}

pub(crate) fn decode_png(bytes: &[u8]) -> Result<AlbedoRgba, String> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let w = info.width;
    let h = info.height;
    let src = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => src.to_vec(),
        png::ColorType::Rgb => {
            let mut o = Vec::with_capacity((w * h * 4) as usize);
            for c in src.as_chunks::<3>().0 {
                o.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
            o
        }
        png::ColorType::Grayscale => src.iter().flat_map(|&g| [g, g, g, 255]).collect::<Vec<_>>(),
        png::ColorType::GrayscaleAlpha => {
            let mut o = Vec::with_capacity((w * h * 4) as usize);
            for c in src.as_chunks::<2>().0 {
                o.extend_from_slice(&[c[0], c[0], c[0], c[1]]);
            }
            o
        }
        png::ColorType::Indexed => return Err("indexed PNG albedo not supported".into()),
    };
    if rgba.len() != (w as usize) * (h as usize) * 4 {
        return Err("png size mismatch".into());
    }
    Ok(AlbedoRgba {
        width: w,
        height: h,
        rgba: Arc::from(rgba),
    })
}

fn read_colors(
    doc: &GltfFile,
    blobs: &[Vec<u8>],
    accessor: usize,
) -> Result<Vec<[f32; 4]>, String> {
    let acc = doc
        .accessors
        .get(accessor)
        .ok_or_else(|| format!("missing accessor {accessor}"))?;
    let ncomp = match acc.type_name.as_str() {
        "VEC3" => 3,
        "VEC4" => 4,
        other => return Err(format!("COLOR_0 must be VEC3 or VEC4, got {other}")),
    };
    if acc.component_type == 5126 {
        let f = read_f32_components(doc, blobs, accessor, ncomp)?;
        let mut out = Vec::with_capacity(acc.count);
        for chunk in f.chunks(ncomp) {
            let a = if ncomp == 4 { chunk[3] } else { 1.0 };
            out.push([chunk[0], chunk[1], chunk[2], a]);
        }
        return Ok(out);
    }
    let denom = match acc.component_type {
        5121 => 255.0,
        5123 => 65535.0,
        other => return Err(format!("unsupported COLOR_0 componentType {other}")),
    };
    let elem = if acc.component_type == 5121 { 1 } else { 2 };
    let view_idx = acc
        .buffer_view
        .ok_or_else(|| "accessor has no bufferView".to_string())?;
    let view = &doc.buffer_views[view_idx];
    let blob = blobs
        .get(view.buffer)
        .ok_or_else(|| format!("missing buffer {}", view.buffer))?;
    let start = view.byte_offset + acc.byte_offset;
    let stride = view.byte_stride.unwrap_or(elem * ncomp);
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let off = start + i * stride;
        let s = blob
            .get(off..off + elem * ncomp)
            .ok_or_else(|| "COLOR_0 out of range".to_string())?;
        let mut c = [0.0f32; 4];
        for k in 0..ncomp {
            let raw = if elem == 1 {
                s[k] as f32
            } else {
                u16::from_le_bytes(s[k * 2..k * 2 + 2].try_into().unwrap()) as f32
            };
            c[k] = if acc.normalized { raw / denom } else { raw };
        }
        if ncomp == 3 {
            c[3] = 1.0;
        }
        out.push(c);
    }
    Ok(out)
}

fn read_tangents(
    doc: &GltfFile,
    blobs: &[Vec<u8>],
    accessor: usize,
) -> Result<Vec<[f32; 4]>, String> {
    let acc = doc
        .accessors
        .get(accessor)
        .ok_or_else(|| format!("missing accessor {accessor}"))?;
    if acc.type_name != "VEC4" || acc.component_type != 5126 {
        return Err("TANGENT must be FLOAT VEC4".into());
    }
    let f = read_f32_components(doc, blobs, accessor, 4)?;
    Ok(f.as_chunks::<4>().0.to_vec())
}

fn read_uv(doc: &GltfFile, blobs: &[Vec<u8>], accessor: usize) -> Result<Vec<[f32; 2]>, String> {
    let acc = doc
        .accessors
        .get(accessor)
        .ok_or_else(|| format!("missing accessor {accessor}"))?;
    if acc.type_name != "VEC2" {
        return Err("TEXCOORD_0 must be VEC2".into());
    }
    match acc.component_type {
        5126 => {
            let f = read_f32_components(doc, blobs, accessor, 2)?;
            Ok(f.as_chunks::<2>().0.to_vec())
        }
        5123 => read_norm_uv(doc, blobs, accessor, 2, 65535.0),
        5121 => read_norm_uv(doc, blobs, accessor, 1, 255.0),
        other => Err(format!("unsupported TEXCOORD_0 componentType {other}")),
    }
}

fn read_norm_uv(
    doc: &GltfFile,
    blobs: &[Vec<u8>],
    accessor: usize,
    elem: usize,
    denom: f32,
) -> Result<Vec<[f32; 2]>, String> {
    let acc = &doc.accessors[accessor];
    let view_idx = acc
        .buffer_view
        .ok_or_else(|| "accessor has no bufferView".to_string())?;
    let view = &doc.buffer_views[view_idx];
    let blob = blobs
        .get(view.buffer)
        .ok_or_else(|| format!("missing buffer {}", view.buffer))?;
    let start = view.byte_offset + acc.byte_offset;
    let stride = view.byte_stride.unwrap_or(elem * 2);
    let mut out = Vec::with_capacity(acc.count);
    for i in 0..acc.count {
        let off = start + i * stride;
        let s = blob
            .get(off..off + elem * 2)
            .ok_or_else(|| "TEXCOORD_0 out of range".to_string())?;
        let (u, v) = if elem == 2 {
            (
                u16::from_le_bytes(s[0..2].try_into().unwrap()) as f32 / denom,
                u16::from_le_bytes(s[2..4].try_into().unwrap()) as f32 / denom,
            )
        } else {
            (s[0] as f32 / denom, s[1] as f32 / denom)
        };
        out.push([u, v]);
    }
    Ok(out)
}

fn read_f32x3(doc: &GltfFile, blobs: &[Vec<u8>], accessor: usize) -> Result<Vec<Vec3>, String> {
    let floats = read_f32_components(doc, blobs, accessor, 3)?;
    Ok(floats
        .as_chunks::<3>()
        .0
        .iter()
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
        .as_chunks::<16>()
        .0
        .iter()
        .map(Mat4::from_cols_array)
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
            Ok(f.as_chunks::<4>().0.to_vec())
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

    /// 1×1 lossless WebP (Pillow). The albedo texel is the image, not a fallback.
    #[test]
    fn webp_base_color_decodes() {
        let webp = "UklGRh4AAABXRUJQVlA4TBEAAAAvAAAAAAdQhXLXo/+BiOh/AAA=";
        let cube = unit_cube_gltf();
        let with = cube.replacen("\"mode\": 4", "\"mode\": 4, \"material\": 0", 1);
        let trimmed = with.trim_end().trim_end_matches('}');
        let json = format!(
            "{trimmed}, \"materials\": [{{\"pbrMetallicRoughness\": {{\"baseColorTexture\": {{\"index\": 0}}}}}}], \"textures\": [{{\"source\": 0}}], \"images\": [{{\"mimeType\": \"image/webp\", \"uri\": \"data:image/webp;base64,{webp}\"}}]}}"
        );
        let mesh = mesh_from_gltf_json(&json, |_| Err("external".into())).expect("webp mesh");
        let img = mesh.albedo.expect("webp albedo");
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        assert!(
            img.rgba[0] > 200 && img.rgba[1] < 30 && img.rgba[2] < 50,
            "webp texel {:?}",
            &img.rgba[..4]
        );
    }

    fn ktx2_data_uri() -> String {
        let bytes = crate::ktx_image::write_uncompressed_ktx2(
            ktx2::Format::BC1_RGBA_SRGB_BLOCK,
            4,
            4,
            &[0x00, 0xF8, 0x00, 0x00, 0, 0, 0, 0],
        )
        .expect("bc1");
        base64_encode(&bytes)
    }

    fn cube_with_base_color(factor: &str, b64: &str) -> String {
        let cube = unit_cube_gltf();
        let with = cube.replacen("\"mode\": 4", "\"mode\": 4, \"material\": 0", 1);
        let trimmed = with.trim_end().trim_end_matches('}');
        format!(
            "{trimmed}, \"materials\": [{{\"pbrMetallicRoughness\": {{\"baseColorFactor\": {factor}, \"baseColorTexture\": {{\"index\": 0}}}}}}], \"textures\": [{{\"source\": 0}}], \"images\": [{{\"mimeType\": \"image/ktx2\", \"uri\": \"data:image/ktx2;base64,{b64}\"}}]}}"
        )
    }

    /// Identity factor keeps the block bytes. They are not decoded to RGBA.
    #[test]
    fn ktx2_base_color_stays_compressed() {
        let json = cube_with_base_color("[1, 1, 1, 1]", &ktx2_data_uri());
        let mesh = mesh_from_gltf_json(&json, |_| Err("external".into())).expect("ktx2 mesh");
        assert!(mesh.albedo.is_none(), "ktx2 is not an RGBA albedo");
        let ktx = mesh.albedo_ktx.expect("compressed base color");
        assert_eq!(ktx.vk_format, ktx2::Format::BC1_RGBA_SRGB_BLOCK.value());
        assert_eq!((ktx.width, ktx.height), (4, 4));
        assert_eq!(ktx.level0.len(), 8);
    }

    /// A tint cannot multiply block bytes, so the 1×1 factor path stays.
    #[test]
    fn ktx2_non_identity_factor_stays_rgba() {
        let json = cube_with_base_color("[1, 0, 0, 1]", &ktx2_data_uri());
        let mesh = mesh_from_gltf_json(&json, |_| Err("external".into())).expect("tinted");
        assert!(mesh.albedo_ktx.is_none());
        let img = mesh.albedo.expect("1x1 factor");
        assert_eq!((img.width, img.height), (1, 1));
        assert_eq!(&img.rgba[..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn clearcoat_extension_lands_on_the_mesh() {
        let cube = unit_cube_gltf();
        let mut with = cube.replacen("\"mode\": 4", "\"mode\": 4, \"material\": 0", 1);
        let trimmed = with.trim_end().trim_end_matches('}').to_string();
        with = format!(
            "{trimmed}, \"materials\": [{{\"extensions\": {{\"KHR_materials_clearcoat\": {{\"clearcoatFactor\": 0.75, \"clearcoatRoughnessFactor\": 0.2}}}}}}]}}"
        );
        let mesh = mesh_from_gltf_json(&with, |_| Err("external".into())).expect("coat");
        assert!((mesh.clearcoat - 0.75).abs() < 1e-4);
        assert!((mesh.clearcoat_roughness - 0.2).abs() < 1e-4);
        let plain = mesh_from_gltf_json(&cube, |_| Err("external".into())).expect("plain");
        assert_eq!(plain.clearcoat, 0.0);
    }

    #[test]
    fn metallic_roughness_texture_lands_on_orm() {
        let cube = unit_cube_gltf();
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGPguqPxHwAEEAIOusqDKwAAAABJRU5ErkJggg==";
        let mut with = cube.replacen("\"mode\": 4", "\"mode\": 4, \"material\": 0", 1);
        assert!(with.ends_with('}') || with.trim_end().ends_with('}'));
        let trimmed = with.trim_end().trim_end_matches('}').to_string();
        with = format!(
            "{trimmed}, \"materials\": [{{\"pbrMetallicRoughness\": {{\"metallicFactor\": 0.5, \"roughnessFactor\": 1.0, \"metallicRoughnessTexture\": {{\"index\": 0}}}}}}], \"textures\": [{{\"source\": 0}}], \"images\": [{{\"uri\": \"data:image/png;base64,{png}\"}}]}}"
        );
        let mesh = mesh_from_gltf_json(&with, |_| Err("external".into())).expect("orm mesh");
        assert!(mesh.orm_rough && mesh.orm_metal);
        let img = mesh.orm.expect("orm image");
        assert!(img.rgba[1] > 180, "roughness stays high, G={}", img.rgba[1]);
        assert!(
            img.rgba[2] < 80,
            "metallicFactor 0.5 halves the metal texel, B={}",
            img.rgba[2]
        );
        let plain = mesh_from_gltf_json(&cube, |_| Err("external".into())).expect("plain");
        assert!(
            plain.orm.is_none(),
            "a cube without a material stays scalar"
        );
    }

    #[test]
    fn pbr_factors_alpha_uv_color_and_sampler() {
        let json = triangle_pbr_gltf();
        let mesh = mesh_from_gltf_json(&json, |_| Err("external".into())).expect("pbr");
        let expected_r = linear_to_srgb_u8(0.5);
        let alb = mesh.albedo.expect("factor baked to 1x1");
        assert_eq!(alb.rgba[0], expected_r, "linear 0.5 encodes near 188");
        assert_eq!(alb.rgba[1], 0);
        assert!(expected_r > 180 && expected_r < 200);
        let orm = mesh.orm.expect("factor-only orm");
        assert_eq!(orm.rgba[2], 0, "metallicFactor 0");
        assert!(
            (orm.rgba[1] as i32 - 64).abs() <= 1,
            "roughness 0.25 → ~64, got {}",
            orm.rgba[1]
        );
        assert!((mesh.emissive - 3.0).abs() < 1e-4);
        assert_eq!(mesh.emissive_color, [1.0, 1.0, 1.0]);
        let em = mesh.emissive_map.expect("emissive factor");
        assert_eq!(em.rgba[1], 255);
        assert!((mesh.alpha_cut - 0.3).abs() < 1e-4);
        assert!(mesh.double_sided);
        assert_eq!(mesh.sampler.wrap_u, TexWrap::Repeat);
        assert!(mesh.sampler.mag_nearest);
        let uv = mesh.vertices[0].uv;
        assert!(
            (uv[0] - 0.25).abs() < 1e-4,
            "scale 2 * 0 + offset 0.25, got {uv:?}"
        );
        assert!((uv[1] - 0.5).abs() < 1e-4, "uv {uv:?}");
        let col = mesh.vertices[0].color;
        assert!(
            (col[0] - 128.0 / 255.0).abs() < 1e-4,
            "COLOR_0 is linear, got {col:?}"
        );
        assert!(mesh.vertices[0].tangent[0].abs() + mesh.vertices[0].tangent[1].abs() > 0.1);
        assert_eq!(mesh.normal_scale, Some(0.5), "normalTexture.scale");
        let opaque =
            mesh_from_gltf_json(&triangle_alpha("OPAQUE", 0.5), |_| Err("x".into())).unwrap();
        assert!((opaque.alpha_cut + 1.0).abs() < 1e-4);
        let blend =
            mesh_from_gltf_json(&triangle_alpha("BLEND", 0.5), |_| Err("x".into())).unwrap();
        assert!(blend.alpha_cut <= -1.5);
    }

    fn triangle_alpha(mode: &str, cutoff: f32) -> String {
        let mut json = triangle_pbr_gltf();
        json = json.replacen("MASK", mode, 1);
        json.replacen("0.3", &format!("{cutoff}"), 1)
    }

    fn triangle_pbr_gltf() -> String {
        // 3 verts. POSITION, NORMAL, TEXCOORD_0, COLOR_0 (u8 normalized), indices.
        let mut bin = Vec::new();
        let pos = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let nrm = [0.0f32, 0.0, 1.0];
        let uvs = [[0.0f32, 0.0], [1.0, 0.0], [0.0, 1.0]];
        for p in pos {
            for c in p {
                bin.extend_from_slice(&c.to_le_bytes());
            }
        }
        let nrm_off = bin.len();
        for _ in 0..3 {
            for c in nrm {
                bin.extend_from_slice(&c.to_le_bytes());
            }
        }
        let uv_off = bin.len();
        for uv in uvs {
            bin.extend_from_slice(&uv[0].to_le_bytes());
            bin.extend_from_slice(&uv[1].to_le_bytes());
        }
        let col_off = bin.len();
        bin.extend_from_slice(&[128, 255, 0, 255, 128, 255, 0, 255, 128, 255, 0, 255]);
        let idx_off = bin.len();
        bin.extend_from_slice(&0u16.to_le_bytes());
        bin.extend_from_slice(&1u16.to_le_bytes());
        bin.extend_from_slice(&2u16.to_le_bytes());
        let b64 = base64_encode(&bin);
        format!(
            r#"{{
  "asset": {{"version": "2.0"}},
  "buffers": [{{"byteLength": {blen}, "uri": "data:application/octet-stream;base64,{b64}"}}],
  "bufferViews": [
    {{"buffer": 0, "byteOffset": 0, "byteLength": {nrm_off}}},
    {{"buffer": 0, "byteOffset": {nrm_off}, "byteLength": {uv_len}}},
    {{"buffer": 0, "byteOffset": {uv_off}, "byteLength": 24}},
    {{"buffer": 0, "byteOffset": {col_off}, "byteLength": 12}},
    {{"buffer": 0, "byteOffset": {idx_off}, "byteLength": 6}}
  ],
  "accessors": [
    {{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3"}},
    {{"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"}},
    {{"bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC2"}},
    {{"bufferView": 3, "componentType": 5121, "count": 3, "type": "VEC4", "normalized": true}},
    {{"bufferView": 4, "componentType": 5123, "count": 3, "type": "SCALAR"}}
  ],
  "samplers": [{{"magFilter": 9728, "minFilter": 9986, "wrapS": 10497, "wrapT": 33071}}],
  "materials": [{{
    "pbrMetallicRoughness": {{
      "baseColorFactor": [0.5, 0.0, 0.0, 1.0],
      "metallicFactor": 0.0,
      "roughnessFactor": 0.25
    }},
    "emissiveFactor": [0.0, 1.0, 0.0],
    "alphaMode": "MASK",
    "alphaCutoff": 0.3,
    "doubleSided": true,
    "extensions": {{"KHR_materials_emissive_strength": {{"emissiveStrength": 3.0}}}},
    "normalTexture": {{
      "index": 0,
      "scale": 0.5,
      "extensions": {{"KHR_texture_transform": {{"offset": [0.25, 0.5], "scale": [2.0, 1.0]}}}}
    }}
  }}],
  "textures": [{{"source": 0, "sampler": 0}}],
  "images": [{{"uri": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGPguqPxHwAEEAIOusqDKwAAAABJRU5ErkJggg=="}}],
  "meshes": [{{"primitives": [{{"attributes": {{"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2, "COLOR_0": 3}}, "indices": 4, "mode": 4, "material": 0}}]}}]
}}"#,
            blen = bin.len(),
            uv_len = uv_off - nrm_off,
        )
    }

    #[test]
    fn authored_metal_rough_factors_become_a_scalar_orm() {
        let cube = unit_cube_gltf();
        let mut defaults = cube.replacen("\"mode\": 4", "\"mode\": 4, \"material\": 0", 1);
        let trimmed = defaults.trim_end().trim_end_matches('}').to_string();
        defaults = format!(
            "{trimmed}, \"materials\": [{{\"pbrMetallicRoughness\": {{\"metallicFactor\": 1.0, \"roughnessFactor\": 1.0}}}}]}}"
        );
        let mesh = mesh_from_gltf_json(&defaults, |_| Err("external".into())).expect("defaults");
        assert!(
            mesh.orm.is_none(),
            "metal 1 / rough 1 is the glTF default and must not override the dump"
        );
        let mut with = cube.replacen("\"mode\": 4", "\"mode\": 4, \"material\": 0", 1);
        let trimmed = with.trim_end().trim_end_matches('}').to_string();
        with = format!(
            "{trimmed}, \"materials\": [{{\"pbrMetallicRoughness\": {{\"metallicFactor\": 0.25, \"roughnessFactor\": 0.40}}}}]}}"
        );
        let mesh = mesh_from_gltf_json(&with, |_| Err("external".into())).expect("factor orm");
        assert!(mesh.orm_rough && mesh.orm_metal);
        let img = mesh.orm.expect("orm");
        assert_eq!(img.rgba[1], 102, "roughnessFactor 0.40");
        assert_eq!(img.rgba[2], 64, "metallicFactor 0.25");
    }

    #[test]
    fn kenney_speeder_keeps_base_color_and_skips_default_metal() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../assets/models/kenney/space-kit/craft_speederC.glb"
        );
        let bytes = std::fs::read(path).expect("speeder glb");
        let mesh = mesh_from_glb(&bytes).expect("mesh");
        assert!(
            mesh.orm.is_none(),
            "Kenney metal 1 / rough 1 must leave lighting to the instance"
        );
        let alb = mesh.albedo.expect("baseColorFactor atlas");
        let mut spread = 0i32;
        for px in alb.rgba.chunks(4) {
            spread = spread.max((px[0] as i32 - px[2] as i32).abs());
        }
        assert!(
            spread > 40,
            "palette should separate red metal from dark hull, spread {spread}"
        );
    }

    #[test]
    fn anim_blend_interpolates_rest_to_clip() {
        let bytes = walk_skinned_vrm();
        let skin = skinned_from_glb(&bytes).expect("vrm");
        let rest = sample_skinned_look_blend(&skin, Some(0.25), 0.0, "blink", 0.0, 0.0, 0.0, 0.0);
        let full = sample_skinned_look_blend(&skin, Some(0.25), 0.0, "blink", 0.0, 0.0, 0.0, 1.0);
        let half = sample_skinned_look_blend(&skin, Some(0.25), 0.0, "blink", 0.0, 0.0, 0.0, 0.5);
        let mut d_full = 0.0f32;
        let mut d_half = 0.0f32;
        for i in 0..rest.vertices.len() {
            let a = Vec3::from_array(rest.vertices[i].pos);
            let b = Vec3::from_array(full.vertices[i].pos);
            let c = Vec3::from_array(half.vertices[i].pos);
            d_full = d_full.max((b - a).length());
            d_half = d_half.max((c - a).length());
        }
        assert!(d_full > 0.02, "blend 1 = clip pose, d_full={d_full}");
        assert!(
            d_half > d_full * 0.3 && d_half < d_full * 0.8,
            "blend 0.5 は rest と clip の間、d_half={d_half} d_full={d_full}"
        );
    }

    #[test]
    fn first_person_hides_third_person_and_auto_parts() {
        use crate::first_person::{FirstPerson, MeshAnnotation};
        let mut part = SkinnedMesh {
            rest: crate::scene3d::MeshData::default(),
            joints: vec![],
            weights: vec![],
            inverse_bind: vec![],
            nodes: vec![],
            skin_joints: vec![],
            clip: None,
            clips: vec![],
            mesh_index: 1,
            humanoid: Default::default(),
            springs: Default::default(),
            morphs: vec![],
            expressions: Default::default(),
            look_at: None,
            constraints: vec![],
            first_person: FirstPerson::default(),
        };
        // mesh 注釈: ThirdPersonOnly → 一人称で隠す
        part.first_person
            .by_mesh
            .insert(1, MeshAnnotation::ThirdPersonOnly);
        assert!(part_hidden_in_first_person(&part));
        // FirstPersonOnly → 残す
        part.first_person
            .by_mesh
            .insert(1, MeshAnnotation::FirstPersonOnly);
        assert!(!part_hidden_in_first_person(&part));
        // Auto → 隠す
        part.first_person.by_mesh.insert(1, MeshAnnotation::Auto);
        assert!(part_hidden_in_first_person(&part));
        // 注釈なし → Auto 扱いで隠す
        part.first_person = FirstPerson::default();
        assert!(part_hidden_in_first_person(&part));
        // node 注釈（VRM 1.0）: スキンジョイントのノードが ThirdPersonOnly
        part.skin_joints = vec![7];
        part.first_person
            .by_node
            .insert(7, MeshAnnotation::ThirdPersonOnly);
        assert!(part_hidden_in_first_person(&part));
        part.first_person
            .by_node
            .insert(7, MeshAnnotation::FirstPersonOnly);
        assert!(!part_hidden_in_first_person(&part));
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

    #[test]
    fn vrm_fixture_is_glb_with_skin_and_humanoid() {
        let bytes = walk_skinned_vrm();
        assert_eq!(&bytes[0..4], b"glTF");
        let skin = skinned_from_glb(&bytes).expect("vrm");
        assert_eq!(skin.rest.vertices.len(), 8);
        assert_eq!(skin.rest.indices.len(), 36);
        assert_eq!(skin.joints.len(), 8);
        assert_eq!(skin.weights.len(), 8);
        assert_eq!(skin.skin_joints.len(), 4);
        assert_eq!(skin.nodes.len(), 5);
        assert_eq!(skin.humanoid.get("hips").copied(), Some(0));
        assert_eq!(skin.humanoid.get("chest").copied(), Some(1));
        assert_eq!(skin.humanoid.get("head").copied(), Some(4));
        let la = skin.look_at.as_ref().expect("lookAt");
        assert_eq!(la.look_at_type, "bone");
        assert_eq!(skin.springs.chains.len(), 1);
        assert_eq!(skin.springs.chains[0].joints.len(), 2);
        assert_eq!(skin.springs.chains[0].joints[0].node, 2);
        assert_eq!(skin.springs.chains[0].joints[1].node, 3);
        let clip = skin.clip.as_ref().expect("Walk clip");
        assert!(clip.name.eq_ignore_ascii_case("walk"));
        let rest = sample_skinned(&skin, 0.0);
        let walk = sample_skinned(&skin, 0.25);
        let mut max_d = 0.0f32;
        for (a, b) in rest.vertices.iter().zip(walk.vertices.iter()) {
            max_d = max_d.max((Vec3::from_array(a.pos) - Vec3::from_array(b.pos)).length());
        }
        assert!(max_d > 0.05, "VRM Walk clip must move verts, max_d={max_d}");
    }

    #[test]
    fn vrm_without_clip_stays_bind_pose() {
        let mut skin = skinned_from_glb(&walk_skinned_vrm()).expect("vrm");
        skin.clip = None;
        let a = sample_skinned(&skin, 0.0);
        let b = sample_skinned(&skin, 0.5);
        for (va, vb) in a.vertices.iter().zip(b.vertices.iter()) {
            let d = (Vec3::from_array(va.pos) - Vec3::from_array(vb.pos)).length();
            assert!(d < 1e-4, "no clip => bind pose at any t");
        }
    }

    #[test]
    fn cloth_step_moves_skinned_verts() {
        let bytes = walk_skinned_vrm();
        let skin = skinned_from_glb(&bytes).expect("vrm");
        assert!(!skin.springs.is_empty());
        let mut sim = skin.springs.clone();
        // 初回は snap（布は動かない）
        let a = sample_skinned_cloth(
            &skin,
            None,
            0.0,
            "blink",
            0.0,
            0.0,
            0.0,
            0.0,
            &mut sim,
            1.0 / 60.0,
        );
        // 布の関節を手でずらす → Verlet → 回転デルタ → 頂点が変わる
        if let Some(c) = sim.chains.first_mut() {
            if c.joints.len() >= 2 {
                c.joints[1].curr = [0.4, 0.85, 0.0];
                c.joints[1].prev = [0.4, 0.85, 0.0];
            }
        }
        let b = sample_skinned_cloth(
            &skin,
            None,
            0.0,
            "blink",
            0.0,
            0.0,
            0.0,
            0.0,
            &mut sim,
            1.0 / 60.0,
        );
        let mut max_d = 0.0f32;
        for (va, vb) in a.vertices.iter().zip(b.vertices.iter()) {
            max_d = max_d.max((Vec3::from_array(va.pos) - Vec3::from_array(vb.pos)).length());
        }
        assert!(max_d > 1e-4, "cloth must move skinned verts, max_d={max_d}");
    }

    #[test]
    fn vrm_hair_yaw_moves_skinned_verts() {
        let bytes = walk_skinned_vrm();
        let skin = skinned_from_glb(&bytes).expect("vrm");
        assert!(!skin.springs.is_empty());
        let rest = sample_skinned_hair(&skin, None, 0.0, "blink", 0.0);
        let sag = sample_skinned_hair(&skin, None, 0.35, "blink", 0.0);
        let mut max_d = 0.0f32;
        for (a, b) in rest.vertices.iter().zip(sag.vertices.iter()) {
            max_d = max_d.max((Vec3::from_array(a.pos) - Vec3::from_array(b.pos)).length());
        }
        assert!(
            max_d > 0.01,
            "hair yaw must move skinned verts before CPU skin, max_d={max_d}"
        );
        let mut sim = skin.springs.clone();
        let y0 = step_springs(&skin, &mut sim, None, 1.0 / 60.0);
        let mut y1 = y0;
        for _ in 0..24 {
            y1 = step_springs(&skin, &mut sim, None, 1.0 / 60.0);
        }
        assert!(
            (y1 - y0).abs() > 1e-4,
            "idle Verlet must change hair yaw, y0={y0} y1={y1}"
        );
    }

    #[test]
    fn walk_gltf_loads_uv_and_base_color() {
        let skin = skinned_from_embedded_gltf(&walk_skinned_gltf()).expect("skin");
        assert!(
            skin.rest.vertices.iter().any(|v| v.uv != [0.0, 0.0]),
            "fixture must carry TEXCOORD_0"
        );
        let alb = skin.rest.albedo.as_ref().expect("baseColor PNG");
        assert!(
            alb.width >= 2 && alb.height >= 2,
            "tiny PNG, got {}x{}",
            alb.width,
            alb.height
        );
        assert_eq!(alb.rgba.len(), (alb.width * alb.height * 4) as usize);
        let unique: std::collections::HashSet<_> =
            alb.rgba.as_chunks::<4>().0.iter().copied().collect();
        assert!(
            unique.len() >= 3,
            "albedo must not be a flat color, unique={}",
            unique.len()
        );
        let sampled = sample_skinned(&skin, 0.25);
        assert_eq!(sampled.vertices[0].uv, skin.rest.vertices[0].uv);
        assert!(sampled.albedo.is_some());
    }

    #[test]
    fn walk_vrm_loads_uv_and_maintex() {
        let skin = skinned_from_glb(&walk_skinned_vrm()).expect("vrm");
        assert!(skin.rest.vertices.iter().any(|v| v.uv != [0.0, 0.0]));
        let alb = skin
            .rest
            .albedo
            .as_ref()
            .expect("VRM0 _MainTex / baseColor");
        assert!(alb.width >= 2 && alb.height >= 2);
        assert_eq!(alb.rgba.len(), (alb.width * alb.height * 4) as usize);
    }

    #[test]
    fn walk_gltf_loads_mtoon_shade() {
        let skin = skinned_from_embedded_gltf(&walk_skinned_gltf()).expect("skin");
        let m = skin.rest.mtoon.expect("VRMC_materials_mtoon");
        assert!((m.shade_color[0] - 0.42).abs() < 1e-4);
        assert!((m.shade_color[1] - 0.28).abs() < 1e-4);
        assert!((m.shade_color[2] - 0.32).abs() < 1e-4);
        assert!((m.shading_toony - 0.88).abs() < 1e-4);
        let sampled = sample_skinned(&skin, 0.25);
        assert_eq!(sampled.mtoon, skin.rest.mtoon);
    }

    #[test]
    fn walk_vrm_loads_mtoon_shade() {
        let skin = skinned_from_glb(&walk_skinned_vrm()).expect("vrm");
        let m = skin.rest.mtoon.expect("VRM0 MToon shade");
        assert!((m.shade_color[0] - 0.42).abs() < 1e-4);
        assert!((m.shade_color[1] - 0.28).abs() < 1e-4);
        assert!((m.shade_color[2] - 0.32).abs() < 1e-4);
        assert!((m.shading_toony - 0.88).abs() < 1e-4);
    }

    #[test]
    fn vrm_parses_morph_targets_and_blink_aa() {
        let skin = skinned_from_glb(&walk_skinned_vrm()).expect("vrm");
        assert_eq!(skin.morphs.len(), 1);
        assert_eq!(skin.morphs[0].len(), 8);
        let max_d = skin.morphs[0]
            .iter()
            .map(|d| d.length())
            .fold(0.0f32, f32::max);
        assert!(
            max_d > 0.1,
            "fixture blink deltas must be non-zero, max_d={max_d}"
        );
        assert!(skin.expressions.by_name.contains_key("blink"));
        assert!(skin.expressions.by_name.contains_key("aa"));
        assert_eq!(skin.expressions.pick(), Some("blink"));
        let rest = sample_skinned_hair(&skin, None, 0.0, "blink", 0.0);
        let blink = sample_skinned_hair(&skin, None, 0.0, "blink", 1.0);
        let mut moved = 0.0f32;
        for (a, b) in rest.vertices.iter().zip(blink.vertices.iter()) {
            moved = moved.max((Vec3::from_array(a.pos) - Vec3::from_array(b.pos)).length());
        }
        assert!(
            moved > 0.05,
            "blink weight 1 must move CPU-skinned verts, moved={moved}"
        );
    }

    #[test]
    fn named_expression_aa_moves_morph_targets() {
        let skin = skinned_from_glb(&walk_skinned_vrm()).expect("vrm");
        assert!(skin.expressions.has("aa"), "walk fixture has the aa viseme");
        let rest = sample_skinned_hair(&skin, None, 0.0, "aa", 0.0);
        let aa = sample_skinned_hair(&skin, None, 0.0, "aa", 1.0);
        let mut moved = 0.0f32;
        for (a, b) in rest.vertices.iter().zip(aa.vertices.iter()) {
            moved = moved.max((Vec3::from_array(a.pos) - Vec3::from_array(b.pos)).length());
        }
        assert!(
            moved > 0.05,
            "aa weight 1 must move CPU-skinned verts, moved={moved}"
        );
        // モデルに無い表情は動かない（空 bind → 無害）。
        let missing = sample_skinned_hair(&skin, None, 0.0, "smile", 1.0);
        let mut drift = 0.0f32;
        for (a, b) in rest.vertices.iter().zip(missing.vertices.iter()) {
            drift = drift.max((Vec3::from_array(a.pos) - Vec3::from_array(b.pos)).length());
        }
        assert!(
            drift < 1e-5,
            "unknown expression must not move verts, drift={drift}"
        );
    }

    #[test]
    fn vrm_look_yaw_moves_skinned_verts() {
        let skin = skinned_from_glb(&walk_skinned_vrm()).expect("vrm");
        assert_eq!(crate::lookat::head_node(&skin.humanoid), Some(4));
        let rest = sample_skinned_look(&skin, None, 0.0, "blink", 0.0, 0.0, 0.0);
        let look = sample_skinned_look(&skin, None, 0.0, "blink", 0.0, 0.6, 0.2);
        let mut max_d = 0.0f32;
        for (a, b) in rest.vertices.iter().zip(look.vertices.iter()) {
            max_d = max_d.max((Vec3::from_array(a.pos) - Vec3::from_array(b.pos)).length());
        }
        assert!(
            max_d > 0.01,
            "head look yaw/pitch must move CPU-skinned verts, max_d={max_d}"
        );
    }

    #[test]
    fn emma_vrm_on_disk_loads_all_textured_parts() {
        let Some(path) = crate::assets::resolve_asset("assets/Emma.vrm") else {
            return;
        };
        let bytes = std::fs::read(&path).expect("read Emma.vrm");
        let parts = skinned_parts_from_glb(&bytes).expect("Emma.vrm parts");
        assert!(
            parts.len() >= 3,
            "VRoid Body+Face+Hair prims, got {}",
            parts.len()
        );
        let verts: usize = parts.iter().map(|p| p.rest.vertices.len()).sum();
        assert!(verts > 1000, "full Emma not first-prim-only, verts={verts}");
        assert!(
            parts.iter().any(|p| p.rest.albedo.is_some()),
            "baseColor must bind on at least one primitive"
        );
        assert!(
            parts.iter().any(|p| p.clip.is_some()),
            "clip-less VRM must bind Mixamo walk"
        );
        let mtoons: Vec<_> = parts.iter().filter_map(|p| p.rest.mtoon).collect();
        assert!(!mtoons.is_empty(), "Emma must carry MToon shade");
        assert!(
            parts.iter().any(|p| p.rest.matcap.is_some()),
            "VRoid hair authors matcap (SphereAdd)"
        );
        assert!(
            parts.iter().any(|p| p.rest.normal.is_some()),
            "VRoid authors normal maps"
        );
        assert!(
            mtoons
                .iter()
                .any(|m| m.rim_color.iter().sum::<f32>() > 0.05 || m.outline_width > 0.0),
            "Emma hair should author rim or outline: {:?}",
            mtoons
                .iter()
                .map(|m| (m.rim_color, m.outline_width))
                .collect::<Vec<_>>()
        );
        let skin = &parts[0];
        let rest = sample_skinned_look(skin, None, 0.0, "blink", 0.0, 0.0, 0.0);
        let walk = sample_skinned_look(skin, Some(0.25), 0.0, "blink", 0.0, 0.0, 0.0);
        let mut max_d = 0.0f32;
        for (a, b) in rest.vertices.iter().zip(walk.vertices.iter()) {
            max_d = max_d.max((Vec3::from_array(a.pos) - Vec3::from_array(b.pos)).length());
        }
        assert!(
            max_d > 0.01,
            "Mixamo walk must move Emma verts, max_d={max_d}"
        );
    }

    #[test]
    fn overlay_moves_upper_body_but_not_legs() {
        let Some(path) = crate::assets::resolve_asset("assets/Emma.vrm") else {
            return;
        };
        let bytes = std::fs::read(&path).expect("read Emma.vrm");
        let parts = skinned_parts_from_glb(&bytes).expect("Emma.vrm parts");
        let skin = &parts[0];
        let Some(_larm) = skin.humanoid.get("leftUpperArm") else {
            return;
        };
        // 腕チェーン（上腕→前腕→手）と脚チェーン（大腿→下腿→足）のノード集合。
        let chain = |names: &[&str]| -> Vec<usize> {
            names
                .iter()
                .filter_map(|n| skin.humanoid.get(*n).copied())
                .collect()
        };
        let arm_nodes = chain(&["leftUpperArm", "leftLowerArm", "leftHand"]);
        let leg_nodes = chain(&["leftUpperLeg", "leftLowerLeg", "leftFoot"]);
        assert!(
            !arm_nodes.is_empty() && !leg_nodes.is_empty(),
            "Emma humanoid must have arm+leg bones"
        );
        // 頂点の支配ジョイント（最大 weight）をノードへ変換。
        let dominant = |i: usize| -> usize {
            let js = skin.joints[i];
            let ws = skin.weights[i];
            let mut best = js[0] as usize;
            let mut bw = ws[0];
            for k in 1..4 {
                if ws[k] > bw {
                    bw = ws[k];
                    best = js[k] as usize;
                }
            }
            skin.skin_joints.get(best).copied().unwrap_or(usize::MAX)
        };
        let rest = sample_skinned_pose(skin, &WalkerPose::default());
        // 左腕をローカル Y 軸周りに大きく回す overlay（weight 1.0）。
        let mut pose = WalkerPose::default();
        pose.overlay_bones
            .insert("leftUpperArm".into(), Quat::from_rotation_y(2.2).to_array());
        pose.overlay_weight = 1.0;
        let moved = sample_skinned_pose(skin, &pose);
        let mut arm_d = 0.0f32;
        let mut leg_d = 0.0f32;
        for i in 0..rest.vertices.len() {
            let d = (Vec3::from_array(rest.vertices[i].pos)
                - Vec3::from_array(moved.vertices[i].pos))
            .length();
            let n = dominant(i);
            if arm_nodes.contains(&n) {
                arm_d = arm_d.max(d);
            }
            if leg_nodes.contains(&n) {
                leg_d = leg_d.max(d);
            }
        }
        assert!(
            arm_d > 0.01,
            "overlay must swing the upper arm, arm_d={arm_d}"
        );
        assert!(
            leg_d < 1e-4,
            "overlay must not move leg verts, leg_d={leg_d}"
        );
        // weight 0 では何も動かない。
        let mut zero = pose.clone();
        zero.overlay_weight = 0.0;
        let none = sample_skinned_pose(skin, &zero);
        for i in 0..rest.vertices.len() {
            let d = (Vec3::from_array(rest.vertices[i].pos)
                - Vec3::from_array(none.vertices[i].pos))
            .length();
            assert!(d < 1e-4, "weight 0 must be a no-op, d={d}");
        }
    }

    #[test]
    fn load_mtoon_parses_rim_and_outline_vrm1() {
        let doc: GltfFile = serde_json::from_value(serde_json::json!({
            "accessors": [], "bufferViews": [], "buffers": [], "meshes": [],
            "materials": [{
                "extensions": {
                    "VRMC_materials_mtoon": {
                        "shadeColorFactor": [0.2, 0.3, 0.4],
                        "shadingToonyFactor": 0.8,
                        "shadingShiftFactor": 0.1,
                        "parametricRimColorFactor": [1.0, 0.5, 0.2],
                        "parametricRimFresnelPowerFactor": 4.0,
                        "parametricRimLiftFactor": 0.2,
                        "outlineWidthMode": "worldCoordinates",
                        "outlineWidthFactor": 0.02,
                        "outlineColorFactor": [0.1, 0.0, 0.0]
                    }
                }
            }]
        }))
        .expect("parse gltf json");
        let prim = Primitive {
            attributes: Attributes {
                position: 0,
                normal: None,
                joints: None,
                weights: None,
                texcoord: None,
                color: None,
                tangent: None,
            },
            indices: None,
            mode: None,
            material: Some(0),
            targets: Vec::new(),
        };
        let shade = load_mtoon(&doc, &prim).expect("mtoon");
        assert_eq!(shade.shade_color, [0.2, 0.3, 0.4]);
        assert!((shade.shading_toony - 0.8).abs() < 1e-4);
        assert!((shade.shading_shift - 0.1).abs() < 1e-4);
        assert_eq!(shade.rim_color, [1.0, 0.5, 0.2]);
        assert!((shade.rim_power - 4.0).abs() < 1e-4);
        assert!((shade.rim_lift - 0.2).abs() < 1e-4);
        assert_eq!(shade.outline_color, [0.1, 0.0, 0.0]);
        assert!((shade.outline_width - 0.02).abs() < 1e-4);
        // GPU pack
        assert_eq!(shade.gpu()[..3], [0.2, 0.3, 0.4]);
        assert_eq!(shade.gpu_rim()[..3], [1.0, 0.5, 0.2]);
        assert!((shade.gpu_rim()[3] - 4.0).abs() < 1e-4);
        assert_eq!(shade.gpu_outline()[..3], [0.1, 0.0, 0.0]);
        assert!((shade.gpu_outline()[3] - 0.02).abs() < 1e-4);
        let s = shade.gpu_shift_lift();
        assert!((s[0] - 0.1).abs() < 1e-4);
        assert!((s[1] - 0.2).abs() < 1e-4);
    }

    #[test]
    fn load_mtoon_parses_vrm0_rim_outline() {
        let doc: GltfFile = serde_json::from_value(serde_json::json!({
            "accessors": [], "bufferViews": [], "buffers": [], "meshes": [],
            "materials": [{}],
            "extensions": {
                "VRM": {
                    "materialProperties": [{
                        "name": "Hair",
                        "shader": "VRM/MToon",
                        "vectorProperties": {
                            "_RimColor": [0.9, 0.3, 0.1],
                            "_OutlineColor": [0.1, 0.1, 0.2]
                        },
                        "floatProperties": {
                            "_RimFresnelPower": 3.0,
                            "_RimLift": 0.6,
                            "_OutlineWidth": 2.0,
                            "_OutlineWidthMode": 1.0
                        }
                    }]
                }
            }
        }))
        .expect("parse vrm0 json");
        let prim = Primitive {
            attributes: Attributes {
                position: 0,
                normal: None,
                joints: None,
                weights: None,
                texcoord: None,
                color: None,
                tangent: None,
            },
            indices: None,
            mode: None,
            material: Some(0),
            targets: Vec::new(),
        };
        let shade = load_mtoon(&doc, &prim).expect("mtoon");
        assert_eq!(shade.rim_color, [0.9, 0.3, 0.1]);
        assert!((shade.rim_power - 3.0).abs() < 1e-4);
        assert!((shade.rim_lift - 0.6).abs() < 1e-4);
        assert_eq!(shade.outline_color, [0.1, 0.1, 0.2]);
        assert!(
            shade.outline_width > 0.0 && shade.outline_width <= 0.04,
            "VRM0 outline width clamped, got {}",
            shade.outline_width
        );
    }
    #[test]
    fn generated_tangents_follow_u_and_handedness() {
        // Quad in XY, normal +Z, u along +X, v along +Y.
        let mk = |p: [f32; 3], uv: [f32; 2]| Vertex3::with_uv(Vec3::from_array(p), Vec3::Z, uv);
        let mut v = vec![
            mk([0.0, 0.0, 0.0], [0.0, 0.0]),
            mk([1.0, 0.0, 0.0], [1.0, 0.0]),
            mk([1.0, 1.0, 0.0], [1.0, 1.0]),
            mk([0.0, 1.0, 0.0], [0.0, 1.0]),
        ];
        let idx = [0u32, 1, 2, 0, 2, 3];
        generate_tangents(&mut v, &idx);
        for vx in &v {
            let t = vx.tangent;
            assert!(
                (t[0] - 1.0).abs() < 1e-5 && t[1].abs() < 1e-5 && t[2].abs() < 1e-5,
                "{t:?}"
            );
            assert_eq!(t[3], 1.0, "right-handed");
        }
        // Mirrored U flips the tangent and the handedness sign.
        for vx in v.iter_mut() {
            vx.uv[0] = 1.0 - vx.uv[0];
        }
        generate_tangents(&mut v, &idx);
        for vx in &v {
            assert!((vx.tangent[0] + 1.0).abs() < 1e-5, "{:?}", vx.tangent);
            assert_eq!(vx.tangent[3], -1.0, "mirrored");
        }
        // Degenerate UVs leave a zero tangent (shader keeps the vertex normal).
        for vx in v.iter_mut() {
            vx.uv = [0.5, 0.5];
        }
        generate_tangents(&mut v, &idx);
        assert_eq!(v[0].tangent, [0.0, 0.0, 0.0, 1.0]);
    }
    fn two_clip_skin() -> SkinnedMesh {
        let mut skin = skinned_from_embedded_gltf(&walk_skinned_gltf()).expect("skin");
        assert_eq!(
            clip_names(&skin),
            vec![skin.clip.as_ref().unwrap().name.clone()]
        );
        // "Wave": hold joint 1 at a fixed 90 degree twist about X.
        let q = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        skin.clips.push(AnimClip {
            name: "Wave".into(),
            duration: 1.0,
            channels: vec![AnimChannel {
                node: skin.skin_joints[1],
                path: ChannelPath::Rotation,
                times: vec![0.0, 1.0],
                values: [q.to_array(), q.to_array()].concat(),
                step: false,
            }],
        });
        skin
    }

    fn pos_sum(m: &MeshData) -> Vec3 {
        m.vertices.iter().map(|v| Vec3::from_array(v.pos)).sum()
    }

    #[test]
    fn named_clip_sampling_picks_the_clip() {
        let skin = two_clip_skin();
        assert!(find_clip(&skin, "wave").is_some(), "case-insensitive");
        assert!(find_clip(&skin, "nope").is_none());
        let pose = |name: Option<&str>| WalkerPose {
            clip: Some(0.5),
            anim_blend: 1.0,
            clip_name: name.map(str::to_string),
            ..WalkerPose::default()
        };
        let walk = pos_sum(&sample_skinned_pose(&skin, &pose(None)));
        let wave = pos_sum(&sample_skinned_pose(&skin, &pose(Some("Wave"))));
        let unknown = pos_sum(&sample_skinned_pose(&skin, &pose(Some("nope"))));
        assert!((walk - wave).length() > 1e-3, "Wave differs from Walk");
        assert!(
            (walk - unknown).length() < 1e-5,
            "unknown falls back to default"
        );
    }

    #[test]
    fn crossfade_weight_blends_between_clips() {
        let skin = two_clip_skin();
        let walk_name = skin.clip.as_ref().unwrap().name.clone();
        let pose = |w: f32| WalkerPose {
            clip: Some(0.5),
            anim_blend: 1.0,
            clip_name: Some("Wave".into()),
            fade_from: Some(walk_name.clone()),
            fade_from_clip: 0.5,
            fade_from_weight: w,
            ..WalkerPose::default()
        };
        let a = pos_sum(&sample_skinned_pose(&skin, &pose(1.0)));
        let b = pos_sum(&sample_skinned_pose(&skin, &pose(0.0)));
        let mid = pos_sum(&sample_skinned_pose(&skin, &pose(0.5)));
        let walk = pos_sum(&sample_skinned_pose(
            &skin,
            &WalkerPose {
                clip: Some(0.5),
                anim_blend: 1.0,
                ..WalkerPose::default()
            },
        ));
        assert!((a - walk).length() < 1e-4, "weight 1 = old clip");
        assert!((a - b).length() > 1e-3);
        assert!(
            (mid - a).length() > 1e-4 && (mid - b).length() > 1e-4,
            "mid is between"
        );
    }
    #[test]
    fn joint_palette_matches_cpu_skinning() {
        let skin = skinned_from_embedded_gltf(&walk_skinned_gltf()).expect("skin");
        let pose = WalkerPose {
            clip: Some(0.37),
            anim_blend: 1.0,
            ..WalkerPose::default()
        };
        let pal = joint_palette(&skin, &pose);
        assert_eq!(pal.len(), skin.skin_joints.len());
        let stream = gpu_skin_stream(&skin).expect("2 joints fit");
        let cpu = sample_skinned_pose(&skin, &pose);
        for (i, v) in skin.rest.vertices.iter().enumerate() {
            // Same math as shader3d.wgsl skin_matrix.
            let mut m = Mat4::ZERO;
            let mut wsum = 0.0;
            for lane in stream[i] {
                let (j, w) = unpack_skin_lane(lane);
                m += pal[j] * w;
                wsum += w;
            }
            let p = (m * (1.0 / wsum)).transform_point3(Vec3::from_array(v.pos));
            let c = Vec3::from_array(cpu.vertices[i].pos);
            assert!((p - c).length() < 1e-3, "vertex {i}: gpu {p} cpu {c}");
        }
    }

    #[test]
    fn rest_pose_palette_is_identity() {
        let skin = skinned_from_embedded_gltf(&walk_skinned_gltf()).expect("skin");
        for m in joint_palette(&skin, &WalkerPose::default()) {
            assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-4), "{m}");
        }
    }

    #[test]
    fn gpu_skin_stream_packs_and_caps() {
        let mut skin = skinned_from_embedded_gltf(&walk_skinned_gltf()).expect("skin");
        let s = gpu_skin_stream(&skin).unwrap();
        for (lanes, (js, ws)) in s.iter().zip(skin.joints.iter().zip(&skin.weights)) {
            for k in 0..4 {
                let (j, w) = unpack_skin_lane(lanes[k]);
                if ws[k] > 1e-6 {
                    assert_eq!(j, js[k] as usize);
                    assert!((w - ws[k].min(0.9999)).abs() < 1e-4);
                }
            }
        }
        skin.skin_joints = (0..MAX_GPU_JOINTS + 1).collect();
        assert!(
            gpu_skin_stream(&skin).is_none(),
            "over the cap stays on CPU"
        );
        let mut pose = WalkerPose::default();
        assert!(gpu_skin_pose_ok(&skin, &pose));
        pose.morph = 0.5;
        assert!(!gpu_skin_pose_ok(&skin, &pose), "morphs stay on CPU");
    }
}
