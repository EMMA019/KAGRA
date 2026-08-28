//! Thin VRM 0 `blendShapeMaster` / VRM 1 `VRMC_vrm` expressions.
//! Named shape (blink / aa) onto CPU-skinned positions. No look-at, no RendererV2.

use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct MorphBind {
    pub index: usize,
    pub weight: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Expressions {
    pub by_name: HashMap<String, Vec<MorphBind>>,
}

impl Expressions {
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }

    /// Official presets first: blink, then aa.
    pub fn pick(&self) -> Option<&str> {
        for n in ["blink", "aa"] {
            if self.by_name.contains_key(n) {
                return Some(n);
            }
        }
        self.by_name.keys().next().map(String::as_str)
    }

    /// Named preset binds. Names are stored lowercased. None = model lacks it.
    pub fn get(&self, name: &str) -> Option<&Vec<MorphBind>> {
        self.by_name.get(&name.trim().to_ascii_lowercase())
    }

    /// Whether the model has this expression (including the auto blink fallback).
    pub fn has(&self, name: &str) -> bool {
        name.eq_ignore_ascii_case("blink") || self.get(name).is_some()
    }
}

fn as_usize(v: Option<&Value>) -> Option<usize> {
    v.and_then(|x| x.as_u64()).map(|n| n as usize)
}

fn as_f32(v: Option<&Value>, default: f32) -> f32 {
    v.and_then(|x| x.as_f64())
        .map(|n| n as f32)
        .unwrap_or(default)
}

fn insert_binds(out: &mut Expressions, name: &str, binds: Vec<MorphBind>) {
    let key = name.trim().to_ascii_lowercase();
    if key.is_empty() || binds.is_empty() {
        return;
    }
    out.by_name.entry(key).or_insert(binds);
}

fn v0_binds(group: &Value) -> Vec<MorphBind> {
    let Some(arr) = group.get("binds").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for b in arr {
        let mesh = as_usize(b.get("mesh")).unwrap_or(0);
        if mesh != 0 {
            continue;
        }
        let Some(index) = as_usize(b.get("index")) else {
            continue;
        };
        let w = as_f32(b.get("weight"), 100.0) / 100.0;
        out.push(MorphBind { index, weight: w });
    }
    out
}

fn v1_binds(expr: &Value) -> Vec<MorphBind> {
    let Some(arr) = expr.get("morphTargetBinds").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for b in arr {
        let Some(index) = as_usize(b.get("index")) else {
            continue;
        };
        out.push(MorphBind {
            index,
            weight: as_f32(b.get("weight"), 1.0),
        });
    }
    out
}

/// Parse VRM 0.x `blendShapeMaster` and/or VRM 1.0 `VRMC_vrm` expressions.
pub fn parse_expressions(extensions: Option<&Value>) -> Expressions {
    let mut out = Expressions::default();
    let Some(ext) = extensions else {
        return out;
    };

    if let Some(groups) = ext
        .pointer("/VRM/blendShapeMaster/blendShapeGroups")
        .and_then(|v| v.as_array())
    {
        for g in groups {
            let preset = g.get("presetName").and_then(|v| v.as_str()).unwrap_or("");
            let name = g.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let key = if !preset.is_empty() { preset } else { name };
            // VRM 0 viseme `a` is the same mouth as VRM 1 `aa`.
            let key = if key.eq_ignore_ascii_case("a") {
                "aa"
            } else {
                key
            };
            insert_binds(&mut out, key, v0_binds(g));
        }
    }

    if let Some(preset) = ext
        .pointer("/VRMC_vrm/expressions/preset")
        .and_then(|v| v.as_object())
    {
        for (name, expr) in preset {
            insert_binds(&mut out, name, v1_binds(expr));
        }
    }
    if let Some(custom) = ext
        .pointer("/VRMC_vrm/expressions/custom")
        .and_then(|v| v.as_object())
    {
        for (name, expr) in custom {
            insert_binds(&mut out, name, v1_binds(expr));
        }
    }
    out
}

/// If the primitive has morphs but no VRM names, expose target 0 as `blink`.
pub fn with_default_names(mut expr: Expressions, n_targets: usize) -> Expressions {
    if n_targets > 0 && expr.is_empty() {
        expr.by_name.insert(
            "blink".into(),
            vec![MorphBind {
                index: 0,
                weight: 1.0,
            }],
        );
    }
    expr
}

/// Idle blink envelope: close ~0.12s every 3s, starts closing at t=0.
pub fn blink_weight(t: f32) -> f32 {
    const PERIOD: f32 = 3.0;
    const CLOSE: f32 = 0.12;
    let p = t.rem_euclid(PERIOD);
    if p >= CLOSE {
        return 0.0;
    }
    let x = p / CLOSE;
    if x < 0.5 {
        x * 2.0
    } else {
        (1.0 - x) * 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_v0_and_v1() {
        let ext = json!({
            "VRM": {
                "blendShapeMaster": {
                    "blendShapeGroups": [{
                        "name": "Blink",
                        "presetName": "blink",
                        "binds": [{"mesh": 0, "index": 0, "weight": 100}]
                    }]
                }
            },
            "VRMC_vrm": {
                "expressions": {
                    "preset": {
                        "aa": {
                            "morphTargetBinds": [{"node": 0, "index": 0, "weight": 1.0}]
                        }
                    }
                }
            }
        });
        let e = parse_expressions(Some(&ext));
        assert!(e.by_name.contains_key("blink"));
        assert!(e.by_name.contains_key("aa"));
        assert_eq!(e.pick(), Some("blink"));
        assert!((e.by_name["blink"][0].weight - 1.0).abs() < 1e-5);
    }

    #[test]
    fn v0_viseme_a_maps_to_aa() {
        let ext = json!({
            "VRM": {
                "blendShapeMaster": {
                    "blendShapeGroups": [{
                        "presetName": "a",
                        "binds": [{"mesh": 0, "index": 0, "weight": 50}]
                    }]
                }
            }
        });
        let e = parse_expressions(Some(&ext));
        assert_eq!(e.pick(), Some("aa"));
        assert!((e.by_name["aa"][0].weight - 0.5).abs() < 1e-5);
    }

    #[test]
    fn unnamed_morph_becomes_blink() {
        let e = with_default_names(Expressions::default(), 1);
        assert_eq!(e.pick(), Some("blink"));
    }

    #[test]
    fn blink_envelope_peaks_then_opens() {
        let mid = blink_weight(0.06);
        assert!(mid > 0.8, "mid close, got {mid}");
        let open = blink_weight(0.5);
        assert!(open.abs() < 1e-5, "open, got {open}");
        let looped = blink_weight(3.06);
        assert!((looped - mid).abs() < 1e-5);
    }

    #[test]
    fn get_and_has_named_preset() {
        let ext = json!({
            "VRMC_vrm": {
                "expressions": {
                    "preset": {
                        "smile": { "morphTargetBinds": [{"index": 1, "weight": 1.0}] }
                    }
                }
            }
        });
        let e = parse_expressions(Some(&ext));
        assert!(e.has("smile"));
        assert!(e.has("Smile"), "names are case-insensitive");
        assert!(!e.has("angry"));
        assert!(e.has("blink"), "auto blink is always available");
        let binds = e.get("smile").expect("smile binds");
        assert_eq!(binds[0].index, 1);
        assert!(e.get("missing").is_none());
        assert!(e.get("").is_none());
    }
}
