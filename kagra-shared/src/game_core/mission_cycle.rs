//! Timed missions on a cycle: the run's recurring goal posts, as a
//! shared part.
//!
//! The design is city_action's mission round (2026-11-16), lifted out
//! of the kit: a small mountain to climb every minute or so, against
//! the mid-run flat. The first mission lands early, to teach the
//! loop; each mission is a quota against a clock; the clock and the
//! count both hold while a boss is up, so a mission and a boss never
//! compete for the same seconds; success is the game's cue to open
//! its reward cards; failure costs nothing but a brief banner. The
//! part owns *when* missions run — scheduling, the clock, the hold,
//! the banners' timing and the count. The game owns what its kinds
//! mean (a kind is an index here), the words on the banners and
//! readouts, and the reward payload.
//!
//! Not to be confused with [`crate::game_core::missions`]: that part
//! is a collect-N tracker for a run's standing objectives (the neon
//! runner's). This one is a timed cycle — missions arrive, expire
//! and are replaced.
//!
//! Determinism: all randomness flows through the part's own
//! [`Rng`] stream, handed to the game's `offer` closure, so a seed
//! replays a run's mission sequence exactly. Runtime-only state (no
//! serde): a run rebuilds its cycle at start, like the kits do.

use crate::game_core::swarm::Rng;

/// The cycle's tuning: when missions land and how long the
/// banners hold. The values are the game's to choose; city_action's
/// are 35 / 60 / 2.0 / 1.2.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MissionCycleSpec {
    /// Run-clock seconds before the first mission may start: early
    /// enough to teach the loop before the first boss.
    pub first_secs: f32,
    /// Rest between one mission ending (either way) and the next
    /// starting: the run breathes between goal posts.
    pub gap_secs: f32,
    /// How long the mission-start banner holds (s).
    pub banner_secs: f32,
    /// How long the mission-failure banner holds (s).
    pub fail_banner_secs: f32,
}

/// One timed mission in flight: a quota and the clock it races.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CycleMission {
    /// The game's kind index (what the mission asks for is the
    /// game's business; the part only carries the number).
    pub kind: usize,
    /// A sub-target index (a kind hunt's prey); the other kinds
    /// ignore it.
    pub target_kind: usize,
    /// Events still owed.
    pub left: u32,
    /// Events the mission asked for (the readout's denominator).
    pub total: u32,
    /// Seconds left on the clock.
    pub secs: f32,
    /// Seconds the clock started with (the bar's denominator).
    pub total_secs: f32,
}

impl CycleMission {
    /// Events banked so far (the readout's numerator).
    pub fn done(&self) -> u32 {
        self.total - self.left
    }

    /// Clock fraction left, for the readout's thin bar.
    pub fn time_frac(&self) -> f32 {
        if self.total_secs > 0.0 {
            (self.secs / self.total_secs).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// What a `step` / `count` call just did, so the game can react:
/// post a banner's words, open reward cards, count score.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CycleEvent {
    /// A mission started (read it back via [`MissionCycle::active`]).
    Started,
    /// The active mission's quota was met.
    Succeeded,
    /// The active mission's clock ran out.
    Failed,
}

/// Which banner is posted. The words are the game's; the part only
/// owns the timing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CycleBanner {
    Start,
    Failed,
}

/// The mission cycle state machine.
#[derive(Clone, Debug)]
pub struct MissionCycle {
    spec: MissionCycleSpec,
    rng: Rng,
    mission: Option<CycleMission>,
    /// Run-clock time the next mission may start at.
    next_at: f32,
    banner: Option<(CycleBanner, f32)>,
    done: u32,
    failed: u32,
}

impl MissionCycle {
    pub fn new(spec: MissionCycleSpec, seed: u32) -> Self {
        Self {
            spec,
            rng: Rng::new(seed),
            mission: None,
            next_at: spec.first_secs,
            banner: None,
            done: 0,
            failed: 0,
        }
    }

    /// Re-arm for a fresh run on a new seed: no mission, no banner,
    /// the first mission due at `first_secs` again.
    pub fn reset(&mut self, seed: u32) {
        self.rng = Rng::new(seed);
        self.mission = None;
        self.next_at = self.spec.first_secs;
        self.banner = None;
        self.done = 0;
        self.failed = 0;
    }

    /// The mission in flight, if any.
    pub fn active(&self) -> Option<&CycleMission> {
        self.mission.as_ref()
    }

    /// The posted banner and its seconds left, if any.
    pub fn banner(&self) -> Option<(CycleBanner, f32)> {
        self.banner
    }

