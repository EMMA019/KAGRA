//! The Warden: the station's last guard, a big drone over an arena.
//!
//! It loops through four beats:
//!
//! 1. **Hover** high over the arena, drifting after the hero and firing aimed bolts.
//! 2. **Warn**: it glows red, slides to one side of the arena and drops low.
//! 3. **Swoop**: it sweeps across at head height. Jump over it, or land on it (a stomp) or
//!    dash into it: that is a hit, it reels and climbs.
//! 4. **Climb** back to hover height.
//!
//! Each hit takes one of `hp`. Touching it any other way sends the hero back. At 0 it breaks
//! apart and the exit opens. A respawn keeps the hits already landed (fair on a long fight)
//! and starts the loop over.

use serde::{Deserialize, Serialize};

use super::foes::Foes;
use crate::game_core::platform::{touches, Body, BodySpec};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Boss {
    /// The arena (x) it flies over, and its high and low lines (m).
    pub x0: f32,
    pub x1: f32,
    pub high: f32,
    pub low: f32,
    pub hp: u32,
    /// Seconds of each beat.
    pub hover_secs: f32,
    pub warn_secs: f32,
    pub swoop_secs: f32,
    pub climb_secs: f32,
    /// Aimed bolts per hover, and their speed (m/s).
    pub shots: u32,
    pub shot_speed: f32,
    /// Each hit makes the next loop this much quicker (0.1 = 10 %).
    pub hurry: f32,
}

impl Default for Boss {
    fn default() -> Self {
        Self {
            x0: 4.0,
            x1: 24.0,
            high: 5.6,
            low: 1.0,
            hp: 3,
            hover_secs: 3.2,
            warn_secs: 0.9,
            swoop_secs: 2.6,
            climb_secs: 0.8,
            shots: 3,
            shot_speed: 6.0,
            hurry: 0.15,
        }
    }
}

/// Half sizes of its hit box (m).
pub const HALF: (f32, f32) = (1.0, 0.55);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beat {
    Hover,
    Warn,
    Swoop,
    Climb,
    Dead,
}

/// What the Warden did this step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BossHits {
    pub hurt: bool,
    pub hit: bool,
    pub killed: bool,
    pub fired: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Warden {
    pub hp: u32,
    pub beat: Beat,
    /// Seconds into the beat.
    pub t: f32,
    pub x: f32,
    pub y: f32,
    /// +1 when the swoop goes right.
    pub dir: f32,
    /// Hit flash (s left), and the shots fired this hover.
    pub flash: f32,
    shots_fired: u32,
    /// Where the swoop started (x).
    from_x: f32,
}

impl Warden {
    pub fn new(b: &Boss) -> Self {
        Self {
            hp: b.hp,
            beat: Beat::Hover,
            t: 0.0,
            x: 0.5 * (b.x0 + b.x1),
            y: b.high,
            dir: 1.0,
            flash: 0.0,
            shots_fired: 0,
            from_x: b.x0,
        }
    }

    pub fn dead(&self) -> bool {
        self.beat == Beat::Dead
    }

    /// After a respawn: the hits stay, the loop starts over high in the middle.
    pub fn restart(&mut self, b: &Boss) {
        if self.dead() {
            return;
        }
        let hp = self.hp;
        *self = Self::new(b);
        self.hp = hp;
    }

    /// The loop runs quicker as it takes hits.
    fn pace(&self, b: &Boss) -> f32 {
        1.0 + b.hurry * (b.hp - self.hp) as f32
    }

    fn enter(&mut self, beat: Beat) {
        self.beat = beat;
        self.t = 0.0;
        self.shots_fired = 0;
    }

    /// Can a stomp or a dash hurt it now? Only when it is low (the warn and the swoop).
    pub fn open(&self) -> bool {
        matches!(self.beat, Beat::Warn | Beat::Swoop) && self.flash <= 0.0
    }

