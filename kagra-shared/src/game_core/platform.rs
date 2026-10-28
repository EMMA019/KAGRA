//! A platformer body in the x-y plane: run, jump, dash, land, against box solids. The feel
//! parts players expect are built in: coyote time (a jump just after running off a ledge
//! still counts), a jump buffer (a press just before landing jumps on landing), a short hop
//! when the button is let go early, one air dash, one-way platforms you jump up through, and
//! riding a moving platform. No physics engine: per-axis moves against rectangles, at the
//! fixed sim step. First used by Station Escape.
//!
//! Units are metres and seconds; +x is right, +y is up; a body's position is its feet.

use serde::{Deserialize, Serialize};

/// A solid rectangle: `x0..x1` by `y0..y1`, moving at `vel` (m/s) this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solid {
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
    pub vel: (f32, f32),
    /// Only its top is solid, and only from above: a body jumps up through it and stands on it.
    pub one_way: bool,
}

impl Solid {
    pub fn rect(x0: f32, x1: f32, y0: f32, y1: f32) -> Self {
        Self {
            x0,
            x1,
            y0,
            y1,
            vel: (0.0, 0.0),
            one_way: false,
        }
    }
}

/// How the body moves. Every field has a default; unknown keys are an error.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BodySpec {
    /// Half width and height (m).
    pub half_w: f32,
    pub height: f32,
    /// Top running speed (m/s) and how fast it gets there on the ground and in the air (m/s²).
    pub run: f32,
    pub accel: f32,
    pub air_accel: f32,
    /// Gravity (m/s², down), the jump's take-off speed (m/s), and the fastest fall (m/s).
    pub gravity: f32,
    pub jump: f32,
    pub max_fall: f32,
    /// Letting go of jump while rising multiplies the upward speed by this (a short hop).
    pub jump_cut: f32,
    /// Gravity is multiplied by this while falling (a snappier arc).
    pub fall_gravity: f32,
    /// Seconds after leaving a ledge that a jump still counts, and before landing that a press
    /// is remembered.
    pub coyote: f32,
    pub buffer: f32,
    /// The dash: speed (m/s), how long (s), the wait before the next (s). One in the air.
    pub dash: f32,
    pub dash_secs: f32,
    pub dash_cool: f32,
}

impl Default for BodySpec {
    fn default() -> Self {
        Self {
            half_w: 0.3,
            height: 1.1,
            run: 6.5,
            accel: 60.0,
            air_accel: 32.0,
            gravity: 30.0,
            jump: 11.5,
            max_fall: 18.0,
            jump_cut: 0.45,
            fall_gravity: 1.35,
            coyote: 0.1,
            buffer: 0.12,
            dash: 15.0,
            dash_secs: 0.16,
            dash_cool: 0.35,
        }
    }
}

impl BodySpec {
    pub fn validate(&self, name: &str) -> Result<(), String> {
        let range = |f: &str, v: f32, lo: f32, hi: f32| {
            if v.is_finite() && (lo..=hi).contains(&v) {
                Ok(())
            } else {
                Err(format!("{name}.{f} must be {lo}..={hi}, got {v}"))
            }
        };
        range("half_w", self.half_w, 0.05, 3.0)?;
        range("height", self.height, 0.1, 6.0)?;
        range("run", self.run, 0.1, 40.0)?;
        range("accel", self.accel, 0.1, 500.0)?;
        range("air_accel", self.air_accel, 0.0, 500.0)?;
        range("gravity", self.gravity, 0.1, 200.0)?;
        range("jump", self.jump, 0.0, 60.0)?;
        range("max_fall", self.max_fall, 0.1, 100.0)?;
        range("jump_cut", self.jump_cut, 0.0, 1.0)?;
        range("fall_gravity", self.fall_gravity, 0.1, 5.0)?;
        range("coyote", self.coyote, 0.0, 1.0)?;
        range("buffer", self.buffer, 0.0, 1.0)?;
        range("dash", self.dash, 0.0, 80.0)?;
        range("dash_secs", self.dash_secs, 0.0, 2.0)?;
        range("dash_cool", self.dash_cool, 0.0, 5.0)
    }
}

/// One step's input. `move_x` is -1..1; `jump` is the button held; `*_pressed` are new presses
/// since the last step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BodyInput {
    pub move_x: f32,
    pub jump: bool,
    pub jump_pressed: bool,
    pub dash_pressed: bool,
}

