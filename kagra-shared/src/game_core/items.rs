//! Pick-up effects every kit can share: the timed effects that are running,
//! the weighted draw of which pick-up appears, and the spec lookups for an
//! effect's colour and length.
//!
//! The effect vocabulary (`ItemEffect`) and the rows (`ItemSpec`,
//! `ItemRules`) live in `game_spec.rs` and are driven by the GameSpec JSON.
//! What a timed effect *does* (a faster cat, a magnet field, a higher jump,
//! doubled score) is the kit's rule: it asks [`Effects`] whether the effect
//! is running and reacts. Only the clock and the draw are generic.

use crate::game_spec::{ItemEffect, ItemSpec};
use serde_json::{json, Value};

/// The effects that run for a while. `Meow` is instant, so it is not here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timed {
    Magnet,
    Boost,
    Jump,
    Double,
}

impl Timed {
    /// Draw and HUD order.
    pub const ALL: [Timed; 4] = [Timed::Magnet, Timed::Boost, Timed::Jump, Timed::Double];

    fn slot(self) -> usize {
        self as usize
    }

    /// The timed effect behind a pick-up, or `None` for an instant one.
    pub fn of(effect: ItemEffect) -> Option<Timed> {
        match effect {
            ItemEffect::Magnet => Some(Timed::Magnet),
            ItemEffect::Boost => Some(Timed::Boost),
            ItemEffect::Superjump => Some(Timed::Jump),
            ItemEffect::Double => Some(Timed::Double),
            ItemEffect::Meow => None,
        }
    }

    pub fn effect(self) -> ItemEffect {
        match self {
            Timed::Magnet => ItemEffect::Magnet,
            Timed::Boost => ItemEffect::Boost,
            Timed::Jump => ItemEffect::Superjump,
            Timed::Double => ItemEffect::Double,
        }
    }
}

/// Seconds left of each timed effect.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Effects {
    left: [f32; 4],
}

impl Effects {
    /// Nothing running (a new run).
    pub fn clear(&mut self) {
        self.left = [0.0; 4];
    }

    /// Run `which` for `secs`. A second pick-up restarts the clock; it does not add.
    pub fn start(&mut self, which: Timed, secs: f32) {
        self.left[which.slot()] = secs;
    }

    pub fn left(&self, which: Timed) -> f32 {
        self.left[which.slot()]
    }

    pub fn active(&self, which: Timed) -> bool {
        self.left[which.slot()] > 0.0
    }

    /// Running and inside the last `warn_secs`: the effect blinks.
    pub fn warning(&self, which: Timed, warn_secs: f32) -> bool {
        let t = self.left(which);
        t > 0.0 && t < warn_secs
    }

    /// One step of the clock.
    pub fn tick(&mut self, dt: f32) {
        for t in &mut self.left {
            *t = (*t - dt).max(0.0);
        }
    }

    /// The `fx` object the page reads.
    pub fn json(&self) -> Value {
        json!({
            "magnet": self.left(Timed::Magnet),
            "boost": self.left(Timed::Boost),
            "superjump": self.left(Timed::Jump),
            "double": self.left(Timed::Double),
        })
    }
}

/// Seconds `effect` lasts in this spec (its row, else the effect's default).
pub fn secs_of(items: &[ItemSpec], effect: ItemEffect) -> f32 {
    items
        .iter()
        .find(|it| it.effect == effect)
        .map(|it| it.secs())
        .unwrap_or_else(|| effect.default_secs())
}

/// The colour of `effect` in this spec (its row, else the effect's default).
pub fn rgb_of(items: &[ItemSpec], effect: ItemEffect) -> [u8; 3] {
    items
        .iter()
        .find(|it| it.effect == effect)
        .map(|it| it.rgb())
        .unwrap_or_else(|| effect.default_color())
}

/// Is the pick-up due at the `ordinal`-th free slot of a section that drops
/// one every `every` slots? `every == 0` means never.
pub fn due(every: u32, ordinal: u32) -> bool {
    every != 0 && ordinal % every == every - 1
}

