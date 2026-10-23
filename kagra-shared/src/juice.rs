//! Shake, hit-stop, squash, and scalar tweens. No renderer and no genre.
//!
//! Easing is [`simple_easing`] (MIT OR Apache-2.0, Thomas Heck). Names match
//! `kagra.juice.ease` (`out_cubic`, `in_quad`, `out_back`, …). An unknown
//! name is smoothstep. [`Juice::tick`] returns the dt the sim should
//! consume. Shake, squash, and tweens keep moving while hit-stop holds
//! that clock.

/// Named curve. `t` is clamped to 0..1. Back and elastic may leave that range.
pub fn ease(name: &str, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match name {
        "linear" => simple_easing::linear(t),
        "in_quad" => simple_easing::quad_in(t),
        "out_quad" => simple_easing::quad_out(t),
        "in_out_quad" => simple_easing::quad_in_out(t),
        "in_cubic" => simple_easing::cubic_in(t),
        "out_cubic" => simple_easing::cubic_out(t),
        "in_out_cubic" => simple_easing::cubic_in_out(t),
        "out_back" => simple_easing::back_out(t),
        "out_bounce" => simple_easing::bounce_out(t),
        "out_elastic" => simple_easing::elastic_out(t),
        _ => t * t * (3.0 - 2.0 * t),
    }
}

/// One float, `duration` seconds, named ease.
#[derive(Clone, Debug)]
pub struct ScalarTween {
    from: f32,
    to: f32,
    duration: f32,
    ease: String,
    elapsed: f32,
    done: bool,
}

impl ScalarTween {
    pub fn value(&self) -> f32 {
        if self.done {
            return self.to;
        }
        let u = ease(&self.ease, self.elapsed / self.duration);
        self.from + (self.to - self.from) * u
    }

    pub fn done(&self) -> bool {
        self.done
    }

    fn step(&mut self, dt: f32) -> bool {
        if self.done {
            return true;
        }
        self.elapsed += dt;
        if self.elapsed + 1e-4 >= self.duration {
            self.elapsed = self.duration;
            self.done = true;
        }
        self.done
    }
}

/// Feel clocks for one scene. `enabled` false makes [`Juice::tick`] a pass-through.
#[derive(Clone, Debug)]
pub struct Juice {
    pub enabled: bool,
    scale: [f32; 3],
    pulse: Option<Pulse>,
    shake_t: f32,
    shake_dur: f32,
    shake_amp: f32,
    shake_freq: f32,
    hit: f32,
    tweens: Vec<ScalarTween>,
}

#[derive(Clone, Debug)]
struct Pulse {
    amount: f32,
    duration: f32,
    t: f32,
}

impl Default for Juice {
    fn default() -> Self {
        Self {
            enabled: true,
            scale: [1.0, 1.0, 1.0],
            pulse: None,
            shake_t: 0.0,
            shake_dur: 0.0,
            shake_amp: 0.0,
            shake_freq: 28.0,
            hit: 0.0,
            tweens: Vec::new(),
        }
    }
}

impl Juice {
    /// Camera offset in meters. The envelope is the square of the time left.
    pub fn shake(&mut self, amplitude: f32, duration: f32, frequency: f32) {
        self.shake_amp = amplitude.max(0.0);
        self.shake_dur = duration.max(1e-4);
        self.shake_freq = frequency.max(0.1);
        self.shake_t = 0.0;
    }

    /// Hold the sim clock. Feel clocks keep running.
    pub fn hitstop(&mut self, seconds: f32) {
        self.hit = self.hit.max(seconds.max(0.0));
    }

    pub fn hit_remaining(&self) -> f32 {
        self.hit
    }

    /// Dip Y, widen XZ, then settle. `amount` 0.22 is about 0.78 tall.
    pub fn squash(&mut self, amount: f32, duration: f32) {
        self.pulse = Some(Pulse {
            amount: -amount.clamp(0.0, 0.65),
            duration: duration.max(1e-3),
            t: 0.0,
        });
    }

    /// Stretch Y, narrow XZ, then settle.
    pub fn stretch(&mut self, amount: f32, duration: f32) {
        self.pulse = Some(Pulse {
            amount: amount.clamp(0.0, 0.65),
            duration: duration.max(1e-3),
            t: 0.0,
        });
    }

    /// Animate a float. The returned index stays valid after the tween ends.
    pub fn tween(&mut self, from: f32, to: f32, duration: f32, ease_name: &str) -> usize {
        let id = self.tweens.len();
        self.tweens.push(ScalarTween {
            from,
            to,
            duration: duration.max(1e-4),
            ease: ease_name.to_string(),
            elapsed: 0.0,
            done: false,
        });
        id
    }

