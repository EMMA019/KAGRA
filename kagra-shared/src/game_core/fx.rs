//! Shared particle emitters: a declarative spec, a pooled simulation and a
//! doc writer that renders through textured point clouds ([`crate::strokes`]).
//!
//! The sim is the one `survivor_arena` tuned in production — fan bursts,
//! per-kind drag/gravity, a floor bounce, pool caps per quality tier — lifted
//! here so any kit declares its effects once in JSON and bursts them by kind
//! index. A kind either stays an untextured soft disc (the historical look)
//! or binds an atlas frame range: flipbook playback is a pure function of
//! particle age, computed on the CPU while the doc is written, so the
//! renderer stays dumb.
//!
//! First-occurrence rule: the pool's clouds name their atlas from the moment
//! they are declared ([`FxPool::declare_slots`], at doc build), so the
//! renderer uploads every atlas on the kit's first frame — never at the
//! first burst. There is no API to introduce an atlas later: the spec is
//! fixed when the pool is built.

use serde::{Deserialize, Serialize};

use crate::game_core::swarm::Rng;
use crate::strokes::{PointBlend, PointCloud};
use crate::world_doc::WorldDoc;

/// Burst-count scale per quality tier (the survivor arena's tuning).
const TIER_COUNT: [f32; 3] = [0.5, 1.0, 1.5];

/// One flipbook atlas: a texture spec plus its frame grid. Frame indices run
/// left-to-right, top-to-bottom, the way sprite sheets ship.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AtlasSpec {
    /// PNG path or `image2d:<id>` — the prop `texture` language, resolved by
    /// [`crate::world_doc::load_png_albedo`].
    pub texture: String,
    pub cols: u32,
    pub rows: u32,
}

impl Default for AtlasSpec {
    fn default() -> Self {
        Self {
            texture: String::new(),
            cols: 1,
            rows: 1,
        }
    }
}

impl AtlasSpec {
    /// UV rect `(u0, v0, u1, v1)` for `frame`, `v0` at the TOP row of the
    /// sheet. Out-of-range frames clamp into the grid.
    pub fn frame_rect(&self, frame: u32) -> [f32; 4] {
        let cols = self.cols.max(1);
        let rows = self.rows.max(1);
        let f = frame.min(cols * rows - 1);
        let (col, row) = (f % cols, f / cols);
        [
            col as f32 / cols as f32,
            row as f32 / rows as f32,
            (col + 1) as f32 / cols as f32,
            (row + 1) as f32 / rows as f32,
        ]
    }
}

/// One particle kind: how it flies and how it looks in each third of its
/// life (young, middle, old — the survivor arena's `FX_LOOK` shape).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KindSpec {
    pub name: String,
    /// Air drag: velocity is scaled by `(1 - drag * dt)` each step.
    pub drag: f32,
    /// Downward pull in m/s^2. Negative floats the particle up.
    pub gravity: f32,
    /// Floor height (m) the particle bounces on.
    pub floor: f32,
    /// Share of vertical velocity kept, sign flipped, on a floor bounce.
    pub bounce: f32,
    /// sRGB colour, shared by all three thirds.
    pub color: [u8; 3],
    /// Billboard radius (m) in the young, middle and old third of life.
    pub sizes: [f32; 3],
    /// Opacity in the young, middle and old third of life.
    pub alphas: [u8; 3],
    /// Index into [`FxSpec::atlases`], or None for a soft disc.
    pub atlas: Option<usize>,
    /// First frame of this kind's flipbook range.
    pub frame0: u32,
    /// Frames in the range. 1 (or 0) is a static frame.
    pub frames: u32,
    /// Loop the range instead of clamping on the last frame. The loop period
    /// is the particle's life unless `fps` is set.
    pub looped: bool,
    /// Frames per second for a looped range. 0 derives it from the life
    /// (`frames / life`), so the sheet loops once per particle.
    pub fps: f32,
    /// Max |rotation drift| in radians/s; each particle draws its own signed
    /// rate at burst. 0 keeps particles unrotated (and the RNG stream short).
    pub spin: f32,
    /// Compositing for this kind's cloud.
    pub blend: PointBlend,
}

impl Default for KindSpec {
    fn default() -> Self {
        Self {
            name: String::new(),
            drag: 0.0,
            gravity: 0.0,
            floor: 0.04,
            bounce: 0.25,
            color: [255, 255, 255],
            sizes: [0.1, 0.1, 0.1],
            alphas: [255, 255, 255],
            atlas: None,
            frame0: 0,
            frames: 1,
            looped: false,
            fps: 0.0,
            spin: 0.0,
            blend: PointBlend::Additive,
        }
    }
}

