//! Lives: how many hits a run survives, and the blink of protection after one.
//!
//! [`LivesSpec`] is the GameSpec section (`lives` in the JSON). [`Lives`] is
//! the run-time half that every kit carries on its [`Core`](super::Core). The
//! kit decides *what* hits (and plays the sparks and sounds); this decides
//! whether the hit costs a life or ends the run.

use serde::{Deserialize, Serialize};

/// `Lives::hurt_t` value that means no recent hit.
pub const HURT_IDLE: f32 = 99.0;

/// Lives: a hit costs one, the player blinks and cannot be hit again for
/// `invuln_secs`, and the last life ends the run as before. `count` 1 is the
/// old one-hit rule (the geometry measurements run that way).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LivesSpec {
    /// 1..=9 (a HUD row has nine slots). The default is 3.
    pub count: u32,
    /// Seconds without damage after a hit.
    pub invuln_secs: f32,
    /// Blinks per second while protected.
    pub blink_hz: f32,
}

impl Default for LivesSpec {
    fn default() -> Self {
        Self {
            count: 3,
            invuln_secs: 1.6,
            blink_hz: 10.0,
        }
    }
}

impl LivesSpec {
    pub const MAX: u32 = 9;

    pub(crate) fn validate(&self) -> Result<(), String> {
        if !(1..=Self::MAX).contains(&self.count) {
            return Err(format!("game spec: lives.count must be 1..={}", Self::MAX));
        }
        if !(0.0..=10.0).contains(&self.invuln_secs) || !self.invuln_secs.is_finite() {
            return Err("game spec: lives.invuln_secs must be 0..=10".into());
        }
        if !(0.0..=30.0).contains(&self.blink_hz) || !self.blink_hz.is_finite() {
            return Err("game spec: lives.blink_hz must be 0..=30".into());
        }
        Ok(())
    }
}

/// What a hit did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// A life went and the player is protected for `invuln_secs`.
    Absorbed,
    /// That was the last life: the run is over.
    Fatal,
}

/// Run-time lives.
#[derive(Clone, Debug)]
pub struct Lives {
    /// Hits left. The run ends when the last one goes.
    pub left: u32,
    /// Seconds of protection left after a hit.
    pub invuln: f32,
    /// Seconds since the last absorbed hit ([`HURT_IDLE`] = none lately).
    pub hurt_t: f32,
}

impl Default for Lives {
    fn default() -> Self {
        Self {
            left: 0,
            invuln: 0.0,
            hurt_t: HURT_IDLE,
        }
    }
}

impl Lives {
    /// A fresh run: all lives, no protection.
    pub fn reset(&mut self, spec: &LivesSpec) {
        self.left = spec.count;
        self.invuln = 0.0;
        self.hurt_t = HURT_IDLE;
    }

    /// Count the protection and the "just hurt" timer down.
    pub fn tick(&mut self, dt: f32) {
        self.invuln = (self.invuln - dt).max(0.0);
        self.hurt_t = (self.hurt_t + dt).min(HURT_IDLE);
    }

    /// Cannot be hit right now.
    pub fn protected(&self) -> bool {
        self.invuln > 0.0
    }

    /// A hit. `last_chance` forces this to be the last one (a chaser caught
    /// the player). Returns whether a life absorbed it.
    pub fn hit(&mut self, spec: &LivesSpec, last_chance: bool) -> Hit {
        if last_chance {
            self.left = 1;
        }
        if self.left > 1 {
            self.left -= 1;
            self.invuln = spec.invuln_secs;
            self.hurt_t = 0.0;
            Hit::Absorbed
        } else {
            self.left = 0;
            Hit::Fatal
        }
    }

    /// A life pick-up. False (and nothing happens) when already full.
    pub fn regain(&mut self, spec: &LivesSpec) -> bool {
        if self.left < spec.count {
            self.left += 1;
            true
        } else {
            false
        }
    }

