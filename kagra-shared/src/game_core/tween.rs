//! Easing and timing: how a value gets from A to B, and when things happen.
//!
//! - [`Ease`]: the usual curves (smooth, back, elastic, bounce), by name in JSON.
//! - [`Tween`]: one value moving over a fixed time with a curve.
//! - [`Timer`]: a countdown that can repeat (spawns, cooldowns, blinking).
//! - [`ping_pong`] and [`pulse`]: there and back, and a 0..1 breathing for prompts.

use serde::{Deserialize, Serialize};

/// An easing curve: `apply(0) = 0`, `apply(1) = 1`. Back and elastic overshoot in between.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ease {
    Linear,
    InQuad,
    OutQuad,
    #[default]
    InOutQuad,
    InCubic,
    OutCubic,
    InOutCubic,
    /// Half a cosine: gentle at both ends (lifts, swings).
    InOutSine,
    /// Pulls back a little before it goes.
    InBack,
    /// Overshoots and settles (a pop-in).
    OutBack,
    /// Springs past the end a few times.
    OutElastic,
    /// Lands and bounces.
    OutBounce,
}

impl Ease {
    /// The curve at `t` (clamped to 0..=1).
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Ease::Linear => t,
            Ease::InQuad => t * t,
            Ease::OutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            Ease::InOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) * 0.5
                }
            }
            Ease::InCubic => t * t * t,
            Ease::OutCubic => 1.0 - (1.0 - t).powi(3),
            Ease::InOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) * 0.5
                }
            }
            Ease::InOutSine => 0.5 - 0.5 * (t * std::f32::consts::PI).cos(),
            Ease::InBack => {
                let (c1, c3) = (1.70158, 2.70158);
                c3 * t * t * t - c1 * t * t
            }
            Ease::OutBack => {
                let (c1, c3) = (1.70158, 2.70158);
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            Ease::OutElastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let c4 = std::f32::consts::TAU / 3.0;
                    2f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }
            Ease::OutBounce => bounce(t),
        }
    }
}

fn bounce(t: f32) -> f32 {
    let (n1, d1) = (7.5625, 2.75);
    if t < 1.0 / d1 {
        n1 * t * t
    } else if t < 2.0 / d1 {
        let t = t - 1.5 / d1;
        n1 * t * t + 0.75
    } else if t < 2.5 / d1 {
        let t = t - 2.25 / d1;
        n1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / d1;
        n1 * t * t + 0.984375
    }
}

/// `a` to `b` by `k` (not clamped, so an overshooting curve overshoots).
pub fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}

/// One value moving from `from` to `to` over `secs`, along `ease`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    pub from: f32,
    pub to: f32,
    pub secs: f32,
    pub ease: Ease,
    /// Seconds since it started (it may wait first: a negative start is a delay).
    pub t: f32,
}

impl Tween {
    pub fn new(from: f32, to: f32, secs: f32, ease: Ease) -> Self {
        Self {
            from,
            to,
            secs: secs.max(1e-4),
            ease,
            t: 0.0,
        }
    }

    /// Start after `secs` of waiting at `from`.
    pub fn delayed(mut self, secs: f32) -> Self {
        self.t = -secs.max(0.0);
        self
    }

    /// A tween that is already at `v` and stays there.
    pub fn hold(v: f32) -> Self {
        Self {
            t: 1.0,
            ..Self::new(v, v, 1.0, Ease::Linear)
        }
    }

    /// Start again from where it is now toward `to` (a target that changed mid-way).
    pub fn retarget(&mut self, to: f32, secs: f32) {
        *self = Self::new(self.value(), to, secs, self.ease);
    }

    pub fn tick(&mut self, dt: f32) {
        self.t = (self.t + dt).min(self.secs);
    }

    /// How far along, 0..=1 (before any curve).
    pub fn progress(&self) -> f32 {
        (self.t / self.secs).clamp(0.0, 1.0)
    }

    pub fn value(&self) -> f32 {
        lerp(self.from, self.to, self.ease.apply(self.progress()))
    }

    pub fn done(&self) -> bool {
        self.t >= self.secs
    }
}

/// A countdown. `repeat` starts it again each time it rings (an interval).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timer {
    pub secs: f32,
    pub left: f32,
    pub repeat: bool,
}

impl Timer {
    /// Rings once after `secs`.
    pub fn once(secs: f32) -> Self {
        Self {
            secs,
            left: secs,
            repeat: false,
        }
    }

    /// Rings every `secs`.
    pub fn every(secs: f32) -> Self {
        Self {
            repeat: true,
            ..Self::once(secs.max(1e-4))
        }
    }

