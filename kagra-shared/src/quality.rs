//! Graphics tiers: `safe`, `mid`, `high`, `max`, defined in JSON and shared by every game kit.
//!
//! The data lives in `quality_tiers.json` (this file's default; `kagra/quality.py` mirrors and
//! validates the same shape). A game may override parts of it with a top-level `quality_tiers`
//! object in its game JSON (`SharedSession::load_game_json` strips that key before the kit sees
//! it). The session lays the active tier over the `WorldDoc` it is about to draw, so a kit does
//! not need to know about tiers at all. Only the two kit-side knobs (`atmo`, `particles`) reach a
//! kit, through [`crate::kit_host::KitHost::quality_hints`].
//!
//! The page (`scripts/preview_site/quality.js`) owns the setting (auto or a tier), the first-run
//! probe and the runtime guard; the numbers for those live in `auto` so a game can tune them.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::world_doc::WorldDoc;

/// Tier names, lightest first.
pub const TIER_NAMES: [&str; 4] = ["safe", "mid", "high", "max"];

const DEFAULT_JSON: &str = include_str!("quality_tiers.json");

/// What one tier changes. `None` fields leave the game's own value alone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tier {
    /// Render-scale cap, canvas pixels per CSS pixel (the page also clamps to the device's ratio).
    pub scale: f32,
    /// WorldDoc `quality` fill: `low`, `mobile`, `high`; `legacy` leaves it unset.
    pub picture: String,
    /// Half-res SSAO radius in pixels; 0 = off.
    pub ssao: f32,
    pub taa: bool,
    /// 1..=3 cascades in the one sun map.
    pub shadow_cascades: u32,
    /// Contact-hardening blocker search (not on the 8-bit WebGPU path).
    pub shadow_pcss: bool,
    pub shadow_softness: f32,
    /// Multiplies the game's own bloom; 0 = off.
    pub bloom_scale: f32,
    /// Sky props (stars, clouds) relative to the game's full set: 0.3 thin, 1 full, above 1 more.
    pub atmo: f32,
    /// Spark and burst particles kept, 0..=1.
    pub particles: f32,
    /// Game switches set when the tier is applied (`fx_post`, `fx_atmo` for HOP-BEAT).
    pub fx: BTreeMap<String, bool>,
}

/// The numbers the page's first-run probe and runtime guard use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoSpec {
    /// Tier the first-run probe starts at, and the one used when probing is impossible.
    pub start: String,
    /// The highest tier auto will pick on its own. `max` is only ever a manual choice.
    pub ceiling: String,
    pub probe_secs: f32,
    pub settle_secs: f32,
    pub good_p95_ms: f32,
    pub good_p99_ms: f32,
    /// Step down when this share of frames in the window is slower than 1.5 intervals.
    pub down_drop_pct: f32,
    pub down_secs: f32,
    /// Calm seconds with a good p95 before one step up is tried.
    pub up_secs: f32,
    /// No two changes closer than this.
    pub cooldown_secs: f32,
    /// After a step up fails, the tier above is not tried again for this long.
    pub ban_secs: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualitySpec {
    pub tiers: BTreeMap<String, Tier>,
    pub auto: AutoSpec,
}

fn num(name: &str, v: f32, lo: f32, hi: f32) -> Result<(), String> {
    if v.is_finite() && v >= lo && v <= hi {
        Ok(())
    } else {
        Err(format!(
            "quality_tiers: {name} must be a number in {lo}..={hi}, got {v}"
        ))
    }
}