    /// Missions succeeded / failed this run.
    pub fn done(&self) -> u32 {
        self.done
    }

    pub fn failed(&self) -> u32 {
        self.failed
    }

    /// Advance the cycle by `dt` on the run clock `now`.
    ///
    /// The banner ticks always. An active mission's clock ticks
    /// only while not `held` (boss up, cards up, run not live —
    /// the game's call); on expiry the mission fails, the fail
    /// banner posts and the next mission is scheduled a gap out.
    /// While idle, a due mission starts when not held: `offer`
    /// rolls the mission on the part's own stream and returns
    /// `(kind, target_kind, total, secs)`, or `None` to defer the
    /// start (the schedule stays due; `offer` is asked again next
    /// tick).
    pub fn step(
        &mut self,
        dt: f32,
        now: f32,
        held: bool,
        offer: impl FnMut(&mut Rng) -> Option<(usize, usize, u32, f32)>,
    ) -> Option<CycleEvent> {
        if let Some((_, left)) = &mut self.banner {
            *left -= dt;
            if *left <= 0.0 {
                self.banner = None;
            }
        }
        if self.mission.is_some() {
            if held {
                return None;
            }
            let expired = {
                let m = self.mission.as_mut().expect("checked above");
                m.secs -= dt;
                m.secs <= 0.0
            };
            if expired {
                self.mission = None;
                self.failed += 1;
                self.next_at = now + self.spec.gap_secs;
                self.banner = Some((CycleBanner::Failed, self.spec.fail_banner_secs));
                return Some(CycleEvent::Failed);
            }
            return None;
        }
        if held || now < self.next_at {
            return None;
        }
        let mut offer = offer;
        let (kind, target_kind, total, secs) = offer(&mut self.rng)?;
        self.mission = Some(CycleMission {
            kind,
            target_kind,
            left: total,
            total,
            secs,
            total_secs: secs,
        });
        self.banner = Some((CycleBanner::Start, self.spec.banner_secs));
        Some(CycleEvent::Started)
    }

