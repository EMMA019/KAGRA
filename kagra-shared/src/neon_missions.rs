//! Per-run missions for Neon Cat: "collect N coins", "run through M gates",
//! "perch once". Progress is read from the run's own counters; this only
//! remembers which goals were already announced so each fires once.

use serde_json::{json, Value};

use crate::neon_fx_spec::{MissionKind, MissionsSpec};

/// The run's counters a mission can watch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub coins: u32,
    pub gates: u32,
    pub perches: u32,
    pub passes: u32,
}

impl Counts {
    pub fn get(&self, kind: MissionKind) -> u32 {
        match kind {
            MissionKind::Coins => self.coins,
            MissionKind::Gates => self.gates,
            MissionKind::Perch => self.perches,
            MissionKind::Passes => self.passes,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Tracker {
    done: Vec<bool>,
}

impl Tracker {
    pub fn reset(&mut self) {
        self.done.clear();
    }

    /// Goals completed since the last call, as indices into `spec.list`.
    /// Always empty when missions are off.
    pub fn update(&mut self, spec: &MissionsSpec, c: Counts) -> Vec<usize> {
        if !spec.enabled {
            return Vec::new();
        }
        self.done.resize(spec.list.len(), false);
        let mut fresh = Vec::new();
        for (i, m) in spec.list.iter().enumerate() {
            if !self.done[i] && c.get(m.kind) >= m.target {
                self.done[i] = true;
                fresh.push(i);
            }
        }
        fresh
    }

    pub fn is_done(&self, i: usize) -> bool {
        self.done.get(i).copied().unwrap_or(false)
    }

    pub fn completed(&self) -> usize {
        self.done.iter().filter(|d| **d).count()
    }

    /// `[{kind, label, target, value, done}]` for the page and the result card.
    pub fn json(&self, spec: &MissionsSpec, c: Counts) -> Value {
        if !spec.enabled {
            return json!([]);
        }
        Value::Array(
            spec.list
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    json!({
                        "kind": m.kind.name(),
                        "label": m.text(),
                        "target": m.target,
                        "value": c.get(m.kind).min(m.target),
                        "done": self.is_done(i),
                    })
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goals_fire_once_and_in_order() {
        let spec = MissionsSpec::default();
        let mut t = Tracker::default();
        assert!(t.update(&spec, Counts::default()).is_empty());
        let c = Counts {
            coins: 40,
            gates: 2,
            perches: 1,
            passes: 0,
        };
        assert_eq!(t.update(&spec, c), vec![0, 2]);
        assert!(t.update(&spec, c).is_empty(), "announced once");
        let c = Counts { gates: 5, ..c };
        assert_eq!(t.update(&spec, c), vec![1]);
        assert_eq!(t.completed(), 3);
        t.reset();
        assert_eq!(t.completed(), 0);
        assert_eq!(t.update(&spec, c), vec![0, 1, 2], "a new run starts over");
    }

    #[test]
    fn off_is_silent_and_json_is_empty() {
        let spec = MissionsSpec::off();
        let mut t = Tracker::default();
        let c = Counts {
            coins: 999,
            gates: 99,
            perches: 9,
            passes: 99,
        };
        assert!(t.update(&spec, c).is_empty());
        assert_eq!(t.json(&spec, c), json!([]));
    }

    #[test]
    fn json_clamps_progress_to_the_target() {
        let spec = MissionsSpec::default();
        let mut t = Tracker::default();
        let c = Counts {
            coins: 100,
            gates: 1,
            perches: 0,
            passes: 0,
        };
        t.update(&spec, c);
        let j = t.json(&spec, c);
        assert_eq!(j[0]["value"], 40);
        assert_eq!(j[0]["done"], true);
        assert_eq!(j[1]["value"], 1);
        assert_eq!(j[1]["done"], false);
        assert_eq!(j[2]["label"], "PERCH");
    }
}
