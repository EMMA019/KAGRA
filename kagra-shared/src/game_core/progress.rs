//! What a staged game keeps between visits: how far the player got, and a record per stage.
//!
//! A kit with stages (or maps, or levels) holds one [`Progress`], reads it from the world's
//! `save` (the page hands back what the kit's `save_json` returned last time, with the
//! manifest's `page.save`), and writes it back with [`Progress::to_value`]. The stage select
//! on the first story card of a run uses [`Progress::can_pick`] and [`Progress::step`].
//!
//! The save is `{"cleared": n, "<key>": [record or null, ...]}`. The record key is the kit's
//! (`best_secs` for a clear time, `best_turns` for a turn count) so a save written before
//! this module existed still reads.

use serde_json::{json, Value};

/// Which records are better.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Better {
    /// Times, turns, falls: lower is better.
    Lower,
    /// Scores, combos: higher is better.
    Higher,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    /// Stages cleared in order from the first: `cleared` stages are done, and the stage after
    /// them is the furthest the player may start from.
    pub cleared: usize,
    records: Vec<Option<f32>>,
    key: &'static str,
    better: Better,
    /// Decimal places kept in the save (a time to 0.1 s: 1).
    places: i32,
}

impl Progress {
    /// Nothing cleared yet, for `stages` stages, keeping records under `key`.
    pub fn new(stages: usize, key: &'static str, better: Better) -> Self {
        Self {
            cleared: 0,
            records: vec![None; stages],
            key,
            better,
            places: 1,
        }
    }

    /// Keep `places` decimal places of each record in the save (0 for counts).
    pub fn with_places(mut self, places: i32) -> Self {
        self.places = places.clamp(0, 6);
        self
    }

    pub fn stages(&self) -> usize {
        self.records.len()
    }

    /// Read what the device kept. Anything odd (an older save, a hand edit, more stages than
    /// the game has now) is clamped or dropped, never an error.
    pub fn load(&mut self, save: &Value) {
        let n = self.stages();
        self.cleared = save
            .get("cleared")
            .and_then(|c| c.as_u64())
            .map_or(0, |c| (c as usize).min(n));
        for (i, r) in self.records.iter_mut().enumerate() {
            *r = save
                .get(self.key)
                .and_then(|b| b.get(i))
                .and_then(|s| s.as_f64())
                .map(|s| s as f32)
                .filter(|s| s.is_finite() && *s >= 0.0);
        }
    }

    /// What to keep on the device.
    pub fn to_value(&self) -> Value {
        // Rounded in f64 so 41.3 is written as 41.3, not 41.29999923706055.
        let k = 10f64.powi(self.places);
        let recs: Vec<Value> = self
            .records
            .iter()
            .map(|r| r.map_or(Value::Null, |v| json!((v as f64 * k).round() / k)))
            .collect();
        let mut out = json!({ "cleared": self.cleared });
        out[self.key] = Value::Array(recs);
        out
    }

    /// The stages the player may start from: the cleared ones and the next.
    pub fn reached(&self) -> usize {
        (self.cleared + 1).min(self.stages()).max(1)
    }

    /// A stage select is worth showing once there is more than one stage to start from.
    pub fn can_pick(&self) -> bool {
        self.reached() > 1
    }

    /// The stage `dir` steps from `stage` among the reached ones, wrapping round.
    pub fn step(&self, stage: usize, dir: i32) -> usize {
        let reach = self.reached() as i32;
        (stage as i32 + dir).rem_euclid(reach) as usize
    }

    pub fn record(&self, stage: usize) -> Option<f32> {
        self.records.get(stage).copied().flatten()
    }

    /// Stage `stage` cleared with `value` (a time, a turn count, a score). Unlocks the next
    /// stage, keeps the record if it is better, and says whether it was a new record.
    pub fn clear(&mut self, stage: usize, value: f32) -> bool {
        if stage >= self.stages() {
            return false;
        }
        self.cleared = self.cleared.max(stage + 1);
        let better = match (self.records[stage], self.better) {
            (None, _) => true,
            (Some(old), Better::Lower) => value < old,
            (Some(old), Better::Higher) => value > old,
        };
        if better {
            self.records[stage] = Some(value);
        }
        better
    }
}

/// Seconds as `m:ss` (a clear time on a card).
pub fn clock(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearing_unlocks_the_next_stage_and_keeps_the_best_time() {
        let mut p = Progress::new(3, "best_secs", Better::Lower);
        assert_eq!((p.reached(), p.can_pick()), (1, false));
        assert!(p.clear(0, 41.34));
        assert!(!p.clear(0, 50.0));
        assert!(p.clear(0, 39.0));
        assert_eq!(p.record(0), Some(39.0));
        assert_eq!((p.reached(), p.can_pick()), (2, true));
        assert_eq!(p.step(0, 1), 1);
        assert_eq!(p.step(1, 1), 0);
        assert_eq!(p.step(0, -1), 1);
        p.clear(2, 10.0);
        assert_eq!(p.reached(), 3);
        assert!(!p.clear(9, 1.0));
    }

    #[test]
    fn higher_records_keep_the_highest() {
        let mut p = Progress::new(1, "best_score", Better::Higher).with_places(0);
        p.clear(0, 100.0);
        assert!(!p.clear(0, 90.0));
        assert!(p.clear(0, 120.0));
        assert_eq!(p.to_value(), json!({"cleared": 1, "best_score": [120.0]}));
    }

    #[test]
    fn a_save_round_trips_and_odd_ones_are_dropped() {
        let mut p = Progress::new(3, "best_secs", Better::Lower);
        p.clear(0, 41.34);
        let v = p.to_value();
        assert_eq!(v, json!({"cleared": 1, "best_secs": [41.3, null, null]}));
        let mut q = Progress::new(3, "best_secs", Better::Lower);
        q.load(&v);
        assert_eq!(q.cleared, 1);
        assert!((q.record(0).unwrap() - 41.3).abs() < 1e-4);
        q.load(&json!({"cleared": 99, "best_secs": [-1, "x"]}));
        assert_eq!(q.cleared, 3);
        assert_eq!((q.record(0), q.record(1)), (None, None));
        q.load(&json!("garbage"));
        assert_eq!(q.cleared, 0);
        assert_eq!(clock(83.9), "1:23");
    }
}
