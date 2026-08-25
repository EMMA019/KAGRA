//! VRM SpringBone（0.x secondaryAnimation / 1.0 VRMC_springBone）。
//! Verlet + 球/カプセルコライダー。ポーズ適用後のワールドで解く。
//!
//! 剛性は UniVRM / three-vrm と同じ `stiffness * dt²` を rest 軸へ掛ける。
//! `(target - curr) * stiffness` だと 1.0 で毎フレーム吸着し、2.0 で紙のように暴れる。
//! 子の無い葉（リボン等）は UniVRM と同じ 7cm の仮想テールを足す。

use nalgebra::Matrix4;
use serde_json::Value;

use crate::vrm_constraint::{q_from_to, qconj, qmul, qnorm, qrotate};

/// UniVRM が子の無いボーンに付ける仮想テール長（メートル）。
pub const VIRTUAL_TAIL_LEN: f32 = 0.07;
/// Alicia / VRoid 袖ヘルパー。作者値が無いときの布らしい硬さ（VRM はだいたい 0–4）。
pub const SLEEVE_STIFFNESS: f32 = 2.4;
pub const SLEEVE_DRAG: f32 = 0.45;
pub const SLEEVE_GRAVITY: [f32; 3] = [0.0, -0.15, 0.0];
pub const SLEEVE_HIT_RADIUS: f32 = 0.02;
/// 外側の筒へ渡すウェイトの上限。芯は腕ボーンに残す。
pub const SLEEVE_TRANSFER: f32 = 0.82;

#[derive(Clone, Debug)]
pub struct SpringCollider {
    pub node_idx: usize,
    pub offset: [f32; 3],
    pub radius: f32,
    pub tail: Option<[f32; 3]>,
}

#[derive(Clone, Debug)]
pub struct SpringJoint {
    pub node_idx: usize,
    pub stiffness: f32,
    pub drag: f32,
    pub gravity: [f32; 3],
    pub radius: f32,
    pub bone_length: f32,
    pub rest_dir_local: [f32; 3],
    pub curr: [f32; 3],
    pub prev: [f32; 3],
    pub target: [f32; 3],
    pub parent_world_rot: [f32; 4],
    /// ノードが無いテール（VRM 0.x の葉 / 合成袖）。回転は書かない。
    pub virtual_tail: bool,
}

