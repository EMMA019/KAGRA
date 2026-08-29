//! VRM 1.0 表情オーバーライド（blink / mouth / lookAt）と isBinary。

use std::collections::HashMap;

use serde_json::Value;

/// `overrideBlink` / `overrideMouth` / `overrideLookAt`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OverrideMode {
    #[default]
    None,
    Block,
    Blend,
}

/// 表情がどの自動チャンネルに属するか。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ExpressionChannel {
    #[default]
    Other,
    Blink,
    Mouth,
    LookAt,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ExpressionMeta {
    pub override_blink: OverrideMode,
    pub override_mouth: OverrideMode,
    pub override_look_at: OverrideMode,
    pub is_binary: bool,
    pub channel: ExpressionChannel,
}

pub fn parse_override_mode(v: Option<&Value>) -> OverrideMode {
    match v.and_then(|x| x.as_str()).unwrap_or("none").to_ascii_lowercase().as_str() {
        "block" => OverrideMode::Block,
        "blend" => OverrideMode::Blend,
        _ => OverrideMode::None,
    }
}

pub fn infer_channel(name: &str) -> ExpressionChannel {
    let n = name.to_ascii_lowercase().replace('-', "_");
    if n.contains("blink") || n.contains("wink") {
        return ExpressionChannel::Blink;
    }
    if n == "lookup"
        || n == "lookdown"
        || n == "lookleft"
        || n == "lookright"
        || n == "look_up"
        || n == "look_down"
        || n == "look_left"
        || n == "look_right"
        || n.contains("look_up")
        || n.contains("look_down")
        || n.contains("look_left")
        || n.contains("look_right")
        || n.contains("lookup")
        || n.contains("lookdown")
        || n.contains("lookleft")
        || n.contains("lookright")
        || n.contains("eye_look")
    {
        return ExpressionChannel::LookAt;
    }
    if matches!(
        n.as_str(),
        "aa" | "ih" | "ou" | "ee" | "oh" | "a" | "i" | "u" | "e" | "o"
    ) || n.contains("fcl_mth_")
        || n.starts_with("vrc.v_")
    {
        return ExpressionChannel::Mouth;
    }
    ExpressionChannel::Other
}

pub fn meta_from_expr_json(name: &str, expr: &Value) -> ExpressionMeta {
    ExpressionMeta {
        override_blink: parse_override_mode(expr.get("overrideBlink")),
        override_mouth: parse_override_mode(expr.get("overrideMouth")),
        override_look_at: parse_override_mode(expr.get("overrideLookAt")),
        is_binary: expr.get("isBinary").and_then(|b| b.as_bool()).unwrap_or(false),
        channel: infer_channel(name),
    }
}

/// アクティブな表情ウェイトに override / isBinary を適用した実効ウェイト。
pub fn effective_expression_weights(
    active: &HashMap<String, f32>,
    meta: &HashMap<String, ExpressionMeta>,
) -> HashMap<String, f32> {
    let mut raw: HashMap<String, f32> = HashMap::new();
    for (name, &w) in active {
        if name.starts_with("__warned_") {
            continue;
        }
        let mut w = w.clamp(0.0, 1.0);
        if let Some(m) = meta.get(name) {
            if m.is_binary {
                w = if w > 0.5 { 1.0 } else { 0.0 };
            }
        }
        if w > 0.0 {
            raw.insert(name.clone(), w);
        }
    }

    let mut blink_block = 0.0f32;
    let mut blink_blend = 1.0f32;
    let mut mouth_block = 0.0f32;
    let mut mouth_blend = 1.0f32;
    let mut look_block = 0.0f32;
    let mut look_blend = 1.0f32;

    for (name, &w) in &raw {
        let Some(m) = meta.get(name) else { continue };
        match m.override_blink {
            OverrideMode::Block => blink_block = blink_block.max(w),
            OverrideMode::Blend => blink_blend *= 1.0 - w,
            OverrideMode::None => {}
        }
        match m.override_mouth {
            OverrideMode::Block => mouth_block = mouth_block.max(w),
            OverrideMode::Blend => mouth_blend *= 1.0 - w,
            OverrideMode::None => {}
        }
        match m.override_look_at {
            OverrideMode::Block => look_block = look_block.max(w),
            OverrideMode::Blend => look_blend *= 1.0 - w,
            OverrideMode::None => {}
        }
    }

    let mut out = HashMap::new();
    for (name, &w) in &raw {
        let channel = meta
            .get(name)
            .map(|m| m.channel)
            .unwrap_or(ExpressionChannel::Other);
        let ew = match channel {
            ExpressionChannel::Blink => {
                if blink_block > 0.0 {
                    0.0
                } else {
                    w * blink_blend
                }
            }
            ExpressionChannel::Mouth => {
                if mouth_block > 0.0 {
                    0.0
                } else {
                    w * mouth_blend
                }
            }
            ExpressionChannel::LookAt => {
                if look_block > 0.0 {
                    0.0
                } else {
                    w * look_blend
                }
            }
            ExpressionChannel::Other => w,
        };
        if ew > 1e-5 {
            out.insert(name.clone(), ew);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn infer_channels() {
        assert_eq!(infer_channel("blink"), ExpressionChannel::Blink);
        assert_eq!(infer_channel("blinkLeft"), ExpressionChannel::Blink);
        assert_eq!(infer_channel("Fcl_EYE_Blink"), ExpressionChannel::Blink);
        assert_eq!(infer_channel("aa"), ExpressionChannel::Mouth);
        assert_eq!(infer_channel("Fcl_MTH_A"), ExpressionChannel::Mouth);
        assert_eq!(infer_channel("lookLeft"), ExpressionChannel::LookAt);
        assert_eq!(infer_channel("happy"), ExpressionChannel::Other);
    }

    #[test]
    fn happy_blocks_blink() {
        let mut active = HashMap::new();
        active.insert("happy".into(), 1.0);
        active.insert("blink".into(), 1.0);
        let mut meta = HashMap::new();
        meta.insert(
            "happy".into(),
            meta_from_expr_json(
                "happy",
                &json!({"overrideBlink": "block", "overrideMouth": "none"}),
            ),
        );
        meta.insert("blink".into(), meta_from_expr_json("blink", &json!({})));
        let eff = effective_expression_weights(&active, &meta);
        assert!((eff["happy"] - 1.0).abs() < 1e-5);
        assert!(!eff.contains_key("blink"));
    }

    #[test]
    fn happy_blends_blink() {
        let mut active = HashMap::new();
        active.insert("happy".into(), 0.5);
        active.insert("blink".into(), 1.0);
        let mut meta = HashMap::new();
        meta.insert(
            "happy".into(),
            meta_from_expr_json("happy", &json!({"overrideBlink": "blend"})),
        );
        meta.insert("blink".into(), meta_from_expr_json("blink", &json!({})));
        let eff = effective_expression_weights(&active, &meta);
        assert!((eff["blink"] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn binary_snaps() {
        let mut active = HashMap::new();
        active.insert("happy".into(), 0.4);
        let mut meta = HashMap::new();
        meta.insert(
            "happy".into(),
            meta_from_expr_json("happy", &json!({"isBinary": true})),
        );
        let eff = effective_expression_weights(&active, &meta);
        assert!(eff.is_empty());
        active.insert("happy".into(), 0.6);
        let eff = effective_expression_weights(&active, &meta);
        assert!((eff["happy"] - 1.0).abs() < 1e-5);
    }
}
