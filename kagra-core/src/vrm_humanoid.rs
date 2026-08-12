//! VRM Humanoid ボーン解決
//!
//! VRoid はノード名が `J_Bip_*`、他ツールは `Head` / `mixamorig:Head` など様々。
//! VRM の `humanoid.humanBones` は標準名（`head`, `leftUpperArm` …）→ ノード index
//! を持つので、ロード時に標準名と `J_Bip_*` の両方を同じ index にエイリアスする。

use std::collections::HashMap;
use serde_json::Value;

/// VRM 標準ボーン名 → VRoid (`J_Bip_*`) 名
pub const STANDARD_TO_JBIP: &[(&str, &str)] = &[
    ("hips", "J_Bip_C_Hips"),
    ("spine", "J_Bip_C_Spine"),
    ("chest", "J_Bip_C_Chest"),
    ("upperChest", "J_Bip_C_UpperChest"),
    ("neck", "J_Bip_C_Neck"),
    ("head", "J_Bip_C_Head"),
    ("leftShoulder", "J_Bip_L_Shoulder"),
    ("leftUpperArm", "J_Bip_L_UpperArm"),
    ("leftLowerArm", "J_Bip_L_LowerArm"),
    ("leftHand", "J_Bip_L_Hand"),
    ("rightShoulder", "J_Bip_R_Shoulder"),
    ("rightUpperArm", "J_Bip_R_UpperArm"),
    ("rightLowerArm", "J_Bip_R_LowerArm"),
    ("rightHand", "J_Bip_R_Hand"),
    ("leftUpperLeg", "J_Bip_L_UpperLeg"),
    ("leftLowerLeg", "J_Bip_L_LowerLeg"),
    ("leftFoot", "J_Bip_L_Foot"),
    ("leftToes", "J_Bip_L_ToeBase"),
    ("rightUpperLeg", "J_Bip_R_UpperLeg"),
    ("rightLowerLeg", "J_Bip_R_LowerLeg"),
    ("rightFoot", "J_Bip_R_Foot"),
    ("rightToes", "J_Bip_R_ToeBase"),
    // 指（よく使うものだけ）
    ("leftThumbProximal", "J_Bip_L_Thumb1"),
    ("leftThumbIntermediate", "J_Bip_L_Thumb2"),
    ("leftThumbDistal", "J_Bip_L_Thumb3"),
    ("leftIndexProximal", "J_Bip_L_Index1"),
    ("leftIndexIntermediate", "J_Bip_L_Index2"),
    ("leftIndexDistal", "J_Bip_L_Index3"),
    ("leftMiddleProximal", "J_Bip_L_Middle1"),
    ("leftMiddleIntermediate", "J_Bip_L_Middle2"),
    ("leftMiddleDistal", "J_Bip_L_Middle3"),
    ("leftRingProximal", "J_Bip_L_Ring1"),
    ("leftRingIntermediate", "J_Bip_L_Ring2"),
    ("leftRingDistal", "J_Bip_L_Ring3"),
    ("leftLittleProximal", "J_Bip_L_Little1"),
    ("leftLittleIntermediate", "J_Bip_L_Little2"),
    ("leftLittleDistal", "J_Bip_L_Little3"),
    ("rightThumbProximal", "J_Bip_R_Thumb1"),
    ("rightThumbIntermediate", "J_Bip_R_Thumb2"),
    ("rightThumbDistal", "J_Bip_R_Thumb3"),
    ("rightIndexProximal", "J_Bip_R_Index1"),
    ("rightIndexIntermediate", "J_Bip_R_Index2"),
    ("rightIndexDistal", "J_Bip_R_Index3"),
    ("rightMiddleProximal", "J_Bip_R_Middle1"),
    ("rightMiddleIntermediate", "J_Bip_R_Middle2"),
    ("rightMiddleDistal", "J_Bip_R_Middle3"),
    ("rightRingProximal", "J_Bip_R_Ring1"),
    ("rightRingIntermediate", "J_Bip_R_Ring2"),
    ("rightRingDistal", "J_Bip_R_Ring3"),
    ("rightLittleProximal", "J_Bip_R_Little1"),
    ("rightLittleIntermediate", "J_Bip_R_Little2"),
    ("rightLittleDistal", "J_Bip_R_Little3"),
];