#[derive(Clone, Debug)]
pub struct SpringChain {
    pub joints: Vec<SpringJoint>,
    pub collider_ids: Vec<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct SpringBoneState {
    pub chains: Vec<SpringChain>,
    pub colliders: Vec<SpringCollider>,
    pub wind: [f32; 3],
    pub enabled: bool,
    pub initialized: bool,
}

impl SpringBoneState {
    pub fn counts(&self) -> (usize, usize, usize) {
        let joints = self.chains.iter().map(|c| c.joints.len()).sum();
        (self.chains.len(), joints, self.colliders.len())
    }
}

fn as_vec3(v: Option<&Value>, default: [f32; 3]) -> [f32; 3] {
    let Some(v) = v else {
        return default;
    };
    if let Some(arr) = v.as_array() {
        if arr.len() >= 3 {
            return [
                arr[0].as_f64().unwrap_or(default[0] as f64) as f32,
                arr[1].as_f64().unwrap_or(default[1] as f64) as f32,
                arr[2].as_f64().unwrap_or(default[2] as f64) as f32,
            ];
        }
    }
    if let Some(obj) = v.as_object() {
        return [
            obj.get("x")
                .and_then(|x| x.as_f64())
                .unwrap_or(default[0] as f64) as f32,
            obj.get("y")
                .and_then(|x| x.as_f64())
                .unwrap_or(default[1] as f64) as f32,
            obj.get("z")
                .and_then(|x| x.as_f64())
                .unwrap_or(default[2] as f64) as f32,
        ];
    }
    default
}

fn vadd(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn vsub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn vsc(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}
fn vdot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn vlen(v: [f32; 3]) -> f32 {
    vdot(v, v).sqrt()
}
fn vnorm(v: [f32; 3]) -> [f32; 3] {
    let l = vlen(v);
    if l < 1e-8 {
        [0.0, 1.0, 0.0]
    } else {
        [v[0] / l, v[1] / l, v[2] / l]
    }
}

pub fn world_pos(m: &Matrix4<f32>) -> [f32; 3] {
    [m[(0, 3)], m[(1, 3)], m[(2, 3)]]
}

pub fn transform_point(m: &Matrix4<f32>, p: [f32; 3]) -> [f32; 3] {
    let v = m * nalgebra::Vector4::new(p[0], p[1], p[2], 1.0);
    [v.x, v.y, v.z]
}

fn q_from_mat3(m: [f32; 9]) -> [f32; 4] {
    let [m00, m10, m20, m01, m11, m21, m02, m12, m22] = m;
    let tr = m00 + m11 + m22;
    if tr > 0.0 {
        let s = 0.5 / (tr + 1.0).sqrt();
        return qnorm([(m21 - m12) * s, (m02 - m20) * s, (m10 - m01) * s, 0.25 / s]);
    }
    if m00 > m11 && m00 > m22 {
        let s = 2.0 * (1.0 + m00 - m11 - m22).sqrt();
        return qnorm([0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s]);
    }
    if m11 > m22 {
        let s = 2.0 * (1.0 + m11 - m00 - m22).sqrt();
        return qnorm([(m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s]);
    }
    let s = 2.0 * (1.0 + m22 - m00 - m11).sqrt();
    qnorm([(m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s])
}

pub fn world_rot(m: &Matrix4<f32>) -> [f32; 4] {
    let c0 = [m[(0, 0)], m[(1, 0)], m[(2, 0)]];
    let c1 = [m[(0, 1)], m[(1, 1)], m[(2, 1)]];
    let c2 = [m[(0, 2)], m[(1, 2)], m[(2, 2)]];
    let sx = vlen(c0).max(1e-8);
    let sy = vlen(c1).max(1e-8);
    let sz = vlen(c2).max(1e-8);
    q_from_mat3([
        c0[0] / sx,
        c0[1] / sx,
        c0[2] / sx,
        c1[0] / sy,
        c1[1] / sy,
        c1[2] / sy,
        c2[0] / sz,
        c2[1] / sz,
        c2[2] / sz,
    ])
}

pub fn collide_sphere(
    point: [f32; 3],
    center: [f32; 3],
    radius: f32,
    fallback_dir: Option<[f32; 3]>,
) -> [f32; 3] {
    let to = vsub(point, center);
    let dist = vlen(to);
    if dist >= radius {
        return point;
    }
    if dist < 1e-8 {
        let n = vnorm(fallback_dir.unwrap_or([0.0, 1.0, 0.0]));
        return vadd(center, vsc(n, radius));
    }
    vadd(center, vsc(to, radius / dist))
}

pub fn collide_capsule(
    point: [f32; 3],
    a: [f32; 3],
    b: [f32; 3],
    radius: f32,
    fallback_dir: Option<[f32; 3]>,
) -> [f32; 3] {
    let ab = vsub(b, a);
    let ab_len2 = vdot(ab, ab);
    if ab_len2 < 1e-12 {
        return collide_sphere(point, a, radius, fallback_dir);
    }
    let t = (vdot(vsub(point, a), ab) / ab_len2).clamp(0.0, 1.0);
    let closest = vadd(a, vsc(ab, t));
    collide_sphere(point, closest, radius, fallback_dir)
}

fn new_joint(node_idx: usize, stiffness: f32, drag: f32, gravity: [f32; 3], radius: f32) -> SpringJoint {
    SpringJoint {
        node_idx,
        stiffness,
        drag,
        gravity,
        radius,
        bone_length: VIRTUAL_TAIL_LEN,
        rest_dir_local: [0.0, 1.0, 0.0],
        curr: [0.0; 3],
        prev: [0.0; 3],
        target: [0.0; 3],
        parent_world_rot: [0.0, 0.0, 0.0, 1.0],
        virtual_tail: false,
    }
}

pub fn virtual_tail_joint(stiffness: f32, drag: f32, gravity: [f32; 3], radius: f32) -> SpringJoint {
    let mut j = new_joint(usize::MAX, stiffness, drag, gravity, radius);
    j.virtual_tail = true;
    j.bone_length = VIRTUAL_TAIL_LEN;
    j.rest_dir_local = [0.0, 1.0, 0.0];
    j
}

/// 袖 / ソデ系のヘルパー名。メッシュ名の generic `cloth` は人体ボーンではないので除外。
pub fn is_sleeve_bone_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("sleeve")
        || lower.contains("sode")
        || name.contains('袖')
        || name.contains("ソデ")
}

/// 腕の芯（〜2cm）は腕に残し、セーラーの外側の筒（〜4cm）をヘルパーへ。
pub fn sleeve_follow(radius: f32) -> f32 {
    let a = 0.022;
    let b = 0.038;
    let t = ((radius - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn radius_to_axis(pos: [f32; 3], origin: [f32; 3], axis: [f32; 3]) -> f32 {
    let rel = vsub(pos, origin);
    let along = vsc(axis, vdot(rel, axis));
    vlen(vsub(rel, along))
}

/// `arm_palette` のウェイトのうち `follow` を `helper_palette` へ移す。
pub fn transfer_sleeve_weights(
    joints: [u32; 4],
    weights: [f32; 4],
    arm_palette: u32,
    helper_palette: u32,
    follow: f32,
) -> ([u32; 4], [f32; 4]) {
    if follow <= 1e-5 {
        return (joints, weights);
    }
    let mut j = joints;
    let mut w = weights;
    let mut arm_w = 0.0;
    for i in 0..4 {
        if j[i] == arm_palette {
            arm_w += w[i];
        }
    }
    let move_w = arm_w * follow.clamp(0.0, 1.0);
    if move_w <= 1e-6 {
        return (j, w);
    }
    if arm_w > 1e-8 {
        let keep = 1.0 - move_w / arm_w;
        for i in 0..4 {
            if j[i] == arm_palette {
                w[i] *= keep;
            }
        }
    }
    if let Some(i) = (0..4).find(|&i| j[i] == helper_palette) {
        w[i] += move_w;
    } else if let Some(i) = (0..4).find(|&i| w[i] <= 1e-6) {
        j[i] = helper_palette;
        w[i] = move_w;
    } else {
        let mut best = 0usize;
        let mut best_w = f32::MAX;
        for i in 0..4 {
            if j[i] != arm_palette && w[i] < best_w {
                best_w = w[i];
                best = i;
            }
        }
        if j[best] != arm_palette {
            j[best] = helper_palette;
            w[best] = move_w;
        }
    }
    let s = w[0] + w[1] + w[2] + w[3];
    if s > 1e-8 {
        w = [w[0] / s, w[1] / s, w[2] / s, w[3] / s];
    }
    (j, w)
}

fn is_under_arm(mut i: usize, arms: &[usize], parents: &[Option<usize>]) -> bool {
    for _ in 0..64 {
        if arms.contains(&i) {
            return true;
        }
        match parents.get(i).copied().flatten() {
            Some(p) => i = p,
            None => return false,
        }
    }
    false
}

/// 既に袖ヘルパー／袖名のチェーンがある（合成ボーンを足さない）。
pub fn has_sleeve_coverage(
    state: &SpringBoneState,
    bone_names: &[String],
    arm_nodes: &[usize],
    parents: &[Option<usize>],
) -> bool {
    for j in state.chains.iter().flat_map(|c| c.joints.iter()) {
        if j.virtual_tail || j.node_idx >= bone_names.len() {
            continue;
        }
        if is_sleeve_bone_name(&bone_names[j.node_idx]) {
            return true;
        }
        if !arm_nodes.contains(&j.node_idx)
            && is_under_arm(j.node_idx, arm_nodes, parents)
        {
            return true;
        }
    }
    false
}

pub fn unbound_sleeve_nodes(bone_names: &[String], used: &[usize]) -> Vec<usize> {
    bone_names
        .iter()
        .enumerate()
        .filter(|(i, n)| is_sleeve_bone_name(n) && !used.contains(i))
        .map(|(i, _)| i)
        .collect()
}

pub fn used_spring_nodes(state: &SpringBoneState) -> Vec<usize> {
    state
        .chains
        .iter()
        .flat_map(|c| {
            c.joints
                .iter()
                .filter(|j| !j.virtual_tail)
                .map(|j| j.node_idx)
        })
        .collect()
}

pub fn push_simple_chain(
    state: &mut SpringBoneState,
    root: usize,
    child: Option<usize>,
    stiffness: f32,
    drag: f32,
    gravity: [f32; 3],
    radius: f32,
    rest_dir_local: [f32; 3],
    bone_length: f32,
) {
    let mut joints = vec![new_joint(root, stiffness, drag, gravity, radius)];
    if let Some(ci) = child {
        joints.push(new_joint(ci, stiffness, drag, gravity, radius));
    } else {
        let mut tail = virtual_tail_joint(stiffness, drag, gravity, radius);
        tail.rest_dir_local = rest_dir_local;
        tail.bone_length = bone_length.max(0.04);
        joints.push(tail);
    }
    let col_ids: Vec<usize> = (0..state.colliders.len()).collect();
    state.chains.push(SpringChain {
        joints,
        collider_ids: col_ids,
    });
}

fn walk_v0_chain(nodes: &[Value], root: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut idx = root;
    loop {
        if idx >= nodes.len() {
            break;
        }
        out.push(idx);
        let Some(ch) = nodes[idx].get("children").and_then(|c| c.as_array()) else {
            break;
        };
        let Some(next) = ch.first().and_then(|v| v.as_u64()) else {
            break;
        };
        idx = next as usize;
    }
    out
}

/// VRM 0.x と 1.0 の両方を読む（ファイルが片方だけならもう一方は空）。
pub fn parse_spring_bones(gltf: &Value) -> SpringBoneState {
    let mut state = SpringBoneState {
        enabled: true,
        ..SpringBoneState::default()
    };
    let empty: Vec<Value> = Vec::new();
    let nodes = gltf.get("nodes").and_then(|n| n.as_array()).unwrap_or(&empty);
    let nlen = nodes.len();

    let vrm0 = gltf.pointer("/extensions/VRM/secondaryAnimation");
    let mut v0_groups: Vec<Vec<usize>> = Vec::new();
    if let Some(sa) = vrm0 {
        if let Some(cgs) = sa.get("colliderGroups").and_then(|v| v.as_array()) {
            for cg in cgs {
                let start = state.colliders.len();
                let node = cg.get("node").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                if let Some(cols) = cg.get("colliders").and_then(|v| v.as_array()) {
                    for col in cols {
                        state.colliders.push(SpringCollider {
                            node_idx: node,
                            offset: as_vec3(col.get("offset"), [0.0; 3]),
                            radius: col.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.05) as f32,
                            tail: None,
                        });
                    }
                }
                v0_groups.push((start..state.colliders.len()).collect());
            }
        }
        if let Some(groups) = sa.get("boneGroups").and_then(|v| v.as_array()) {
            for g in groups {
                let stiff = g.get("stiffiness").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
                let drag = g.get("dragForce").and_then(|v| v.as_f64()).unwrap_or(0.4) as f32;
                let gp = g.get("gravityPower").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let gd = as_vec3(g.get("gravityDir"), [0.0, -1.0, 0.0]);
                let grav = [gd[0] * gp, gd[1] * gp, gd[2] * gp];
                let rad = g.get("hitRadius").and_then(|v| v.as_f64()).unwrap_or(0.02) as f32;
                let mut col_ids = Vec::new();
                if let Some(ids) = g.get("colliderGroups").and_then(|v| v.as_array()) {
                    for gi in ids {
                        if let Some(i) = gi.as_u64() {
                            if (i as usize) < v0_groups.len() {
                                col_ids.extend_from_slice(&v0_groups[i as usize]);
                            }
                        }
                    }
                }
                if col_ids.is_empty() {
                    col_ids = (0..state.colliders.len()).collect();
                }
                if let Some(bones) = g.get("bones").and_then(|v| v.as_array()) {
                    for b in bones {
                        let Some(root) = b.as_u64() else { continue };
                        let idxs = walk_v0_chain(nodes, root as usize);
                        let mut joints = idxs
                            .into_iter()
                            .filter(|&i| i < nlen)
                            .map(|i| new_joint(i, stiff, drag, grav, rad))
                            .collect::<Vec<_>>();
                        if joints.len() == 1 {
                            joints.push(virtual_tail_joint(stiff, drag, grav, rad));
                        }
                        if joints.len() >= 2 {
                            state.chains.push(SpringChain {
                                joints,
                                collider_ids: col_ids.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    let sb1 = gltf.pointer("/extensions/VRMC_springBone");
    let mut v1_groups: Vec<Vec<usize>> = Vec::new();
    if let Some(sb1) = sb1 {
        if let Some(cols) = sb1.get("colliders").and_then(|v| v.as_array()) {
            for col in cols {
                let node = col.get("node").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let shape = col.get("shape").cloned().unwrap_or(Value::Null);
                if let Some(cap) = shape.get("capsule") {
                    state.colliders.push(SpringCollider {
                        node_idx: node,
                        offset: as_vec3(cap.get("offset"), [0.0; 3]),
                        radius: cap.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.05) as f32,
                        tail: Some(as_vec3(cap.get("tail"), [0.0; 3])),
                    });
                } else {
                    let sph = shape.get("sphere").cloned().unwrap_or(Value::Null);
                    state.colliders.push(SpringCollider {
                        node_idx: node,
                        offset: as_vec3(sph.get("offset"), [0.0; 3]),
                        radius: sph.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.05) as f32,
                        tail: None,
                    });
                }
            }
        }
        if let Some(cgs) = sb1.get("colliderGroups").and_then(|v| v.as_array()) {
            for cg in cgs {
                let ids = cg
                    .get("colliders")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_u64().map(|i| i as usize))
                            .collect()
                    })
                    .unwrap_or_default();
                v1_groups.push(ids);
            }
        }
        if let Some(springs) = sb1.get("springs").and_then(|v| v.as_array()) {
            for sp in springs {
                let mut joints = Vec::new();
                if let Some(js) = sp.get("joints").and_then(|v| v.as_array()) {
                    for jd in js {
                        let ni = jd.get("node").and_then(|v| v.as_u64()).unwrap_or(u64::MAX) as usize;
                        if ni >= nlen {
                            continue;
                        }
                        let gp = jd.get("gravityPower").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        let gd = as_vec3(jd.get("gravityDir"), [0.0, -1.0, 0.0]);
                        joints.push(new_joint(
                            ni,
                            jd.get("stiffness").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
                            jd.get("dragForce").and_then(|v| v.as_f64()).unwrap_or(0.4) as f32,
                            [gd[0] * gp, gd[1] * gp, gd[2] * gp],
                            jd.get("hitRadius").and_then(|v| v.as_f64()).unwrap_or(0.02) as f32,
                        ));
                    }
                }
                let mut col_ids = Vec::new();
                if let Some(ids) = sp.get("colliderGroups").and_then(|v| v.as_array()) {
                    for gi in ids {
                        if let Some(i) = gi.as_u64() {
                            if (i as usize) < v1_groups.len() {
                                col_ids.extend_from_slice(&v1_groups[i as usize]);
                            }
                        }
                    }
                }
                if col_ids.is_empty() {
                    col_ids = (0..state.colliders.len()).collect();
                }
                if joints.len() >= 2 {
                    state.chains.push(SpringChain {
                        joints,
                        collider_ids: col_ids,
                    });
                }
            }
        }
    }
    state
}

fn joint_world_pos(j: &SpringJoint, parent_pos: [f32; 3], parent_q: [f32; 4], world_mats: &[Matrix4<f32>]) -> [f32; 3] {
    if j.virtual_tail || j.node_idx >= world_mats.len() {
        return vadd(parent_pos, vsc(vnorm(qrotate(parent_q, j.rest_dir_local)), j.bone_length));
    }
    world_pos(&world_mats[j.node_idx])
}

pub fn init_rest(state: &mut SpringBoneState, world_mats: &[Matrix4<f32>]) {
    for chain in &mut state.chains {
        for i in 0..chain.joints.len().saturating_sub(1) {
            let pi = chain.joints[i].node_idx;
            if chain.joints[i + 1].virtual_tail {
                if chain.joints[i + 1].bone_length < 0.001 {
                    chain.joints[i + 1].bone_length = VIRTUAL_TAIL_LEN;
                }
                continue;
            }
            let ci = chain.joints[i + 1].node_idx;
            if pi >= world_mats.len() || ci >= world_mats.len() {
                continue;
            }
            let p_parent = world_pos(&world_mats[pi]);
            let p_child = world_pos(&world_mats[ci]);
            let world_dir = vsub(p_child, p_parent);
            let bone_len = vlen(world_dir);
            chain.joints[i + 1].bone_length = if bone_len > 0.001 {
                bone_len
            } else {
                VIRTUAL_TAIL_LEN
            };
            let parent_q = world_rot(&world_mats[pi]);
            chain.joints[i + 1].rest_dir_local = qrotate(qconj(parent_q), vnorm(world_dir));
        }
        for i in 0..chain.joints.len() {
            let (parent_pos, parent_q) = if i == 0 {
                ([0.0; 3], [0.0, 0.0, 0.0, 1.0])
            } else {
                let parent_pos = chain.joints[i - 1].curr;
                let pni = chain.joints[i - 1].node_idx;
                let parent_q = if pni < world_mats.len() {
                    world_rot(&world_mats[pni])
                } else {
                    chain.joints[i].parent_world_rot
                };
                (parent_pos, parent_q)
            };
            let p = joint_world_pos(&chain.joints[i], parent_pos, parent_q, world_mats);
            chain.joints[i].curr = p;
            chain.joints[i].prev = p;
            chain.joints[i].target = p;
        }
    }
}

pub fn snap_to_world(state: &mut SpringBoneState, world_mats: &[Matrix4<f32>]) {
    for chain in &mut state.chains {
        for i in 0..chain.joints.len() {
            let (parent_pos, parent_q) = if i == 0 {
                ([0.0; 3], [0.0, 0.0, 0.0, 1.0])
            } else {
                let parent_pos = chain.joints[i - 1].curr;
                let pni = chain.joints[i - 1].node_idx;
                let parent_q = if pni < world_mats.len() {
                    world_rot(&world_mats[pni])
                } else {
                    [0.0, 0.0, 0.0, 1.0]
                };
                (parent_pos, parent_q)
            };
            let p = joint_world_pos(&chain.joints[i], parent_pos, parent_q, world_mats);
            chain.joints[i].curr = p;
            chain.joints[i].prev = p;
            chain.joints[i].target = p;
        }
    }
}

fn collide_chain(
    state: &SpringBoneState,
    chain: &SpringChain,
    point: [f32; 3],
    hit_radius: f32,
    fallback: [f32; 3],
    world_mats: &[Matrix4<f32>],
) -> [f32; 3] {
    let ids: Vec<usize> = if chain.collider_ids.is_empty() {
        (0..state.colliders.len()).collect()
    } else {
        chain.collider_ids.clone()
    };
    let mut pos = point;
    for ci in ids {
        if ci >= state.colliders.len() {
            continue;
        }
        let c = &state.colliders[ci];
        if c.node_idx >= world_mats.len() {
            continue;
        }
        let m = &world_mats[c.node_idx];
        let center = transform_point(m, c.offset);
        let rad = c.radius + hit_radius;
        pos = if let Some(tail) = c.tail {
            collide_capsule(pos, center, transform_point(m, tail), rad, Some(fallback))
        } else {
            collide_sphere(pos, center, rad, Some(fallback))
        };
    }
    pos
}

/// 1 ステップ進めて、書き戻す `(node_idx, local_rot)` を返す。
pub fn step(
    state: &mut SpringBoneState,
    world_mats: &[Matrix4<f32>],
    parents: &[Option<usize>],
    dt: f32,
) -> Vec<(usize, [f32; 4])> {
    if !state.enabled || state.chains.is_empty() {
        return Vec::new();
    }
    let dt = dt.min(1.0 / 30.0);
    if !state.initialized {
        snap_to_world(state, world_mats);
        state.initialized = true;
        return Vec::new();
    }

    let wind = state.wind;
    let n_chains = state.chains.len();
    for ci in 0..n_chains {
        let n_joints = state.chains[ci].joints.len();
        for i in 0..n_joints {
            if i == 0 {
                let ni = state.chains[ci].joints[i].node_idx;
                if ni < world_mats.len() {
                    let p = world_pos(&world_mats[ni]);
                    state.chains[ci].joints[i].curr = p;
                    state.chains[ci].joints[i].prev = p;
                    state.chains[ci].joints[i].target = p;
                }
                continue;
            }
            let parent_pos = state.chains[ci].joints[i - 1].curr;
            let pni = state.chains[ci].joints[i - 1].node_idx;
            let parent_q = if pni < world_mats.len() {
                world_rot(&world_mats[pni])
            } else {
                [0.0, 0.0, 0.0, 1.0]
            };
            state.chains[ci].joints[i].parent_world_rot = parent_q;
            let rest_world = qrotate(parent_q, state.chains[ci].joints[i].rest_dir_local);
            let bone_len = state.chains[ci].joints[i].bone_length;
            let target = vadd(parent_pos, vsc(vnorm(rest_world), bone_len));
            state.chains[ci].joints[i].target = target;

            let j = &state.chains[ci].joints[i];
            let vel = vsc(vsub(j.curr, j.prev), 1.0 - j.drag);
            // UniVRM / three-vrm: rest 軸へ stiffness * dt²。lerp だと硬すぎるか紙になる。
            let spring = vsc(vnorm(rest_world), j.stiffness * dt * dt);
            let external = vadd(vsc(j.gravity, dt * dt), vsc(wind, dt * dt));
            let mut new_pos = vadd(vadd(j.curr, vel), vadd(spring, external));
            let to_new = vsub(new_pos, parent_pos);
            let dist = vlen(to_new);
            if dist > 1e-6 {
                new_pos = vadd(parent_pos, vsc(to_new, bone_len / dist));
            }
            let hit_r = j.radius;
            new_pos = collide_chain(state, &state.chains[ci], new_pos, hit_r, rest_world, world_mats);
            let to_new = vsub(new_pos, parent_pos);
            let dist = vlen(to_new);
            if dist > 1e-6 {
                new_pos = vadd(parent_pos, vsc(to_new, bone_len / dist));
            }
            state.chains[ci].joints[i].prev = state.chains[ci].joints[i].curr;
            state.chains[ci].joints[i].curr = new_pos;
        }
    }

    let mut updates = Vec::new();
    for chain in &state.chains {
        for i in 0..chain.joints.len().saturating_sub(1) {
            let j = &chain.joints[i];
            let jn = &chain.joints[i + 1];
            let target_dir = vnorm(vsub(jn.target, j.curr));
            let curr_dir = vnorm(vsub(jn.curr, j.curr));
            if vlen(target_dir) < 0.001 || vlen(curr_dir) < 0.001 {
                continue;
            }
            let delta_world = q_from_to(target_dir, curr_dir);
            let pw = jn.parent_world_rot;
            let delta_local = qnorm(qmul(qconj(pw), qmul(delta_world, pw)));
            let pose_local = if let Some(pi) = parents.get(j.node_idx).and_then(|p| *p) {
                if pi < world_mats.len() && j.node_idx < world_mats.len() {
                    qnorm(qmul(qconj(world_rot(&world_mats[pi])), world_rot(&world_mats[j.node_idx])))
                } else {
                    world_rot(&world_mats[j.node_idx])
                }
            } else if j.node_idx < world_mats.len() {
                world_rot(&world_mats[j.node_idx])
            } else {
                [0.0, 0.0, 0.0, 1.0]
            };
            updates.push((j.node_idx, qnorm(qmul(delta_local, pose_local))));
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn collide_sphere_outside_unchanged() {
        let p = collide_sphere([2.0, 0.0, 0.0], [0.0, 0.0, 0.0], 1.0, None);
        assert!((p[0] - 2.0).abs() < 1e-6);
    }

    #[test]
    fn collide_sphere_pushes_out() {
        let p = collide_sphere([0.1, 0.0, 0.0], [0.0, 0.0, 0.0], 1.0, None);
        assert!((p[0] - 1.0).abs() < 1e-5);
        assert!(p[1].abs() < 1e-5);
    }

    #[test]
    fn collide_sphere_center_uses_fallback() {
        let p = collide_sphere([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 0.5, Some([0.0, 1.0, 0.0]));
        assert!((p[1] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn parse_v0_and_v1() {
        let gltf = json!({
            "nodes": [
                {"name": "root", "children": [1, 3]},
                {"name": "hair", "children": [2]},
                {"name": "hair2"},
                {"name": "chest"}
            ],
            "extensions": {
                "VRM": {
                    "secondaryAnimation": {
                        "boneGroups": [{
                            "bones": [1],
                            "colliderGroups": [0],
                            "stiffiness": 1.0,
                            "dragForce": 0.4,
                            "hitRadius": 0.02
                        }],
                        "colliderGroups": [{
                            "node": 3,
                            "colliders": [{"offset": {"x": 0, "y": 0, "z": 0}, "radius": 0.1}]
                        }]
                    }
                },
                "VRMC_springBone": {
                    "colliders": [{
                        "node": 3,
                        "shape": {"capsule": {"offset": [0, 0, 0], "tail": [0, 0.2, 0], "radius": 0.08}}
                    }],
                    "colliderGroups": [{"colliders": [0]}],
                    "springs": [{
                        "joints": [
                            {"node": 1, "stiffness": 1.0},
                            {"node": 2, "stiffness": 1.0}
                        ],
                        "colliderGroups": [0]
                    }]
                }
            }
        });
        let st = parse_spring_bones(&gltf);
        assert!(st.colliders.len() >= 2);
        assert!(st.chains.len() >= 2);
        assert_eq!(st.chains[0].collider_ids, vec![0]);
        assert!(st.colliders.iter().any(|c| c.tail.is_some()));
    }

    #[test]
    fn first_step_snaps_without_writes() {
        let mut st = SpringBoneState {
            enabled: true,
            chains: vec![SpringChain {
                joints: vec![
                    new_joint(0, 1.0, 0.4, [0.0; 3], 0.02),
                    new_joint(1, 1.0, 0.4, [0.0; 3], 0.02),
                ],
                collider_ids: vec![],
            }],
            ..SpringBoneState::default()
        };
        let mats = [
            Matrix4::new_translation(&nalgebra::Vector3::new(0.0, 0.0, 0.0)),
            Matrix4::new_translation(&nalgebra::Vector3::new(0.0, 1.0, 0.0)),
        ];
        init_rest(&mut st, &mats);
        let out = step(&mut st, &mats, &[None, Some(0)], 1.0 / 60.0);
        assert!(out.is_empty());
        assert!(st.initialized);
    }

    #[test]
    fn parse_v0_leaf_gets_virtual_tail() {
        let gltf = json!({
            "nodes": [
                {"name": "head", "children": [1]},
                {"name": "ribbon_L"}
            ],
            "extensions": {
                "VRM": {
                    "secondaryAnimation": {
                        "boneGroups": [{
                            "bones": [1],
                            "stiffiness": 2.0,
                            "dragForce": 0.7,
                            "hitRadius": 0.02
                        }],
                        "colliderGroups": []
                    }
                }
            }
        });
        let st = parse_spring_bones(&gltf);
        assert_eq!(st.chains.len(), 1);
        assert_eq!(st.chains[0].joints.len(), 2);
        assert!(!st.chains[0].joints[0].virtual_tail);
        assert!(st.chains[0].joints[1].virtual_tail);
        assert!((st.chains[0].joints[1].bone_length - VIRTUAL_TAIL_LEN).abs() < 1e-6);
    }

    fn rest_chain(stiffness: f32) -> (SpringBoneState, [Matrix4<f32>; 2]) {
        let mut st = SpringBoneState {
            enabled: true,
            initialized: true,
            chains: vec![SpringChain {
                joints: vec![
                    new_joint(0, stiffness, 0.4, [0.0; 3], 0.02),
                    new_joint(1, stiffness, 0.4, [0.0; 3], 0.02),
                ],
                collider_ids: vec![],
            }],
            ..SpringBoneState::default()
        };
        let mats = [
            Matrix4::new_translation(&nalgebra::Vector3::new(0.0, 0.0, 0.0)),
            Matrix4::new_translation(&nalgebra::Vector3::new(0.0, 1.0, 0.0)),
        ];
        init_rest(&mut st, &mats);
        st.initialized = true;
        (st, mats)
    }

    #[test]
    fn stiffness_dt2_does_not_snap_or_explode() {
        let (mut st, mats) = rest_chain(1.0);
        st.chains[0].joints[1].curr = [0.6, 0.8, 0.0];
        st.chains[0].joints[1].prev = [0.6, 0.8, 0.0];
        let _ = step(&mut st, &mats, &[None, Some(0)], 1.0 / 60.0);
        let curr = st.chains[0].joints[1].curr;
        let target = st.chains[0].joints[1].target;
        let dist = vlen(vsub(curr, target));
        // Old lerp (stiffness=1) would sit on the target. Fabric still lags.
        assert!(
            dist > 0.25,
            "stiffness*dt² must not glue the tail in one frame, dist={dist}"
        );
        let len = vlen(vsub(curr, st.chains[0].joints[0].curr));
        assert!((len - 1.0).abs() < 0.02, "length constraint, len={len}");
    }

    #[test]
    fn stiffness_dt2_recovers_after_parent_hold() {
        let (mut st, mats) = rest_chain(2.4);
        st.chains[0].joints[1].curr = [0.6, 0.8, 0.0];
        st.chains[0].joints[1].prev = [0.6, 0.8, 0.0];
        let start = vlen(vsub(st.chains[0].joints[1].curr, [0.0, 1.0, 0.0]));
        for _ in 0..180 {
            let _ = step(&mut st, &mats, &[None, Some(0)], 1.0 / 60.0);
        }
        let dist = vlen(vsub(st.chains[0].joints[1].curr, [0.0, 1.0, 0.0]));
        assert!(dist < start, "fabric returns toward rest, start={start} now={dist}");
        assert!(dist < 0.55, "3s at stiffness 2.4 should settle some, dist={dist}");
    }

    #[test]
    fn sleeve_follow_inner_glued_outer_moves() {
        assert!(sleeve_follow(0.018) < 0.02);
        assert!(sleeve_follow(0.040) > 0.95);
        let mid = sleeve_follow(0.030);
        assert!(mid > 0.2 && mid < 0.9);
    }

    #[test]
    fn transfer_sleeve_weights_moves_outer_mass() {
        let (j, w) = transfer_sleeve_weights([3, 0, 0, 0], [1.0, 0.0, 0.0, 0.0], 3, 62, 0.82);
        assert_eq!(j[0], 3);
        assert!(j.iter().any(|&x| x == 62));
        let helper: f32 = (0..4).map(|i| if j[i] == 62 { w[i] } else { 0.0 }).sum();
        let arm: f32 = (0..4).map(|i| if j[i] == 3 { w[i] } else { 0.0 }).sum();
        assert!((helper - 0.82).abs() < 0.02);
        assert!((arm - 0.18).abs() < 0.02);
        let (j2, w2) = transfer_sleeve_weights([3, 1, 2, 4], [0.7, 0.1, 0.1, 0.1], 3, 9, 0.0);
        assert_eq!(j2, [3, 1, 2, 4]);
        assert!((w2[0] - 0.7).abs() < 1e-5);
    }

    #[test]
    fn sleeve_name_and_coverage() {
        assert!(is_sleeve_bone_name("J_Sec_L_Sleeve"));
        assert!(is_sleeve_bone_name("袖_L"));
        assert!(!is_sleeve_bone_name("cloth"));
        assert!(!is_sleeve_bone_name("skirt_01_01"));
        let mut st = SpringBoneState {
            enabled: true,
            chains: vec![SpringChain {
                joints: vec![
                    new_joint(1, 1.0, 0.4, [0.0; 3], 0.02),
                    virtual_tail_joint(1.0, 0.4, [0.0; 3], 0.02),
                ],
                collider_ids: vec![],
            }],
            ..SpringBoneState::default()
        };
        let names = vec![
            "LeftArm".into(),
            "J_Sec_L_Sleeve".into(),
            "LeftForeArm".into(),
        ];
        let parents = vec![None, Some(0), Some(0)];
        assert!(has_sleeve_coverage(&st, &names, &[0, 2], &parents));
        st.chains[0].joints[0].node_idx = 2;
        let names2 = vec!["LeftArm".into(), "hair".into(), "LeftForeArm".into()];
        assert!(!has_sleeve_coverage(&st, &names2, &[0, 2], &parents));
    }
}
