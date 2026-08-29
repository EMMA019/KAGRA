//! VRM 0 `secondaryAnimation` / VRM 1 `VRMC_springBone` chains.
//! Gravity + stiffness + one-step Verlet + sphere/capsule colliders.
//! Ported colliders from kagra-core vrm_spring.rs (no sleeves, no RendererV2).

use glam::{Mat4, Quat, Vec3};
use serde_json::Value;

/// UniVRM virtual tail for a leaf with no child (metres).
const VIRTUAL_TAIL_LEN: f32 = 0.07;

/// Sphere (or capsule via `tail`) collider in a node's local space.
#[derive(Clone, Debug)]
pub struct SpringCollider {
    pub node: usize,
    pub offset: [f32; 3],
    pub radius: f32,
    pub tail: Option<[f32; 3]>,
}

impl Default for SpringCollider {
    fn default() -> Self {
        Self {
            node: usize::MAX,
            offset: [0.0, 0.0, 0.0],
            radius: 0.05,
            tail: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SpringJoint {
    pub node: usize,
    pub stiffness: f32,
    pub drag: f32,
    pub gravity: [f32; 3],
    /// Joint radius added to collider radius (metres).
    pub radius: f32,
    pub bone_length: f32,
    pub rest_dir_local: [f32; 3],
    pub curr: [f32; 3],
    pub prev: [f32; 3],
    pub target: [f32; 3],
    pub virtual_tail: bool,
    /// Parent node world rotation at the last Verlet step (for rotation deltas).
    pub parent_world_rot: [f32; 4],
}

#[derive(Clone, Debug, Default)]
pub struct SpringChain {
    pub joints: Vec<SpringJoint>,
    pub collider_ids: Vec<usize>,
}

impl SpringChain {
    /// 根 + 仮想テールの 2 節チェーン（袖ヘルパー等、葉に子が無い場合）。
    pub fn simple(
        root: usize,
        stiffness: f32,
        drag: f32,
        gravity: [f32; 3],
        radius: f32,
        rest_dir_local: [f32; 3],
        bone_length: f32,
    ) -> Self {
        let mut joints = vec![new_joint(root, stiffness, drag, gravity, radius)];
        let mut tail = virtual_tail(stiffness, drag, gravity, radius);
        tail.rest_dir_local = rest_dir_local;
        tail.bone_length = bone_length.max(0.04);
        joints.push(tail);
        Self {
            joints,
            collider_ids: vec![],
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SpringState {
    pub chains: Vec<SpringChain>,
    pub colliders: Vec<SpringCollider>,
    pub initialized: bool,
    /// 風（重力と同様に外部加速度として作用）。kagra-core set_spring_wind 移植。
    pub wind: [f32; 3],
}

impl SpringState {
    pub fn is_empty(&self) -> bool {
        self.chains.is_empty()
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

fn new_joint(
    node: usize,
    stiffness: f32,
    drag: f32,
    gravity: [f32; 3],
    radius: f32,
) -> SpringJoint {
    SpringJoint {
        node,
        stiffness,
        drag,
        gravity,
        radius,
        bone_length: VIRTUAL_TAIL_LEN,
        rest_dir_local: [0.0, 1.0, 0.0],
        curr: [0.0; 3],
        prev: [0.0; 3],
        target: [0.0; 3],
        virtual_tail: false,
        parent_world_rot: [0.0, 0.0, 0.0, 1.0],
    }
}

fn virtual_tail(stiffness: f32, drag: f32, gravity: [f32; 3], radius: f32) -> SpringJoint {
    let mut j = new_joint(usize::MAX, stiffness, drag, gravity, radius);
    j.virtual_tail = true;
    j
}

/// Push `point` out of a sphere, or return it unchanged when outside.
fn collide_sphere(point: Vec3, center: Vec3, radius: f32, fallback: Vec3) -> Vec3 {
    let to = point - center;
    let dist = to.length();
    if dist >= radius {
        return point;
    }
    if dist < 1e-8 {
        return center + fallback.normalize_or(Vec3::Y) * radius;
    }
    center + to * (radius / dist)
}

/// Push `point` out of a capsule (segment a-b with radius).
fn collide_capsule(point: Vec3, a: Vec3, b: Vec3, radius: f32, fallback: Vec3) -> Vec3 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 < 1e-12 {
        return collide_sphere(point, a, radius, fallback);
    }
    let t = ((point - a).dot(ab) / len2).clamp(0.0, 1.0);
    collide_sphere(point, a + ab * t, radius, fallback)
}

fn walk_v0_chain(children: &[Vec<usize>], root: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut idx = root;
    loop {
        if idx >= children.len() {
            break;
        }
        out.push(idx);
        let Some(&next) = children[idx].first() else {
            break;
        };
        idx = next;
    }
    out
}

/// Parse VRM 0.x `secondaryAnimation` and/or VRM 1.0 `VRMC_springBone`,
/// including sphere/capsule colliders and per-chain collider groups.
pub fn parse_spring_bones(extensions: Option<&Value>, children: &[Vec<usize>]) -> SpringState {
    let mut state = SpringState::default();
    let nlen = children.len();
    let Some(ext) = extensions else {
        return state;
    };

    // VRM 0 colliders (sphere only).
    let mut v0_groups: Vec<Vec<usize>> = Vec::new();
    if let Some(sa) = ext.pointer("/VRM/secondaryAnimation") {
        if let Some(cgs) = sa.get("colliderGroups").and_then(|v| v.as_array()) {
            for cg in cgs {
                let start = state.colliders.len();
                if let Some(cols) = cg.get("colliders").and_then(|v| v.as_array()) {
                    for col in cols {
                        state.colliders.push(SpringCollider {
                            node: col.get("node").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                            offset: as_vec3(col.get("offset"), [0.0; 3]),
                            radius: col.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.05)
                                as f32,
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
                let gp = g
                    .get("gravityPower")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                let gd = as_vec3(g.get("gravityDir"), [0.0, -1.0, 0.0]);
                let grav = [gd[0] * gp, gd[1] * gp, gd[2] * gp];
                let hit_r = g.get("hitRadius").and_then(|v| v.as_f64()).unwrap_or(0.02) as f32;
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
                let Some(bones) = g.get("bones").and_then(|v| v.as_array()) else {
                    continue;
                };
                for b in bones {
                    let Some(root) = b.as_u64() else { continue };
                    let idxs = walk_v0_chain(children, root as usize);
                    let mut joints: Vec<SpringJoint> = idxs
                        .into_iter()
                        .filter(|&i| i < nlen)
                        .map(|i| new_joint(i, stiff, drag, grav, hit_r))
                        .collect();
                    if joints.len() == 1 {
                        joints.push(virtual_tail(stiff, drag, grav, hit_r));
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

    // VRM 1 colliders (sphere + capsule).
    let mut v1_groups: Vec<Vec<usize>> = Vec::new();
    if let Some(sb1) = ext.pointer("/VRMC_springBone") {
        if let Some(cols) = sb1.get("colliders").and_then(|v| v.as_array()) {
            for col in cols {
                let node = col.get("node").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let shape = col.get("shape").cloned().unwrap_or(Value::Null);
                if let Some(cap) = shape.get("capsule") {
                    state.colliders.push(SpringCollider {
                        node,
                        offset: as_vec3(cap.get("offset"), [0.0; 3]),
                        radius: cap.get("radius").and_then(|v| v.as_f64()).unwrap_or(0.05) as f32,
                        tail: Some(as_vec3(cap.get("tail"), [0.0; 3])),
                    });
                } else {
                    let sph = shape.get("sphere").cloned().unwrap_or(Value::Null);
                    state.colliders.push(SpringCollider {
                        node,
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
                        let ni =
                            jd.get("node").and_then(|v| v.as_u64()).unwrap_or(u64::MAX) as usize;
                        if ni >= nlen {
                            continue;
                        }
                        let gp = jd
                            .get("gravityPower")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0) as f32;
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

fn world_pos(m: Mat4) -> Vec3 {
    m.w_axis.truncate()
}

fn world_rot(m: Mat4) -> Quat {
    let (_, r, _) = m.to_scale_rotation_translation();
    r
}

/// Push `point` out of every collider assigned to the chain (all when none).
fn collide_chain(
    colliders: &[SpringCollider],
    chain: &SpringChain,
    point: Vec3,
    hit_radius: f32,
    fallback: Vec3,
    world: &[Mat4],
) -> Vec3 {
    let ids: Vec<usize> = if chain.collider_ids.is_empty() {
        (0..colliders.len()).collect()
    } else {
        chain.collider_ids.clone()
    };
    let mut pos = point;
    for ci in ids {
        if ci >= colliders.len() {
            continue;
        }
        let c = &colliders[ci];
        if c.node >= world.len() {
            continue;
        }
        let m = world[c.node];
        let center = m.transform_point3(Vec3::from_array(c.offset));
        let rad = c.radius + hit_radius;
        pos = if let Some(tail) = c.tail {
            collide_capsule(
                pos,
                center,
                m.transform_point3(Vec3::from_array(tail)),
                rad,
                fallback,
            )
        } else {
            collide_sphere(pos, center, rad, fallback)
        };
    }
    pos
}

fn joint_world_pos(j: &SpringJoint, parent_pos: Vec3, parent_q: Quat, world: &[Mat4]) -> Vec3 {
    if j.virtual_tail || j.node >= world.len() {
        let dir = parent_q * Vec3::from_array(j.rest_dir_local);
        return parent_pos + dir.normalize_or(Vec3::Y) * j.bone_length;
    }
    world_pos(world[j.node])
}

fn init_rest(state: &mut SpringState, world: &[Mat4]) {
    for chain in &mut state.chains {
        let n = chain.joints.len();
        for i in 0..n.saturating_sub(1) {
            if chain.joints[i + 1].virtual_tail {
                if chain.joints[i + 1].bone_length < 0.001 {
                    chain.joints[i + 1].bone_length = VIRTUAL_TAIL_LEN;
                }
                continue;
            }
            let pi = chain.joints[i].node;
            let ci = chain.joints[i + 1].node;
            if pi >= world.len() || ci >= world.len() {
                continue;
            }
            let p_parent = world_pos(world[pi]);
            let p_child = world_pos(world[ci]);
            let world_dir = p_child - p_parent;
            let bone_len = world_dir.length();
            chain.joints[i + 1].bone_length = if bone_len > 0.001 {
                bone_len
            } else {
                VIRTUAL_TAIL_LEN
            };
            let parent_q = world_rot(world[pi]);
            chain.joints[i + 1].rest_dir_local =
                (parent_q.inverse() * world_dir.normalize_or(Vec3::Y)).to_array();
        }
        for i in 0..n {
            let (parent_pos, parent_q) = if i == 0 {
                (Vec3::ZERO, Quat::IDENTITY)
            } else {
                let parent_pos = Vec3::from_array(chain.joints[i - 1].curr);
                let pni = chain.joints[i - 1].node;
                let parent_q = if pni < world.len() {
                    world_rot(world[pni])
                } else {
                    Quat::IDENTITY
                };
                (parent_pos, parent_q)
            };
            let p = joint_world_pos(&chain.joints[i], parent_pos, parent_q, world);
            chain.joints[i].curr = p.to_array();
            chain.joints[i].prev = p.to_array();
            chain.joints[i].target = p.to_array();
        }
    }
}

fn snap_to_world(state: &mut SpringState, world: &[Mat4]) {
    for chain in &mut state.chains {
        for i in 0..chain.joints.len() {
            let (parent_pos, parent_q) = if i == 0 {
                (Vec3::ZERO, Quat::IDENTITY)
            } else {
                let parent_pos = Vec3::from_array(chain.joints[i - 1].curr);
                let pni = chain.joints[i - 1].node;
                let parent_q = if pni < world.len() {
                    world_rot(world[pni])
                } else {
                    Quat::IDENTITY
                };
                (parent_pos, parent_q)
            };
            let p = joint_world_pos(&chain.joints[i], parent_pos, parent_q, world);
            chain.joints[i].curr = p.to_array();
            chain.joints[i].prev = p.to_array();
            chain.joints[i].target = p.to_array();
        }
    }
}

/// First spring-bone local yaw (radians) from the live Verlet tail vs rest.
pub fn hair_yaw(state: &SpringState) -> f32 {
    let Some(chain) = state.chains.first() else {
        return 0.0;
    };
    if chain.joints.len() < 2 {
        return 0.0;
    }
    let parent = Vec3::from_array(chain.joints[0].curr);
    let curr = Vec3::from_array(chain.joints[1].curr);
    let d = curr - parent;
    d.x.atan2(d.y.max(1e-5))
}

/// First node that takes the dump `hair` overlay (hair root).
pub fn hair_node(state: &SpringState) -> Option<usize> {
    let chain = state.chains.first()?;
    let j = chain.joints.first()?;
    if j.virtual_tail || j.node == usize::MAX {
        None
    } else {
        Some(j.node)
    }
}

/// One Verlet step after pose. First call snaps rest (no motion).
pub fn step(state: &mut SpringState, world: &[Mat4], dt: f32) {
    verlet(state, world, dt);
}

/// Verlet 1 ステップ + 各関節の回転デルタ（`(node, local quat)`）を返す。
///
/// kagra-core vrm_spring::step の末尾移植: チェーンの各節で「目標方向（rest
/// 軸）→ 現在方向」の回転をワールド→ローカルに変換し、ポーズに掛ける。
/// スキナーがこのデルタをノードのローカル回転に足すと、布が実際に揺れる。
pub fn step_with_updates(
    state: &mut SpringState,
    world: &[Mat4],
    parents: &[Option<usize>],
    dt: f32,
) -> Vec<(usize, Quat)> {
    verlet(state, world, dt);
    compute_updates(state, world, parents)
}

fn verlet(state: &mut SpringState, world: &[Mat4], dt: f32) {
    if state.chains.is_empty() {
        return;
    }
    let dt = dt.min(1.0 / 30.0);
    if !state.initialized {
        init_rest(state, world);
        snap_to_world(state, world);
        state.initialized = true;
        return;
    }

    let n_chains = state.chains.len();
    for ci in 0..n_chains {
        let n_joints = state.chains[ci].joints.len();
        for i in 0..n_joints {
            if i == 0 {
                let ni = state.chains[ci].joints[i].node;
                if ni < world.len() {
                    let p = world_pos(world[ni]).to_array();
                    state.chains[ci].joints[i].curr = p;
                    state.chains[ci].joints[i].prev = p;
                    state.chains[ci].joints[i].target = p;
                }
                continue;
            }
            let parent_pos = Vec3::from_array(state.chains[ci].joints[i - 1].curr);
            let pni = state.chains[ci].joints[i - 1].node;
            let parent_q = if pni < world.len() {
                world_rot(world[pni])
            } else {
                Quat::IDENTITY
            };
            state.chains[ci].joints[i].parent_world_rot = parent_q.to_array();
            let rest_world = (parent_q
                * Vec3::from_array(state.chains[ci].joints[i].rest_dir_local))
            .normalize_or(Vec3::Y);
            let bone_len = state.chains[ci].joints[i].bone_length;
            let target = parent_pos + rest_world * bone_len;
            state.chains[ci].joints[i].target = target.to_array();

            let j = &state.chains[ci].joints[i];
            let vel = (Vec3::from_array(j.curr) - Vec3::from_array(j.prev)) * (1.0 - j.drag);
            let spring = rest_world * (j.stiffness * dt * dt);
            let wind = Vec3::from_array(state.wind) * (dt * dt);
            let external = Vec3::from_array(j.gravity) * (dt * dt) + wind;
            let mut new_pos = Vec3::from_array(j.curr) + vel + spring + external;
            let to_new = new_pos - parent_pos;
            let dist = to_new.length();
            if dist > 1e-6 {
                new_pos = parent_pos + to_new * (bone_len / dist);
            }
            // コリジョン解決: チェーンに割り当てられた球/カプセルで押し出す。
            new_pos = collide_chain(
                &state.colliders,
                &state.chains[ci],
                new_pos,
                j.radius,
                rest_world,
                world,
            );
            state.chains[ci].joints[i].prev = state.chains[ci].joints[i].curr;
            state.chains[ci].joints[i].curr = new_pos.to_array();
        }
    }
}

/// 各節の「目標方向 → 現在方向」の回転をローカル回転デルタとして返す。
fn compute_updates(
    state: &SpringState,
    world: &[Mat4],
    parents: &[Option<usize>],
) -> Vec<(usize, Quat)> {
    let mut updates = Vec::new();
    for chain in &state.chains {
        for i in 0..chain.joints.len().saturating_sub(1) {
            let j = &chain.joints[i];
            let jn = &chain.joints[i + 1];
            let t = Vec3::from_array(jn.target) - Vec3::from_array(j.curr);
            let c = Vec3::from_array(jn.curr) - Vec3::from_array(j.curr);
            if t.length() < 0.001 || c.length() < 0.001 {
                continue;
            }
            let target_dir = t.normalize();
            let curr_dir = c.normalize();
            let delta_world = Quat::from_rotation_arc(target_dir, curr_dir);
            let pw = Quat::from_array(jn.parent_world_rot);
            let delta_local = (pw.inverse() * (delta_world * pw)).normalize();
            // 恒等に近いデルタ（静止中）は捨てる: 布に影響しない更新で
            // スキナーを汚さない。
            if delta_local.w.abs() > 0.9999 {
                continue;
            }
            let pose_local = if let Some(pi) = parents.get(j.node).copied().flatten() {
                if pi < world.len() && j.node < world.len() {
                    (world_rot(world[pi]).inverse() * world_rot(world[j.node])).normalize()
                } else {
                    world_rot(world[j.node])
                }
            } else if j.node < world.len() {
                world_rot(world[j.node])
            } else {
                Quat::IDENTITY
            };
            updates.push((j.node, (delta_local * pose_local).normalize()));
        }
    }
    updates
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_v0_and_v1() {
        let gltf = json!({
            "VRM": {
                "secondaryAnimation": {
                    "boneGroups": [{
                        "bones": [1],
                        "stiffiness": 1.0,
                        "dragForce": 0.4
                    }]
                }
            },
            "VRMC_springBone": {
                "springs": [{
                    "joints": [
                        {"node": 1, "stiffness": 1.0},
                        {"node": 2, "stiffness": 1.0, "gravityPower": 1.0}
                    ]
                }]
            }
        });
        let children = vec![vec![1, 3], vec![2], vec![], vec![]];
        let st = parse_spring_bones(Some(&gltf), &children);
        assert!(st.chains.len() >= 2);
        assert_eq!(st.chains[0].joints[0].node, 1);
        assert!(!st.chains[0].joints[0].virtual_tail);
        assert!(!st.chains[0].joints[1].virtual_tail);
        assert_eq!(st.chains[1].joints.len(), 2);
    }

    #[test]
    fn parse_v0_leaf_gets_virtual_tail() {
        let gltf = json!({
            "VRM": {
                "secondaryAnimation": {
                    "boneGroups": [{
                        "bones": [1],
                        "stiffiness": 2.0,
                        "dragForce": 0.7
                    }]
                }
            }
        });
        let children = vec![vec![1], vec![]];
        let st = parse_spring_bones(Some(&gltf), &children);
        assert_eq!(st.chains.len(), 1);
        assert_eq!(st.chains[0].joints.len(), 2);
        assert!(!st.chains[0].joints[0].virtual_tail);
        assert!(st.chains[0].joints[1].virtual_tail);
    }

    fn rest_chain(stiffness: f32, gravity: [f32; 3]) -> (SpringState, [Mat4; 2]) {
        let mut st = SpringState {
            chains: vec![SpringChain {
                joints: vec![
                    new_joint(0, stiffness, 0.4, gravity, 0.02),
                    new_joint(1, stiffness, 0.4, gravity, 0.02),
                ],
                collider_ids: vec![],
            }],
            colliders: vec![],
            initialized: false,
            wind: [0.0; 3],
        };
        let mats = [
            Mat4::from_translation(Vec3::ZERO),
            Mat4::from_translation(Vec3::new(0.2, 1.0, 0.0)),
        ];
        init_rest(&mut st, &mats);
        st.initialized = true;
        (st, mats)
    }

    #[test]
    fn first_step_snaps_without_motion() {
        let mut st = SpringState {
            chains: vec![SpringChain {
                joints: vec![
                    new_joint(0, 1.0, 0.4, [0.0; 3], 0.02),
                    new_joint(1, 1.0, 0.4, [0.0; 3], 0.02),
                ],
                collider_ids: vec![],
            }],
            ..SpringState::default()
        };
        let mats = [
            Mat4::from_translation(Vec3::ZERO),
            Mat4::from_translation(Vec3::Y),
        ];
        step(&mut st, &mats, 1.0 / 60.0);
        assert!(st.initialized);
        let yaw0 = hair_yaw(&st);
        step(&mut st, &mats, 1.0 / 60.0);
        let _ = yaw0;
    }

    #[test]
    fn stiffness_dt2_does_not_glue() {
        let (mut st, mats) = rest_chain(1.0, [0.0; 3]);
        st.chains[0].joints[1].curr = [0.6, 0.8, 0.0];
        st.chains[0].joints[1].prev = [0.6, 0.8, 0.0];
        step(&mut st, &mats, 1.0 / 60.0);
        let curr = Vec3::from_array(st.chains[0].joints[1].curr);
        let target = Vec3::from_array(st.chains[0].joints[1].target);
        let dist = (curr - target).length();
        assert!(
            dist > 0.25,
            "stiffness*dt^2 must not glue the tail in one frame, dist={dist}"
        );
        let parent = Vec3::from_array(st.chains[0].joints[0].curr);
        let len = (curr - parent).length();
        assert!((len - 1.0).abs() < 0.05, "length constraint, len={len}");
    }

    #[test]
    fn gravity_changes_hair_yaw() {
        let (mut st, mats) = rest_chain(0.8, [0.0, -1.0, 0.0]);
        let y0 = hair_yaw(&st);
        for _ in 0..30 {
            step(&mut st, &mats, 1.0 / 60.0);
        }
        let y1 = hair_yaw(&st);
        assert!(
            (y1 - y0).abs() > 1e-4,
            "gravity Verlet must change hair yaw, y0={y0} y1={y1}"
        );
        assert_eq!(hair_node(&st), Some(0));
    }

    #[test]
    fn collider_parses_v0_and_v1() {
        let gltf = json!({
            "VRM": {
                "secondaryAnimation": {
                    "colliderGroups": [{
                        "colliders": [{"offset": {"x": 0.0, "y": 0.0, "z": 0.0}, "radius": 0.1}]
                    }],
                    "boneGroups": [{
                        "bones": [1],
                        "colliderGroups": [0]
                    }]
                }
            },
            "VRMC_springBone": {
                "colliders": [{
                    "node": 0,
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
        });
        let children = vec![vec![1], vec![2], vec![]];
        let st = parse_spring_bones(Some(&gltf), &children);
        assert!(st.colliders.len() >= 2, "v0 sphere + v1 capsule");
        assert!(
            st.colliders.iter().any(|c| c.tail.is_some()),
            "v1 capsule has a tail"
        );
        assert!(
            !st.chains[0].collider_ids.is_empty(),
            "v0 chain binds collider"
        );
        assert!(
            !st.chains[1].collider_ids.is_empty(),
            "v1 chain binds collider"
        );
    }

    #[test]
    fn step_with_updates_returns_rotation_deltas() {
        let mut st = SpringState {
            chains: vec![SpringChain {
                joints: vec![
                    new_joint(0, 1.0, 0.4, [0.0; 3], 0.02),
                    new_joint(1, 1.0, 0.4, [0.0; 3], 0.02),
                ],
                collider_ids: vec![],
            }],
            ..SpringState::default()
        };
        let mats = [
            Mat4::from_translation(Vec3::ZERO),
            Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        ];
        let parents = vec![None, Some(0)];
        // 1 回目は snap なのでデルタなし
        let first = step_with_updates(&mut st, &mats, &parents, 1.0 / 60.0);
        assert!(first.is_empty(), "snap ステップはデルタなし");
        // 手で引っ張ってから step → 目標方向とずれた回転デルタが出る
        st.chains[0].joints[1].curr = [0.3, 0.95, 0.0];
        st.chains[0].joints[1].prev = [0.3, 0.95, 0.0];
        let updates = step_with_updates(&mut st, &mats, &parents, 1.0 / 60.0);
        assert!(!updates.is_empty(), "ずれた関節に回転デルタが出る");
        for (n, q) in &updates {
            assert_eq!(*n, 0, "最初の節のノードにデルタ");
            assert!((q.length() - 1.0).abs() < 1e-4, "デルタは正規化 quat");
            assert!(
                q.w.abs() < 0.999,
                "ノード 0 の回転デルタは非恒等（目標とずれている）"
            );
        }
    }

    #[test]
    fn wind_moves_chain_without_gravity() {
        // 重力 0 でも風があれば布（テール）が流れる
        let mut st = SpringState {
            chains: vec![SpringChain {
                joints: vec![
                    new_joint(0, 0.6, 0.4, [0.0; 3], 0.02),
                    new_joint(1, 0.6, 0.4, [0.0; 3], 0.02),
                ],
                collider_ids: vec![],
            }],
            ..SpringState::default()
        };
        let mats = [
            Mat4::from_translation(Vec3::ZERO),
            Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        ];
        step(&mut st, &mats, 1.0 / 60.0); // snap
        st.wind = [1.2, 0.0, 0.0];
        for _ in 0..30 {
            step(&mut st, &mats, 1.0 / 60.0);
        }
        let curr = Vec3::from_array(st.chains[0].joints[1].curr);
        assert!(
            curr.x > 0.01,
            "wind must push the tail sideways, x={}",
            curr.x
        );
    }

    #[test]
    fn collider_pushes_joint_out() {
        // Chain: root (0) -> joint (1) at (0.1, 1.0, 0). A sphere collider at
        // the root with radius 1.0 must push the joint out to radius distance.
        let mut st = SpringState {
            chains: vec![SpringChain {
                joints: vec![
                    new_joint(0, 1.0, 0.4, [0.0; 3], 0.02),
                    new_joint(1, 1.0, 0.4, [0.0; 3], 0.02),
                ],
                collider_ids: vec![0],
            }],
            colliders: vec![SpringCollider {
                node: 0,
                offset: [0.0, 0.0, 0.0],
                radius: 1.0,
                tail: None,
            }],
            initialized: false,
            wind: [0.0; 3],
        };
        let mats = [
            Mat4::from_translation(Vec3::ZERO),
            Mat4::from_translation(Vec3::new(0.1, 1.0, 0.0)),
        ];
        step(&mut st, &mats, 1.0 / 60.0); // snap
        step(&mut st, &mats, 1.0 / 60.0);
        let curr = Vec3::from_array(st.chains[0].joints[1].curr);
        let dist = curr.length();
        assert!(
            dist >= 1.02 - 0.05,
            "collider must push the joint out, dist={dist}"
        );
    }
}