/// VRM 0.x / 1.0 の humanBones を (標準名 → ノード index) で返す。
pub fn parse_human_bones(gltf: &Value) -> HashMap<String, usize> {
    let mut map = HashMap::new();

    // VRM 1.0: extensions.VRMC_vrm.humanoid.humanBones = { "hips": {"node": N}, ... }
    if let Some(obj) = gltf
        .pointer("/extensions/VRMC_vrm/humanoid/humanBones")
        .and_then(|v| v.as_object())
    {
        for (name, entry) in obj {
            if let Some(node) = entry.get("node").and_then(|n| n.as_u64()) {
                map.insert(name.clone(), node as usize);
            }
        }
    }

    // VRM 0.x: extensions.VRM.humanoid.humanBones = [ {"bone":"hips","node":N}, ... ]
    if let Some(arr) = gltf
        .pointer("/extensions/VRM/humanoid/humanBones")
        .and_then(|v| v.as_array())
    {
        for entry in arr {
            let name = entry
                .get("bone")
                .and_then(|b| b.as_str())
                .unwrap_or("");
            if name.is_empty() {
                continue;
            }
            if let Some(node) = entry.get("node").and_then(|n| n.as_u64()) {
                map.entry(name.to_string()).or_insert(node as usize);
            }
        }
    }

    map
}

/// `bone_index` に標準名と `J_Bip_*` エイリアスを足す。
///
/// 既に同名キーがある場合は上書きしない（実ノード名を優先）。
/// humanBones 由来の標準名は必ず登録する。
pub fn apply_humanoid_aliases(
    bone_index: &mut HashMap<String, usize>,
    human_bones: &HashMap<String, usize>,
) {
    for (std_name, &node_idx) in human_bones {
        bone_index
            .entry(std_name.clone())
            .or_insert(node_idx);

        if let Some((_, jbip)) = STANDARD_TO_JBIP.iter().find(|(s, _)| *s == std_name) {
            bone_index
                .entry((*jbip).to_string())
                .or_insert(node_idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_vrm1_human_bones() {
        let gltf = json!({
            "extensions": {
                "VRMC_vrm": {
                    "humanoid": {
                        "humanBones": {
                            "hips": {"node": 10},
                            "head": {"node": 3}
                        }
                    }
                }
            }
        });
        let map = parse_human_bones(&gltf);
        assert_eq!(map.get("hips"), Some(&10));
        assert_eq!(map.get("head"), Some(&3));
    }

    #[test]
    fn parse_vrm0_human_bones() {
        let gltf = json!({
            "extensions": {
                "VRM": {
                    "humanoid": {
                        "humanBones": [
                            {"bone": "hips", "node": 7},
                            {"bone": "leftUpperArm", "node": 21}
                        ]
                    }
                }
            }
        });
        let map = parse_human_bones(&gltf);
        assert_eq!(map.get("hips"), Some(&7));
        assert_eq!(map.get("leftUpperArm"), Some(&21));
    }

    #[test]
    fn aliases_fill_jbip_without_clobbering_real_names() {
        let mut bone_index = HashMap::new();
        // 実ノードが別の名前
        bone_index.insert("MyHead".to_string(), 3usize);
        let human = HashMap::from([
            ("head".to_string(), 3usize),
            ("hips".to_string(), 10usize),
        ]);
        apply_humanoid_aliases(&mut bone_index, &human);
        assert_eq!(bone_index.get("head"), Some(&3));
        assert_eq!(bone_index.get("J_Bip_C_Head"), Some(&3));
        assert_eq!(bone_index.get("hips"), Some(&10));
        assert_eq!(bone_index.get("J_Bip_C_Hips"), Some(&10));
        // 実名は残る
        assert_eq!(bone_index.get("MyHead"), Some(&3));
    }

    #[test]
    fn aliases_do_not_overwrite_existing_jbip_node() {
        let mut bone_index = HashMap::new();
        // VRoid: J_Bip が実ノード名
        bone_index.insert("J_Bip_C_Head".to_string(), 78usize);
        let human = HashMap::from([("head".to_string(), 78usize)]);
        apply_humanoid_aliases(&mut bone_index, &human);
        assert_eq!(bone_index.get("J_Bip_C_Head"), Some(&78));
        assert_eq!(bone_index.get("head"), Some(&78));
    }
}