/// The declarative effect set of a kit: atlases, kinds and the pool cap per
/// quality tier (low, mid, high) — the survivor arena's `FxSpec.cap` shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FxSpec {
    pub atlases: Vec<AtlasSpec>,
    pub kinds: Vec<KindSpec>,
    pub cap: [u32; 3],
}

impl Default for FxSpec {
    fn default() -> Self {
        Self {
            atlases: Vec::new(),
            kinds: Vec::new(),
            cap: [48, 160, 480],
        }
    }
}

/// One live particle. Fields are public: kits that draw their own clouds
/// (the survivor arena's bucketed doc) read them directly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub vx: f32,
    pub vy: f32,
    pub vz: f32,
    pub age: f32,
    pub life: f32,
    pub kind: u16,
    /// On-screen rotation (radians) and its per-second drift.
    pub rot: f32,
    pub spin: f32,
}

/// A pooled particle simulation over an [`FxSpec`].
#[derive(Clone, Debug)]
pub struct FxPool {
    spec: FxSpec,
    rng: Rng,
    particles: Vec<Particle>,
    cap: usize,
    count_scale: f32,
    scale: f32,
    size_scale: f32,
}

impl FxPool {
    /// Build a pool on the mid quality tier. The spec is fixed for the
    /// pool's life: atlases cannot appear later (first-occurrence rule).
    pub fn new(spec: FxSpec, seed: u32) -> Self {
        let mut pool = Self {
            cap: spec.cap[1] as usize,
            spec,
            rng: Rng::new(seed),
            particles: Vec::new(),
            count_scale: TIER_COUNT[1],
            scale: 1.0,
            size_scale: 1.0,
        };
        pool.particles = Vec::with_capacity(pool.cap);
        pool
    }

    pub fn spec(&self) -> &FxSpec {
        &self.spec
    }

    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    pub fn len(&self) -> usize {
        self.particles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.particles.is_empty()
    }

    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Pick the pool cap and burst-count scale of quality `tier` (0, 1, 2).
    pub fn set_tier(&mut self, tier: usize) {
        let tier = tier.min(2);
        self.cap = self.spec.cap[tier] as usize;
        self.count_scale = TIER_COUNT[tier];
    }

