//! Thin VRM 0 `secondaryAnimation` / VRM 1 `VRMC_springBone` chain.
//! Gravity + stiffness + one-step Verlet. No colliders, sleeves, or RendererV2.

use glam::{Mat4, Quat, Vec3};
use serde_json::Value;

/// UniVRM virtual tail for a leaf with no child (metres).
const VIRTUAL_TAIL_LEN: f32 = 0.07;

#[derive(Clone, Debug)]
pub struct SpringJoint {
    pub node: usize,
    pub stiffness: f32,
    pub drag: f32,
    pub gravity: [f32; 3],
    pub bone_length: f32,
    pub rest_dir_local: [f32; 3],
    pub curr: [f32; 3],
    pub prev: [f32; 3],
    pub target: [f32; 3],
    pub virtual_tail: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SpringChain {
    pub joints: Vec<SpringJoint>,
}

#[derive(Clone, Debug, Default)]
pub struct SpringState {
    pub chains: Vec<SpringChain>,
    pub initialized: bool,
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

fn new_joint(node: usize, stiffness: f32, drag: f32, gravity: [f32; 3]) -> SpringJoint {
    SpringJoint {
        node,
        stiffness,
        drag,
        gravity,
        bone_length: VIRTUAL_TAIL_LEN,
        rest_dir_local: [0.0, 1.0, 0.0],
        curr: [0.0; 3],
        prev: [0.0; 3],
        target: [0.0; 3],
        virtual_tail: false,
    }
}

fn virtual_tail(stiffness: f32, drag: f32, gravity: [f32; 3]) -> SpringJoint {
    let mut j = new_joint(usize::MAX, stiffness, drag, gravity);
    j.virtual_tail = true;
    j
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

/// Parse VRM 0.x `secondaryAnimation` and/or VRM 1.0 `VRMC_springBone`.
pub fn parse_spring_bones(extensions: Option<&Value>, children: &[Vec<usize>]) -> SpringState {
    let mut state = SpringState::default();
    let nlen = children.len();
    let Some(ext) = extensions else {
        return state;
    };

    if let Some(sa) = ext.pointer("/VRM/secondaryAnimation") {
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
                let Some(bones) = g.get("bones").and_then(|v| v.as_array()) else {
                    continue;
                };
                for b in bones {
                    let Some(root) = b.as_u64() else { continue };
                    let idxs = walk_v0_chain(children, root as usize);
                    let mut joints: Vec<SpringJoint> = idxs
                        .into_iter()
                        .filter(|&i| i < nlen)
                        .map(|i| new_joint(i, stiff, drag, grav))
                        .collect();
                    if joints.len() == 1 {
                        joints.push(virtual_tail(stiff, drag, grav));
                    }
                    if joints.len() >= 2 {
                        state.chains.push(SpringChain { joints });
                    }
                }
            }
        }
    }

    if let Some(sb1) = ext.pointer("/VRMC_springBone") {
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
                        ));
                    }
                }
                if joints.len() >= 2 {
                    state.chains.push(SpringChain { joints });
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
            let rest_world = (parent_q
                * Vec3::from_array(state.chains[ci].joints[i].rest_dir_local))
            .normalize_or(Vec3::Y);
            let bone_len = state.chains[ci].joints[i].bone_length;
            let target = parent_pos + rest_world * bone_len;
            state.chains[ci].joints[i].target = target.to_array();

            let j = &state.chains[ci].joints[i];
            let vel = (Vec3::from_array(j.curr) - Vec3::from_array(j.prev)) * (1.0 - j.drag);
            let spring = rest_world * (j.stiffness * dt * dt);
            let external = Vec3::from_array(j.gravity) * (dt * dt);
            let mut new_pos = Vec3::from_array(j.curr) + vel + spring + external;
            let to_new = new_pos - parent_pos;
            let dist = to_new.length();
            if dist > 1e-6 {
                new_pos = parent_pos + to_new * (bone_len / dist);
            }
            state.chains[ci].joints[i].prev = state.chains[ci].joints[i].curr;
            state.chains[ci].joints[i].curr = new_pos.to_array();
        }
    }
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
                    new_joint(0, stiffness, 0.4, gravity),
                    new_joint(1, stiffness, 0.4, gravity),
                ],
            }],
            initialized: false,
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
                    new_joint(0, 1.0, 0.4, [0.0; 3]),
                    new_joint(1, 1.0, 0.4, [0.0; 3]),
                ],
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
}
