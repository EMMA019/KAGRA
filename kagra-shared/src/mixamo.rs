//! Thin Mixamo locomotion retarget for clip-less VRM/glTF humanoids.
//!
//! V2 `avatar.bind_locomotion()` rest+roll compensation (not raw bind*delta):
//!     N = W_src * delta_src * inv(W_src)
//!     delta_dst = inv(W_dst) * N * W_dst
//!     local = bind_local * delta_dst
//! Identity Mixamo delta stays dest rest (A-pose stays A-pose).

use std::collections::HashMap;

use glam::{Quat, Vec3};
use serde::Deserialize;

use crate::gltf_load::{AnimChannel, AnimClip, ChannelPath, NodeRest, SkinnedMesh};

const MIXAMO_WALK_JSON: &str = include_str!("../tests/fixtures/mixamo_walk.json");

const ID: Quat = Quat::IDENTITY;

/// Mixamo / J_Bip name -> VRM 0/1 + node-name aliases already in `humanoid`.
const BONE_ALIASES: &[(&str, &[&str])] = &[
    (
        "J_Bip_C_Hips",
        &["hips", "J_Bip_C_Hips", "Hips", "mixamorig:Hips"],
    ),
    (
        "J_Bip_C_Spine",
        &["spine", "J_Bip_C_Spine", "Spine", "mixamorig:Spine"],
    ),
    (
        "J_Bip_C_Chest",
        &[
            "chest",
            "J_Bip_C_Chest",
            "Chest",
            "Spine1",
            "mixamorig:Spine1",
        ],
    ),
    (
        "J_Bip_C_UpperChest",
        &[
            "upperChest",
            "J_Bip_C_UpperChest",
            "UpperChest",
            "Spine2",
            "mixamorig:Spine2",
        ],
    ),
    (
        "J_Bip_C_Neck",
        &["neck", "J_Bip_C_Neck", "Neck", "mixamorig:Neck"],
    ),
    (
        "J_Bip_C_Head",
        &["head", "J_Bip_C_Head", "Head", "mixamorig:Head"],
    ),
    (
        "J_Bip_L_Shoulder",
        &[
            "leftShoulder",
            "J_Bip_L_Shoulder",
            "LeftShoulder",
            "mixamorig:LeftShoulder",
        ],
    ),
    (
        "J_Bip_L_UpperArm",
        &[
            "leftUpperArm",
            "J_Bip_L_UpperArm",
            "LeftArm",
            "LeftUpperArm",
            "mixamorig:LeftArm",
        ],
    ),
    (
        "J_Bip_L_LowerArm",
        &[
            "leftLowerArm",
            "J_Bip_L_LowerArm",
            "LeftForeArm",
            "LeftLowerArm",
            "mixamorig:LeftForeArm",
        ],
    ),
    (
        "J_Bip_L_Hand",
        &["leftHand", "J_Bip_L_Hand", "LeftHand", "mixamorig:LeftHand"],
    ),
    (
        "J_Bip_R_Shoulder",
        &[
            "rightShoulder",
            "J_Bip_R_Shoulder",
            "RightShoulder",
            "mixamorig:RightShoulder",
        ],
    ),
    (
        "J_Bip_R_UpperArm",
        &[
            "rightUpperArm",
            "J_Bip_R_UpperArm",
            "RightArm",
            "RightUpperArm",
            "mixamorig:RightArm",
        ],
    ),
    (
        "J_Bip_R_LowerArm",
        &[
            "rightLowerArm",
            "J_Bip_R_LowerArm",
            "RightForeArm",
            "RightLowerArm",
            "mixamorig:RightForeArm",
        ],
    ),
    (
        "J_Bip_R_Hand",
        &[
            "rightHand",
            "J_Bip_R_Hand",
            "RightHand",
            "mixamorig:RightHand",
        ],
    ),
    (
        "J_Bip_L_UpperLeg",
        &[
            "leftUpperLeg",
            "J_Bip_L_UpperLeg",
            "LeftUpLeg",
            "LeftUpperLeg",
            "mixamorig:LeftUpLeg",
        ],
    ),
    (
        "J_Bip_L_LowerLeg",
        &[
            "leftLowerLeg",
            "J_Bip_L_LowerLeg",
            "LeftLeg",
            "LeftLowerLeg",
            "mixamorig:LeftLeg",
        ],
    ),
    (
        "J_Bip_L_Foot",
        &["leftFoot", "J_Bip_L_Foot", "LeftFoot", "mixamorig:LeftFoot"],
    ),
    (
        "J_Bip_R_UpperLeg",
        &[
            "rightUpperLeg",
            "J_Bip_R_UpperLeg",
            "RightUpLeg",
            "RightUpperLeg",
            "mixamorig:RightUpLeg",
        ],
    ),
    (
        "J_Bip_R_LowerLeg",
        &[
            "rightLowerLeg",
            "J_Bip_R_LowerLeg",
            "RightLeg",
            "RightLowerLeg",
            "mixamorig:RightLeg",
        ],
    ),
    (
        "J_Bip_R_Foot",
        &[
            "rightFoot",
            "J_Bip_R_Foot",
            "RightFoot",
            "mixamorig:RightFoot",
        ],
    ),
];