    /// Extra burst-count multiplier (the `quality_hints` particle share).
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.max(0.0);
    }

    /// Sprite-size multiplier applied while the doc is written
    /// ([`FxPool::refill`]): 1.0 keeps the authored sizes. A kit on a
    /// lower graphics tier trims it to shave the peak overdraw of its
    /// biggest flipbooks — a blast sprite's fill cost grows with its
    /// area — without touching burst counts or the sim. Pools that
    /// never call it (and kits that write their own clouds) are
    /// unaffected.
    pub fn set_size_scale(&mut self, scale: f32) {
        self.size_scale = scale.max(0.0);
    }

    /// Drop every particle and reseed (a run restart).
    pub fn reset(&mut self, seed: u32) {
        self.rng = Rng::new(seed);
        self.particles.clear();
    }

    pub fn clear(&mut self) {
        self.particles.clear();
    }

    /// A burst of `n` particles of `kind` (scaled by tier and quality hint,
    /// rounded up), thrown from `at` along (`dir[0]`, `dir[1]`) with a fan
    /// spread and an upward `up`. Dropped silently when the pool is full.
    #[allow(clippy::too_many_arguments)]
    pub fn burst(
        &mut self,
        kind: usize,
        n: u32,
        at: [f32; 3],
        dir: [f32; 2],
        speed: f32,
        fan: f32,
        up: f32,
        life: f32,
    ) {
        let Some(ks) = self.spec.kinds.get(kind) else {
            return;
        };
        let spin_max = ks.spin;
        let n = (n as f32 * self.count_scale * self.scale).ceil() as u32;
        for _ in 0..n {
            if self.particles.len() >= self.cap {
                return;
            }
            let a = self.rng.range(0.0, std::f32::consts::TAU);
            let sp = speed * self.rng.range(0.4, 1.0);
            let vy = up * self.rng.range(0.5, 1.0);
            let life = life * self.rng.range(0.7, 1.0);
            let spin = if spin_max != 0.0 {
                self.rng.range(-spin_max, spin_max)
            } else {
                0.0
            };
            self.particles.push(Particle {
                x: at[0],
                y: at[1],
                z: at[2],
                vx: dir[0] * speed * 0.7 + a.cos() * sp * fan,
                vy,
                vz: dir[1] * speed * 0.7 + a.sin() * sp * fan,
                age: 0.0,
                life,
                kind: kind as u16,
                rot: 0.0,
                spin,
            });
        }
    }

    /// Advance every particle: age, per-kind drag/gravity, floor bounce,
    /// rotation drift. Dead particles are swap-removed.
    pub fn step(&mut self, dt: f32) {
        for i in (0..self.particles.len()).rev() {
            let kind = self.particles[i].kind as usize;
            let (drag, grav, floor, bounce) = self
                .spec
                .kinds
                .get(kind)
                .map(|k| (k.drag, k.gravity, k.floor, k.bounce))
                .unwrap_or((0.0, 0.0, 0.04, 0.25));
            let p = &mut self.particles[i];
            p.age += dt;
            if p.age >= p.life {
                self.particles.swap_remove(i);
                continue;
            }
            let k = (1.0 - drag * dt).max(0.0);
            p.vx *= k;
            p.vz *= k;
            p.vy = p.vy * k - grav * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.z += p.vz * dt;
            if p.y < floor {
                p.y = floor;
                p.vy = -p.vy * bounce;
            }
            p.rot += p.spin * dt;
        }
    }

    /// The flipbook frame a particle of this kind shows at `age` / `life`.
    pub fn frame_of(kind: &KindSpec, age: f32, life: f32) -> u32 {
        if kind.frames <= 1 {
            return kind.frame0;
        }
        if kind.looped {
            let fps = if kind.fps > 0.0 {
                kind.fps
            } else if life > 0.0 {
                kind.frames as f32 / life
            } else {
                0.0
            };
            kind.frame0 + ((age.max(0.0) * fps) as u32) % kind.frames
        } else {
            let t = if life > 0.0 { age / life } else { 0.0 };
            kind.frame0 + ((t.max(0.0) * kind.frames as f32) as u32).min(kind.frames - 1)
        }
    }

    /// Which third of its life a particle is in (0 young, 1 middle, 2 old).
    fn third(p: &Particle) -> usize {
        ((p.age / p.life * 3.0) as usize).min(2)
    }

    /// Number of point clouds the pool owns in a doc: one per kind. Every
    /// cloud names its atlas from declaration, so no separate declaration
    /// clouds are needed.
    pub fn slot_count(&self) -> usize {
        self.spec.kinds.len()
    }

    /// Push the pool's (empty) clouds onto `doc` and return the first index.
    /// Call once when the doc is built: the clouds carry their atlas specs
    /// from this moment, which is what uploads the atlases on the kit's
    /// first frame — before any burst can happen.
    pub fn declare_slots(&self, doc: &mut WorldDoc) -> usize {
        let base = doc.points.len();
        for (i, kind) in self.spec.kinds.iter().enumerate() {
            doc.points.push(PointCloud {
                id: format!("fx-{i}-{}", kind.name),
                atlas: kind
                    .atlas
                    .and_then(|a| self.spec.atlases.get(a))
                    .map(|a| a.texture.clone()),
                blend: kind.blend,
                ..PointCloud::default()
            });
        }
        base
    }

    /// Refill the clouds [`FxPool::declare_slots`] pushed at `base` from the
    /// live particles: positions plus per-point tint, radius, flipbook frame
    /// and rotation.
    pub fn refill(&self, doc: &mut WorldDoc, base: usize) {
        for i in 0..self.spec.kinds.len() {
            let Some(cloud) = doc.points.get_mut(base + i) else {
                return;
            };
            cloud.positions.clear();
            cloud.tints.clear();
            cloud.sizes.clear();
            cloud.frames.clear();
            cloud.rotations.clear();
        }
        for p in &self.particles {
            let kind = &self.spec.kinds[p.kind as usize];
            let Some(cloud) = doc.points.get_mut(base + p.kind as usize) else {
                continue;
            };
            let b = Self::third(p);
            cloud.positions.push([p.x, p.y, p.z]);
            cloud
                .tints
                .push([kind.color[0], kind.color[1], kind.color[2], kind.alphas[b]]);
            cloud.sizes.push(kind.sizes[b] * self.size_scale);
            if let Some(atlas) = kind.atlas.and_then(|a| self.spec.atlases.get(a)) {
                cloud
                    .frames
                    .push(atlas.frame_rect(Self::frame_of(kind, p.age, p.life)));
                cloud.rotations.push(p.rot);
            }
        }
    }

    /// Declare and refill in one call, for kits that rebuild their doc
    /// wholesale each frame.
    pub fn write_doc(&self, doc: &mut WorldDoc) -> usize {
        let base = self.declare_slots(doc);
        self.refill(doc, base);
        base
    }
}

