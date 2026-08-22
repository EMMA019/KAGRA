//! VRMC_node_constraint 1.0（rotation / roll / aim）。

use serde_json::Value;

#[derive(Clone, Copy, Debug)]
pub enum ConstraintKind {
    Rotation { source: usize, weight: f32 },
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
    let Some(c) = ext.get("constraint") else { return };
    if let Some(rot) = c.get("rotation") {
        let source = rot.get("source").and_then(|s| s.as_u64()).unwrap_or(u64::MAX) as usize;
        let weight = rot.get("weight").and_then(|w| w.as_f64()).unwrap_or(1.0) as f32;
        if source != usize::MAX {
            out.push(NodeConstraint {
                dest: node_idx,
                kind: ConstraintKind::Rotation { source, weight },
            });
        }
    }
    if let Some(roll) = c.get("roll") {
        let source = roll.get("source").and_then(|s| s.as_u64()).unwrap_or(u64::MAX) as usize;
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
        let source = aim.get("source").and_then(|s| s.as_u64()).unwrap_or(u64::MAX) as usize;
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

/// 各ノードの `extensions.VRMC_node_constraint` を集める。
pub fn parse_node_constraints(gltf: &Value) -> Vec<NodeConstraint> {
    let mut out = Vec::new();
    let Some(nodes) = gltf.get("nodes").and_then(|n| n.as_array()) else {
        return out;
    };
    for (i, node) in nodes.iter().enumerate() {
        if let Some(ext) = node.pointer("/extensions/VRMC_node_constraint") {
            parse_one(i, ext, &mut out);
        }
    }
    out
}

pub fn has_aim(constraints: &[NodeConstraint]) -> bool {
    constraints
        .iter()
        .any(|c| matches!(c.kind, ConstraintKind::Aim { .. }))
}

// ── xyzw クォータニオン ──────────────────────────────────────

pub fn qmul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

pub fn qconj(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

pub fn qnorm(q: [f32; 4]) -> [f32; 4] {
    let l = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if l < 1e-8 {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        [q[0] / l, q[1] / l, q[2] / l, q[3] / l]
    }
}

pub fn qrotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let [qx, qy, qz, qw] = q;
    let [vx, vy, vz] = v;
    let tx = 2.0 * (qy * vz - qz * vy);
    let ty = 2.0 * (qz * vx - qx * vz);
    let tz = 2.0 * (qx * vy - qy * vx);
    [
        vx + qw * tx + (qy * tz - qz * ty),
        vy + qw * ty + (qz * tx - qx * tz),
        vz + qw * tz + (qx * ty - qy * tx),
    ]
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

fn vcross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn q_from_to(from: [f32; 3], to: [f32; 3]) -> [f32; 4] {
    let f = vnorm(from);
    let t = vnorm(to);
    let d = vdot(f, t).clamp(-1.0, 1.0);
    if d > 0.9999 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    if d < -0.9999 {
        let mut perp = vcross(f, [1.0, 0.0, 0.0]);
        if vlen(perp) < 0.001 {
            perp = vcross(f, [0.0, 1.0, 0.0]);
        }
        let p = vnorm(perp);
        return [p[0], p[1], p[2], 0.0];
    }
    let ax = vnorm(vcross(f, t));
    let ang = d.acos();
    let s = (ang * 0.5).sin();
    qnorm([ax[0] * s, ax[1] * s, ax[2] * s, (ang * 0.5).cos()])
}

pub fn qslerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    let mut b = b;
    let mut dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    if dot < 0.0 {
        b = [-b[0], -b[1], -b[2], -b[3]];
        dot = -dot;
    }
    if dot > 0.9995 {
        return qnorm([
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
            a[3] + (b[3] - a[3]) * t,
        ]);
    }
    let theta = dot.clamp(-1.0, 1.0).acos();
    let sin_t = theta.sin();
    let w0 = ((1.0 - t) * theta).sin() / sin_t;
    let w1 = (t * theta).sin() / sin_t;
    qnorm([
        a[0] * w0 + b[0] * w1,
        a[1] * w0 + b[1] * w1,
        a[2] * w0 + b[2] * w1,
        a[3] * w0 + b[3] * w1,
    ])
}

/// `dst = slerp(dst.rest, srcDelta * dst.rest, weight)`
pub fn apply_rotation(
    src_local: [f32; 4],
    src_rest: [f32; 4],
    dst_rest: [f32; 4],
    weight: f32,
) -> [f32; 4] {
    let src_delta = qmul(src_local, qconj(src_rest));
    let target = qmul(src_delta, dst_rest);
    qslerp(dst_rest, target, weight)
}

/// ソースのロール成分だけをコピーする。
pub fn apply_roll(
    src_local: [f32; 4],
    src_rest: [f32; 4],
    dst_rest: [f32; 4],
    axis: [f32; 3],
    weight: f32,
) -> [f32; 4] {
    let src_delta = qmul(src_local, qconj(src_rest));
    let delta_axis = qrotate(src_delta, axis);
    let to_src = q_from_to(axis, delta_axis);
    let roll = qmul(qconj(to_src), src_delta);
    let target = qmul(roll, dst_rest);
    qslerp(dst_rest, target, weight)
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
            ConstraintKind::Roll { source, weight, axis } => {
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
        let src = [s, 0.0, 0.0, s]; // 90° X
        let rest = [0.0, 0.0, 0.0, 1.0];
        let out = apply_rotation(src, rest, rest, 1.0);
        assert!((out[0] - s).abs() < 1e-4);
        assert!((out[3] - s).abs() < 1e-4);
    }

    #[test]
    fn roll_extracts_twist() {
        let s = (0.5f32).sqrt();
        let src = [s, 0.0, 0.0, s]; // 90° X
        let rest = [0.0, 0.0, 0.0, 1.0];
        let out = apply_roll(src, rest, rest, [1.0, 0.0, 0.0], 1.0);
        // 軸が X なので 90° X はそのままロール
        assert!((out[0] - s).abs() < 1e-4);
        assert!((out[3] - s).abs() < 1e-4);
    }
}
