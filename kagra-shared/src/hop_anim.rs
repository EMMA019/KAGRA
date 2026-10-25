//! HOP-BEAT's procedural animation: pure functions and one small spring, no
//! renderer and no sim state, so every curve is unit-tested.
//!
//! The hero and the enemies are single meshes, so "animation" is scale, tilt
//! and offset per frame. Everything that repeats is driven by the beat clock
//! (`beat_pos`, the fraction of the beat), never by wall time, so the bunny
//! bobs on the same grid the music plays on.

use std::f32::consts::TAU;

/// A damped spring (underdamped on purpose: the overshoot is the point).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wobble {
    pub x: f32,
    pub v: f32,
}

impl Wobble {
    /// `omega` rad/s, `zeta` damping ratio (0.3 floppy, 0.5 springy, 1.0 none).
    pub fn step(&mut self, target: f32, omega: f32, zeta: f32, dt: f32) {
        // Semi-implicit Euler in small slices: stable for a 60 Hz frame at omega < 60.
        let n = ((dt * 240.0).ceil() as u32).clamp(1, 16);
        let h = dt / n as f32;
        for _ in 0..n {
            let a = omega * omega * (target - self.x) - 2.0 * zeta * omega * self.v;
            self.v += a * h;
            self.x += self.v * h;
        }
    }
}

/// The body's squash spring: positive = squashed (wide and short), negative = stretched.
pub const SQUASH_OMEGA: f32 = 24.0;
pub const SQUASH_ZETA: f32 = 0.42;
/// The ears trail the body: floppier and slower.
pub const EAR_OMEGA: f32 = 17.0;
pub const EAR_ZETA: f32 = 0.28;

/// Squash spring target from the pose: ground idle breathes on the beat, the
/// air stretches along the direction of travel.
pub fn squash_target(grounded: bool, vy: f32, beat_flash: f32) -> f32 {
    if grounded {
        0.07 * beat_flash
    } else {
        // Rising stretches the most, falling stretches a little, the apex is round.
        let r = (vy / 6.0).clamp(-1.0, 1.0);
        -(if r > 0.0 { 0.30 * r } else { 0.18 * -r })
    }
}

/// Body scale `[x, y, z]` for a squash value (volume roughly kept).
pub fn body_scale(squash: f32) -> [f32; 3] {
    let s = squash.clamp(-0.6, 1.0);
    let sy = 1.0 - 0.30 * s;
    let sx = 1.0 + 0.24 * s;
    [sx, sy, sx]
}

/// Ear pitch (radians, + folds back over the head). The ears lag the
/// vertical speed and breathe a little on the beat.
pub fn ear_target(vy: f32, beat_pos: f32, grounded: bool) -> f32 {
    let lag = (vy * 0.07).clamp(-0.55, 0.55);
    let sway = if grounded {
        0.07 * (beat_pos * TAU).sin()
    } else {
        0.0
    };
    lag + sway
}

/// Idle bob on the beat for a standing hero, metres.
pub fn idle_bob(beat_pos: f32) -> f32 {
    0.05 * (0.5 - 0.5 * (beat_pos * TAU).cos())
}

/// The hero's animation state: the two springs and the speed they react to.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeroAnim {
    pub squash: Wobble,
    pub ear: Wobble,
    prev_y: f32,
    /// Vertical speed (m/s), smoothed over a few frames so a one-frame jump does not snap the ears.
    pub vy: f32,
    primed: bool,
}

impl HeroAnim {
    /// Once per sim step. `y` is the hero's height, `grounded` standing on the slab.
    pub fn update(&mut self, dt: f32, y: f32, grounded: bool, beat_pos: f32, beat_flash: f32) {
        if dt <= 0.0 {
            return;
        }
        if !self.primed {
            self.prev_y = y;
            self.primed = true;
        }
        let raw = (y - self.prev_y) / dt;
        self.prev_y = y;
        // A step of more than 8 m/s is a teleport (a revive, a restart), not motion.
        let raw = if raw.abs() > 14.0 { 0.0 } else { raw };
        self.vy += (raw - self.vy) * (1.0 - (-dt * 25.0).exp());
        self.squash.step(
            squash_target(grounded, self.vy, beat_flash),
            SQUASH_OMEGA,
            SQUASH_ZETA,
            dt,
        );
        self.ear.step(
            ear_target(self.vy, beat_pos, grounded),
            EAR_OMEGA,
            EAR_ZETA,
            dt,
        );
    }

    /// Feet met the slab. `strength` 0..1.
    pub fn on_land(&mut self, strength: f32) {
        self.squash.x = self.squash.x.max(0.9 * strength);
        self.squash.v = 0.0;
        self.ear.v -= 7.0 * strength;
    }

