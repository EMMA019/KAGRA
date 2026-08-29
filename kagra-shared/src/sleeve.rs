//! 袖ヘルパーボーン自動生成（kagra-core vrm.rs `ensure_sleeve_cloth` 移植）。
//!
//! 袖ボーンが無い VRM（VRoid 等）の腕に、上腕/前腕・前腕/手のリンクごとに
//! ヘルパーボーンを足し、袖の外側筒ウェイトをヘルパーへ移して SpringBone
//! チェーンを張る。これで袖が重力/風で揺れる。
//!
//! ロード時（`skinned_from_prim`）に一度だけ走る。スキニング・布シミュは
//! 既存の経路（`sample_skinned_cloth`）がそのままヘルパーを扱う。

use glam::{Mat4, Quat, Vec3};

use crate::gltf_load::{node_parents, NodeRest, SkinnedMesh};
use crate::spring::SpringChain;

pub const SLEEVE_STIFFNESS: f32 = 2.4;
pub const SLEEVE_DRAG: f32 = 0.45;
pub const SLEEVE_GRAVITY: [f32; 3] = [0.0, -0.15, 0.0];
pub const SLEEVE_HIT_RADIUS: f32 = 0.02;
/// 外側筒のウェイトのうちヘルパーへ移す割合。
pub const SLEEVE_TRANSFER: f32 = 0.82;

/// 上腕→前腕 / 前腕→手（左右）。ヘルパー名は重複防止の名前空間。
const ARM_LINKS: [(&str, &str, &str); 4] = [
    ("leftUpperArm", "leftLowerArm", "_kagraSleeveLU"),
    ("leftLowerArm", "leftHand", "_kagraSleeveLL"),
    ("rightUpperArm", "rightLowerArm", "_kagraSleeveRU"),
    ("rightLowerArm", "rightHand", "_kagraSleeveRL"),
];

/// 袖 / ソデ系のボーン名。メッシュ名の generic `cloth` は人体ボーンではないので除外。
pub fn is_sleeve_bone_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("sleeve") || lower.contains("sode") || name.contains('袖') || name.contains("ソデ")
}

