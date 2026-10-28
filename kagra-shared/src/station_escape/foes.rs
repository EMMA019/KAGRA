//! The station's defences: patrol drones and wall turrets.
//!
//! A drone flies back and forth between `x0` and `x1` at height `y`. Touch it and you fall
//! back to the checkpoint; land on it from above (a stomp) or dash through it and it is
//! wrecked, and a stomp bounces you up. Wrecked drones come back when you respawn.
//!
//! A turret fires a bolt along `dir` every `every` seconds. A bolt flies straight until it
//! hits a solid or has gone `range` metres; touch one and you fall back. Bolts are a fixed
//! pool, so a frame never allocates.

use serde::{Deserialize, Serialize};

use crate::game_core::platform::{touches, Body, BodySpec, Solid};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Drone {
    pub x0: f32,
    pub x1: f32,
    pub y: f32,
    /// m/s along its patrol, and where it starts (0..1 of the way from x0).
    pub speed: f32,
    pub phase: f32,
}

impl Default for Drone {
    fn default() -> Self {
        Self {
            x0: 0.0,
            x1: 4.0,
            y: 1.0,
            speed: 2.0,
            phase: 0.0,
        }
    }
}

impl Drone {
    /// Where it is at stage time `t`, and which way it is heading (+1 / -1).
    pub fn at(&self, t: f32) -> (f32, f32, f32) {
        let span = (self.x1 - self.x0).abs().max(0.01);
        let period = 2.0 * span / self.speed.max(0.01);
        let u = (t / period + self.phase).rem_euclid(1.0);
        let (k, dir) = if u < 0.5 {
            (u * 2.0, 1.0)
        } else {
            (2.0 - u * 2.0, -1.0)
        };
        let x = self.x0.min(self.x1) + span * k;
        // A slow hover bob.
        (x, self.y + 0.12 * (t * 2.3 + self.phase * 7.0).sin(), dir)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Turret {
    /// The muzzle.
    pub x: f32,
    pub y: f32,
    /// -1 fires left, +1 right.
    pub dir: f32,
    /// Seconds between shots, the first shot's delay, bolt speed (m/s) and range (m).
    pub every: f32,
    pub phase: f32,
    pub speed: f32,
    pub range: f32,
}

impl Default for Turret {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.6,
            dir: -1.0,
            every: 2.0,
            phase: 0.0,
            speed: 7.0,
            range: 16.0,
        }
    }
}

/// Half sizes of a drone's and a bolt's hit box (m).
pub const DRONE_HALF: (f32, f32) = (0.45, 0.3);
pub const BOLT_HALF: f32 = 0.15;
/// Bolts in flight at most.
pub const BOLTS: usize = 16;

/// A bolt in flight: x, y, vx, vy, metres left.
pub type Bolt = [f32; 5];

/// What the foes did to the hero this step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hits {
    pub hurt: bool,
    pub stomped: u32,
    pub wrecked: u32,
    pub fired: u32,
}

/// The foes of one stage, as they are now.
#[derive(Clone, Debug, Default)]
pub struct Foes {
    pub alive: Vec<bool>,
    /// Seconds each turret waits before its next shot.
    pub reload: Vec<f32>,
    pub bolts: [Option<Bolt>; BOLTS],
}

impl Foes {
    pub fn new(drones: &[Drone], turrets: &[Turret]) -> Self {
        Self {
            alive: vec![true; drones.len()],
            reload: turrets.iter().map(|t| t.phase.max(0.0)).collect(),
            bolts: [None; BOLTS],
        }
    }

    /// Back as the stage started (a respawn): drones mended, bolts gone, turrets reset.
    pub fn reset(&mut self, turrets: &[Turret]) {
        self.alive.iter_mut().for_each(|a| *a = true);
        for (r, t) in self.reload.iter_mut().zip(turrets) {
            *r = t.phase.max(0.0);
        }
        self.bolts = [None; BOLTS];
    }

    /// Put a bolt in flight (a boss's aimed shot). False when the pool is full.
    pub fn fire(&mut self, x: f32, y: f32, vx: f32, vy: f32, range: f32) -> bool {
        match self.bolts.iter_mut().find(|b| b.is_none()) {
            Some(slot) => {
                *slot = Some([x, y, vx, vy, range]);
                true
            }
            None => false,
        }
    }

