//! Juice: springs, shake, hitstop, sparks. Deterministic (seeded LCG).

/// Critically damped scalar. `omega` is radians per second.
#[derive(Clone, Copy, Debug, Default)]
pub struct Spring1 {
    pub pos: f32,
    pub vel: f32,
}

impl Spring1 {
    pub fn snap(&mut self, target: f32) {
        self.pos = target;
        self.vel = 0.0;
    }

    pub fn step(&mut self, target: f32, omega: f32, dt: f32) {
        let omega = omega.max(0.01);
        let exp = (-omega * dt).exp();
        let change = self.pos - target;
        let temp = (self.vel + omega * change) * dt;
        self.pos = target + (change + temp) * exp;
        self.vel = (self.vel - omega * temp) * exp;
    }
}

/// Same clock as `kagra.juice.Juice.camera_offset`. Amplitude decays with
/// the square of the time still left. Three sines, meters.
#[derive(Clone, Copy, Debug)]
pub struct JuiceShake {
    t: f32,
    dur: f32,
    amp: f32,
    freq: f32,
}

impl Default for JuiceShake {
    fn default() -> Self {
        Self {
            t: 0.0,
            dur: 0.0,
            amp: 0.0,
            freq: 28.0,
        }
    }
}

impl JuiceShake {
    /// `kagra.juice.Juice.shake` defaults are 0.08 m and 0.18 s at 28 Hz.
    pub fn shake(&mut self, amplitude: f32, duration: f32) {
        self.amp = amplitude.max(0.0);
        self.dur = duration.max(1e-4);
        self.freq = 28.0;
        self.t = 0.0;
    }

    pub fn tick(&mut self, dt: f32) {
        if self.dur > 0.0 && self.t < self.dur {
            self.t += dt;
        }
    }

    pub fn offset(&self) -> [f32; 3] {
        if self.dur <= 1e-6 || self.t >= self.dur {
            return [0.0, 0.0, 0.0];
        }
        let u = 1.0 - (self.t / self.dur);
        let amp = self.amp * u * u;
        let phase = self.t * self.freq;
        [
            (phase * 17.0).sin() * amp,
            (phase * 23.0).sin() * amp * 0.55,
            (phase * 11.0).sin() * amp * 0.25,
        ]
    }
}

#[derive(Clone, Debug)]
pub struct Spark {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub vx: f32,
    pub vy: f32,
    pub vz: f32,
    pub life: f32,
    pub max_life: f32,
    pub size: f32,
    pub color: [u32; 3],
}

/// Shake (decaying scalar), hitstop, landing bump, and spark pool.
#[derive(Debug, Default)]
pub struct Juice {
    pub shake: f32,
    pub hitstop: f32,
    pub land_squash: f32,
    /// Landing bump. Same offset as `Juice.camera_offset`.
    pub land: JuiceShake,
    pub sparks: Vec<Spark>,
    pub seed: u32,
}

/// Spark pool cap (phones).
pub const MAX_SPARKS: usize = 36;

impl Juice {
    pub fn seeded(seed: u32) -> Self {
        Self {
            seed,
            ..Self::default()
        }
    }

    pub fn rng(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.seed >> 8) & 0x00ff_ffff) as f32 / 16_777_216.0
    }

    /// Hit feel: big shake plus a short freeze.
    pub fn hit(&mut self, shake: f32, hitstop: f32) {
        self.shake = shake;
        self.hitstop = hitstop;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn burst(
        &mut self,
        x: f32,
        y: f32,
        z: f32,
        n: u32,
        color: [u32; 3],
        speed: f32,
        size: f32,
    ) {
        for _ in 0..n {
            if self.sparks.len() >= MAX_SPARKS {
                self.sparks.remove(0);
            }
            let ang = self.rng() * std::f32::consts::TAU;
            let rad = self.rng();
            let spd = speed * (0.4 + self.rng());
            let vy = 1.1 + self.rng() * speed * 0.4;
            let max_life = 0.28 + self.rng() * 0.28;
            let spark_size = size * (0.7 + self.rng() * 0.6);
            self.sparks.push(Spark {
                x,
                y,
                z,
                vx: ang.cos() * spd * rad,
                vy,
                vz: ang.sin() * spd * rad - 1.5,
                life: 0.0,
                max_life,
                size: spark_size,
                color,
            });
        }
    }

    /// Integrate sparks; drop expired ones and any `blocked` reports solid.
    pub fn tick_sparks(&mut self, dt: f32, blocked: impl Fn(f32, f32, f32) -> bool) {
        let mut i = 0;
        while i < self.sparks.len() {
            let s = &mut self.sparks[i];
            s.life += dt;
            if s.life >= s.max_life {
                self.sparks.remove(i);
                continue;
            }
            s.vy -= 9.0 * dt;
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            s.z += s.vz * dt;
            // Settle on the floor instead of sinking through it.
            let floor = s.size * 0.5;
            if s.y < floor {
                s.y = floor;
                s.vy = -s.vy * 0.25;
                s.vx *= 0.6;
                s.vz *= 0.6;
            }
            if blocked(s.x, s.y, s.z) {
                self.sparks.remove(i);
                continue;
            }
            i += 1;
        }
    }

    /// Per-frame decay of the shake scalar and landing bump.
    pub fn decay(&mut self, dt: f32) {
        self.shake = (self.shake - dt * 0.85).max(0.0);
        self.land.tick(dt);
    }
}