/// 腕の芯（〜2cm）は腕に残し、セーラーの外側の筒（〜4cm）をヘルパーへ。
fn sleeve_follow(radius: f32) -> f32 {
    let a = 0.022;
    let b = 0.038;
    let t = ((radius - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 点から軸までの距離。
fn radius_to_axis(pos: Vec3, origin: Vec3, axis: Vec3) -> f32 {
    let rel = pos - origin;
    let along = axis * rel.dot(axis);
    (rel - along).length()
}

/// `arm_palette` のウェイトのうち `follow` を `helper_palette` へ移す。
fn transfer_sleeve_weights(
    joints: [u16; 4],
    weights: [f32; 4],
    arm_palette: u32,
    helper_palette: u32,
    follow: f32,
) -> ([u16; 4], [f32; 4]) {
    if follow <= 1e-5 {
        return (joints, weights);
    }
    let mut j = joints;
    let mut w = weights;
    let mut arm_w = 0.0;
    for i in 0..4 {
        if j[i] as u32 == arm_palette {
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
            if j[i] as u32 == arm_palette {
                w[i] *= keep;
            }
        }
    }
    if let Some(i) = (0..4).find(|&i| j[i] as u32 == helper_palette) {
        w[i] += move_w;
    } else if let Some(i) = (0..4).find(|&i| w[i] <= 1e-6) {
        j[i] = helper_palette as u16;
        w[i] = move_w;
    } else {
        let mut best = 0usize;
        let mut best_w = f32::MAX;
        for i in 0..4 {
            if j[i] as u32 != arm_palette && w[i] < best_w {
                best_w = w[i];
                best = i;
            }
        }
        if j[best] as u32 != arm_palette {
            j[best] = helper_palette as u16;
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
fn has_sleeve_coverage(
    springs: &crate::spring::SpringState,
    names: &[String],
    arm_nodes: &[usize],
    parents: &[Option<usize>],
) -> bool {
    for j in springs.chains.iter().flat_map(|c| c.joints.iter()) {
        if j.virtual_tail || j.node >= names.len() {
            continue;
        }
        if is_sleeve_bone_name(&names[j.node]) {
            return true;
        }
        if !arm_nodes.contains(&j.node) && is_under_arm(j.node, arm_nodes, parents) {
            return true;
        }
    }
    false
}

/// Rest（バインド）ポーズのワールド行列。親→子の順（glTF のトポロジ順）。
fn bind_world_mats(nodes: &[NodeRest]) -> Vec<Mat4> {
    let parents = node_parents(nodes);
    let n = nodes.len();
    let mut global = vec![Mat4::IDENTITY; n];
    for i in 0..n {
        let local = Mat4::from_scale_rotation_translation(
            nodes[i].scale,
            nodes[i].rotation,
            nodes[i].translation,
        );
        global[i] = match parents[i] {
            Some(p) => global[p] * local,
            None => local,
        };
    }
    global
}

/// 袖ボーンが無いスキンにヘルパーを足し、外側筒ウェイトを移す（ロード時に 1 回）。
pub fn ensure_sleeve_cloth(skin: &mut SkinnedMesh) {
    let arm_nodes: Vec<usize> = ["leftUpperArm", "leftLowerArm", "rightUpperArm", "rightLowerArm"]
        .iter()
        .filter_map(|k| skin.humanoid.get(*k).copied())
        .collect();
    let names: Vec<String> = skin.nodes.iter().map(|n| n.name.clone()).collect();
    let parents = node_parents(&skin.nodes);
    if arm_nodes.is_empty() || has_sleeve_coverage(&skin.springs, &names, &arm_nodes, &parents) {
        return;
    }
    let bind = bind_world_mats(&skin.nodes);

    // ヘルパーを追加し、パレット + スプリングチェーンを張る
    for &(arm_key, next_key, helper_name) in &ARM_LINKS {
        let Some(&arm) = skin.humanoid.get(arm_key) else {
            continue;
        };
        let Some(&nxt) = skin.humanoid.get(next_key) else {
            continue;
        };
        if arm >= skin.nodes.len() || nxt >= skin.nodes.len() {
            continue;
        }
        if names.iter().any(|n| n == helper_name) {
            continue; // 同名ヘルパーが既にある
        }
        let origin = bind[arm].w_axis.truncate();
        let next_p = bind[nxt].w_axis.truncate();
        let delta = next_p - origin;
        let arm_len = delta.length();
        if arm_len < 0.02 {
            continue;
        }
        let axis_w = delta / arm_len;
        let lt = skin.nodes[nxt].translation;
        let llen = lt.length().max(1e-8);
        let axis_local = lt / llen;
        let helper_trans = axis_local * (arm_len * 0.45);

        let helper_idx = skin.nodes.len();
        skin.nodes.push(NodeRest {
            name: helper_name.to_string(),
            children: vec![],
            translation: helper_trans,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        });
        skin.springs.chains.push(SpringChain::simple(
            helper_idx,
            SLEEVE_STIFFNESS,
            SLEEVE_DRAG,
            SLEEVE_GRAVITY,
            SLEEVE_HIT_RADIUS,
            axis_local.to_array(),
            (arm_len * 0.40).max(0.05),
        ));

        // パレットにヘルパーを追加（このスキンが腕をスキンしている場合）
        let Some(arm_pal) = skin.skin_joints.iter().position(|&n| n == arm) else {
            continue;
        };
        if skin.skin_joints.contains(&helper_idx) {
            continue;
        }
        let helper_pal = skin.skin_joints.len();
        skin.skin_joints.push(helper_idx);
        let bind2 = bind_world_mats(&skin.nodes);
        let inv = bind2
            .get(helper_idx)
            .map(|m| m.inverse())
            .unwrap_or(Mat4::IDENTITY);
        skin.inverse_bind.push(inv);

        // 外側筒ウェイトを移す
        for i in 0..skin.joints.len() {
            let pos = Vec3::from_array(skin.rest.vertices[i].pos);
            let rad = radius_to_axis(pos, origin, axis_w);
            let follow = sleeve_follow(rad) * SLEEVE_TRANSFER;
            let (nj, nw) = transfer_sleeve_weights(
                skin.joints[i],
                skin.weights[i],
                arm_pal as u32,
                helper_pal as u32,
                follow,
            );
            skin.joints[i] = nj;
            skin.weights[i] = nw;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleeve_name_detection() {
        assert!(is_sleeve_bone_name("J_Sec_L_Sleeve"));
        assert!(is_sleeve_bone_name("袖_L"));
        assert!(!is_sleeve_bone_name("cloth"));
        assert!(!is_sleeve_bone_name("skirt_01_01"));
    }

    #[test]
    fn follow_curve_inner_glued_outer_moves() {
        assert!(sleeve_follow(0.018) < 0.02);
        assert!(sleeve_follow(0.040) > 0.95);
        let mid = sleeve_follow(0.030);
        assert!(mid > 0.3 && mid < 0.8);
    }

    #[test]
    fn transfer_moves_outer_mass() {
        let (j, w) = transfer_sleeve_weights([3, 0, 0, 0], [1.0, 0.0, 0.0, 0.0], 3, 62, 0.82);
        let helper: f32 = (0..4).map(|i| if j[i] == 62 { w[i] } else { 0.0 }).sum();
        assert!((helper - 0.82).abs() < 0.02);
        let (j2, w2) = transfer_sleeve_weights([3, 1, 2, 4], [0.7, 0.1, 0.1, 0.1], 3, 9, 0.0);
        assert_eq!(j2, [3, 1, 2, 4]);
        assert_eq!(w2, [0.7, 0.1, 0.1, 0.1]);
    }

    #[test]
    fn helper_added_to_armless_vrm() {
        // 腕（humanoid）があり、袖チェーンが無いスキン → ヘルパー + チェーンが足される
        let mut skin = sleeve_fixture();
        let n0 = skin.nodes.len();
        let j0 = skin.skin_joints.len();
        let c0 = skin.springs.chains.len();
        ensure_sleeve_cloth(&mut skin);
        assert!(skin.nodes.len() > n0, "helper nodes added");
        assert!(skin.skin_joints.len() > j0, "helper palette added");
        assert!(skin.springs.chains.len() > c0, "sleeve spring chains added");
        // ウェイトが再正規化されている（合計 1）
        for w in &skin.weights {
            let s: f32 = w.iter().sum();
            assert!((s - 1.0).abs() < 1e-4, "weights renormalized, s={s}");
        }
    }

    #[test]
    fn helper_skips_when_sleeve_coverage_exists() {
        let mut skin = sleeve_fixture();
        ensure_sleeve_cloth(&mut skin);
        let n = skin.nodes.len();
        ensure_sleeve_cloth(&mut skin); // 2 回目は追加しない
        assert_eq!(skin.nodes.len(), n, "idempotent: no second helper pass");
    }

    /// 腕 2 本 + 手を持つ最小のヒューマノイドスキン。
    fn sleeve_fixture() -> SkinnedMesh {
        use crate::gltf_load::NodeRest;
        let node = |name: &str, trans: [f32; 3]| NodeRest {
            name: name.to_string(),
            children: vec![],
            translation: trans.into(),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
        let mut skin = SkinnedMesh {
            rest: crate::scene3d::MeshData::default(),
            joints: vec![],
            weights: vec![],
            inverse_bind: vec![],
            nodes: vec![
                node("hips", [0.0, 0.0, 0.0]),
                node("leftUpperArm", [0.2, 1.4, 0.0]),
                node("leftLowerArm", [0.2, 1.2, 0.0]),
                node("leftHand", [0.2, 1.0, 0.0]),
                node("rightUpperArm", [-0.2, 1.4, 0.0]),
                node("rightLowerArm", [-0.2, 1.2, 0.0]),
                node("rightHand", [-0.2, 1.0, 0.0]),
            ],
            skin_joints: vec![0, 1, 2, 3, 4, 5, 6],
            clip: None,
            humanoid: [
                ("hips", 0),
                ("leftUpperArm", 1),
                ("leftLowerArm", 2),
                ("leftHand", 3),
                ("rightUpperArm", 4),
                ("rightLowerArm", 5),
                ("rightHand", 6),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
            springs: Default::default(),
            morphs: vec![],
            expressions: Default::default(),
            look_at: None,
            constraints: vec![],
            first_person: Default::default(),
        };
        // 親子関係 + 袖の外側筒頂点（上腕軸から離れた位置）を 1 個置く
        for (i, parent) in [(1usize, 0usize), (2, 1), (3, 2), (4, 0), (5, 4), (6, 5)] {
            skin.nodes[parent].children.push(i);
        }
        skin.inverse_bind = vec![Mat4::IDENTITY; 7];
        // 左上腕の外側筒頂点（上腕軸から離れた位置）
        skin.rest.vertices.push(crate::scene3d::Vertex3 {
            pos: [0.24, 1.3, 0.06],
            normal: [0.0; 3],
            uv: [0.0; 2],
        });
        skin.joints.push([1, 0, 0, 0]);
        skin.weights.push([1.0, 0.0, 0.0, 0.0]);
        skin
    }
}