    pub fn value(&self, index: usize) -> Option<f32> {
        self.tweens.get(index).map(ScalarTween::value)
    }

    pub fn scale(&self) -> [f32; 3] {
        self.scale
    }

    pub fn camera_offset(&self) -> [f32; 3] {
        if !self.enabled || self.shake_dur <= 1e-6 || self.shake_t >= self.shake_dur {
            return [0.0, 0.0, 0.0];
        }
        let u = 1.0 - (self.shake_t / self.shake_dur);
        let amp = self.shake_amp * u * u;
        let phase = self.shake_t * self.shake_freq;
        [
            (phase * 17.0).sin() * amp,
            (phase * 23.0).sin() * amp * 0.55,
            (phase * 11.0).sin() * amp * 0.25,
        ]
    }

    /// Advance feel clocks. Returns the dt the sim should consume.
    pub fn tick(&mut self, dt: f32) -> f32 {
        let dt = dt.max(0.0);
        if !self.enabled {
            return dt;
        }
        self.advance(dt);
        if self.hit > 0.0 {
            if dt < self.hit {
                self.hit -= dt;
                return 0.0;
            }
            let rest = dt - self.hit;
            self.hit = 0.0;
            return rest;
        }
        dt
    }

    fn advance(&mut self, dt: f32) {
        if self.shake_dur > 0.0 && self.shake_t < self.shake_dur {
            self.shake_t += dt;
        }
        if let Some(pulse) = self.pulse.as_mut() {
            pulse.t += dt;
            let u = if pulse.duration <= 1e-6 {
                1.0
            } else {
                (pulse.t / pulse.duration).min(1.0)
            };
            let k = if u < 0.35 {
                let k = u / 0.35;
                k * k
            } else {
                (1.0 - (u - 0.35) / 0.65).max(0.0)
            };
            let sy = (1.0 + pulse.amount * k).max(0.35);
            let sx = 1.0 / sy.sqrt();
            self.scale = [sx, sy, sx];
            if u >= 1.0 {
                self.scale = [1.0, 1.0, 1.0];
                self.pulse = None;
            }
        }
        for tw in &mut self.tweens {
            tw.step(dt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_match_simple_easing_and_clamp() {
        assert!((ease("linear", 0.25) - 0.25).abs() < 1e-6);
        assert!((ease("out_cubic", 0.0)).abs() < 1e-6);
        assert!((ease("out_cubic", 1.0) - 1.0).abs() < 1e-6);
        assert!(ease("out_cubic", 0.5) > 0.5);
        assert!((ease("out_cubic", 0.4) - simple_easing::cubic_out(0.4)).abs() < 1e-6);
        assert!((ease("in_quad", 0.3) - simple_easing::quad_in(0.3)).abs() < 1e-6);
        assert!((ease("out_back", 0.7) - simple_easing::back_out(0.7)).abs() < 1e-5);
        assert!((ease("nope", 0.0)).abs() < 1e-6);
        assert!((ease("out_cubic", 2.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hitstop_freezes_the_sim_clock_and_shake_keeps_moving() {
        let mut juice = Juice::default();
        juice.hitstop(0.05);
        juice.shake(0.2, 0.3, 28.0);
        assert!(juice.tick(0.02).abs() < 1e-6);
        assert!(juice.hit_remaining() > 0.02);
        let offset = juice.camera_offset();
        assert!(offset[0].abs() + offset[1].abs() > 0.0);
        let rest = juice.tick(0.08);
        assert!((rest - 0.05).abs() < 1e-5, "{rest}");
        assert!(juice.hit_remaining().abs() < 1e-6);
    }

    #[test]
    fn squash_then_settles_and_tween_uses_the_named_curve() {
        let mut juice = Juice::default();
        juice.squash(0.22, 0.18);
        juice.tick(0.02);
        let sy = juice.scale()[1];
        assert!(sy < 1.0, "{sy}");
        assert!(juice.scale()[0] > 1.0);
        for _ in 0..20 {
            juice.tick(0.02);
        }
        assert!((juice.scale()[1] - 1.0).abs() < 1e-4);
        let id = juice.tween(0.0, 10.0, 0.2, "linear");
        juice.tick(0.1);
        let mid = juice.value(id).unwrap();
        assert!((mid - 5.0).abs() < 0.05, "{mid}");
        juice.tick(0.2);
        assert!((juice.value(id).unwrap() - 10.0).abs() < 1e-3);
    }
}
