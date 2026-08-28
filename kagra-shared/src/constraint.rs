//! VRMC_node_constraint 1.0 (rotation / roll / aim). Port of kagra-core
//! vrm_constraint.rs onto glam. Aim parses but is not applied yet (no look
//! source in the shared runtime).

use glam::{Quat, Vec3};
use serde_json::Value;

#[derive(Clone, Copy, Debug)]
pub enum ConstraintKind {
    Rotation {
        source: usize,
        weight: f32,
    },
    Roll {
        source: usize,
        weight: f32,
        axis: [f32; 3],
    },
    Aim {
        source: usize,
        weight: f32,
        aim_axis: [f32; 3],
    },
}

#[derive(Clone, Copy, Debug)]
pub struct NodeConstraint {
    pub dest: usize,
    pub kind: ConstraintKind,
}

fn parse_axis(v: Option<&Value>, default: [f32; 3]) -> [f32; 3] {
    match v.and_then(|x| x.as_str()).unwrap_or("") {
        "PositiveX" | "X" => [1.0, 0.0, 0.0],
        "NegativeX" => [-1.0, 0.0, 0.0],
        "PositiveY" | "Y" => [0.0, 1.0, 0.0],
        "NegativeY" => [0.0, -1.0, 0.0],
        "PositiveZ" | "Z" => [0.0, 0.0, 1.0],
        "NegativeZ" => [0.0, 0.0, -1.0],
        _ => default,
    }
}

fn parse_one(node_idx: usize, ext: &Value, out: &mut Vec<NodeConstraint>) {
    let Some(c) = ext.get("constraint") else {
        return;
    };
    if let Some(rot) = c.get("rotation") {
        let source = rot
            .get("source")
            .and_then(|s| s.as_u64())
            .unwrap_or(u64::MAX) as usize;
        let weight = rot.get("weight").and_then(|w| w.as_f64()).unwrap_or(1.0) as f32;
        if source != usize::MAX {
            out.push(NodeConstraint {
                dest: node_idx,
                kind: ConstraintKind::Rotation { source, weight },
            });
        }
    }
    if let Some(roll) = c.get("roll") {
        let source = roll
            .get("source")
            .and_then(|s| s.as_u64())
            .unwrap_or(u64::MAX) as usize;
        let weight = roll.get("weight").and_then(|w| w.as_f64()).unwrap_or(1.0) as f32;
        let axis = parse_axis(roll.get("rollAxis"), [0.0, 1.0, 0.0]);
        if source != usize::MAX {
            out.push(NodeConstraint {
                dest: node_idx,
                kind: ConstraintKind::Roll {
                    source,
                    weight,
                    axis,
                },
            });
        }
    }
    if let Some(aim) = c.get("aim") {
        let source = aim
            .get("source")
            .and_then(|s| s.as_u64())
            .unwrap_or(u64::MAX) as usize;
        let weight = aim.get("weight").and_then(|w| w.as_f64()).unwrap_or(1.0) as f32;
        let aim_axis = parse_axis(aim.get("aimAxis"), [0.0, 0.0, 1.0]);
        if source != usize::MAX {
            out.push(NodeConstraint {
                dest: node_idx,
                kind: ConstraintKind::Aim {
                    source,
                    weight,
                    aim_axis,
                },
            });
        }
    }
}

/// Parse `extensions.VRMC_node_constraint` from every node.
pub fn parse_node_constraints(gltf: &Value) -> Vec<NodeConstraint> {
    let mut out = Vec::new();
    if let Some(nodes) = gltf.get("nodes").and_then(|v| v.as_array()) {
        for (i, node) in nodes.iter().enumerate() {
            if let Some(ext) = node.pointer("/extensions/VRMC_node_constraint") {
                parse_one(i, ext, &mut out);
            }
        }
    }
    out
}

/// Parse from per-node `extensions` values (gltf_load `GltfNode.extensions`).
pub fn parse_from_node_extensions(exts: &[Option<Value>]) -> Vec<NodeConstraint> {
    let mut out = Vec::new();
    for (i, ext) in exts.iter().enumerate() {
        if let Some(ext) = ext {
            if let Some(c) = ext.get("VRMC_node_constraint") {
                parse_one(i, c, &mut out);
            }
        }
    }
    out
}

/// `dst = slerp(dst_rest, srcDelta * dst_rest, weight)`.
pub fn apply_rotation(src_local: Quat, src_rest: Quat, dst_rest: Quat, weight: f32) -> Quat {
    let src_delta = src_local * src_rest.inverse();
    let target = src_delta * dst_rest;
    dst_rest.slerp(target, weight)
}

/// Copy only the source's roll (twist) around `axis`.
pub fn apply_roll(
    src_local: Quat,
    src_rest: Quat,
    dst_rest: Quat,
    axis: Vec3,
    weight: f32,
) -> Quat {
    let src_delta = src_local * src_rest.inverse();
    let delta_axis = src_delta * axis;
    let to_src = Quat::from_rotation_arc(axis, delta_axis);
    let roll = to_src.inverse() * src_delta;
    let target = roll * dst_rest;
    dst_rest.slerp(target, weight)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_roll_constraint() {
        let gltf = json!({
            "nodes": [
                {"name": "hand"},
                {
                    "name": "twist",
                    "extensions": {
                        "VRMC_node_constraint": {
                            "constraint": {
                                "roll": {"source": 0, "rollAxis": "Y", "weight": 0.8}
                            }
                        }
                    }
                }
            ]
        });
        let cs = parse_node_constraints(&gltf);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].dest, 1);
        match cs[0].kind {
            ConstraintKind::Roll {
                source,
                weight,
                axis,
            } => {
                assert_eq!(source, 0);
                assert!((weight - 0.8).abs() < 1e-5);
                assert!((axis[1] - 1.0).abs() < 1e-5);
            }
            _ => panic!("expected roll"),
        }
    }

    #[test]
    fn rotation_copies_delta() {
        // ソースが X 90°、rest は identity → dest も同じ回転
        let s = (0.5f32).sqrt();
        let src = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let rest = Quat::IDENTITY;
        let out = apply_rotation(src, rest, rest, 1.0);
        assert!((out.x - s).abs() < 1e-4);
        assert!((out.w - s).abs() < 1e-4);
    }

    #[test]
    fn roll_extracts_twist() {
        let src = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        let rest = Quat::IDENTITY;
        // X 軸ロール: X 回転はそのまま X ロールとして出る。
        let out = apply_roll(src, rest, rest, Vec3::X, 1.0);
        assert!((out.x - src.x).abs() < 1e-3, "x={} src_x={}", out.x, src.x);
        assert!((out.w - src.w).abs() < 1e-3);
        // 直交軸 (Y) にはロール成分が出ない。
        let out_y = apply_roll(src, rest, rest, Vec3::Y, 1.0);
        assert!(
            out_y.x.abs() < 1e-3,
            "y-roll must be identity-ish, x={}",
            out_y.x
        );
    }
}