/// What happened this step (for sound and picture).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BodyEvents {
    pub jumped: bool,
    /// Landed, with the speed it hit the ground at (m/s).
    pub landed: Option<f32>,
    pub dashed: bool,
    /// Hit its head on a ceiling.
    pub bonked: bool,
    /// Index of the solid it stands on after the step.
    pub ground: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    /// Feet (m).
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub grounded: bool,
    /// +1 right, -1 left.
    pub facing: f32,
    coyote_t: f32,
    buffer_t: f32,
    dash_t: f32,
    dash_cool_t: f32,
    air_dash: bool,
    rising_held: bool,
}

impl Body {
    pub fn at(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            grounded: false,
            facing: 1.0,
            coyote_t: 0.0,
            buffer_t: 0.0,
            dash_t: 0.0,
            dash_cool_t: 0.0,
            air_dash: true,
            rising_held: false,
        }
    }

    /// True during a dash.
    pub fn dashing(&self) -> bool {
        self.dash_t > 0.0
    }

    /// Put the body somewhere at rest (a respawn).
    pub fn reset_to(&mut self, x: f32, y: f32) {
        *self = Self {
            facing: self.facing,
            ..Self::at(x, y)
        };
    }

    /// Its box: x0, x1, y0, y1.
    pub fn bounds(&self, spec: &BodySpec) -> (f32, f32, f32, f32) {
        (
            self.x - spec.half_w,
            self.x + spec.half_w,
            self.y,
            self.y + spec.height,
        )
    }

    /// One fixed step. `solids` are where they are at the start of the step; their `vel` is
    /// how far they move during it (a body standing on one rides it).
    pub fn step(
        &mut self,
        spec: &BodySpec,
        input: BodyInput,
        solids: &[Solid],
        dt: f32,
    ) -> BodyEvents {
        let mut ev = BodyEvents::default();
        // Ride the platform under the feet before anything else.
        if self.grounded {
            if let Some(i) = self.standing_on(spec, solids) {
                self.x += solids[i].vel.0 * dt;
                self.y += solids[i].vel.1 * dt;
            }
        }
        self.coyote_t = if self.grounded {
            spec.coyote
        } else {
            (self.coyote_t - dt).max(0.0)
        };
        self.buffer_t = if input.jump_pressed {
            spec.buffer
        } else {
            (self.buffer_t - dt).max(0.0)
        };
        self.dash_cool_t = (self.dash_cool_t - dt).max(0.0);
        let mx = input.move_x.clamp(-1.0, 1.0);
        if mx.abs() > 0.1 {
            self.facing = mx.signum();
        }
        // Dash: a burst along the facing, no gravity while it lasts.
        if input.dash_pressed
            && self.dash_cool_t <= 0.0
            && (self.grounded || self.air_dash)
            && spec.dash > 0.0
        {
            self.dash_t = spec.dash_secs;
            self.dash_cool_t = spec.dash_cool;
            if !self.grounded {
                self.air_dash = false;
            }
            self.vx = self.facing * spec.dash;
            self.vy = 0.0;
            ev.dashed = true;
        }
        if self.dash_t > 0.0 {
            self.dash_t -= dt;
            if self.dash_t <= 0.0 {
                self.vx = self.facing * spec.run;
            }
        } else {
            let want = mx * spec.run;
            let a = if self.grounded {
                spec.accel
            } else {
                spec.air_accel
            };
            let d = want - self.vx;
            self.vx += d.clamp(-a * dt, a * dt);
            // Jump (with coyote time and the buffer).
            if self.buffer_t > 0.0 && self.coyote_t > 0.0 {
                self.vy = spec.jump;
                self.buffer_t = 0.0;
                self.coyote_t = 0.0;
                self.grounded = false;
                self.rising_held = true;
                ev.jumped = true;
            }
            if self.rising_held && !input.jump && self.vy > 0.0 {
                self.vy *= spec.jump_cut;
                self.rising_held = false;
            }
            if self.vy <= 0.0 {
                self.rising_held = false;
            }
            let g = if self.vy < 0.0 {
                spec.gravity * spec.fall_gravity
            } else {
                spec.gravity
            };
            self.vy = (self.vy - g * dt).max(-spec.max_fall);
        }
        // Move x, then y, stopping at solids.
        let fall_speed = -self.vy;
        self.move_x(spec, solids, self.vx * dt);
        let (hit_floor, hit_ceiling) = self.move_y(spec, solids, self.vy * dt);
        let was = self.grounded;
        self.grounded = hit_floor || (self.vy <= 0.0 && self.standing_on(spec, solids).is_some());
        if self.grounded {
            self.vy = 0.0;
            self.air_dash = true;
            if !was {
                ev.landed = Some(fall_speed.max(0.0));
            }
        }
        if hit_ceiling {
            self.vy = self.vy.min(0.0);
            ev.bonked = true;
        }
        ev.ground = if self.grounded {
            self.standing_on(spec, solids)
        } else {
            None
        };
        ev
    }

    /// The solid right under the feet, if any.
    pub fn standing_on(&self, spec: &BodySpec, solids: &[Solid]) -> Option<usize> {
        let (x0, x1, y0, _) = self.bounds(spec);
        solids
            .iter()
            .position(|s| x1 > s.x0 + 1e-4 && x0 < s.x1 - 1e-4 && (y0 - s.y1).abs() < 0.03)
    }

    fn move_x(&mut self, spec: &BodySpec, solids: &[Solid], dx: f32) {
        if dx == 0.0 {
            return;
        }
        self.x += dx;
        let (x0, x1, y0, y1) = self.bounds(spec);
        for s in solids {
            if s.one_way {
                continue;
            }
            let overlap = x1 > s.x0 && x0 < s.x1 && y1 > s.y0 + 1e-3 && y0 < s.y1 - 1e-3;
            if !overlap {
                continue;
            }
            if dx > 0.0 {
                self.x = s.x0 - spec.half_w - 1e-4;
            } else {
                self.x = s.x1 + spec.half_w + 1e-4;
            }
            self.vx = 0.0;
            if self.dash_t > 0.0 {
                self.dash_t = 0.0;
            }
        }
    }

    /// Returns (landed on a floor, hit a ceiling).
    fn move_y(&mut self, spec: &BodySpec, solids: &[Solid], dy: f32) -> (bool, bool) {
        let before = self.y;
        self.y += dy;
        let (x0, x1, y0, y1) = self.bounds(spec);
        let mut floor = false;
        let mut ceiling = false;
        for s in solids {
            let across = x1 > s.x0 + 1e-4 && x0 < s.x1 - 1e-4;
            if !across {
                continue;
            }
            if s.one_way {
                // Only from above: the feet were at or over its top before the step.
                if dy <= 0.0 && before >= s.y1 - 1e-3 && y0 < s.y1 {
                    self.y = s.y1;
                    floor = true;
                }
                continue;
            }
            if !(y1 > s.y0 && y0 < s.y1) {
                continue;
            }
            if dy <= 0.0 {
                self.y = s.y1;
                floor = true;
            } else {
                self.y = s.y0 - spec.height;
                ceiling = true;
            }
        }
        (floor, ceiling)
    }
}

