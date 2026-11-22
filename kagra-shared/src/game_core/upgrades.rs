//! Permanent upgrades: a data-driven table of base-stat levels a kit declares
//! in JSON, and the level state a kit keeps in its save.
//!
//! The table is content: each row names a stat (the kit's own word — this part
//! never learns what "damage" is), what one level adds, the level cap, and a
//! geometric price curve. The state is the save's piece: one level per row id.
//! A kit reads [`UpgradeTable::bonus`] at run start and folds the numbers into
//! its own channels; buying is the kit's wallet talking to
//! [`UpgradeTable::next_cost`].
//!
//! Rules mirror the house style of [`crate::game_core::progress`]: parsing a
//! bad *table* is a spec error and fails loud at load; loading a bad *state*
//! never errors — levels clamp to their row's cap and unknown ids drop.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// One row of an [`UpgradeTable`]: a stat, its per-level gain, its cap and
/// its price curve. Costs are computed, never stored per level:
/// the next level costs `round(cost_base × cost_growth^level)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UpgradeRow {
    /// The row's own id (the save's key for its level).
    pub id: String,
    /// The stat this row feeds — the kit's word, summed by [`UpgradeTable::bonus`].
    pub stat: String,
    /// What one level adds (a fraction for multipliers, a flat amount for
    /// pools like max HP — the kit decides when it reads the bonus).
    pub per_level: f32,
    /// The hard cap on levels.
    pub max_level: u32,
    /// The price of the first level.
    pub cost_base: u32,
    /// The price multiplier per level already owned (1.0 = flat prices).
    pub cost_growth: f32,
}

impl Default for UpgradeRow {
    fn default() -> Self {
        Self {
            id: String::new(),
            stat: String::new(),
            per_level: 0.0,
            max_level: 0,
            cost_base: 0,
            cost_growth: 1.0,
        }
    }
}

/// A kit's declared upgrade rows, parsed from its spec JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct UpgradeTable {
    rows: Vec<UpgradeRow>,
}

impl UpgradeTable {
    /// Parse a table from a spec value (an array of row objects). A malformed
    /// table is a spec error: fail loud, like `CityActionSpec::from_value`.
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let rows: Vec<UpgradeRow> =
            serde_json::from_value(v.clone()).map_err(|e| format!("upgrades table: {e}"))?;
        Self::from_rows(rows)
    }

    /// Validate rows parsed elsewhere (a kit's typed spec) into a table.
    pub fn from_rows(rows: Vec<UpgradeRow>) -> Result<Self, String> {
        for (i, r) in rows.iter().enumerate() {
            if r.id.is_empty() || r.stat.is_empty() {
                return Err(format!("upgrades row {i}: id and stat must be named"));
            }
            if !r.per_level.is_finite() || r.per_level == 0.0 {
                return Err(format!(
                    "upgrades row {}: per_level must be a nonzero finite number",
                    r.id
                ));
            }
            if r.max_level == 0 || r.max_level > 10_000 {
                return Err(format!(
                    "upgrades row {}: max_level must be 1..=10000",
                    r.id
                ));
            }
            if !r.cost_growth.is_finite() || r.cost_growth < 1.0 {
                return Err(format!(
                    "upgrades row {}: cost_growth must be finite and >= 1.0",
                    r.id
                ));
            }
            if rows[..i].iter().any(|o| o.id == r.id) {
                return Err(format!("upgrades row {}: duplicate id", r.id));
            }
        }
        Ok(Self { rows })
    }

    /// The declared rows, in declaration order (a catalog's display order).
    pub fn rows(&self) -> &[UpgradeRow] {
        &self.rows
    }

    /// One row by id.
    pub fn row(&self, id: &str) -> Option<&UpgradeRow> {
        self.rows.iter().find(|r| r.id == id)
    }

    /// The price of the next level of `id` under `state`, or None when the
    /// row is unknown or already at its cap.
    pub fn next_cost(&self, state: &UpgradeState, id: &str) -> Option<u32> {
        let row = self.row(id)?;
        let level = state.level(id);
        if level >= row.max_level {
            return None;
        }
        let cost = row.cost_base as f64 * (row.cost_growth as f64).powi(level as i32);
        Some(cost.round().min(u32::MAX as f64) as u32)
    }

    /// The total bonus for `stat`: Σ per_level × level over its rows.
    pub fn bonus(&self, state: &UpgradeState, stat: &str) -> f32 {
        self.rows
            .iter()
            .filter(|r| r.stat == stat)
            .map(|r| r.per_level * state.level(&r.id) as f32)
            .sum()
    }
}

/// The levels a player owns, as kept in a kit's save.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpgradeState {
    levels: BTreeMap<String, u32>,
}

