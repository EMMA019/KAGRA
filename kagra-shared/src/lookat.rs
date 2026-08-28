//! Thin VRM 0 `firstPerson` lookAt / VRM 1 `VRMC_vrm.lookAt`.
//! Yaw/pitch head (eyes if present) toward a target. Neck uses Mixamo
//! rest+roll compensation (`retarget_delta`), not raw bind*delta.

use std::collections::HashMap;

use glam::{Quat, Vec3};
use serde_json::Value;

#[derive(Clone, Copy, Debug)]
pub struct RangeMap {
    pub input_max_value: f32,
    pub output_scale: f32,
}

impl Default for RangeMap {
    fn default() -> Self {
        Self {
            input_max_value: 90.0,
            output_scale: 10.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LookAt {
    pub look_at_type: String,
    pub offset_from_head_bone: [f32; 3],
    pub horizontal_inner: RangeMap,
    pub horizontal_outer: RangeMap,
    pub vertical_down: RangeMap,
    pub vertical_up: RangeMap,
}

impl Default for LookAt {
    fn default() -> Self {
        Self {
            look_at_type: "bone".into(),
            offset_from_head_bone: [0.0, 0.06, 0.0],
            horizontal_inner: RangeMap::default(),
            horizontal_outer: RangeMap::default(),
            vertical_down: RangeMap::default(),
            vertical_up: RangeMap::default(),
        }
    }
}

fn as_f32(v: Option<&Value>, default: f32) -> f32 {
    v.and_then(|x| x.as_f64())
        .map(|n| n as f32)
        .unwrap_or(default)
}

fn range_map_v1(v: Option<&Value>) -> RangeMap {
    let mut rm = RangeMap::default();
    let Some(v) = v else {
        return rm;
    };
    rm.input_max_value = as_f32(v.get("inputMaxValue"), rm.input_max_value);
    rm.output_scale = as_f32(v.get("outputScale"), rm.output_scale);
    rm
}

fn range_map_v0(v: Option<&Value>) -> RangeMap {
    let mut rm = RangeMap::default();
    let Some(v) = v else {
        return rm;
    };
    if v.get("xRange").is_some() {
        rm.input_max_value = as_f32(v.get("xRange"), rm.input_max_value);
    } else {
        rm.input_max_value = as_f32(v.get("inputMaxValue"), rm.input_max_value);
    }
    if v.get("yRange").is_some() {
        rm.output_scale = as_f32(v.get("yRange"), rm.output_scale);
    } else {
        rm.output_scale = as_f32(v.get("outputScale"), rm.output_scale);
    }
    rm
}

fn offset_v1(v: Option<&Value>) -> [f32; 3] {
    let Some(arr) = v.and_then(|v| v.as_array()) else {
        return [0.0, 0.06, 0.0];
    };
    [
        arr.first().and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
        arr.get(1).and_then(|x| x.as_f64()).unwrap_or(0.06) as f32,
        arr.get(2).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
    ]
}

fn offset_v0(v: Option<&Value>) -> [f32; 3] {
    let Some(obj) = v else {
        return [0.0, 0.06, 0.0];
    };
    if obj.is_array() {
        return offset_v1(Some(obj));
    }
    [
        as_f32(obj.get("x"), 0.0),
        as_f32(obj.get("y"), 0.06),
        as_f32(obj.get("z"), 0.0),
    ]
}

fn look_type(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "expression" | "blendshape" => "expression".into(),
        _ => "bone".into(),
    }
}

/// Parse VRM 1 `VRMC_vrm.lookAt` and/or VRM 0 `firstPerson` lookAt maps.
pub fn parse_look_at(extensions: Option<&Value>) -> Option<LookAt> {
    let ext = extensions?;
    if let Some(look) = ext.pointer("/VRMC_vrm/lookAt").filter(|v| v.is_object()) {
        let typ = look.get("type").and_then(|t| t.as_str()).unwrap_or("bone");
        return Some(LookAt {
            look_at_type: look_type(typ),
            offset_from_head_bone: offset_v1(look.get("offsetFromHeadBone")),
            horizontal_inner: range_map_v1(look.get("rangeMapHorizontalInner")),
            horizontal_outer: range_map_v1(look.get("rangeMapHorizontalOuter")),
            vertical_down: range_map_v1(look.get("rangeMapVerticalDown")),
            vertical_up: range_map_v1(look.get("rangeMapVerticalUp")),
        });
    }
    if let Some(fp) = ext.pointer("/VRM/firstPerson").filter(|v| v.is_object()) {
        let nested = fp.get("lookAt").filter(|v| v.is_object());
        let src = nested.unwrap_or(fp);
        let typ_raw = src
            .get("lookAtTypeName")
            .or_else(|| src.get("type"))
            .and_then(|t| t.as_str())
            .or_else(|| fp.get("lookAtTypeName").and_then(|t| t.as_str()))
            .unwrap_or("Bone");
        return Some(LookAt {
            look_at_type: look_type(typ_raw),
            offset_from_head_bone: offset_v0(
                src.get("firstPersonBoneOffset")
                    .or_else(|| src.get("offsetFromHeadBone"))
                    .or_else(|| fp.get("firstPersonBoneOffset")),
            ),
            horizontal_inner: range_map_v0(
                src.get("lookAtHorizontalInner")
                    .or_else(|| src.get("rangeMapHorizontalInner")),
            ),
            horizontal_outer: range_map_v0(
                src.get("lookAtHorizontalOuter")
                    .or_else(|| src.get("rangeMapHorizontalOuter")),
            ),
            vertical_down: range_map_v0(
                src.get("lookAtVerticalDown")
                    .or_else(|| src.get("rangeMapVerticalDown")),
            ),
            vertical_up: range_map_v0(
                src.get("lookAtVerticalUp")
                    .or_else(|| src.get("rangeMapVerticalUp")),
            ),
        });
    }
    None
}

pub fn head_node(humanoid: &HashMap<String, usize>) -> Option<usize> {
    bone(humanoid, &["head", "Head", "J_Bip_C_Head"])
}

pub fn neck_node(humanoid: &HashMap<String, usize>) -> Option<usize> {
    bone(humanoid, &["neck", "Neck", "J_Bip_C_Neck"])
}

pub fn eye_nodes(humanoid: &HashMap<String, usize>) -> [Option<usize>; 2] {
    [
        bone(
            humanoid,
            &["leftEye", "LeftEye", "J_Adj_L_FaceEye", "eye_L"],
        ),
        bone(
            humanoid,
            &["rightEye", "RightEye", "J_Adj_R_FaceEye", "eye_R"],
        ),
    ]
}

fn bone(humanoid: &HashMap<String, usize>, names: &[&str]) -> Option<usize> {
    for n in names {
        if let Some(&i) = humanoid.get(*n) {
            return Some(i);
        }
    }
    None
}

/// Head yaw/pitch in walker space: +yaw looks right, +pitch looks up.
/// "Right" follows the chase-cam convention: camera at look - fwd*dist looks
/// along +fwd, so screen-right = fwd × up = (-c, 0, s).
pub fn yaw_pitch_toward(from: Vec3, to: Vec3, facing_yaw: f32) -> (f32, f32) {
    let d = to - from;
    if d.length_squared() < 1e-10 {
        return (0.0, 0.0);
    }
    let d = d.normalize();
    let (s, c) = facing_yaw.sin_cos();
    let forward = Vec3::new(s, 0.0, c);
    let right = Vec3::new(-c, 0.0, s);
    let local = Vec3::new(d.dot(right), d.dot(Vec3::Y), d.dot(forward));
    let horiz = (local.x * local.x + local.z * local.z).sqrt();
    let yaw = local.x.atan2(local.z);
    let pitch = local.y.atan2(horiz.max(1e-8));
    (yaw, pitch)
}

/// Clamp head yaw/pitch to lookAt inputMaxValue (degrees).
pub fn clamp_head(look: &LookAt, yaw: f32, pitch: f32) -> (f32, f32) {
    let yaw_max = look.horizontal_outer.input_max_value.to_radians().max(1e-4);
    let pitch_up = look.vertical_up.input_max_value.to_radians().max(1e-4);
    let pitch_down = look.vertical_down.input_max_value.to_radians().max(1e-4);
    (
        yaw.clamp(-yaw_max, yaw_max),
        pitch.clamp(-pitch_down, pitch_up),
    )
}

fn apply_range(angle_rad: f32, rm: &RangeMap) -> f32 {
    let deg = angle_rad.to_degrees();
    let max = rm.input_max_value.abs().max(1e-4);
    let t = (deg / max).clamp(-1.0, 1.0);
    (t * rm.output_scale).to_radians()
}

/// Eye bone yaw/pitch from range maps (outputScale degrees).
pub fn map_eyes(look: &LookAt, yaw: f32, pitch: f32) -> (f32, f32) {
    let rm_y = if yaw >= 0.0 {
        &look.horizontal_outer
    } else {
        &look.horizontal_inner
    };
    let rm_p = if pitch >= 0.0 {
        &look.vertical_up
    } else {
        &look.vertical_down
    };
    (apply_range(yaw, rm_y), apply_range(pitch, rm_p))
}

pub fn look_quat(yaw: f32, pitch: f32) -> Quat {
    Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch)
}

/// Mixamo rest+roll: `local = current * retarget_delta(src, I, world_rest)`.
/// At rest `current == bind`, so this is `bind * delta_dst`, not raw `bind * src`.
pub fn compensated_local(current: Quat, world_rest: Quat, delta_src: Quat) -> Quat {
    let delta_dst = crate::mixamo::retarget_delta(delta_src, Quat::IDENTITY, world_rest);
    (current * delta_dst).normalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_v0_first_person_and_v1_look_at() {
        let ext = json!({
            "VRM": {
                "firstPerson": {
                    "lookAtTypeName": "Bone",
                    "firstPersonBoneOffset": {"x": 0.0, "y": 0.06, "z": 0.0},
                    "lookAtHorizontalOuter": {"xRange": 90.0, "yRange": 10.0}
                }
            }
        });
        let v0 = parse_look_at(Some(&ext)).expect("v0");
        assert_eq!(v0.look_at_type, "bone");
        assert!((v0.horizontal_outer.input_max_value - 90.0).abs() < 1e-4);
        assert!((v0.horizontal_outer.output_scale - 10.0).abs() < 1e-4);

        let ext1 = json!({
            "VRMC_vrm": {
                "lookAt": {
                    "type": "bone",
                    "offsetFromHeadBone": [0.0, 0.08, 0.0],
                    "rangeMapHorizontalInner": {"inputMaxValue": 80.0, "outputScale": 12.0},
                    "rangeMapHorizontalOuter": {"inputMaxValue": 80.0, "outputScale": 12.0},
                    "rangeMapVerticalDown": {"inputMaxValue": 70.0, "outputScale": 8.0},
                    "rangeMapVerticalUp": {"inputMaxValue": 70.0, "outputScale": 8.0}
                }
            }
        });
        let v1 = parse_look_at(Some(&ext1)).expect("v1");
        assert_eq!(v1.look_at_type, "bone");
        assert!((v1.offset_from_head_bone[1] - 0.08).abs() < 1e-5);
        assert!((v1.horizontal_inner.input_max_value - 80.0).abs() < 1e-4);
        assert!((v1.vertical_up.output_scale - 8.0).abs() < 1e-4);
    }

    #[test]
    fn camera_behind_is_yaw_pi_clamped_to_90() {
        let from = Vec3::new(0.0, 1.2, 0.0);
        let cam = Vec3::new(0.0, 4.4, -12.2);
        let (yaw, pitch) = yaw_pitch_toward(from, cam, 0.0);
        assert!(yaw.abs() > 2.5, "chase cam is behind facing +Z, yaw={yaw}");
        assert!(pitch > 0.1, "cam is above head, pitch={pitch}");
        let look = LookAt::default();
        let (cy, _cp) = clamp_head(&look, yaw, pitch);
        assert!((cy.abs() - 90f32.to_radians()).abs() < 1e-3);
        let (ey, _) = map_eyes(&look, yaw, pitch);
        assert!(
            ey.abs() <= 10f32.to_radians() + 1e-4,
            "eyes use outputScale, ey={ey}"
        );
    }

    #[test]
    fn camera_on_screen_right_yaws_head_right() {
        let from = Vec3::new(0.0, 1.2, 0.0);
        // Walker faces +Z (yaw=0); chase cam sits behind at -Z, so screen-right
        // = fwd × up = -X. A camera to the walker's screen-right must yaw the
        // head right (+), not mirror it to the left.
        let cam_right = Vec3::new(-6.0, 1.6, 0.0);
        let (yaw, _pitch) = yaw_pitch_toward(from, cam_right, 0.0);
        assert!(
            yaw > 0.0,
            "screen-right cam must yaw the head right, yaw={yaw}"
        );
        let cam_left = Vec3::new(6.0, 1.6, 0.0);
        let (yaw2, _pitch2) = yaw_pitch_toward(from, cam_left, 0.0);
        assert!(
            yaw2 < 0.0,
            "screen-left cam must yaw the head left, yaw={yaw2}"
        );
    }

    #[test]
    fn rolled_neck_is_not_raw_bind_delta() {
        let world = Quat::from_axis_angle(Vec3::X, 1.1);
        let src = Quat::from_rotation_y(0.5);
        let raw = src;
        let dst = crate::mixamo::retarget_delta(src, Quat::IDENTITY, world);
        let d = (raw.xyz() - dst.xyz()).length();
        assert!(d > 0.05, "rolled rest must change delta, d={d}");
        let ident = crate::mixamo::retarget_delta(src, Quat::IDENTITY, Quat::IDENTITY);
        assert!(src.dot(ident).abs() > 0.99);
        let applied = compensated_local(Quat::IDENTITY, world, src);
        assert!((applied.dot(dst)).abs() > 0.99);
    }
}
