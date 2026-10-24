//! Neon Cat's mission counters: coins, gates, perches and passes. The goal
//! list and the tracker are generic (`game_core::missions`); this only says
//! where each number comes from.

use crate::game_core::missions::{Counters, MissionKind};
#[cfg(test)]
use crate::game_core::missions::{MissionsSpec, Tracker};
#[cfg(test)]
use serde_json::json;

/// The run's counters a mission can watch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub coins: u32,
    pub gates: u32,
    pub perches: u32,
    pub passes: u32,
}

impl Counters for Counts {
    fn count(&self, kind: MissionKind) -> u32 {
        self.get(kind)
    }
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