#[derive(Deserialize)]
struct MixamoClipFile {
    #[serde(default)]
    src_worlds: HashMap<String, [f32; 4]>,
    #[serde(default)]
    frame_time: f32,
    #[serde(default)]
    frames: Vec<HashMap<String, [f32; 4]>>,
}

/// If `skin` has no clip and a humanoid, retarget the bundled Mixamo walk onto it.
/// No-op when a Walk/first clip is already present.
pub fn bind_locomotion(skin: &mut SkinnedMesh) -> bool {
    if skin.clip.is_some() {
        return false;
    }
    if skin.nodes.is_empty() || skin.humanoid.is_empty() {
        return false;
    }
    let Some(clip) = retarget_walk_onto(skin) else {
        return false;
    };
    if clip.channels.is_empty() {
        return false;
    }
    skin.clip = Some(clip);
    true
}

fn retarget_walk_onto(skin: &SkinnedMesh) -> Option<AnimClip> {
    let file: MixamoClipFile = serde_json::from_str(MIXAMO_WALK_JSON).ok()?;
    if file.frames.is_empty() {
        return None;
    }
    let dt = if file.frame_time > 1e-6 {
        file.frame_time
    } else {
        1.0 / 30.0
    };
    let n = file.frames.len();
    let mut times = Vec::with_capacity(n);
    for i in 0..n {
        times.push(i as f32 * dt);
    }
    let duration = if n > 1 { times[n - 1].max(dt) } else { dt };
    let world_dst = rest_world_rotations(&skin.nodes);
    let src_worlds = if file.src_worlds.is_empty() {
        mixamo_tpose_worlds()
    } else {
        file.src_worlds
            .iter()
            .map(|(k, q)| (k.clone(), quat_xyzw(*q)))
            .collect()
    };
    let mut channels = Vec::new();
    let bones: Vec<String> = file.frames[0].keys().cloned().collect();
    for name in bones {
        let Some(node) = resolve_bone(&skin.humanoid, &name) else {
            continue;
        };
        if node >= skin.nodes.len() {
            continue;
        }
        let wd = world_dst.get(node).copied().unwrap_or(ID);
        let ws = src_worlds.get(&name).copied().unwrap_or(ID);
        let bind_local = skin.nodes[node].rotation;
        let mut values = Vec::with_capacity(n * 4);
        for frame in &file.frames {
            let delta_src = frame.get(&name).copied().map(quat_xyzw).unwrap_or(ID);
            let delta_dst = retarget_delta(delta_src, ws, wd);
            let local = (bind_local * delta_dst).normalize();
            values.extend_from_slice(&local.to_array());
        }
        channels.push(AnimChannel {
            node,
            path: ChannelPath::Rotation,
            times: times.clone(),
            values,
            step: false,
        });
    }
    Some(AnimClip {
        name: "Walk".into(),
        duration,
        channels,
    })
}

pub fn retarget_delta(delta_src: Quat, world_src: Quat, world_dst: Quat) -> Quat {
    let ws = norm(world_src);
    let wd = norm(world_dst);
    let d = norm(delta_src);
    let n = ws * d * ws.inverse();
    (wd.inverse() * n * wd).normalize()
}