/// Which pick-up a slot holds: the weighted draw over the spec's rows, keyed
/// by a hash of the slot so the same spec places the same pick-ups every run.
/// `None` when the rows are empty or all weights are 0.
pub fn weighted(items: &[ItemSpec], hash: u32) -> Option<ItemEffect> {
    let total: u32 = items.iter().map(|i| i.weight).sum();
    if total == 0 {
        return None;
    }
    let mut pick = hash % total;
    for it in items {
        if pick < it.weight {
            return Some(it.effect);
        }
        pick -= it.weight;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<ItemSpec> {
        vec![
            ItemSpec::new(ItemEffect::Magnet, 2),
            ItemSpec::new(ItemEffect::Meow, 1),
        ]
    }

    #[test]
    fn a_pickup_restarts_the_clock_and_time_runs_out() {
        let mut fx = Effects::default();
        assert!(!fx.active(Timed::Boost));
        fx.start(Timed::Boost, 2.0);
        fx.tick(0.5);
        assert!((fx.left(Timed::Boost) - 1.5).abs() < 1e-6);
        fx.start(Timed::Boost, 2.0);
        assert!(
            (fx.left(Timed::Boost) - 2.0).abs() < 1e-6,
            "restart, not add"
        );
        fx.tick(5.0);
        assert!(!fx.active(Timed::Boost));
        assert_eq!(fx.left(Timed::Boost), 0.0);
        fx.start(Timed::Magnet, 1.0);
        fx.clear();
        assert_eq!(fx, Effects::default());
    }

    #[test]
    fn the_warning_is_the_last_stretch_only() {
        let mut fx = Effects::default();
        assert!(
            !fx.warning(Timed::Jump, 1.5),
            "not running is not a warning"
        );
        fx.start(Timed::Jump, 3.0);
        assert!(!fx.warning(Timed::Jump, 1.5));
        fx.tick(1.6);
        assert!(fx.warning(Timed::Jump, 1.5));
    }

    #[test]
    fn instant_effects_have_no_timer_and_the_rest_round_trip() {
        assert_eq!(Timed::of(ItemEffect::Meow), None);
        for t in Timed::ALL {
            assert_eq!(Timed::of(t.effect()), Some(t));
        }
        let mut fx = Effects::default();
        fx.start(Timed::Double, 7.0);
        assert_eq!(fx.json()["double"], 7.0);
        assert_eq!(fx.json()["magnet"], 0.0);
    }

    #[test]
    fn spec_rows_override_the_defaults() {
        let mut items = rows();
        assert_eq!(
            secs_of(&items, ItemEffect::Magnet),
            ItemEffect::Magnet.default_secs()
        );
        items[0].duration = 4.0;
        items[0].color = [1, 2, 3];
        assert_eq!(secs_of(&items, ItemEffect::Magnet), 4.0);
        assert_eq!(rgb_of(&items, ItemEffect::Magnet), [1, 2, 3]);
        assert_eq!(
            rgb_of(&items, ItemEffect::Boost),
            ItemEffect::Boost.default_color(),
            "no row: the effect's own colour"
        );
    }

    #[test]
    fn the_cadence_and_the_draw_are_exact() {
        assert!(!due(0, 0) && !due(0, 5));
        let hits: Vec<u32> = (0..9).filter(|&n| due(3, n)).collect();
        assert_eq!(hits, vec![2, 5, 8]);
        assert!(due(1, 0) && due(1, 7));
        let items = rows();
        assert_eq!(weighted(&items, 0), Some(ItemEffect::Magnet));
        assert_eq!(weighted(&items, 1), Some(ItemEffect::Magnet));
        assert_eq!(weighted(&items, 2), Some(ItemEffect::Meow));
        assert_eq!(weighted(&items, 3), Some(ItemEffect::Magnet), "wraps");
        assert_eq!(weighted(&[], 4), None);
        assert_eq!(weighted(&[ItemSpec::new(ItemEffect::Boost, 0)], 4), None);
    }
}