    /// One step: turrets fire, bolts fly, and the hero meets them. A stomp sets the body's
    /// upward speed to `bounce`.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        drones: &[Drone],
        turrets: &[Turret],
        solids: &[Solid],
        body: &mut Body,
        spec: &BodySpec,
        t: f32,
        dt: f32,
        bounce: f32,
    ) -> Hits {
        let mut hits = Hits::default();
        for (i, tu) in turrets.iter().enumerate() {
            self.reload[i] -= dt;
            if self.reload[i] <= 0.0 {
                self.reload[i] += tu.every.max(0.2);
                if let Some(slot) = self.bolts.iter_mut().find(|b| b.is_none()) {
                    *slot = Some([tu.x, tu.y, tu.dir.signum() * tu.speed, 0.0, tu.range]);
                    hits.fired += 1;
                }
            }
        }
        for slot in &mut self.bolts {
            let Some(b) = slot.as_mut() else {
                continue;
            };
            let (dx, dy) = (b[2] * dt, b[3] * dt);
            b[0] += dx;
            b[1] += dy;
            b[4] -= dx.hypot(dy);
            let hit_wall = solids
                .iter()
                .any(|s| !s.one_way && b[0] > s.x0 && b[0] < s.x1 && b[1] > s.y0 && b[1] < s.y1);
            if b[4] <= 0.0 || hit_wall {
                *slot = None;
                continue;
            }
            if touches(
                body,
                spec,
                b[0] - BOLT_HALF,
                b[0] + BOLT_HALF,
                b[1] - BOLT_HALF,
                b[1] + BOLT_HALF,
            ) {
                hits.hurt = true;
                *slot = None;
            }
        }
        for (i, d) in drones.iter().enumerate() {
            if !self.alive[i] {
                continue;
            }
            let (x, y, _) = d.at(t);
            let (hw, hh) = DRONE_HALF;
            if !touches(body, spec, x - hw, x + hw, y - hh, y + hh) {
                continue;
            }
            if body.dashing() {
                self.alive[i] = false;
                hits.wrecked += 1;
            } else if body.vy < 0.0 && body.y > y {
                self.alive[i] = false;
                hits.stomped += 1;
                body.vy = bounce;
                body.y = body.y.max(y + hh);
            } else {
                hits.hurt = true;
            }
        }
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_core::platform::BodyInput;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn a_drone_patrols_out_and_back() {
        let d = Drone {
            x0: 0.0,
            x1: 4.0,
            speed: 2.0,
            ..Drone::default()
        };
        assert!(d.at(0.0).0.abs() < 1e-3);
        assert!((d.at(2.0).0 - 4.0).abs() < 1e-3, "out in 2 s");
        assert!(d.at(3.0).2 < 0.0, "then back");
        assert!(d.at(4.0).0.abs() < 1e-3);
    }

    #[test]
    fn touching_hurts_stomping_and_dashing_wreck() {
        let spec = BodySpec::default();
        let drones = [Drone {
            x0: 0.0,
            x1: 0.0001,
            y: 1.0,
            speed: 0.1,
            ..Drone::default()
        }];
        let mut foes = Foes::new(&drones, &[]);
        let mut b = Body::at(0.0, 0.5);
        b.vy = 2.0;
        let h = foes.step(&drones, &[], &[], &mut b, &spec, 0.0, DT, 9.0);
        assert!(h.hurt, "from below hurts");
        let mut b = Body::at(0.0, 1.2);
        b.vy = -3.0;
        let h = foes.step(&drones, &[], &[], &mut b, &spec, 0.0, DT, 9.0);
        assert_eq!(h.stomped, 1);
        assert_eq!(b.vy, 9.0, "bounce");
        foes.reset(&[]);
        let mut b = Body::at(-1.0, 0.6);
        let floor = [Solid::rect(-9.0, 9.0, -1.0, 0.6)];
        b.step(
            &spec,
            BodyInput {
                move_x: 1.0,
                dash_pressed: true,
                ..Default::default()
            },
            &floor,
            DT,
        );
        assert!(b.dashing());
        let h = foes.step(&drones, &[], &floor, &mut b, &spec, 0.0, DT, 9.0);
        let h2 = (0..10)
            .map(|_| {
                b.step(&spec, BodyInput::default(), &floor, DT);
                foes.step(&drones, &[], &floor, &mut b, &spec, 0.0, DT, 9.0)
            })
            .fold(h, |a, x| Hits {
                wrecked: a.wrecked + x.wrecked,
                hurt: a.hurt || x.hurt,
                ..a
            });
        assert_eq!(h2.wrecked, 1, "dash wrecks");
        assert!(!h2.hurt);
    }

    #[test]
    fn turrets_fire_bolts_that_walls_stop() {
        let spec = BodySpec::default();
        let turrets = [Turret {
            x: 5.0,
            y: 0.5,
            dir: -1.0,
            every: 1.0,
            phase: 0.0,
            speed: 10.0,
            range: 20.0,
        }];
        let wall = [Solid::rect(1.0, 1.5, 0.0, 3.0)];
        let mut foes = Foes::new(&[], &turrets);
        let mut far = Body::at(-5.0, 0.0);
        let mut fired = 0;
        for _ in 0..120 {
            let h = foes.step(&[], &turrets, &wall, &mut far, &spec, 0.0, DT, 9.0);
            fired += h.fired;
            assert!(!h.hurt, "the wall stops them");
            for b in foes.bolts.iter().flatten() {
                assert!(b[0] > 1.0);
            }
        }
        assert_eq!(fired, 2);
        let mut foes = Foes::new(&[], &turrets);
        let mut near = Body::at(2.0, 0.0);
        let hurt = (0..60).any(|_| {
            foes.step(&[], &turrets, &[], &mut near, &spec, 0.0, DT, 9.0)
                .hurt
        });
        assert!(hurt, "an open line hits");
    }
}
