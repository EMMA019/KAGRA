//! VRM LookAt メタデータ（VRM 1.0 `VRMC_vrm.lookAt` / VRM 0.x `VRM.firstPerson`）

use serde_json::Value;

/// 1 軸の range map（角度入力 → ボーン/表情スケール）
#[derive(Clone, Debug)]
pub struct VrmLookAtRangeMap {
    pub input_max_value: f32,
    pub output_scale: f32,
}

impl Default for VrmLookAtRangeMap {
    fn default() -> Self {
        Self {
            input_max_value: 90.0,
            output_scale: 10.0,
        }
    }
}

/// VRM LookAt 拡張メタデータ
#[derive(Clone, Debug)]
pub struct VrmLookAtMeta {
    /// `"bone"` | `"expression"`
    pub look_at_type: String,
    pub offset_from_head_bone: [f32; 3],
    pub range_map_horizontal_inner: VrmLookAtRangeMap,
    pub range_map_horizontal_outer: VrmLookAtRangeMap,
    pub range_map_vertical_down: VrmLookAtRangeMap,
    pub range_map_vertical_up: VrmLookAtRangeMap,
}

impl Default for VrmLookAtMeta {
    fn default() -> Self {
        Self {
            look_at_type: "bone".to_string(),
            offset_from_head_bone: [0.0, 0.06, 0.0],
            range_map_horizontal_inner: VrmLookAtRangeMap::default(),
            range_map_horizontal_outer: VrmLookAtRangeMap::default(),
            range_map_vertical_down: VrmLookAtRangeMap::default(),
            range_map_vertical_up: VrmLookAtRangeMap::default(),
        }
    }
}

fn parse_range_map_v1(v: Option<&Value>) -> VrmLookAtRangeMap {
    let mut rm = VrmLookAtRangeMap::default();
    let Some(v) = v else { return rm };
    if let Some(x) = v.get("inputMaxValue").and_then(|x| x.as_f64()) {
        rm.input_max_value = x as f32;
    }
    if let Some(x) = v.get("outputScale").and_then(|x| x.as_f64()) {
        rm.output_scale = x as f32;
    }
    rm
}

/// VRM 0.x: `xRange` = inputMaxValue, `yRange` = outputScale
fn parse_range_map_v0(v: Option<&Value>) -> VrmLookAtRangeMap {
    let mut rm = VrmLookAtRangeMap::default();
    let Some(v) = v else { return rm };
    if let Some(x) = v.get("xRange").and_then(|x| x.as_f64()) {
        rm.input_max_value = x as f32;
    } else if let Some(x) = v.get("inputMaxValue").and_then(|x| x.as_f64()) {
        rm.input_max_value = x as f32;
    }
    if let Some(y) = v.get("yRange").and_then(|y| y.as_f64()) {
        rm.output_scale = y as f32;
    } else if let Some(y) = v.get("outputScale").and_then(|y| y.as_f64()) {
        rm.output_scale = y as f32;
    }
    rm
}

fn parse_offset_v1(v: Option<&Value>) -> [f32; 3] {
    let Some(arr) = v.and_then(|v| v.as_array()) else {
        return [0.0, 0.06, 0.0];
    };
    [
        arr.first().and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
        arr.get(1).and_then(|x| x.as_f64()).unwrap_or(0.06) as f32,
        arr.get(2).and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
    ]
}

fn parse_offset_v0(v: Option<&Value>) -> [f32; 3] {
    let Some(obj) = v else {
        return [0.0, 0.06, 0.0];
    };
    if obj.is_array() {
        return parse_offset_v1(Some(obj));
    }
    [
        obj.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
        obj.get("y").and_then(|x| x.as_f64()).unwrap_or(0.06) as f32,
        obj.get("z").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32,
    ]
}

/// `extensions.VRMC_vrm.lookAt` または `extensions.VRM.firstPerson` をパース
pub fn parse_look_at(gltf: &Value) -> Option<VrmLookAtMeta> {
    // VRM 1.0
    if let Some(look) = gltf
        .pointer("/extensions/VRMC_vrm/lookAt")
        .filter(|v| v.is_object())
    {
        let typ = look
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("bone")
            .to_lowercase();
        let look_at_type = match typ.as_str() {
            "expression" | "blendshape" => "expression".to_string(),
            _ => "bone".to_string(),
        };
        return Some(VrmLookAtMeta {
            look_at_type,
            offset_from_head_bone: parse_offset_v1(look.get("offsetFromHeadBone")),
            range_map_horizontal_inner: parse_range_map_v1(look.get("rangeMapHorizontalInner")),
            range_map_horizontal_outer: parse_range_map_v1(look.get("rangeMapHorizontalOuter")),
            range_map_vertical_down: parse_range_map_v1(look.get("rangeMapVerticalDown")),
            range_map_vertical_up: parse_range_map_v1(look.get("rangeMapVerticalUp")),
        });
    }

    // VRM 0.x
    if let Some(fp) = gltf
        .pointer("/extensions/VRM/firstPerson")
        .filter(|v| v.is_object())
    {
        let typ_raw = fp
            .get("lookAtTypeName")
            .and_then(|t| t.as_str())
            .unwrap_or("Bone");
        let look_at_type = match typ_raw.to_lowercase().as_str() {
            "blendshape" | "expression" => "expression".to_string(),
            _ => "bone".to_string(),
        };
        return Some(VrmLookAtMeta {
            look_at_type,
            offset_from_head_bone: parse_offset_v0(fp.get("firstPersonBoneOffset")),
            range_map_horizontal_inner: parse_range_map_v0(fp.get("lookAtHorizontalInner")),
            range_map_horizontal_outer: parse_range_map_v0(fp.get("lookAtHorizontalOuter")),
            range_map_vertical_down: parse_range_map_v0(fp.get("lookAtVerticalDown")),
            range_map_vertical_up: parse_range_map_v0(fp.get("lookAtVerticalUp")),
        });
    }

    None
}
