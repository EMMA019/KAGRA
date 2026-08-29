//! VRM firstPerson メッシュ注釈と、Auto 時の頭部三角形除去。

use std::collections::{HashMap, HashSet};

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MeshAnnotation {
    #[default]
    Auto,
    Both,
    FirstPersonOnly,
    ThirdPersonOnly,
}

impl MeshAnnotation {
    fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "both" => Self::Both,
            "firstpersononly" | "first_person_only" => Self::FirstPersonOnly,
            "thirdpersononly" | "third_person_only" => Self::ThirdPersonOnly,
            _ => Self::Auto,
        }
    }
}

/// mesh index → 注釈、node index → 注釈。
pub fn parse_mesh_annotations(gltf: &Value) -> (HashMap<usize, MeshAnnotation>, HashMap<usize, MeshAnnotation>) {
    let mut by_mesh = HashMap::new();
    let mut by_node = HashMap::new();

    if let Some(arr) = gltf
        .pointer("/extensions/VRMC_vrm/firstPerson/meshAnnotations")
        .and_then(|v| v.as_array())
    {
        for a in arr {
            let typ = a
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("auto");
            let flag = MeshAnnotation::from_str(typ);
            if let Some(n) = a.get("node").and_then(|x| x.as_u64()) {
                by_node.insert(n as usize, flag);
            }
            if let Some(m) = a.get("mesh").and_then(|x| x.as_u64()) {
                by_mesh.insert(m as usize, flag);
            }
        }
    }

    if let Some(arr) = gltf
        .pointer("/extensions/VRM/firstPerson/meshAnnotations")
        .and_then(|v| v.as_array())
    {
        for a in arr {
            let typ = a
                .get("firstPersonFlag")
                .or_else(|| a.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("Auto");
            let flag = MeshAnnotation::from_str(typ);
            if let Some(m) = a.get("mesh").and_then(|x| x.as_u64()) {
                by_mesh.entry(m as usize).or_insert(flag);
            }
            if let Some(n) = a.get("node").and_then(|x| x.as_u64()) {
                by_node.entry(n as usize).or_insert(flag);
            }
        }
    }

    (by_mesh, by_node)
}

/// head / 目 / 顎とその子孫ノード。
pub fn collect_head_nodes(
    parents: &[Option<usize>],
    human_bones: &HashMap<String, usize>,
) -> HashSet<usize> {
    let mut roots = Vec::new();
    for name in ["head", "leftEye", "rightEye", "jaw"] {
        if let Some(&i) = human_bones.get(name) {
            roots.push(i);
        }
    }
    if roots.is_empty() {
        return HashSet::new();
    }
    let mut head = HashSet::new();
    for (i, _parent) in parents.iter().enumerate() {
        let mut walk = Some(i);
        let mut hops = 0;
        while let Some(n) = walk {
            if roots.contains(&n) {
                head.insert(i);
                break;
            }
            walk = parents.get(n).and_then(|p| *p);
            hops += 1;
            if hops > parents.len() {
                break;
            }
        }
    }
    head
}

/// 頭ボーンに乗っている頂点を含む三角形を落とす。
pub fn erase_head_triangles(
    indices: &[u32],
    joints: &[[u32; 4]],
    weights: &[[f32; 4]],
    skin_joints: &[usize],
    head_nodes: &HashSet<usize>,
    threshold: f32,
) -> Vec<u32> {
    if head_nodes.is_empty() || indices.len() < 3 {
        return indices.to_vec();
    }
    let n = joints.len().min(weights.len());
    let mut is_head = vec![false; n];
    for i in 0..n {
        let mut wsum = 0.0f32;
        for k in 0..4 {
            let ji = joints[i][k] as usize;
            if let Some(&node) = skin_joints.get(ji) {
                if head_nodes.contains(&node) {
                    wsum += weights[i][k];
                }
            }
        }
        is_head[i] = wsum > threshold;
    }
    let mut out = Vec::with_capacity(indices.len());
    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            break;
        }
        let a = tri[0] as usize;
        let b = tri[1] as usize;
        let c = tri[2] as usize;
        let drop = [a, b, c]
            .iter()
            .any(|&i| i < is_head.len() && is_head[i]);
        if !drop {
            out.extend_from_slice(tri);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_v1_and_v0() {
        let gltf = json!({
            "extensions": {
                "VRMC_vrm": {
                    "firstPerson": {
                        "meshAnnotations": [
                            {"node": 2, "type": "thirdPersonOnly"},
                            {"node": 3, "type": "firstPersonOnly"}
                        ]
                    }
                },
                "VRM": {
                    "firstPerson": {
                        "meshAnnotations": [
                            {"mesh": 0, "firstPersonFlag": "Auto"}
                        ]
                    }
                }
            }
        });
        let (by_mesh, by_node) = parse_mesh_annotations(&gltf);
        assert_eq!(by_node.get(&2).copied(), Some(MeshAnnotation::ThirdPersonOnly));
        assert_eq!(by_node.get(&3).copied(), Some(MeshAnnotation::FirstPersonOnly));
        assert_eq!(by_mesh.get(&0).copied(), Some(MeshAnnotation::Auto));
    }

    #[test]
    fn erase_weighted_head() {
        let indices = [0u32, 1, 2, 3, 4, 5];
        let joints = [
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [0, 0, 0, 0],
            [1, 0, 0, 0],
            [1, 0, 0, 0],
            [1, 0, 0, 0],
        ];
        let weights = [
            [1.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
        ];
        let skin = vec![10usize, 20];
        let mut head = HashSet::new();
        head.insert(10);
        let kept = erase_head_triangles(&indices, &joints, &weights, &skin, &head, 0.2);
        assert_eq!(kept, vec![3, 4, 5]);
    }
}