// ---------------------------------------------------------------------
// Atmosphere parts: continuous ambient emitters + lamp flicker.
//
// Added for city_action's eerie-streets expansion (2026-11-18) as
// shared parts, not kit one-offs: a kit declares an [`FxSpec`] for
// its ambient kinds, a list of [`AmbientEmitter`]s that keep those
// kinds populated, and a [`FlickerSpec`] for its light halos. Both
// halves are data + deterministic stepping (seeded RNG, pure
// functions of time), so replays and tests see the same streets.

/// One declared continuous emitter: it keeps a pool kind populated
/// over an area — ground mist pooling on a street, ash motes over a
/// field, embers above a forge. The emitter owns no particles; it
/// bursts into an [`FxPool`] on a timer, so the pool's caps and
/// quality scales bound it like any other effect.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AmbientEmitter {
    /// Index of the kind in the pool's [`FxSpec::kinds`].
    pub kind: usize,
    /// Spawn area: centre x/z and half extents x/z (world metres).
    pub area: [f32; 4],
    /// Spawn height range (world metres).
    pub height: [f32; 2],
    /// Mean seconds between batches (jittered ±30% per batch).
    pub every: f32,
    /// Particles per batch. Batches, not single spawns, so the
    /// pool's count scale — which rounds — still lands particles at
    /// the lowest quality share: a batch of 4 at scale 0.3 keeps 1.
    pub batch: u32,
    /// Drift direction (x/z, need not be unit): the burst direction.
    pub drift: [f32; 2],
    /// Drift speed (m/s) handed to the burst.
    pub speed: f32,
    /// Upward speed (m/s) handed to the burst (0 = stays level).
    pub up: f32,
    /// Burst life (s); the pool's own 0.7..=1.0 jitter applies.
    pub life: f32,
}

/// The runtime half of a declared emitter set: per-emitter timers
/// and the seeded RNG the spawn points draw from. Deterministic for
/// a given seed and step sequence.
#[derive(Clone, Debug)]
pub struct AmbientEmitters {
    specs: Vec<AmbientEmitter>,
    timers: Vec<f32>,
    rng: Rng,
}

impl AmbientEmitters {
    pub fn new(specs: Vec<AmbientEmitter>, seed: u32) -> Self {
        let mut emitters = Self {
            specs,
            timers: Vec::new(),
            rng: Rng::new(seed),
        };
        emitters.reset(seed);
        emitters
    }

    /// Reseed and re-stagger the timers (a fresh run gets fresh
    /// streets, not the last run's leftovers).
    pub fn reset(&mut self, seed: u32) {
        self.rng = Rng::new(seed);
        self.timers = self
            .specs
            .iter()
            .map(|s| s.every.max(0.05) * self.rng.range(0.1, 1.0))
            .collect();
    }

    /// The declared set, for tests and tooling.
    pub fn specs(&self) -> &[AmbientEmitter] {
        &self.specs
    }

    /// Fire any batches that came due into `pool`.
    pub fn step(&mut self, pool: &mut FxPool, dt: f32) {
        for (i, s) in self.specs.iter().enumerate() {
            self.timers[i] -= dt;
            while self.timers[i] <= 0.0 {
                self.timers[i] += s.every.max(0.05) * self.rng.range(0.7, 1.3);
                let x = s.area[0] + self.rng.range(-s.area[2], s.area[2]);
                let z = s.area[1] + self.rng.range(-s.area[3], s.area[3]);
                let y = self.rng.range(s.height[0], s.height[1]);
                pool.burst(
                    s.kind,
                    s.batch,
                    [x, y, z],
                    s.drift,
                    s.speed,
                    0.35,
                    s.up,
                    s.life,
                );
            }
        }
    }

    /// Fill the pool to steady state: step emitters and pool in
    /// small slices for `seconds` of sim time, so the first frame
    /// already wears the full atmosphere instead of building it up
    /// live over the first seconds of a run (the first-occurrence
    /// rule: nothing first-appears mid-run).
    pub fn prime(&mut self, pool: &mut FxPool, seconds: f32) {
        let steps = (seconds / 0.25).ceil().max(0.0) as u32;
        for _ in 0..steps {
            self.step(pool, 0.25);
            pool.step(0.25);
        }
    }
}

