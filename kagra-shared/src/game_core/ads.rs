//! A no-op place for a portal SDK to plug in later.
//!
//! The page calls the same two names (`ads.break`, `ads.rewarded` in
//! `scripts/preview_site/ads.js`). This side records the request and returns
//! at once: a break is not shown, a rewarded ad is not granted. A future host
//! replaces the body of [`Ads::rewarded`] and the JS functions; the call sites
//! (level end, death, the continue button) stay.

/// One recorded request. `shown` / `granted` are what a real SDK would answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdEvent {
    pub id: u32,
    pub slot: String,
    pub kind: &'static str,
}

#[derive(Clone, Debug, Default)]
pub struct Ads {
    seq: u32,
    pub events: Vec<AdEvent>,
}

impl Ads {
    pub fn new() -> Self {
        Self::default()
    }

    /// `ads.break(slot)`. Returns the id the page can forward. Nothing is shown.
    pub fn break_slot(&mut self, slot: &str) -> u32 {
        self.push("break", slot)
    }

    /// `ads.rewarded(slot)`. Always `false` until a portal SDK is plugged in:
    /// the player is not granted a reward by the stub itself.
    pub fn rewarded(&mut self, slot: &str) -> bool {
        self.push("rewarded", slot);
        false
    }

    /// The latest request of `kind` (`"break"` or `"rewarded"`), as `(id, slot)`.
    pub fn last(&self, kind: &str) -> Option<(u32, &str)> {
        self.events
            .iter()
            .rev()
            .find(|e| e.kind == kind)
            .map(|e| (e.id, e.slot.as_str()))
    }

    fn push(&mut self, kind: &'static str, slot: &str) -> u32 {
        self.seq = self.seq.wrapping_add(1).max(1);
        self.events.push(AdEvent {
            id: self.seq,
            slot: slot.to_string(),
            kind,
        });
        self.seq
    }
}

/// Dispatch the two portal names. Unknown names do nothing and return false.
pub fn call(ads: &mut Ads, method: &str, slot: &str) -> bool {
    match method {
        "break" => {
            ads.break_slot(slot);
            false
        }
        "rewarded" => ads.rewarded(slot),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn break_and_rewarded_are_a_noop_that_records_the_slot() {
        let mut ads = Ads::new();
        let id = ads.break_slot("level_end");
        assert_eq!(id, 1);
        assert!(!ads.rewarded("continue"));
        assert!(!call(&mut ads, "break", "death"));
        assert!(!call(&mut ads, "rewarded", "continue"));
        assert!(!call(&mut ads, "banner", "top"));
        let slots: Vec<_> = ads
            .events
            .iter()
            .map(|e| (e.kind, e.slot.as_str()))
            .collect();
        assert_eq!(
            slots,
            vec![
                ("break", "level_end"),
                ("rewarded", "continue"),
                ("break", "death"),
                ("rewarded", "continue"),
            ]
        );
        assert_eq!(ads.last("break").map(|p| p.1), Some("death"));
        assert_eq!(ads.last("rewarded").map(|p| p.1), Some("continue"));
    }
}