    /// The press: a short crouch that the spring throws into the stretch.
    pub fn on_takeoff(&mut self) {
        self.squash.x = self.squash.x.max(0.35);
        self.squash.v = -14.0;
        self.ear.v += 5.0;
    }

    /// Landed on an enemy: a hard squash.
    pub fn on_stomp(&mut self) {
        self.squash.x = 1.0;
        self.squash.v = 0.0;
        self.ear.v -= 9.0;
    }
}

/// What an enemy does this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyPose {
    pub scale: [f32; 3],
    pub roll: f32,
    pub yaw: f32,
    pub dy: f32,
    /// Extra glow (0..1), for a hit or a stomp.
    pub flash: f32,
}

impl Default for EnemyPose {
    fn default() -> Self {
        Self {
            scale: [1.0; 3],
            roll: 0.0,
            yaw: 0.0,
            dy: 0.0,
            flash: 0.0,
        }
    }
}

/// Idle / walk / fly for one enemy look, from the beat fraction `u` (0..1,
/// 0 = on the beat) and `pulse` (1 on the beat, decaying).
pub fn enemy_pose(look: &str, u: f32, pulse: f32, step_phase: f32) -> EnemyPose {
    let mut p = EnemyPose::default();
    match look {
        "slime" => {
            // Anticipation: a slow stretch up in the last fifth of the beat, then the squish on it.
            let ant = ((u - 0.8) / 0.2).clamp(0.0, 1.0);
            let sy = 1.0 - 0.16 * pulse + 0.07 * ant;
            let sx = 1.0 + 0.10 * pulse - 0.04 * ant;
            p.scale = [sx, sy, sx];
        }
        "beetle" => {
            // A rocking gait: the shell rolls with each step and the head nods.
            let w = step_phase * TAU;
            p.roll = 0.10 * w.sin();
            p.yaw = 0.06 * (w * 0.5).sin();
            p.scale = [1.0, 1.0 - 0.05 * pulse, 1.0];
        }
        _ => {
            // Wings: two beats of flap per beat of music, wide on the down stroke.
            let f = (u * 2.0 * TAU).sin();
            p.scale = [1.0 + 0.14 * f, 1.0 - 0.10 * f, 1.0];
            p.roll = 0.18 * (u * TAU).sin();
            p.dy = 0.05 * f;
        }
    }
    p
}

/// The stomped enemy: squashes flat in 40 ms with a pop outward, holds, then
/// shrinks away. `age` seconds since the stomp, `life` its length.
pub fn stomp_scale(age: f32, life: f32) -> [f32; 3] {
    let a = age.max(0.0);
    let squash = (a / 0.04).clamp(0.0, 1.0);
    let sy = 1.0 - 0.82 * squash;
    let wide = 1.0 + 0.55 * squash;
    let u = (a / life.max(1e-3)).clamp(0.0, 1.0);
    let gone = ((u - 0.6) / 0.4).clamp(0.0, 1.0);
    let k = 1.0 - gone * gone;
    [wide * k, sy * k.max(0.05), wide * k]
}