/// A declared lamp-flicker behaviour: a gentle irregular wobble on
/// every lamp, plus a harder sputter that rotates lamp to lamp.
/// [`flicker_intensity`] is a pure function of (slot, slots, t) —
/// no state, no RNG stream — so a kit evaluates it per lamp while
/// building its frame doc and a replay draws the same flicker.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlickerSpec {
    /// Wobble depth: intensity swings within `1 - depth ..= 1`.
    pub depth: f32,
    /// Wobble rate (cycles per second, roughly).
    pub rate: f32,
    /// Sputter depth: the sputtering lamp dips toward
    /// `1 - sputter_depth` of its wobbled brightness.
    pub sputter_depth: f32,
    /// Seconds each lamp's sputter window lasts before it rotates on.
    pub sputter_secs: f32,
}

impl Default for FlickerSpec {
    fn default() -> Self {
        Self {
            depth: 0.15,
            rate: 0.9,
            sputter_depth: 0.75,
            sputter_secs: 2.2,
        }
    }
}

/// Deterministic 0..1 hash for flicker phases (splitmix-style).
fn flicker_hash(n: u32) -> f32 {
    let mut x = n ^ 0x9E37_79B9;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    x = x.wrapping_mul(0xC2B2_AE35);
    x ^= x >> 16;
    (x >> 8) as f32 / 16_777_216.0
}