impl Tier {
    pub fn validate(&self, name: &str) -> Result<(), String> {
        num(&format!("{name}.scale"), self.scale, 0.5, 4.0)?;
        if !["legacy", "low", "mobile", "high"].contains(&self.picture.as_str()) {
            return Err(format!(
                "quality_tiers: {name}.picture must be legacy|low|mobile|high, got {:?}",
                self.picture
            ));
        }
        num(&format!("{name}.ssao"), self.ssao, 0.0, 16.0)?;
        num(
            &format!("{name}.shadow_cascades"),
            self.shadow_cascades as f32,
            1.0,
            3.0,
        )?;
        num(
            &format!("{name}.shadow_softness"),
            self.shadow_softness,
            0.25,
            8.0,
        )?;
        num(&format!("{name}.bloom_scale"), self.bloom_scale, 0.0, 2.0)?;
        num(&format!("{name}.atmo"), self.atmo, 0.0, 2.0)?;
        num(&format!("{name}.particles"), self.particles, 0.0, 1.0)?;
        for k in self.fx.keys() {
            if k.is_empty() || k.len() > 32 {
                return Err(format!("quality_tiers: {name}.fx has a bad switch name"));
            }
        }
        Ok(())
    }

    /// Lay this tier over a document the kit just built. An explicit `quality` the kit set is
    /// replaced (the tier is the player's choice); fields the tier leaves alone stay.
    pub fn apply(&self, doc: &mut WorldDoc) {
        doc.quality = match self.picture.as_str() {
            "legacy" => None,
            p => Some(p.to_string()),
        };
        doc.ssao = Some(self.ssao);
        doc.taa = Some(self.taa);
        doc.shadow_cascades = Some(self.shadow_cascades);
        doc.shadow_pcss = Some(self.shadow_pcss);
        doc.shadow_softness = Some(self.shadow_softness);
        if let Some(b) = doc.bloom {
            doc.bloom = Some(b * self.bloom_scale);
        }
    }
}

impl QualitySpec {
    pub fn validate(&self) -> Result<(), String> {
        for n in TIER_NAMES {
            match self.tiers.get(n) {
                Some(t) => t.validate(n)?,
                None => return Err(format!("quality_tiers: tier {n:?} is missing")),
            }
        }
        if let Some(extra) = self
            .tiers
            .keys()
            .find(|k| !TIER_NAMES.contains(&k.as_str()))
        {
            return Err(format!("quality_tiers: unknown tier {extra:?}"));
        }
        let a = &self.auto;
        let idx = |n: &str| TIER_NAMES.iter().position(|t| *t == n);
        let (Some(start), Some(ceil)) = (idx(&a.start), idx(&a.ceiling)) else {
            return Err("quality_tiers: auto.start and auto.ceiling must be tier names".into());
        };
        if start > ceil && a.start != "max" {
            return Err("quality_tiers: auto.start must not be above auto.ceiling".into());
        }
        num("auto.probe_secs", a.probe_secs, 1.0, 30.0)?;
        num("auto.settle_secs", a.settle_secs, 0.0, 10.0)?;
        num("auto.good_p95_ms", a.good_p95_ms, 8.0, 100.0)?;
        num("auto.good_p99_ms", a.good_p99_ms, 8.0, 200.0)?;
        num("auto.down_drop_pct", a.down_drop_pct, 0.5, 100.0)?;
        num("auto.down_secs", a.down_secs, 1.0, 60.0)?;
        num("auto.up_secs", a.up_secs, 5.0, 3600.0)?;
        num("auto.cooldown_secs", a.cooldown_secs, 1.0, 600.0)?;
        num("auto.ban_secs", a.ban_secs, 0.0, 86400.0)?;
        // Monotone scale: a higher tier never renders at a lower resolution.
        let scales: Vec<f32> = TIER_NAMES.iter().map(|n| self.tiers[*n].scale).collect();
        if scales.windows(2).any(|w| w[0] > w[1]) {
            return Err("quality_tiers: scale must not fall from safe to max".into());
        }
        Ok(())
    }

    pub fn tier(&self, name: &str) -> Option<&Tier> {
        self.tiers.get(name)
    }