    /// Whether the player is drawn this frame: hidden on the off half of each
    /// blink cycle while protected.
    pub fn blink_shown(&self, spec: &LivesSpec) -> bool {
        let hz = spec.blink_hz;
        self.invuln <= 0.0 || hz <= 0.0 || (self.invuln * hz * 2.0).floor() as i64 % 2 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_cost_a_life_then_end_the_run() {
        let spec = LivesSpec::default();
        let mut l = Lives::default();
        assert_eq!(l.hurt_t, HURT_IDLE);
        l.reset(&spec);
        assert_eq!(l.left, 3);
        assert_eq!(l.hit(&spec, false), Hit::Absorbed);
        assert_eq!((l.left, l.invuln, l.hurt_t), (2, 1.6, 0.0));
        assert!(l.protected());
        assert_eq!(l.hit(&spec, false), Hit::Absorbed);
        assert_eq!(l.left, 1);
        assert_eq!(l.hit(&spec, false), Hit::Fatal);
        assert_eq!(l.left, 0);
    }

    #[test]
    fn a_last_chance_hit_ends_the_run_at_once() {
        let spec = LivesSpec::default();
        let mut l = Lives::default();
        l.reset(&spec);
        assert_eq!(l.hit(&spec, true), Hit::Fatal);
        assert_eq!(l.left, 0);
    }

    #[test]
    fn protection_counts_down_and_the_blink_follows_it() {
        let spec = LivesSpec::default();
        let mut l = Lives::default();
        l.reset(&spec);
        assert!(l.blink_shown(&spec));
        l.hit(&spec, false);
        let shown: Vec<bool> = (0..8)
            .map(|_| {
                l.tick(0.02);
                l.blink_shown(&spec)
            })
            .collect();
        assert!(shown.contains(&true) && shown.contains(&false));
        l.tick(5.0);
        assert!(!l.protected() && l.blink_shown(&spec));
        assert!((l.hurt_t - 5.16).abs() < 1e-3);
        l.tick(500.0);
        assert_eq!(l.hurt_t, HURT_IDLE, "capped");
    }

    #[test]
    fn a_pickup_refills_up_to_the_count_only() {
        let spec = LivesSpec::default();
        let mut l = Lives::default();
        l.reset(&spec);
        assert!(!l.regain(&spec));
        l.hit(&spec, false);
        assert!(l.regain(&spec));
        assert_eq!(l.left, 3);
    }

    #[test]
    fn spec_limits() {
        let mut s = LivesSpec::default();
        s.validate().unwrap();
        s.count = 0;
        assert!(s.validate().is_err());
        s.count = 10;
        assert!(s.validate().is_err());
        let s = LivesSpec {
            blink_hz: 31.0,
            ..LivesSpec::default()
        };
        assert!(s.validate().is_err());
        assert!(serde_json::from_str::<LivesSpec>(r#"{"count":2,"x":1}"#).is_err());
    }

    /// The vectors the Python mirror (`kagra/run_rules.py`) replays too.
    #[test]
    fn replays_the_golden_vectors() {
        let v: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/run_rules_vectors.json"))
                .unwrap();
        for case in v["lives"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let spec: LivesSpec = serde_json::from_value(case["spec"].clone()).unwrap();
            spec.validate().unwrap();
            let mut l = Lives::default();
            for s in case["steps"].as_array().unwrap() {
                match s["op"].as_str().unwrap() {
                    "reset" => l.reset(&spec),
                    "tick" => l.tick(s["dt"].as_f64().unwrap() as f32),
                    "hit" => {
                        let got = l.hit(&spec, s["last_chance"].as_bool().unwrap());
                        let want = match s["expect"].as_str().unwrap() {
                            "absorbed" => Hit::Absorbed,
                            _ => Hit::Fatal,
                        };
                        assert_eq!(got, want, "{name}");
                    }
                    "regain" => {
                        assert_eq!(l.regain(&spec), s["expect"].as_bool().unwrap(), "{name}")
                    }
                    "state" => {
                        assert_eq!(l.left as u64, s["left"].as_u64().unwrap(), "{name}");
                        let near =
                            |a: f32, k: &str| (a as f64 - s[k].as_f64().unwrap()).abs() < 1e-4;
                        assert!(near(l.invuln, "invuln"), "{name}: invuln {}", l.invuln);
                        assert!(near(l.hurt_t, "hurt_t"), "{name}: hurt_t {}", l.hurt_t);
                        assert_eq!(l.protected(), s["protected"].as_bool().unwrap(), "{name}");
                        if let Some(b) = s["blink"].as_bool() {
                            assert_eq!(l.blink_shown(&spec), b, "{name}");
                        }
                    }
                    op => panic!("{op}"),
                }
            }
        }
    }
}