    /// Count one game event (a kill, a gem, …) toward the active
    /// mission when `pred` accepts it for that mission. Returns
    /// [`CycleEvent::Succeeded`] on the completing count, which
    /// also schedules the next mission a gap out. The game gates
    /// held events itself (city_action's rule: while a boss is up,
    /// kills are not offered to the mission at all).
    pub fn count(&mut self, now: f32, pred: impl Fn(&CycleMission) -> bool) -> Option<CycleEvent> {
        let m = self.mission.as_mut()?;
        if !pred(m) {
            return None;
        }
        m.left = m.left.saturating_sub(1);
        if m.left > 0 {
            return None;
        }
        self.mission = None;
        self.done += 1;
        self.next_at = now + self.spec.gap_secs;
        Some(CycleEvent::Succeeded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: MissionCycleSpec = MissionCycleSpec {
        first_secs: 35.0,
        gap_secs: 60.0,
        banner_secs: 2.0,
        fail_banner_secs: 1.2,
    };

    /// A plain slay offer on the part's own stream.
    fn slay(rng: &mut Rng) -> Option<(usize, usize, u32, f32)> {
        let _ = rng.next_u32();
        Some((0, 0, 3, 25.0))
    }

    #[test]
    fn the_first_mission_starts_at_first_secs_and_not_before() {
        let mut c = MissionCycle::new(SPEC, 7);
        assert_eq!(c.step(1.0, 34.9, false, slay), None);
        assert!(c.active().is_none());
        assert_eq!(c.step(0.2, 35.1, false, slay), Some(CycleEvent::Started));
        let m = c.active().expect("mission in flight");
        assert_eq!((m.kind, m.total, m.left), (0, 3, 3));
        assert_eq!(c.banner(), Some((CycleBanner::Start, SPEC.banner_secs)));
    }

    #[test]
    fn the_clock_and_the_start_hold_while_held() {
        let mut c = MissionCycle::new(SPEC, 7);
        assert_eq!(c.step(1.0, 40.0, true, slay), None, "no start while held");
        assert_eq!(c.step(0.1, 40.1, false, slay), Some(CycleEvent::Started));
        let before = c.active().expect("in flight").secs;
        assert_eq!(c.step(5.0, 45.0, true, slay), None);
        assert_eq!(c.active().expect("still in flight").secs, before);
    }

    #[test]
    fn expiry_fails_posts_the_fail_banner_and_schedules_the_next() {
        let mut c = MissionCycle::new(SPEC, 7);
        c.step(0.1, 35.1, false, slay);
        assert_eq!(c.step(26.0, 61.1, false, slay), Some(CycleEvent::Failed));
        assert!(c.active().is_none());
        assert_eq!(c.failed(), 1);
        assert_eq!(
            c.banner(),
            Some((CycleBanner::Failed, SPEC.fail_banner_secs))
        );
        // The fail banner lives exactly its span.
        c.step(SPEC.fail_banner_secs, 62.3, false, slay);
        assert_eq!(c.banner(), None);
        // The next mission is a gap after the failure, not before.
        assert_eq!(c.step(0.1, 61.1 + SPEC.gap_secs - 0.1, false, slay), None);
        assert_eq!(
            c.step(0.2, 61.1 + SPEC.gap_secs + 0.1, false, slay),
            Some(CycleEvent::Started)
        );
    }

    #[test]
    fn the_completing_count_succeeds_once_and_schedules_the_next() {
        let mut c = MissionCycle::new(SPEC, 7);
        c.step(0.1, 35.1, false, slay);
        assert_eq!(c.count(40.0, |_| true), None);
        assert_eq!(c.count(41.0, |_| true), None);
        assert_eq!(c.active().expect("in flight").done(), 2);
        assert_eq!(c.count(42.0, |_| true), Some(CycleEvent::Succeeded));
        assert_eq!(c.done(), 1);
        assert!(c.active().is_none());
        // Further counts are inert until the next mission starts.
        assert_eq!(c.count(43.0, |_| true), None);
        assert_eq!(c.step(0.1, 42.0 + SPEC.gap_secs - 0.1, false, slay), None);
        assert_eq!(
            c.step(0.2, 42.0 + SPEC.gap_secs + 0.1, false, slay),
            Some(CycleEvent::Started)
        );
    }

    #[test]
    fn a_rejected_count_never_moves_the_quota() {
        let mut c = MissionCycle::new(SPEC, 7);
        c.step(0.1, 35.1, false, |_| Some((1, 2, 2, 30.0)));
        assert_eq!(c.count(40.0, |m| m.target_kind == 3), None);
        assert_eq!(c.active().expect("in flight").left, 2);
        assert_eq!(
            c.count(41.0, |m| m.target_kind == 2),
            None,
            "one accepted count is progress, not completion"
        );
        assert_eq!(c.active().expect("in flight").left, 1);
    }

    #[test]
    fn an_offer_of_none_defers_without_consuming_the_schedule() {
        let mut c = MissionCycle::new(SPEC, 7);
        assert_eq!(c.step(0.1, 35.1, false, |_| None), None);
        assert!(c.active().is_none());
        assert_eq!(c.step(0.1, 35.2, false, slay), Some(CycleEvent::Started));
    }

    #[test]
    fn the_same_seed_replays_the_same_offer_sequence() {
        fn run(seed: u32) -> Vec<u32> {
            let mut c = MissionCycle::new(SPEC, seed);
            let mut totals = Vec::new();
            let mut now = 0.0;
            for _ in 0..4 {
                now += SPEC.first_secs + SPEC.gap_secs + 1.0;
                let ev = c.step(0.1, now, false, |rng| {
                    Some((0, 0, 5 + rng.next_u32() % 10, 25.0))
                });
                assert_eq!(ev, Some(CycleEvent::Started));
                totals.push(c.active().expect("in flight").total);
                // Fail it out so the cycle can offer the next.
                assert_eq!(
                    c.step(30.0, now + 30.0, false, |_| None),
                    Some(CycleEvent::Failed)
                );
                now += 30.0;
            }
            totals
        }
        assert_eq!(run(42), run(42));
        assert_ne!(run(42), run(43));
    }

    #[test]
    fn reset_rearms_the_first_mission_and_clears_the_score() {
        let mut c = MissionCycle::new(SPEC, 7);
        c.step(0.1, 35.1, false, slay);
        c.count(36.0, |_| true);
        c.count(36.0, |_| true);
        assert_eq!(c.count(36.0, |_| true), Some(CycleEvent::Succeeded));
        c.reset(9);
        assert_eq!((c.done(), c.failed()), (0, 0));
        assert!(c.active().is_none());
        assert_eq!(c.banner(), None);
        assert_eq!(c.step(0.1, 34.0, false, slay), None);
        assert_eq!(c.step(0.1, 35.1, false, slay), Some(CycleEvent::Started));
    }
}