    /// The built-in tiers (`quality_tiers.json`).
    pub fn builtin() -> Self {
        let spec: QualitySpec =
            serde_json::from_str(DEFAULT_JSON).expect("quality_tiers.json is valid");
        spec.validate().expect("quality_tiers.json passes validate");
        spec
    }

    /// The built-in tiers with a game's `quality_tiers` object laid over them, key by key.
    /// Unknown keys and out-of-range numbers are refused.
    pub fn with_overrides(over: &Value) -> Result<Self, String> {
        let mut base: Value = serde_json::from_str(DEFAULT_JSON).map_err(|e| e.to_string())?;
        merge(&mut base, over);
        let spec: QualitySpec =
            serde_json::from_value(base).map_err(|e| format!("quality_tiers: {e}"))?;
        spec.validate()?;
        Ok(spec)
    }
}

pub fn merge_json(base: &mut Value, over: &Value) {
    merge(base, over)
}

fn merge(base: &mut Value, over: &Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(k) {
                    Some(slot) if slot.is_object() && v.is_object() => merge(slot, v),
                    _ => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (slot, v) => *slot = v.clone(),
    }
}

/// The page-facing view: the spec plus the active tier name.
pub fn to_json(spec: &QualitySpec, active: &str) -> String {
    let mut v = serde_json::to_value(spec).unwrap_or(Value::Null);
    if let Value::Object(m) = &mut v {
        m.insert("active".into(), Value::String(active.into()));
    }
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builtin_parses_and_validates() {
        let s = QualitySpec::builtin();
        assert_eq!(s.tiers.len(), 4);
        assert_eq!(s.auto.ceiling, "high");
        assert!(s.tiers["safe"].scale < s.tiers["max"].scale);
    }

    #[test]
    fn overrides_merge_key_by_key() {
        let s = QualitySpec::with_overrides(&json!({"tiers": {"mid": {"scale": 1.25}}})).unwrap();
        assert_eq!(s.tiers["mid"].scale, 1.25);
        assert_eq!(s.tiers["mid"].atmo, 0.75, "other keys keep the default");
        assert_eq!(s.tiers["high"], QualitySpec::builtin().tiers["high"]);
    }

    #[test]
    fn bad_overrides_are_refused() {
        for bad in [
            json!({"tiers": {"mid": {"scale": 9.0}}}),
            json!({"tiers": {"mid": {"sparkle": 1}}}),
            json!({"tiers": {"ultra": {}}}),
            json!({"tiers": {"mid": {"picture": "cinematic"}}}),
            json!({"auto": {"ceiling": "huge"}}),
            json!({"auto": {"up_secs": 0.1}}),
            json!({"tiers": {"safe": {"scale": 2.0}}}),
            json!({"tiers": {"mid": {"particles": 1.5}}}),
        ] {
            assert!(QualitySpec::with_overrides(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn apply_lays_the_tier_over_a_doc() {
        let s = QualitySpec::builtin();
        let mut doc = WorldDoc {
            bloom: Some(0.16),
            quality: Some("high".into()),
            ..WorldDoc::default()
        };
        s.tiers["safe"].apply(&mut doc);
        assert_eq!(doc.quality.as_deref(), Some("low"));
        assert_eq!(doc.bloom, Some(0.0));
        assert_eq!(doc.shadow_cascades, Some(1));
        s.tiers["max"].apply(&mut doc);
        assert_eq!(doc.taa, Some(true));
        assert_eq!(doc.shadow_cascades, Some(3));
        let mut none = WorldDoc::default();
        s.tiers["mid"].apply(&mut none);
        assert_eq!(none.bloom, None, "a game with no bloom stays without");
        assert_eq!(none.quality, None);
    }

    #[test]
    fn json_view_carries_the_active_tier() {
        let v: Value = serde_json::from_str(&to_json(&QualitySpec::builtin(), "mid")).unwrap();
        assert_eq!(v["active"], "mid");
        assert!(v["tiers"]["max"]["ssao"].as_f64().unwrap() > 0.0);
    }
}