    /// Advance by `dt`; the number of times it rang (a repeating timer can ring more than once
    /// in a long frame; a one-shot rings once and then stays done).
    pub fn tick(&mut self, dt: f32) -> u32 {
        if self.left <= 0.0 && !self.repeat {
            return 0;
        }
        self.left -= dt;
        if self.left > 0.0 {
            return 0;
        }
        if !self.repeat {
            self.left = 0.0;
            return 1;
        }
        let mut rang = 0;
        while self.left <= 0.0 {
            self.left += self.secs;
            rang += 1;
        }
        rang
    }

    pub fn done(&self) -> bool {
        !self.repeat && self.left <= 0.0
    }

    /// 0 at the start, 1 when it rings.
    pub fn progress(&self) -> f32 {
        if self.secs <= 0.0 {
            1.0
        } else {
            (1.0 - self.left / self.secs).clamp(0.0, 1.0)
        }
    }

    pub fn restart(&mut self) {
        self.left = self.secs;
    }
}

/// There and back: 0 → 1 over `secs`, then 1 → 0 over `secs`, eased, forever. Also returns
/// the direction (+1 going out, -1 coming back), for a velocity.
pub fn ping_pong(t: f32, secs: f32, ease: Ease) -> (f32, f32) {
    let secs = secs.max(1e-4);
    let u = t.rem_euclid(2.0 * secs) / secs;
    if u < 1.0 {
        (ease.apply(u), 1.0)
    } else {
        (ease.apply(2.0 - u), -1.0)
    }
}

/// A slow breathing between `lo` and 1 (`hz` times a second): "tap to start" prompts.
pub fn pulse(t: f32, hz: f32, lo: f32) -> f32 {
    let k = 0.5 + 0.5 * (t * hz * std::f32::consts::TAU).sin();
    lo + (1.0 - lo) * k
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Ease; 12] = [
        Ease::Linear,
        Ease::InQuad,
        Ease::OutQuad,
        Ease::InOutQuad,
        Ease::InCubic,
        Ease::OutCubic,
        Ease::InOutCubic,
        Ease::InOutSine,
        Ease::InBack,
        Ease::OutBack,
        Ease::OutElastic,
        Ease::OutBounce,
    ];

    #[test]
    fn every_curve_starts_at_zero_and_ends_at_one() {
        for e in ALL {
            assert!(e.apply(0.0).abs() < 1e-4, "{e:?} at 0");
            assert!((e.apply(1.0) - 1.0).abs() < 1e-4, "{e:?} at 1");
            assert_eq!(e.apply(-3.0), e.apply(0.0));
            assert_eq!(e.apply(7.0), e.apply(1.0));
        }
        // Back overshoots, the gentle ones do not.
        assert!((0..100).any(|i| Ease::OutBack.apply(i as f32 / 100.0) > 1.0));
        assert!((0..100).all(|i| Ease::InOutSine.apply(i as f32 / 100.0) <= 1.0));
        assert!((Ease::InOutQuad.apply(0.5) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn curves_read_from_json_by_name() {
        let e: Ease = serde_json::from_str("\"out_back\"").unwrap();
        assert_eq!(e, Ease::OutBack);
        assert!(serde_json::from_str::<Ease>("\"wobbly\"").is_err());
    }

    #[test]
    fn a_tween_waits_moves_and_retargets() {
        let mut tw = Tween::new(0.0, 10.0, 1.0, Ease::Linear).delayed(0.5);
        tw.tick(0.25);
        assert_eq!(tw.value(), 0.0);
        tw.tick(0.75);
        assert!((tw.value() - 5.0).abs() < 1e-4);
        tw.retarget(0.0, 2.0);
        assert!((tw.value() - 5.0).abs() < 1e-4);
        tw.tick(5.0);
        assert!(tw.done() && tw.value() == 0.0);
        assert_eq!(Tween::hold(3.0).value(), 3.0);
    }

    #[test]
    fn timers_ring_once_or_every_interval() {
        let mut once = Timer::once(1.0);
        assert_eq!(once.tick(0.6), 0);
        assert!((once.progress() - 0.6).abs() < 1e-5);
        assert_eq!(once.tick(0.6), 1);
        assert_eq!(once.tick(5.0), 0);
        assert!(once.done());
        let mut every = Timer::every(0.5);
        assert_eq!(every.tick(1.2), 2);
        assert_eq!(every.tick(0.2), 0);
        assert_eq!(every.tick(0.1), 1);
        assert!(!every.done());
    }

    #[test]
    fn ping_pong_goes_out_and_back() {
        assert_eq!(ping_pong(0.5, 1.0, Ease::Linear), (0.5, 1.0));
        assert_eq!(ping_pong(1.5, 1.0, Ease::Linear), (0.5, -1.0));
        assert!((ping_pong(2.0, 1.0, Ease::Linear).0).abs() < 1e-5);
        assert!(pulse(0.0, 1.0, 0.4) >= 0.4 && pulse(0.3, 1.0, 0.4) <= 1.0);
    }
}
