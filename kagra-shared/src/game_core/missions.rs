//! Missions: small per-run goals (collect N, pass M, ...) shown on the HUD and
//! the result card.
//!
//! [`MissionsSpec`] is the GameSpec section (`missions` in the JSON). A kit
//! hands the run's counters to a [`Tracker`] through [`Counters`]; the tracker
//! only remembers which goals were already announced so each fires once, and
//! writes the `[{kind, label, target, value, done}]` the page reads.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The run's counters a mission can watch. A kit maps each [`MissionKind`]
/// to its own number (0 for a kind it does not have).
pub trait Counters {
    fn count(&self, kind: MissionKind) -> u32;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MissionsSpec {
    pub enabled: bool,
    /// The pool the run's goals are drawn from, in order. At most 5. A goal
    /// the course cannot offer (a perch with the stair lane off) is skipped.
    pub list: Vec<MissionSpec>,
    /// How many goals a run shows, drawn from the front of the pool after
    /// the ones the course cannot offer are left out (0..=5; 0 = all).
    pub draw: u32,
    /// Score added each time one is completed. 0 keeps scores comparable
    /// with runs that have no missions.
    pub bonus: u32,
    /// Draw the goals (and the best score) on the in-play HUD.
    pub hud: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MissionKind {
    /// Coins picked up this run.
    #[default]
    Coins,
    /// Stone arches run through this run.
    Gates,
    /// Different ledges landed on this run (needs the stair lane or decks).
    Perch,
    /// Obstacles left behind this run, hit or not.
    Passes,
}

impl MissionKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Coins => "coins",
            Self::Gates => "gates",
            Self::Perch => "perch",
            Self::Passes => "passes",
        }
    }

    /// Whether the course can offer this goal: a perch needs a ledge to stand
    /// on (the stair lane or the decks), everything else works on the street.
    pub fn available(self, perch: bool, decks: bool) -> bool {
        match self {
            Self::Perch => perch || decks,
            _ => true,
        }
    }

    pub fn default_label(self) -> &'static str {
        match self {
            Self::Coins => "COINS",
            Self::Gates => "GATES",
            Self::Perch => "PERCH",
            Self::Passes => "PASSED",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MissionSpec {
    pub kind: MissionKind,
    /// How many to reach (1..=9999).
    pub target: u32,
    /// HUD wording. Empty uses the kind's own (`COINS`, `GATES`, `PERCH`, `PASSED`).
    pub label: String,
}

impl Default for MissionSpec {
    fn default() -> Self {
        Self {
            kind: MissionKind::Coins,
            target: 1,
            label: String::new(),
        }
    }
}

impl MissionSpec {
    pub fn text(&self) -> &str {
        if self.label.is_empty() {
            self.kind.default_label()
        } else {
            &self.label
        }
    }
}

impl Default for MissionsSpec {
    fn default() -> Self {
        let m = |kind, target| MissionSpec {
            kind,
            target,
            label: String::new(),
        };
        Self {
            enabled: true,
            list: vec![
                m(MissionKind::Coins, 40),
                m(MissionKind::Gates, 5),
                m(MissionKind::Perch, 1),
                m(MissionKind::Passes, 30),
            ],
            draw: 3,
            bonus: 0,
            hud: true,
        }
    }
}

impl MissionsSpec {
    pub fn validate(&self) -> Result<(), String> {
        if self.list.len() > 5 {
            return Err("game spec: missions.list has at most 5 goals".into());
        }
        if self.draw > 5 {
            return Err("game spec: missions.draw must be 0..=5".into());
        }
        if self.bonus > 100_000 {
            return Err("game spec: missions.bonus must be 0..=100000".into());
        }
        for m in &self.list {
            if !(1..=9999).contains(&m.target) {
                return Err("game spec: a mission target must be 1..=9999".into());
            }
            if m.label.chars().count() > 16 {
                return Err("game spec: a mission label has at most 16 characters".into());
            }
        }
        Ok(())
    }

    /// The goals this course really shows: the pool without what the course
    /// cannot offer (`perch`: the stair lane is on; `decks`: the upper
    /// course is on), cut to `draw`. With the stair lane and decks off the
    /// pool gives coins, gates and obstacles passed.
    pub fn active(&self, perch: bool, decks: bool) -> MissionsSpec {
        let mut m = self.clone();
        m.list.retain(|g| g.kind.available(perch, decks));
        if m.draw > 0 {
            m.list.truncate(m.draw as usize);
        }
        m
    }

    /// Switched off, as `GameSpec::legacy()` wants it.
    pub fn off() -> Self {
        Self {
            enabled: false,
            ..Self::default()
        }
    }
}

/// Which goals are done this run.
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
    pub fn update<C: Counters>(&mut self, spec: &MissionsSpec, c: C) -> Vec<usize> {
        if !spec.enabled {
            return Vec::new();
        }
        self.done.resize(spec.list.len(), false);
        let mut fresh = Vec::new();
        for (i, m) in spec.list.iter().enumerate() {
            if !self.done[i] && c.count(m.kind) >= m.target {
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
    pub fn json<C: Counters>(&self, spec: &MissionsSpec, c: C) -> Value {
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
                        "value": c.count(m.kind).min(m.target),
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

    /// A kit's counters: coins and passes only.
    #[derive(Clone, Copy)]
    struct Toy {
        coins: u32,
        passes: u32,
    }

    impl Counters for Toy {
        fn count(&self, kind: MissionKind) -> u32 {
            match kind {
                MissionKind::Coins => self.coins,
                MissionKind::Passes => self.passes,
                _ => 0,
            }
        }
    }

    fn two_goals() -> MissionsSpec {
        MissionsSpec {
            list: vec![
                MissionSpec {
                    kind: MissionKind::Coins,
                    target: 5,
                    label: String::new(),
                },
                MissionSpec {
                    kind: MissionKind::Passes,
                    target: 2,
                    label: "JUMPS".into(),
                },
            ],
            ..MissionsSpec::default()
        }
    }

    #[test]
    fn any_kit_can_drive_the_tracker() {
        let spec = two_goals();
        let mut t = Tracker::default();
        assert!(t
            .update(
                &spec,
                Toy {
                    coins: 0,
                    passes: 0
                }
            )
            .is_empty());
        assert_eq!(
            t.update(
                &spec,
                Toy {
                    coins: 9,
                    passes: 1
                }
            ),
            vec![0]
        );
        assert!(t
            .update(
                &spec,
                Toy {
                    coins: 9,
                    passes: 1
                }
            )
            .is_empty());
        assert_eq!(
            t.update(
                &spec,
                Toy {
                    coins: 9,
                    passes: 2
                }
            ),
            vec![1]
        );
        assert_eq!(t.completed(), 2);
        let j = t.json(
            &spec,
            Toy {
                coins: 9,
                passes: 7,
            },
        );
        assert_eq!(j[0]["value"], 5, "clamped to the target");
        assert_eq!(j[1]["label"], "JUMPS");
        assert_eq!(j[1]["done"], true);
        t.reset();
        assert_eq!(t.completed(), 0);
    }

    #[test]
    fn off_is_silent() {
        let spec = MissionsSpec::off();
        let mut t = Tracker::default();
        assert!(t
            .update(
                &spec,
                Toy {
                    coins: 99,
                    passes: 99
                }
            )
            .is_empty());
        assert_eq!(
            t.json(
                &spec,
                Toy {
                    coins: 1,
                    passes: 1
                }
            ),
            json!([])
        );
    }

    #[test]
    fn the_pool_leaves_out_what_the_course_cannot_offer() {
        let spec = MissionsSpec::default();
        let kinds = |p, d| -> Vec<&'static str> {
            spec.active(p, d)
                .list
                .iter()
                .map(|m| m.kind.name())
                .collect()
        };
        assert_eq!(kinds(false, false), ["coins", "gates", "passes"]);
        assert_eq!(kinds(true, false), ["coins", "gates", "perch"]);
        assert_eq!(kinds(false, true), ["coins", "gates", "perch"]);
    }

    /// Counts keyed by kind name, as the golden vectors write them.
    struct Named(std::collections::BTreeMap<String, u32>);

    impl Counters for Named {
        fn count(&self, kind: MissionKind) -> u32 {
            self.0.get(kind.name()).copied().unwrap_or(0)
        }
    }

    /// The vectors the Python mirror (`kagra/run_rules.py`) replays too.
    #[test]
    fn replays_the_golden_vectors() {
        let v: Value =
            serde_json::from_str(include_str!("../../tests/fixtures/run_rules_vectors.json"))
                .unwrap();
        for case in v["missions"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let pool: MissionsSpec = if case["spec"].is_null() {
                MissionsSpec::default()
            } else {
                serde_json::from_value(case["spec"].clone()).unwrap()
            };
            let mut active = pool.active(
                case["perch"].as_bool().unwrap(),
                case["decks"].as_bool().unwrap(),
            );
            active.draw = 0;
            let mut t = Tracker::default();
            for s in case["steps"].as_array().unwrap() {
                let counts = || {
                    Named(
                        s["counts"]
                            .as_object()
                            .unwrap()
                            .iter()
                            .map(|(k, v)| (k.clone(), v.as_u64().unwrap() as u32))
                            .collect(),
                    )
                };
                let fresh: Vec<u64> = t
                    .update(&active, counts())
                    .into_iter()
                    .map(|i| i as u64)
                    .collect();
                let want: Vec<u64> = s["fresh"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_u64().unwrap())
                    .collect();
                assert_eq!(fresh, want, "{name}");
                assert_eq!(
                    t.completed() as u64,
                    s["completed"].as_u64().unwrap(),
                    "{name}"
                );
                if !s["json"].is_null() {
                    assert_eq!(t.json(&active, counts()), s["json"], "{name}");
                }
            }
        }
    }
}