pub fn mixamo_tpose_worlds() -> HashMap<String, Quat> {
    let left = quat_from_axes(
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    );
    let right = quat_from_axes(
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(0.0, -1.0, 0.0),
    );
    let hips = ID;
    let mut m = HashMap::new();
    for name in [
        "J_Bip_C_Hips",
        "J_Bip_C_Spine",
        "J_Bip_C_Chest",
        "J_Bip_C_UpperChest",
        "J_Bip_C_Neck",
        "J_Bip_C_Head",
    ] {
        m.insert(name.into(), hips);
    }
    for name in [
        "J_Bip_L_Shoulder",
        "J_Bip_L_UpperArm",
        "J_Bip_L_LowerArm",
        "J_Bip_L_Hand",
    ] {
        m.insert(name.into(), left);
    }
    for name in [
        "J_Bip_R_Shoulder",
        "J_Bip_R_UpperArm",
        "J_Bip_R_LowerArm",
        "J_Bip_R_Hand",
    ] {
        m.insert(name.into(), right);
    }
    m
}

pub fn vroid_tpose_worlds(rolled: bool) -> HashMap<String, Quat> {
    let (left, right) = if rolled {
        (
            quat_from_axes(
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, -1.0),
            ),
            quat_from_axes(
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
        )
    } else {
        (
            quat_from_axes(
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ),
            quat_from_axes(
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, -1.0),
            ),
        )
    };
    let mut m = HashMap::new();
    m.insert("J_Bip_C_Hips".into(), ID);
    m.insert("J_Bip_L_UpperArm".into(), left);
    m.insert("J_Bip_R_UpperArm".into(), right);
    m.insert("J_Bip_L_LowerArm".into(), left);
    m.insert("J_Bip_R_LowerArm".into(), right);
    m
}

pub fn vroid_apose_worlds(drop: f32) -> HashMap<String, Quat> {
    let mut base = vroid_tpose_worlds(true);
    let hang_l = Quat::from_axis_angle(Vec3::Z, -drop.abs());
    let hang_r = Quat::from_axis_angle(Vec3::Z, drop.abs());
    for (name, extra) in [
        ("J_Bip_L_UpperArm", hang_l),
        ("J_Bip_L_LowerArm", hang_l),
        ("J_Bip_R_UpperArm", hang_r),
        ("J_Bip_R_LowerArm", hang_r),
    ] {
        if let Some(q) = base.get(name).copied() {
            base.insert(name.into(), extra * q);
        }
    }
    base
}

pub fn mixamo_hang_delta(radians: f32, side_left: bool) -> Quat {
    let sign = if side_left { 1.0 } else { -1.0 };
    Quat::from_axis_angle(Vec3::new(sign, 0.0, 0.0), radians)
}

pub fn bone_dir(world_q: Quat, local_axis: Vec3) -> Vec3 {
    norm(world_q) * local_axis
}

pub fn animated_bone_dir(world_rest: Quat, delta_local: Quat, local_axis: Vec3) -> Vec3 {
    bone_dir(norm(world_rest) * norm(delta_local), local_axis)
}

pub fn folded_forward(rest_dir: Vec3, anim_dir: Vec3, thresh: f32) -> bool {
    let fwd = anim_dir.z.abs();
    let along = rest_dir.dot(anim_dir).abs();
    fwd >= thresh && along < 0.55
}

pub(crate) fn rest_world_rotations(nodes: &[NodeRest]) -> Vec<Quat> {
    let n = nodes.len();
    let mut parent = vec![None; n];
    for (i, node) in nodes.iter().enumerate() {
        for &c in &node.children {
            if c < n {
                parent[c] = Some(i);
            }
        }
    }
    let mut world = vec![ID; n];
    let mut done = vec![false; n];
    for i in 0..n {
        rest_world_rec(i, nodes, &parent, &mut world, &mut done);
    }
    world
}

fn rest_world_rec(
    i: usize,
    nodes: &[NodeRest],
    parent: &[Option<usize>],
    world: &mut [Quat],
    done: &mut [bool],
) {
    if done[i] {
        return;
    }
    if let Some(p) = parent[i] {
        rest_world_rec(p, nodes, parent, world, done);
        world[i] = world[p] * nodes[i].rotation;
    } else {
        world[i] = nodes[i].rotation;
    }
    done[i] = true;
}

fn resolve_bone(humanoid: &HashMap<String, usize>, j_bip: &str) -> Option<usize> {
    if let Some(&i) = humanoid.get(j_bip) {
        return Some(i);
    }
    for (name, aliases) in BONE_ALIASES {
        if *name == j_bip {
            for a in *aliases {
                if let Some(&i) = humanoid.get(*a) {
                    return Some(i);
                }
            }
        }
        for a in *aliases {
            if *a == j_bip {
                if let Some(&i) = humanoid.get(*name) {
                    return Some(i);
                }
                for b in *aliases {
                    if let Some(&i) = humanoid.get(*b) {
                        return Some(i);
                    }
                }
            }
        }
    }
    None
}