impl UpgradeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read kept levels from a save value (an object of id → level). Every
    /// declared row lands in the state (0 when absent), clamped to its cap;
    /// unknown ids and odd values drop. Never an error.
    pub fn load(&mut self, v: &Value, table: &UpgradeTable) {
        self.levels.clear();
        for row in table.rows() {
            let level = v
                .get(&row.id)
                .and_then(|l| l.as_u64())
                .map_or(0, |l| (l as u32).min(row.max_level));
            self.levels.insert(row.id.clone(), level);
        }
    }

    /// What to keep in the save: every level the state holds.
    pub fn to_value(&self) -> Value {
        let mut out = json!({});
        for (id, level) in &self.levels {
            out[id] = json!(level);
        }
        out
    }

    /// The level of one row (0 when unowned or unknown).
    pub fn level(&self, id: &str) -> u32 {
        self.levels.get(id).copied().unwrap_or(0)
    }

    /// Set one row's level. The caller clamps (a purchase raises by one after
    /// [`UpgradeTable::next_cost`] said a next level exists).
    pub fn set_level(&mut self, id: &str, level: u32) {
        self.levels.insert(id.to_string(), level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> UpgradeTable {
        UpgradeTable::from_value(&json!([
            {"id": "atk", "stat": "damage", "per_level": 0.03, "max_level": 10,
             "cost_base": 150, "cost_growth": 1.6},
            {"id": "hp", "stat": "max_hp", "per_level": 10.0, "max_level": 10,
             "cost_base": 120, "cost_growth": 1.6},
            {"id": "spd", "stat": "speed", "per_level": 0.015, "max_level": 5,
             "cost_base": 200, "cost_growth": 1.7}
        ]))
        .unwrap()
    }

    #[test]
    fn costs_climb_the_geometric_curve_and_stop_at_the_cap() {
        let t = table();
        let mut s = UpgradeState::new();
        s.load(&json!({}), &t);
        assert_eq!(t.next_cost(&s, "atk"), Some(150));
        s.set_level("atk", 1);
        assert_eq!(t.next_cost(&s, "atk"), Some(240));
        s.set_level("atk", 2);
        assert_eq!(t.next_cost(&s, "atk"), Some(384));
        s.set_level("atk", 10);
        assert_eq!(t.next_cost(&s, "atk"), None, "at the cap there is no next");
        assert_eq!(t.next_cost(&s, "nope"), None, "unknown rows have no price");
        // The full atk ladder costs what the curve sums to.
        let mut s = UpgradeState::new();
        s.load(&json!({}), &t);
        let mut total = 0u32;
        while let Some(c) = t.next_cost(&s, "atk") {
            total += c;
            let l = s.level("atk");
            s.set_level("atk", l + 1);
        }
        assert_eq!(total, 27_238);
    }

    #[test]
    fn bonuses_sum_per_level_over_the_rows_of_a_stat() {
        let t = table();
        let mut s = UpgradeState::new();
        s.load(&json!({"atk": 3, "hp": 2, "spd": 4}), &t);
        assert!((t.bonus(&s, "damage") - 0.09).abs() < 1e-6);
        assert!((t.bonus(&s, "max_hp") - 20.0).abs() < 1e-6);
        assert!((t.bonus(&s, "speed") - 0.06).abs() < 1e-6);
        assert_eq!(t.bonus(&s, "luck"), 0.0);
    }

    #[test]
    fn a_state_load_clamps_drops_and_round_trips() {
        let t = table();
        let mut s = UpgradeState::new();
        s.load(&json!({"atk": 99, "hp": -3, "spd": "x", "zzz": 5}), &t);
        assert_eq!(s.level("atk"), 10, "clamped to the row cap");
        assert_eq!(s.level("hp"), 0, "a negative level drops");
        assert_eq!(s.level("spd"), 0, "a non-number drops");
        assert_eq!(
            s.to_value(),
            json!({"atk": 10, "hp": 0, "spd": 0}),
            "only declared rows are kept"
        );
        s.load(&json!("garbage"), &t);
        assert_eq!(s.level("atk"), 0, "a non-object loads as all-zero");
    }

    #[test]
    fn a_bad_table_fails_loud() {
        assert!(UpgradeTable::from_value(&json!([
            {"id": "a", "stat": "damage", "per_level": 0.1, "max_level": 3,
             "cost_base": 10, "cost_growth": 1.5},
            {"id": "a", "stat": "speed", "per_level": 0.1, "max_level": 3,
             "cost_base": 10, "cost_growth": 1.5}
        ]))
        .is_err());
        assert!(UpgradeTable::from_value(&json!([
            {"id": "a", "stat": "damage", "per_level": 0.1, "max_level": 0,
             "cost_base": 10, "cost_growth": 1.5}
        ]))
        .is_err());
        assert!(UpgradeTable::from_value(&json!({"not": "an array"})).is_err());
    }
}