/// The light intensity multiplier for lamp `slot` of `slots` at
/// time `t` under `spec`: 1.0 is full brightness; the value never
/// reaches 0 (a sputtering lamp stumbles, it does not die).
pub fn flicker_intensity(spec: &FlickerSpec, slot: usize, slots: usize, t: f32) -> f32 {
    if slots == 0 {
        return 1.0;
    }
    let ph = flicker_hash(slot as u32) * std::f32::consts::TAU;
    let w = (t * spec.rate * std::f32::consts::TAU + ph).sin() * 0.6
        + (t * spec.rate * 2.33 * std::f32::consts::TAU + ph * 1.7).sin() * 0.4;
    let mut v = 1.0 - spec.depth * (0.5 + 0.5 * w);
    let win = (t / spec.sputter_secs.max(0.05)).floor() as i64;
    let active = win.rem_euclid(slots as i64) as usize;
    if slot == active {
        // Jagged dips: a fast stepped hash decides each 1/24 s slice.
        let slice = (t * 24.0).floor() as i64;
        let j = flicker_hash(
            (win as u32).wrapping_mul(0x27D4_EB2F) ^ (slice as u32).wrapping_mul(0x85EB_CA6B),
        );
        if j < 0.55 {
            v *= 1.0 - spec.sputter_depth * (0.35 + 0.65 * (j / 0.55));
        }
    }
    v.clamp(0.05, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_kind_spec() -> FxSpec {
        FxSpec {
            atlases: vec![AtlasSpec {
                texture: "image2d:0".into(),
                cols: 4,
                rows: 2,
            }],
            kinds: vec![
                KindSpec {
                    name: "spark".into(),
                    drag: 1.6,
                    gravity: 12.0,
                    color: [185, 240, 255],
                    sizes: [0.11, 0.08, 0.05],
                    alphas: [255, 235, 190],
                    ..KindSpec::default()
                },
                KindSpec {
                    name: "flame".into(),
                    drag: 2.6,
                    gravity: 0.0,
                    color: [255, 190, 150],
                    sizes: [0.22, 0.34, 0.42],
                    alphas: [200, 90, 150],
                    atlas: Some(0),
                    frame0: 2,
                    frames: 4,
                    spin: 1.5,
                    ..KindSpec::default()
                },
            ],
            cap: [4, 8, 16],
        }
    }

    /// The pool's burst + step, replayed with the survivor arena's original
    /// formulas (copied here verbatim) over the same seeded RNG. Any drift
    /// between the lift and the original math fails this test.
    #[test]
    fn burst_and_step_match_the_survivor_formulas() {
        let mut spec = two_kind_spec();
        spec.cap = [64, 64, 64]; // headroom: this test is about the math
        let mut pool = FxPool::new(spec.clone(), 1234);
        pool.set_tier(1); // count scale 1.0: burst counts pass through
        pool.burst(0, 6, [1.0, 0.5, 2.0], [0.6, -0.8], 5.0, 1.0, 2.5, 0.35);
        pool.burst(1, 3, [0.0, 0.6, 0.0], [0.0, 0.0], 4.5, 1.0, 2.5, 0.8);

        // Reference: the arena's burst() and step_fx() bodies.
        let mut rng = Rng::new(1234);
        let (mut xs, mut ys, mut zs, mut vxs, mut vys, mut vzs, mut ages, mut lifes, mut kinds) = (
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
        );
        let mut push_burst = |rng: &mut Rng,
                              kind: u8,
                              n: u32,
                              at: [f32; 3],
                              dir: [f32; 2],
                              speed: f32,
                              fan: f32,
                              up: f32,
                              life: f32,
                              spin: f32| {
            for _ in 0..n {
                let a = rng.range(0.0, std::f32::consts::TAU);
                let sp = speed * rng.range(0.4, 1.0);
                xs.push(at[0]);
                ys.push(at[1]);
                zs.push(at[2]);
                vxs.push(dir[0] * speed * 0.7 + a.cos() * sp * fan);
                vys.push(up * rng.range(0.5, 1.0));
                vzs.push(dir[1] * speed * 0.7 + a.sin() * sp * fan);
                ages.push(0.0);
                lifes.push(life * rng.range(0.7, 1.0));
                kinds.push(kind);
                if spin != 0.0 {
                    // The pool draws one spin value per spinning particle.
                    rng.range(-spin, spin);
                }
            }
        };
        push_burst(
            &mut rng,
            0,
            6,
            [1.0, 0.5, 2.0],
            [0.6, -0.8],
            5.0,
            1.0,
            2.5,
            0.35,
            0.0,
        );
        push_burst(
            &mut rng,
            1,
            3,
            [0.0, 0.6, 0.0],
            [0.0, 0.0],
            4.5,
            1.0,
            2.5,
            0.8,
            1.5,
        );
        assert_eq!(pool.len(), xs.len());
        for (i, p) in pool.particles().iter().enumerate() {
            assert_eq!(p.kind as usize, kinds[i] as usize);
            for (got, want) in [
                (p.x, xs[i]),
                (p.y, ys[i]),
                (p.z, zs[i]),
                (p.vx, vxs[i]),
                (p.vy, vys[i]),
                (p.vz, vzs[i]),
                (p.life, lifes[i]),
            ] {
                assert!(
                    (got - want).abs() < 1e-6,
                    "burst field {i}: {got} vs {want}"
                );
            }
        }
        // Step both by the arena's step_fx() math.
        let dt = 1.0 / 60.0;
        for _ in 0..20 {
            pool.step(dt);
            let mut i = xs.len();
            while i > 0 {
                i -= 1;
                ages[i] += dt;
                if ages[i] >= lifes[i] {
                    for v in [
                        &mut xs, &mut ys, &mut zs, &mut vxs, &mut vys, &mut vzs, &mut ages,
                        &mut lifes,
                    ] {
                        v.swap_remove(i);
                    }
                    kinds.swap_remove(i);
                    continue;
                }
                let (drag, grav) = match kinds[i] {
                    0 => (1.6, 12.0),
                    _ => (2.6, 0.0),
                };
                let k = (1.0 - drag * dt).max(0.0);
                vxs[i] *= k;
                vzs[i] *= k;
                vys[i] = vys[i] * k - grav * dt;
                xs[i] += vxs[i] * dt;
                ys[i] += vys[i] * dt;
                zs[i] += vzs[i] * dt;
                if ys[i] < 0.04 {
                    ys[i] = 0.04;
                    vys[i] = -vys[i] * 0.25;
                }
            }
            assert_eq!(pool.len(), xs.len(), "pool and reference diverged");
            for (j, p) in pool.particles().iter().enumerate() {
                assert!((p.x - xs[j]).abs() < 1e-5, "step x {j}");
                assert!((p.y - ys[j]).abs() < 1e-5, "step y {j}");
                assert!((p.z - zs[j]).abs() < 1e-5, "step z {j}");
                assert!((p.vy - vys[j]).abs() < 1e-5, "step vy {j}");
            }
        }
    }

    #[test]
    fn pool_cap_and_tier_scaling_drop_overflow() {
        let mut pool = FxPool::new(two_kind_spec(), 7);
        pool.set_tier(0); // cap 4, count scale 0.5
        pool.burst(0, 6, [0.0; 3], [0.0, 0.0], 1.0, 1.0, 1.0, 1.0);
        assert_eq!(pool.len(), 3); // ceil(6 * 0.5)
        pool.burst(0, 6, [0.0; 3], [0.0, 0.0], 1.0, 1.0, 1.0, 1.0);
        assert_eq!(pool.len(), 4); // capped, the rest dropped
        pool.set_scale(0.0);
        pool.clear();
        pool.burst(0, 6, [0.0; 3], [0.0, 0.0], 1.0, 1.0, 1.0, 1.0);
        assert_eq!(pool.len(), 0);
        // An unknown kind is a no-op, never a panic.
        pool.burst(9, 4, [0.0; 3], [0.0, 0.0], 1.0, 1.0, 1.0, 1.0);
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn flipbook_frames_clamp_loop_and_grid() {
        let atlas = AtlasSpec {
            texture: "sheet.png".into(),
            cols: 4,
            rows: 2,
        };
        assert_eq!(atlas.frame_rect(0), [0.0, 0.0, 0.25, 0.5]);
        assert_eq!(atlas.frame_rect(5), [0.25, 0.5, 0.5, 1.0]);
        assert_eq!(atlas.frame_rect(99), [0.75, 0.5, 1.0, 1.0]); // clamped
        let mut kind = KindSpec {
            frame0: 2,
            frames: 4,
            ..KindSpec::default()
        };
        // One-shot: progress over the life, clamped on the last frame.
        assert_eq!(FxPool::frame_of(&kind, 0.0, 1.0), 2);
        assert_eq!(FxPool::frame_of(&kind, 0.5, 1.0), 4);
        assert_eq!(FxPool::frame_of(&kind, 0.999, 1.0), 5);
        // Looped at an explicit fps wraps.
        kind.looped = true;
        kind.fps = 4.0;
        assert_eq!(FxPool::frame_of(&kind, 0.1, 10.0), 2);
        assert_eq!(FxPool::frame_of(&kind, 1.1, 10.0), 2); // (1.1*4)=4 -> wraps to 0
        assert_eq!(FxPool::frame_of(&kind, 1.3, 10.0), 3);
        // A static kind never leaves frame0.
        kind.frames = 1;
        assert_eq!(FxPool::frame_of(&kind, 0.5, 1.0), 2);
    }

    #[test]
    fn declaration_precedes_any_spawn() {
        // First-occurrence pin: a fresh pool with zero bursts already writes
        // clouds that NAME their atlases, so the renderer uploads them on
        // the kit's first frame. There is no later moment at which an atlas
        // could first appear.
        let pool = FxPool::new(two_kind_spec(), 3);
        let mut doc = WorldDoc::default();
        let base = pool.write_doc(&mut doc);
        assert_eq!(doc.points.len(), base + 2);
        assert_eq!(doc.points[base].atlas, None); // spark: untextured kind
        assert_eq!(doc.points[base + 1].atlas.as_deref(), Some("image2d:0"));
        assert!(doc.points.iter().all(|c| c.positions.is_empty()));
        // The renderer's batching keeps the empty declaration alive.
        let batches = crate::strokes::collect_point_batches(&doc.points);
        assert!(batches
            .iter()
            .any(|b| b.atlas.as_deref() == Some("image2d:0")));
    }

    #[test]
    fn refill_writes_per_point_flipbook_state() {
        let mut pool = FxPool::new(two_kind_spec(), 3);
        pool.set_tier(2);
        pool.burst(1, 4, [1.0, 2.0, 3.0], [0.0, 0.0], 1.0, 1.0, 1.0, 1.0);
        pool.step(0.1);
        let mut doc = WorldDoc::default();
        let base = pool.write_doc(&mut doc);
        let cloud = &doc.points[base + 1];
        assert_eq!(cloud.positions.len(), pool.len());
        assert_eq!(cloud.frames.len(), pool.len());
        assert_eq!(cloud.rotations.len(), pool.len());
        assert_eq!(cloud.tints.len(), pool.len());
        // Young third: first size/alpha of the flame row.
        assert_eq!(cloud.tints[0], [255, 190, 150, 200]);
        assert!((cloud.sizes[0] - 0.22).abs() < 1e-6);
        // Frame rect lies inside the declared 4x2 grid.
        let uv = cloud.frames[0];
        assert!(uv[0] >= 0.0 && uv[2] <= 1.0 && uv[1] >= 0.0 && uv[3] <= 1.0);
    }

    #[test]
    fn size_scale_trims_refill_sizes_only() {
        let mut pool = FxPool::new(two_kind_spec(), 3);
        pool.burst(1, 2, [0.0; 3], [0.0, 0.0], 1.0, 1.0, 1.0, 1.0);
        // The default keeps the authored sizes.
        let mut doc = WorldDoc::default();
        let base = pool.write_doc(&mut doc);
        assert!((doc.points[base + 1].sizes[0] - 0.22).abs() < 1e-6);
        pool.set_size_scale(0.5);
        let mut doc = WorldDoc::default();
        let base = pool.write_doc(&mut doc);
        assert!((doc.points[base + 1].sizes[0] - 0.11).abs() < 1e-6);
        // Counts and the sim are untouched by the size knob.
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn spec_roundtrips_through_json() {
        let spec = two_kind_spec();
        let json = serde_json::to_string(&spec).unwrap();
        let back: FxSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, spec);
        // A sparse manifest block fills defaults (cap, floor, bounce).
        let sparse: FxSpec =
            serde_json::from_str(r#"{"kinds":[{"name":"puff","color":[10,20,30],"atlas":null}]}"#)
                .unwrap();
        assert_eq!(sparse.cap, [48, 160, 480]);
        assert_eq!(sparse.kinds[0].floor, 0.04);
        assert_eq!(sparse.kinds[0].bounce, 0.25);
        assert_eq!(sparse.kinds[0].blend, PointBlend::Additive);
    }

    fn mist_emitter() -> AmbientEmitter {
        AmbientEmitter {
            kind: 0,
            area: [0.0, 0.0, 5.0, 5.0],
            height: [0.3, 0.9],
            every: 1.0,
            batch: 2,
            drift: [0.6, 0.2],
            speed: 0.5,
            up: 0.0,
            life: 6.0,
        }
    }

    #[test]
    fn ambient_emitters_prime_to_steady_state_inside_the_area() {
        let mut spec = two_kind_spec();
        spec.cap = [64, 64, 64];
        let mut pool = FxPool::new(spec, 7);
        let mut emitters = AmbientEmitters::new(vec![mist_emitter()], 99);
        emitters.prime(&mut pool, 9.0);
        // Steady state is roughly life/every batches of 2 ≈ 12 alive;
        // jitter and the count scale keep it a band, never the cap.
        assert!(pool.len() >= 6, "primed pool too thin: {}", pool.len());
        assert!(pool.len() <= 64);
        for p in pool.particles() {
            // Spawned inside the area, then drifted at most
            // speed * life plus the burst fan.
            assert!(p.x.abs() <= 5.0 + 0.5 * 6.0 + 1.5, "x escaped: {}", p.x);
            assert!(p.z.abs() <= 5.0 + 0.5 * 6.0 + 1.5, "z escaped: {}", p.z);
            assert!((0.0..=1.2).contains(&p.y), "y out of band: {}", p.y);
        }
        // And it keeps breathing: stepping on spawns replacements as
        // the primed ones age out.
        for _ in 0..240 {
            emitters.step(&mut pool, 1.0 / 60.0);
            pool.step(1.0 / 60.0);
        }
        assert!(pool.len() >= 4, "pool died out: {}", pool.len());
    }

    #[test]
    fn ambient_emitters_are_deterministic_per_seed() {
        let run = || {
            let mut spec = two_kind_spec();
            spec.cap = [64, 64, 64];
            let mut pool = FxPool::new(spec, 7);
            let mut emitters = AmbientEmitters::new(vec![mist_emitter()], 99);
            for _ in 0..120 {
                emitters.step(&mut pool, 1.0 / 60.0);
                pool.step(1.0 / 60.0);
            }
            pool.particles().to_vec()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn flicker_wobbles_and_the_rotating_lamp_sputters() {
        let spec = FlickerSpec::default();
        let slots = 11;
        let mut global_min = 1.0f32;
        for slot in 0..slots {
            let mut slot_max = 0.0f32;
            let mut t = 0.0f32;
            while t < 60.0 {
                let v = flicker_intensity(&spec, slot, slots, t);
                assert!((0.05..=1.0).contains(&v), "flicker out of range: {v}");
                slot_max = slot_max.max(v);
                global_min = global_min.min(v);
                t += 0.01;
            }
            // The wobble always comes back near full brightness.
            assert!(slot_max > 0.95, "slot {slot} never recovers: {slot_max}");
        }
        // Somewhere in a minute, a sputter dips a lamp hard.
        assert!(global_min < 0.45, "no sputter dip seen: {global_min}");
        // Pure function: same question, same answer.
        assert_eq!(
            flicker_intensity(&spec, 3, slots, 12.345),
            flicker_intensity(&spec, 3, slots, 12.345)
        );
        // No lamps: full brightness, no panic.
        assert_eq!(flicker_intensity(&spec, 0, 0, 1.0), 1.0);
    }
}