fn quat_xyzw(q: [f32; 4]) -> Quat {
    norm(Quat::from_xyzw(q[0], q[1], q[2], q[3]))
}

fn norm(q: Quat) -> Quat {
    let n = q.length();
    if n > 1e-8 {
        q / n
    } else {
        ID
    }
}

fn quat_from_axes(x_axis: Vec3, y_axis: Vec3, z_axis: Vec3) -> Quat {
    let m00 = x_axis.x;
    let m10 = x_axis.y;
    let m20 = x_axis.z;
    let m01 = y_axis.x;
    let m11 = y_axis.y;
    let m21 = y_axis.z;
    let m02 = z_axis.x;
    let m12 = z_axis.y;
    let m22 = z_axis.z;
    let trace = m00 + m11 + m22;
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        Quat::from_xyzw((m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, 0.25 * s)
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        Quat::from_xyzw(0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s)
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        Quat::from_xyzw((m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s)
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        Quat::from_xyzw((m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s)
    };
    norm(q)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gltf_load::{
        is_tpose_humanoid_spec, sample_skinned, skinned_from_embedded_gltf, skinned_from_glb,
        tpose_humanoid_vrm, walk_skinned_gltf, walk_skinned_vrm,
    };
    use std::f32::consts::FRAC_PI_2;

    const HANG: &str = include_str!("../../tests/fixtures/synthetic_mixamo_hang.json");

    fn load_hang() -> (Vec<HashMap<String, Quat>>, HashMap<String, Quat>) {
        let file: MixamoClipFile = serde_json::from_str(HANG).unwrap();
        let src = file
            .src_worlds
            .iter()
            .map(|(k, q)| (k.clone(), quat_xyzw(*q)))
            .collect();
        let frames = file
            .frames
            .iter()
            .map(|f| f.iter().map(|(k, q)| (k.clone(), quat_xyzw(*q))).collect())
            .collect();
        (frames, src)
    }

    #[test]
    fn tpose_mixamo_on_tpose_vroid_does_not_fold_forward() {
        let (frames, src) = load_hang();
        let dst = vroid_tpose_worlds(true);
        let hang = frames[1]["J_Bip_L_UpperArm"];
        let w_src = src["J_Bip_L_UpperArm"];
        let w_dst = dst["J_Bip_L_UpperArm"];
        let rest_dir = animated_bone_dir(w_dst, ID, Vec3::Y);
        let raw_dir = animated_bone_dir(w_dst, hang, Vec3::Y);
        let fixed = retarget_delta(hang, w_src, w_dst);
        let fixed_dir = animated_bone_dir(w_dst, fixed, Vec3::Y);
        assert!(folded_forward(rest_dir, raw_dir, 0.55));
        assert!(raw_dir.z.abs() > 0.55);
        assert!(!folded_forward(rest_dir, fixed_dir, 0.55));
        assert!(fixed_dir.z.abs() < 0.35);
        assert!(rest_dir.dot(fixed_dir) < 0.5);
    }

    #[test]
    fn apose_identity_stays_at_rest() {
        let src = mixamo_tpose_worlds();
        let dst = vroid_apose_worlds(0.7);
        let w_dst = dst["J_Bip_L_UpperArm"];
        let rest_dir = animated_bone_dir(w_dst, ID, Vec3::Y);
        let delta = retarget_delta(ID, src["J_Bip_L_UpperArm"], w_dst);
        let anim_dir = animated_bone_dir(w_dst, delta, Vec3::Y);
        assert!(rest_dir.dot(anim_dir) > 0.98);
        assert!(!folded_forward(rest_dir, anim_dir, 0.55));
    }

    #[test]
    fn apose_hang_does_not_fold_forward() {
        let (frames, src) = load_hang();
        let dst = vroid_apose_worlds(0.7);
        let hang = frames[1]["J_Bip_L_UpperArm"];
        let w_dst = dst["J_Bip_L_UpperArm"];
        let rest_dir = animated_bone_dir(w_dst, ID, Vec3::Y);
        let raw_dir = animated_bone_dir(w_dst, hang, Vec3::Y);
        let fixed = retarget_delta(hang, src["J_Bip_L_UpperArm"], w_dst);
        let fixed_dir = animated_bone_dir(w_dst, fixed, Vec3::Y);
        assert!(folded_forward(rest_dir, raw_dir, 0.55));
        assert!(!folded_forward(rest_dir, fixed_dir, 0.55));
        assert!(fixed_dir.z.abs() < 0.40);
    }

    #[test]
    fn walk_clip_kept_when_present() {
        let skin = skinned_from_embedded_gltf(&walk_skinned_gltf()).unwrap();
        let mut clone = skin.clone();
        let bound = bind_locomotion(&mut clone);
        assert!(!bound, "Walk clip must keep current behavior");
        assert_eq!(
            clone.clip.as_ref().map(|c| c.name.as_str()),
            skin.clip.as_ref().map(|c| c.name.as_str())
        );
    }

    #[test]
    fn tpose_humanoid_binds_mixamo_walk() {
        assert!(is_tpose_humanoid_spec("tpose_humanoid.vrm"));
        assert!(!is_tpose_humanoid_spec("Emma.vrm"));
        let bytes = tpose_humanoid_vrm();
        let mut skin = skinned_from_glb(&bytes).unwrap();
        skin.clip = None;
        assert!(bind_locomotion(&mut skin));
        let clip = skin.clip.as_ref().expect("Mixamo Walk");
        assert!(clip.name.eq_ignore_ascii_case("walk"));
        assert!(clip.duration > 0.2);
        assert!(!clip.channels.is_empty());
        let rest = sample_skinned(&skin, 0.0);
        // sample_skinned at 0 is first Mixamo key; compare against rest mesh.
        let walk = {
            let t = (clip.duration * 0.35).max(0.05);
            sample_skinned(&skin, t)
        };
        let mut max_d = 0.0f32;
        for (a, b) in skin.rest.vertices.iter().zip(walk.vertices.iter()) {
            let d = (Vec3::from_array(a.pos) - Vec3::from_array(b.pos)).length();
            max_d = max_d.max(d);
        }
        assert!(
            max_d > 0.02,
            "Mixamo retarget must move verts off bind, max_d={max_d}"
        );
        let _ = rest;
    }

    #[test]
    fn vrm_with_walk_clip_unchanged() {
        let skin = skinned_from_glb(&walk_skinned_vrm()).unwrap();
        assert!(skin.clip.is_some());
        let mut c = skin.clone();
        assert!(!bind_locomotion(&mut c));
    }

    #[test]
    fn hang_delta_matches_v2_local_x() {
        let q = mixamo_hang_delta(FRAC_PI_2, true);
        assert!((q.x.abs() - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.02);
        assert!(q.y.abs() < 0.02 && q.z.abs() < 0.02);
    }

    #[test]
    fn play_world_no_clip_humanoid_advances_clip() {
        use crate::collectathon::WalkInput;
        use crate::world_play::WorldPlay;
        const DUMP: &str = include_str!("../tests/fixtures/mixamo_walker_world.json");
        let mut play = WorldPlay::from_json(DUMP).unwrap();
        play.start();
        assert_eq!(
            play.doc.player.as_ref().unwrap().gltf.as_deref(),
            Some("tpose_humanoid.vrm")
        );
        play.input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert!(play.doc.player.as_ref().unwrap().clip > 0.2);
        let walk = play
            .doc
            .compile_meshes()
            .into_iter()
            .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
            .unwrap()
            .1;
        let bind = {
            let mut idle = play.doc.clone();
            if let Some(w) = idle.player.as_mut() {
                w.clip = 0.0;
            }
            for w in &mut idle.walkers {
                w.clip = 0.0;
            }
            idle.compile_meshes()
                .into_iter()
                .find(|(id, _)| id.0 >= crate::world_doc::MESH_GLTF_BASE)
                .unwrap()
                .1
        };
        let mut max_d = 0.0f32;
        for (a, b) in bind.vertices.iter().zip(walk.vertices.iter()) {
            let d = (glam::Vec3::from_array(a.pos) - glam::Vec3::from_array(b.pos)).length();
            max_d = max_d.max(d);
        }
        assert!(max_d > 0.02, "play_world Mixamo walk max_d={max_d}");
        play.input = WalkInput::default();
        play.tick(1.0 / 60.0);
        assert_eq!(play.doc.player.as_ref().unwrap().clip, 0.0);
    }
}