    /// One step.
    pub fn step(
        &mut self,
        b: &Boss,
        foes: &mut Foes,
        body: &mut Body,
        spec: &BodySpec,
        dt: f32,
        bounce: f32,
    ) -> BossHits {
        let mut out = BossHits::default();
        if self.dead() {
            return out;
        }
        self.flash = (self.flash - dt).max(0.0);
        let pace = self.pace(b);
        self.t += dt * pace;
        let ease = |k: f32| {
            let k = k.clamp(0.0, 1.0);
            k * k * (3.0 - 2.0 * k)
        };
        match self.beat {
            Beat::Hover => {
                // Drift after the hero, inside the arena, with a slow bob.
                let want = body.x.clamp(b.x0 + 2.0, b.x1 - 2.0);
                self.x += (want - self.x) * (dt * 0.9).min(1.0);
                self.y = b.high + 0.25 * (self.t * 2.0).sin();
                let gap = b.hover_secs / (b.shots + 1) as f32;
                if self.shots_fired < b.shots && self.t >= gap * (self.shots_fired + 1) as f32 {
                    self.shots_fired += 1;
                    let (dx, dy) = (body.x - self.x, body.y + 0.5 - (self.y - 0.3));
                    let d = dx.hypot(dy).max(0.01);
                    let v = b.shot_speed * pace;
                    if foes.fire(self.x, self.y - 0.3, dx / d * v, dy / d * v, 30.0) {
                        out.fired += 1;
                    }
                }
                if self.t >= b.hover_secs {
                    // Swoop from the side further from the hero, toward them.
                    self.dir = if body.x > 0.5 * (b.x0 + b.x1) {
                        1.0
                    } else {
                        -1.0
                    };
                    self.from_x = if self.dir > 0.0 { b.x0 } else { b.x1 };
                    self.enter(Beat::Warn);
                }
            }
            Beat::Warn => {
                let k = ease(self.t / b.warn_secs);
                self.x += (self.from_x - self.x) * k.min(1.0) * 0.25;
                self.y += (b.low - self.y) * k.min(1.0) * 0.25;
                if self.t >= b.warn_secs {
                    self.x = self.from_x;
                    self.y = b.low;
                    self.enter(Beat::Swoop);
                }
            }
            Beat::Swoop => {
                let k = ease(self.t / b.swoop_secs);
                let to = if self.dir > 0.0 { b.x1 } else { b.x0 };
                self.x = self.from_x + (to - self.from_x) * k;
                self.y = b.low + 0.15 * (self.t * 9.0).sin();
                if self.t >= b.swoop_secs {
                    self.enter(Beat::Climb);
                }
            }
            Beat::Climb => {
                let k = ease(self.t / b.climb_secs);
                self.y = b.low + (b.high - b.low) * k;
                if self.t >= b.climb_secs {
                    self.enter(Beat::Hover);
                }
            }
            Beat::Dead => {}
        }
        // The hero meets it.
        let (hw, hh) = HALF;
        if touches(
            body,
            spec,
            self.x - hw,
            self.x + hw,
            self.y - hh,
            self.y + hh,
        ) {
            // Landing on it: coming down, or at the top of a jump, with the feet over its middle.
            let from_above = body.vy < 1.0 && body.y > self.y - 0.1;
            if self.open() && (from_above || body.dashing()) {
                self.hp = self.hp.saturating_sub(1);
                self.flash = 0.6;
                out.hit = true;
                if from_above {
                    body.vy = bounce;
                    body.y = body.y.max(self.y + hh);
                }
                if self.hp == 0 {
                    self.enter(Beat::Dead);
                    out.killed = true;
                } else {
                    self.enter(Beat::Climb);
                }
            } else if self.flash <= 0.0 {
                out.hurt = true;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn run_until(w: &mut Warden, b: &Boss, beat: Beat, body: &mut Body) {
        let spec = BodySpec::default();
        let mut foes = Foes::default();
        for _ in 0..2000 {
            if w.beat == beat {
                return;
            }
            w.step(b, &mut foes, body, &spec, DT, 9.0);
        }
        panic!("never reached {beat:?}");
    }

    #[test]
    fn it_hovers_fires_then_swoops_low_toward_the_hero() {
        let b = Boss::default();
        let mut w = Warden::new(&b);
        let spec = BodySpec::default();
        let mut foes = Foes::default();
        let mut far = Body::at(b.x1 - 1.0, -50.0);
        let mut fired = 0;
        while w.beat == Beat::Hover {
            fired += w.step(&b, &mut foes, &mut far, &spec, DT, 9.0).fired;
        }
        assert_eq!(fired, b.shots);
        assert!(
            foes.bolts.iter().flatten().any(|bo| bo[3] < 0.0),
            "aimed down at the hero"
        );
        run_until(&mut w, &b, Beat::Swoop, &mut far);
        assert!(
            (w.y - b.low).abs() < 0.2 && (w.x - b.x0).abs() < 0.1,
            "low, from the far side"
        );
        run_until(&mut w, &b, Beat::Climb, &mut far);
        assert!((w.x - b.x1).abs() < 0.1, "swept across");
    }

    #[test]
    fn stomps_in_the_swoop_hurt_it_and_touching_it_high_hurts_you() {
        let b = Boss::default();
        let spec = BodySpec::default();
        let mut foes = Foes::default();
        let mut w = Warden::new(&b);
        // High: touching hurts the hero.
        let mut body = Body::at(w.x, w.y - 0.5);
        body.vy = 2.0;
        assert!(w.step(&b, &mut foes, &mut body, &spec, DT, 9.0).hurt);
        // Low in the swoop: a stomp lands, three end it.
        let mut away = Body::at(-100.0, -100.0);
        for k in 0..b.hp {
            run_until(&mut w, &b, Beat::Swoop, &mut away);
            let mut hero = Body::at(w.x, w.y + HALF.1 - 0.05);
            hero.vy = -4.0;
            let h = w.step(&b, &mut foes, &mut hero, &spec, DT, 9.0);
            assert!(h.hit, "hit {k}");
            assert_eq!(hero.vy, 9.0, "bounce");
        }
        assert!(w.dead());
        // A respawn keeps the damage.
        let mut w = Warden::new(&b);
        w.hp = 1;
        w.beat = Beat::Swoop;
        w.restart(&b);
        assert_eq!((w.hp, w.beat), (1, Beat::Hover));
    }
}