/// Overlap of a body's box with a rectangle (hazards, pickups, doors).
pub fn touches(body: &Body, spec: &BodySpec, x0: f32, x1: f32, y0: f32, y1: f32) -> bool {
    let (bx0, bx1, by0, by1) = body.bounds(spec);
    bx1 > x0 && bx0 < x1 && by1 > y0 && by0 < y1
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn floor() -> Vec<Solid> {
        vec![Solid::rect(-50.0, 50.0, -1.0, 0.0)]
    }

    fn run(b: &mut Body, s: &BodySpec, solids: &[Solid], input: BodyInput, steps: usize) {
        for _ in 0..steps {
            b.step(s, input, solids, DT);
        }
    }

    #[test]
    fn it_falls_lands_and_runs() {
        let s = BodySpec::default();
        let mut b = Body::at(0.0, 3.0);
        let mut landed = None;
        for _ in 0..120 {
            if let Some(v) = b.step(&s, BodyInput::default(), &floor(), DT).landed {
                landed = Some(v);
            }
        }
        assert!(b.grounded && b.y.abs() < 1e-3);
        assert!(landed.unwrap() > 5.0, "lands with speed");
        let go = BodyInput {
            move_x: 1.0,
            ..Default::default()
        };
        run(&mut b, &s, &floor(), go, 60);
        assert!((b.vx - s.run).abs() < 1e-3 && b.x > 4.0);
    }

    #[test]
    fn a_held_jump_goes_higher_than_a_tap() {
        let s = BodySpec::default();
        let peak = |hold: usize| {
            let mut b = Body::at(0.0, 0.0);
            run(&mut b, &s, &floor(), BodyInput::default(), 2);
            let mut top: f32 = 0.0;
            for i in 0..90 {
                let input = BodyInput {
                    jump: i < hold,
                    jump_pressed: i == 0,
                    ..Default::default()
                };
                b.step(&s, input, &floor(), DT);
                top = top.max(b.y);
            }
            top
        };
        let (tap, held) = (peak(2), peak(60));
        assert!(held > 2.0, "a full jump clears 2 m: {held}");
        assert!(tap < held * 0.6, "a tap is a hop: {tap} vs {held}");
    }

    #[test]
    fn coyote_time_and_the_buffer_forgive_late_and_early_presses() {
        let s = BodySpec::default();
        let ledge = vec![Solid::rect(-10.0, 0.0, -1.0, 0.0)];
        // Run off the ledge, press jump 3 steps later: still a jump.
        let mut b = Body::at(-1.0, 0.0);
        run(&mut b, &s, &ledge, BodyInput::default(), 2);
        let go = BodyInput {
            move_x: 1.0,
            ..Default::default()
        };
        while b.grounded {
            b.step(&s, go, &ledge, DT);
        }
        run(&mut b, &s, &ledge, go, 3);
        let ev = b.step(
            &s,
            BodyInput {
                move_x: 1.0,
                jump: true,
                jump_pressed: true,
                ..Default::default()
            },
            &ledge,
            DT,
        );
        assert!(ev.jumped, "coyote jump");
        // Press 4 steps before landing: it jumps on landing.
        let mut b = Body::at(0.0, 0.35);
        b.vy = -5.0;
        b.step(
            &s,
            BodyInput {
                jump: true,
                jump_pressed: true,
                ..Default::default()
            },
            &floor(),
            DT,
        );
        let held = BodyInput {
            jump: true,
            ..Default::default()
        };
        let mut jumped = false;
        for _ in 0..10 {
            jumped |= b.step(&s, held, &floor(), DT).jumped;
        }
        assert!(jumped, "buffered jump");
    }

    #[test]
    fn walls_stop_it_and_ceilings_bonk() {
        let s = BodySpec::default();
        let mut solids = floor();
        solids.push(Solid::rect(2.0, 3.0, 0.0, 5.0));
        solids.push(Solid::rect(-3.0, -1.0, 1.6, 2.0));
        let mut b = Body::at(0.0, 0.0);
        let go = BodyInput {
            move_x: 1.0,
            ..Default::default()
        };
        run(&mut b, &s, &solids, go, 90);
        assert!(
            (b.x - (2.0 - s.half_w)).abs() < 0.01,
            "against the wall: {}",
            b.x
        );
        let mut b = Body::at(-2.0, 0.0);
        run(&mut b, &s, &solids, BodyInput::default(), 2);
        let mut bonk = false;
        for i in 0..40 {
            let ev = b.step(
                &s,
                BodyInput {
                    jump: true,
                    jump_pressed: i == 0,
                    ..Default::default()
                },
                &solids,
                DT,
            );
            bonk |= ev.bonked;
            assert!(b.y + s.height <= 1.6 + 1e-3);
        }
        assert!(bonk);
    }

    #[test]
    fn one_way_platforms_let_it_up_and_hold_it() {
        let s = BodySpec::default();
        let mut solids = floor();
        solids.push(Solid {
            one_way: true,
            ..Solid::rect(-2.0, 2.0, 1.6, 1.8)
        });
        let mut b = Body::at(0.0, 0.0);
        run(&mut b, &s, &solids, BodyInput::default(), 2);
        let jump = BodyInput {
            jump: true,
            jump_pressed: true,
            ..Default::default()
        };
        b.step(&s, jump, &solids, DT);
        run(
            &mut b,
            &s,
            &solids,
            BodyInput {
                jump: true,
                ..Default::default()
            },
            90,
        );
        assert!(
            b.grounded && (b.y - 1.8).abs() < 1e-3,
            "stands on it: {}",
            b.y
        );
    }

    #[test]
    fn the_dash_crosses_a_gap_a_jump_alone_does_not_and_rides_platforms() {
        let s = BodySpec::default();
        let gap = vec![
            Solid::rect(-20.0, 0.0, -1.0, 0.0),
            Solid::rect(5.8, 30.0, -1.0, 0.0),
        ];
        let cross = |dash: bool| {
            let mut b = Body::at(-6.0, 0.0);
            let mut jumped = false;
            for i in 0..240 {
                let near = b.x > -0.4 && !jumped;
                let input = BodyInput {
                    move_x: 1.0,
                    jump: true,
                    jump_pressed: near,
                    dash_pressed: dash && b.vy < 0.5 && b.x > 2.0 && b.x < 3.0,
                };
                jumped |= near;
                b.step(&s, input, &gap, DT);
                let _ = i;
            }
            b.y > -0.5
        };
        assert!(!cross(false), "the jump alone falls short");
        assert!(cross(true), "jump + air dash makes it");
        // Riding: a platform moving right carries the body.
        let lift = vec![Solid {
            vel: (2.0, 0.0),
            ..Solid::rect(-1.0, 1.0, -0.2, 0.0)
        }];
        let mut b = Body::at(0.0, 0.0);
        b.grounded = true;
        run(&mut b, &s, &lift, BodyInput::default(), 30);
        assert!((b.x - 1.0).abs() < 0.05, "rode it: {}", b.x);
    }
}