/// The enemy that hurt the hero: a puff outward and a flash that settle in
/// 0.3 s. `age` seconds since the hit.
pub fn hit_puff(age: f32) -> EnemyPose {
    let u = (age / 0.3).clamp(0.0, 1.0);
    let k = 1.0 + 0.28 * (1.0 - u) * (u * 9.0).cos().max(0.0);
    EnemyPose {
        scale: [k, k, k],
        flash: 1.0 - u,
        ..EnemyPose::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(w: &mut Wobble, target: f32, omega: f32, zeta: f32, secs: f32) -> (f32, f32) {
        let (mut lo, mut hi) = (w.x, w.x);
        let mut t = 0.0;
        while t < secs {
            w.step(target, omega, zeta, 1.0 / 60.0);
            lo = lo.min(w.x);
            hi = hi.max(w.x);
            t += 1.0 / 60.0;
        }
        (lo, hi)
    }

    #[test]
    fn the_squash_spring_overshoots_once_then_settles() {
        let mut w = Wobble { x: 0.9, v: 0.0 };
        let (lo, _) = settle(&mut w, 0.0, SQUASH_OMEGA, SQUASH_ZETA, 1.0);
        assert!(lo < -0.05, "no rebound stretch: {lo}");
        assert!(w.x.abs() < 0.01 && w.v.abs() < 0.05, "{w:?}");
    }

    #[test]
    fn the_spring_is_stable_at_a_slow_frame() {
        let mut w = Wobble { x: 1.0, v: 0.0 };
        for _ in 0..120 {
            w.step(0.0, EAR_OMEGA, EAR_ZETA, 1.0 / 20.0);
            assert!(w.x.is_finite() && w.x.abs() < 2.0);
        }
        assert!(w.x.abs() < 0.05);
    }

    #[test]
    fn the_air_stretches_the_ground_breathes() {
        assert!(squash_target(false, 6.0, 0.0) < -0.25);
        assert!(squash_target(false, -6.0, 0.0) > -0.2 && squash_target(false, -6.0, 0.0) < 0.0);
        assert!(squash_target(false, 0.0, 0.0).abs() < 1e-6);
        assert!(squash_target(true, 0.0, 1.0) > 0.0);
        let s = body_scale(0.8);
        assert!(s[1] < 0.8 && s[0] > 1.15);
        let s = body_scale(-0.3);
        assert!(s[1] > 1.08 && s[0] < 1.0);
        assert_eq!(body_scale(9.0), body_scale(1.0));
    }

    #[test]
    fn the_ears_trail_the_motion_and_sway_on_the_beat_only_on_the_ground() {
        assert!(ear_target(6.0, 0.0, false) > 0.3);
        assert!(ear_target(-6.0, 0.0, false) < -0.3);
        assert_eq!(ear_target(0.0, 0.25, false), 0.0);
        assert!(ear_target(0.0, 0.25, true) > 0.05);
        assert!(idle_bob(0.0) < 1e-6 && idle_bob(0.5) > 0.04);
    }

    #[test]
    fn enemies_move_on_the_beat_clock() {
        // Slime: squished on the beat, stretched just before the next one.
        let on = enemy_pose("slime", 0.0, 1.0, 0.0);
        let before = enemy_pose("slime", 0.98, 0.0, 0.0);
        assert!(on.scale[1] < 0.9 && before.scale[1] > 1.04);
        // Beetle: the roll swings both ways over a step.
        let a = enemy_pose("beetle", 0.0, 0.0, 0.25).roll;
        let b = enemy_pose("beetle", 0.0, 0.0, 0.75).roll;
        assert!(a > 0.05 && b < -0.05);
        // Bat: two flaps a beat.
        let f = |u: f32| enemy_pose("bat", u, 0.0, 0.0).scale[0];
        assert!((f(0.125) - f(0.625)).abs() < 1e-4 && f(0.125) > 1.1 && f(0.375) < 0.9);
    }

    #[test]
    fn a_stomped_enemy_flattens_fast_and_vanishes() {
        assert_eq!(stomp_scale(0.0, 0.35)[1], 1.0);
        let flat = stomp_scale(0.05, 0.35);
        assert!(flat[1] < 0.2 && flat[0] > 1.4);
        let end = stomp_scale(0.35, 0.35);
        assert!(end[0] < 0.01 && end[1] < 0.01);
        for i in 0..40 {
            let s = stomp_scale(i as f32 * 0.01, 0.35);
            assert!(s.iter().all(|v| v.is_finite() && *v >= 0.0));
        }
    }

    #[test]
    fn the_hero_crouches_stretches_and_squashes_on_landing() {
        let mut a = HeroAnim::default();
        let dt = 1.0 / 60.0;
        for _ in 0..30 {
            a.update(dt, 0.0, true, 0.0, 0.0);
        }
        assert!(a.squash.x.abs() < 0.02);
        a.on_takeoff();
        assert!(a.squash.x > 0.3);
        // Rising fast: the spring ends up stretched.
        let mut y = 0.0;
        let mut min = 0.0f32;
        for _ in 0..12 {
            y += 5.0 * dt;
            a.update(dt, y, false, 0.1, 0.0);
            min = min.min(a.squash.x);
        }
        assert!(min < -0.15, "no takeoff stretch: {min}");
        // The ears trail: pitched back while rising, forward falling.
        assert!(a.ear.x > 0.05);
        a.on_land(1.0);
        assert!(a.squash.x >= 0.9);
        let mut rebound = 0.0f32;
        for _ in 0..40 {
            a.update(dt, 0.0, true, 0.0, 0.0);
            rebound = rebound.min(a.squash.x);
        }
        assert!(rebound < -0.03, "no rebound: {rebound}");
        assert!(a.squash.x.abs() < 0.03);
    }

    #[test]
    fn a_teleport_does_not_fling_the_ears() {
        let mut a = HeroAnim::default();
        a.update(1.0 / 60.0, 0.0, true, 0.0, 0.0);
        a.update(1.0 / 60.0, 3.0, false, 0.0, 0.0);
        assert!(a.vy.abs() < 0.5 && a.ear.x.abs() < 0.1);
    }

    #[test]
    fn a_hit_puffs_then_settles() {
        let p = hit_puff(0.0);
        assert!(p.scale[0] > 1.2 && p.flash > 0.9);
        let q = hit_puff(0.3);
        assert_eq!(q.scale, [1.0; 3]);
        assert_eq!(q.flash, 0.0);
    }
}
