//! Swarm parts for arena kits (survivor style): a fixed pool of foes in
//! structure-of-arrays form, a uniform grid that answers "who is near here" in
//! a few cell reads, and the director's spawn budget.
//!
//! Nothing here knows about a hero, a weapon or a picture. The kit owns those;
//! this holds the numbers and loops that must stay cheap with hundreds of
//! bodies on a phone. Every buffer is sized once (`with_capacity`), a frame
//! allocates nothing, and removing a foe is a swap with the last one, so the
//! arrays stay dense.

/// A pool of foes. Index `i` is valid for `i < len()`; a removal moves the last
/// foe into the hole, so loop backwards when removing while walking.
#[derive(Clone, Debug, Default)]
pub struct Foes {
    pub x: Vec<f32>,
    pub z: Vec<f32>,
    pub hp: Vec<f32>,
    /// Index into the kit's enemy type list.
    pub kind: Vec<u8>,
    /// Seconds of hit flash left.
    pub flash: Vec<f32>,
    /// Seconds alive (for animation and the spawn pop).
    pub age: Vec<f32>,
    /// Stable id: shots remember whom they hit, and indices move.
    pub uid: Vec<u32>,
    cap: usize,
    next_uid: u32,
}

impl Foes {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            x: Vec::with_capacity(cap),
            z: Vec::with_capacity(cap),
            hp: Vec::with_capacity(cap),
            kind: Vec::with_capacity(cap),
            flash: Vec::with_capacity(cap),
            age: Vec::with_capacity(cap),
            uid: Vec::with_capacity(cap),
            cap,
            next_uid: 1,
        }
    }

    pub fn len(&self) -> usize {
        self.x.len()
    }

    pub fn is_empty(&self) -> bool {
        self.x.is_empty()
    }

    pub fn cap(&self) -> usize {
        self.cap
    }

    pub fn clear(&mut self) {
        self.x.clear();
        self.z.clear();
        self.hp.clear();
        self.kind.clear();
        self.flash.clear();
        self.age.clear();
        self.uid.clear();
        self.next_uid = 1;
    }

    /// Add a foe. `None` when the pool is full (the cap is the performance budget).
    pub fn spawn(&mut self, kind: u8, x: f32, z: f32, hp: f32) -> Option<usize> {
        if self.len() >= self.cap {
            return None;
        }
        self.x.push(x);
        self.z.push(z);
        self.hp.push(hp);
        self.kind.push(kind);
        self.flash.push(0.0);
        self.age.push(0.0);
        self.uid.push(self.next_uid);
        self.next_uid = self.next_uid.wrapping_add(1).max(1);
        Some(self.len() - 1)
    }

    /// Remove foe `i` by moving the last one into its place.
    pub fn remove(&mut self, i: usize) {
        self.x.swap_remove(i);
        self.z.swap_remove(i);
        self.hp.swap_remove(i);
        self.kind.swap_remove(i);
        self.flash.swap_remove(i);
        self.age.swap_remove(i);
        self.uid.swap_remove(i);
    }
}

/// A uniform grid over a square arena (`-half..half` on both axes), rebuilt
/// each step by a counting sort. Queries read the cells a circle covers.
#[derive(Clone, Debug)]
pub struct Grid {
    half: f32,
    cell: f32,
    n: usize,
    start: Vec<u32>,
    order: Vec<u32>,
    slot: Vec<u32>,
}

impl Grid {
    /// `half`: arena half size (m). `cell`: cell size (m), about the largest body diameter.
    pub fn new(half: f32, cell: f32, cap: usize) -> Self {
        let cell = cell.max(0.25);
        let n = ((half * 2.0 / cell).ceil() as usize + 2).max(3);
        Self {
            half,
            cell,
            n,
            start: vec![0; n * n + 1],
            order: Vec::with_capacity(cap),
            slot: Vec::with_capacity(cap),
        }
    }

    fn axis(&self, v: f32) -> usize {
        (((v + self.half) / self.cell).floor().max(0.0) as usize).min(self.n - 1)
    }

    /// Sort the points `(xs[i], zs[i])` into cells. No allocation after the first
    /// few calls (the buffers keep their capacity).
    pub fn rebuild(&mut self, xs: &[f32], zs: &[f32]) {
        let count = xs.len().min(zs.len());
        self.start.iter_mut().for_each(|c| *c = 0);
        self.slot.clear();
        for i in 0..count {
            let c = (self.axis(zs[i]) * self.n + self.axis(xs[i])) as u32;
            self.slot.push(c);
            self.start[c as usize + 1] += 1;
        }
        for c in 0..self.n * self.n {
            self.start[c + 1] += self.start[c];
        }
        self.order.clear();
        self.order.resize(count, 0);
        // `fill` walks each cell's block; reuse `start` as write cursors, then restore.
        for i in 0..count {
            let c = self.slot[i] as usize;
            let at = self.start[c];
            self.order[at as usize] = i as u32;
            self.start[c] += 1;
        }
        for c in (1..=self.n * self.n).rev() {
            self.start[c] = self.start[c - 1];
        }
        self.start[0] = 0;
    }

    /// Call `f(i)` for every point in a cell the circle `(x, z, r)` touches. The
    /// caller still checks the real distance.
    pub fn query(&self, x: f32, z: f32, r: f32, mut f: impl FnMut(usize)) {
        let (x0, x1) = (self.axis(x - r), self.axis(x + r));
        let (z0, z1) = (self.axis(z - r), self.axis(z + r));
        for cz in z0..=z1 {
            let row = cz * self.n;
            for cx in x0..=x1 {
                let c = row + cx;
                for k in self.start[c]..self.start[c + 1] {
                    f(self.order[k as usize] as usize);
                }
            }
        }
    }

    /// The nearest point within `max_r` (index and distance), or `None`.
    pub fn nearest(
        &self,
        xs: &[f32],
        zs: &[f32],
        x: f32,
        z: f32,
        max_r: f32,
    ) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        // Widen the search one ring of cells at a time; stop one ring after the first hit.
        let rings = (max_r / self.cell).ceil() as i32 + 1;
        let (cx, cz) = (self.axis(x) as i32, self.axis(z) as i32);
        let mut found_at: Option<i32> = None;
        for ring in 0..=rings {
            if let Some(f) = found_at {
                if ring > f + 1 {
                    break;
                }
            }
            for dz in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs().max(dz.abs()) != ring {
                        continue;
                    }
                    let (gx, gz) = (cx + dx, cz + dz);
                    if gx < 0 || gz < 0 || gx >= self.n as i32 || gz >= self.n as i32 {
                        continue;
                    }
                    let c = gz as usize * self.n + gx as usize;
                    for k in self.start[c]..self.start[c + 1] {
                        let i = self.order[k as usize] as usize;
                        let d = ((xs[i] - x).powi(2) + (zs[i] - z).powi(2)).sqrt();
                        if d <= max_r && best.is_none_or(|(_, b)| d < b) {
                            best = Some((i, d));
                            found_at.get_or_insert(ring);
                        }
                    }
                }
            }
        }
        best
    }
}

/// How many foes the director wants born this step. `rate(t)` rises from
/// `start` to `end` foes per second over `ramp` seconds along `t^curve`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Budget {
    acc: f32,
}

impl Budget {
    pub fn rate(start: f32, end: f32, ramp: f32, curve: f32, t: f32) -> f32 {
        let u = (t / ramp.max(0.001)).clamp(0.0, 1.0).powf(curve.max(0.1));
        start + (end - start) * u
    }

    /// Whole foes to spawn this step; the remainder carries over.
    pub fn take(&mut self, rate: f32, dt: f32) -> u32 {
        self.acc += rate.max(0.0) * dt;
        let n = self.acc.floor();
        self.acc -= n;
        n as u32
    }

    pub fn reset(&mut self) {
        self.acc = 0.0;
    }
}

/// A small deterministic generator (xorshift32): the same seed gives the same
/// run, which is what the daily challenge and the bot tests rely on.
#[derive(Clone, Copy, Debug)]
pub struct Rng(pub u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(if seed == 0 { 0x9e37_79b9 } else { seed })
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// 0.0..1.0
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scatter(n: usize, seed: u32, half: f32) -> (Vec<f32>, Vec<f32>) {
        let mut r = Rng::new(seed);
        let xs = (0..n).map(|_| r.range(-half, half)).collect();
        let zs = (0..n).map(|_| r.range(-half, half)).collect();
        (xs, zs)
    }

    #[test]
    fn query_and_nearest_agree_with_brute_force() {
        let (xs, zs) = scatter(300, 7, 12.0);
        let mut g = Grid::new(12.0, 1.0, 300);
        g.rebuild(&xs, &zs);
        let mut r = Rng::new(99);
        for _ in 0..40 {
            let (x, z, rad) = (
                r.range(-12.0, 12.0),
                r.range(-12.0, 12.0),
                r.range(0.3, 3.0),
            );
            let mut got = Vec::new();
            g.query(x, z, rad, |i| {
                if (xs[i] - x).hypot(zs[i] - z) <= rad {
                    got.push(i);
                }
            });
            got.sort_unstable();
            let want: Vec<usize> = (0..xs.len())
                .filter(|&i| (xs[i] - x).hypot(zs[i] - z) <= rad)
                .collect();
            assert_eq!(got, want);
            let near = g.nearest(&xs, &zs, x, z, 6.0);
            let brute = (0..xs.len())
                .map(|i| (i, (xs[i] - x).hypot(zs[i] - z)))
                .filter(|(_, d)| *d <= 6.0)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            assert_eq!(near.map(|n| n.0), brute.map(|n| n.0));
        }
    }

    #[test]
    fn grid_handles_points_outside_the_arena_and_empty_input() {
        let mut g = Grid::new(5.0, 1.0, 4);
        g.rebuild(&[], &[]);
        assert!(g.nearest(&[], &[], 0.0, 0.0, 3.0).is_none());
        let (xs, zs) = ([99.0, -99.0], [0.0, 99.0]);
        g.rebuild(&xs, &zs);
        let mut seen = 0;
        g.query(99.0, 0.0, 1.0, |_| seen += 1);
        assert!(seen >= 1, "clamped into the edge cell");
    }

    #[test]
    fn pool_is_capped_dense_and_never_reallocates() {
        let mut f = Foes::with_capacity(8);
        let ptr = f.x.as_ptr();
        for i in 0..8 {
            assert!(f.spawn(0, i as f32, 0.0, 1.0).is_some());
        }
        assert!(f.spawn(0, 0.0, 0.0, 1.0).is_none(), "full pool refuses");
        let last = f.uid[7];
        f.remove(2);
        assert_eq!(f.len(), 7);
        assert_eq!(f.uid[2], last, "the last foe filled the hole");
        assert!(f.spawn(0, 0.0, 0.0, 1.0).is_some());
        assert_eq!(f.x.as_ptr(), ptr, "no reallocation inside the cap");
        let mut ids = f.uid.clone();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 8, "uids stay unique");
    }

    #[test]
    fn budget_carries_the_remainder_and_follows_the_curve() {
        let mut b = Budget::default();
        let total: u32 = (0..600).map(|_| b.take(2.5, 1.0 / 60.0)).sum();
        assert_eq!(total, 25);
        assert_eq!(Budget::rate(1.0, 9.0, 100.0, 1.0, 0.0), 1.0);
        assert_eq!(Budget::rate(1.0, 9.0, 100.0, 1.0, 100.0), 9.0);
        assert!(
            Budget::rate(1.0, 9.0, 100.0, 2.0, 50.0) < 5.0,
            "curve > 1 bends late"
        );
        assert_eq!(Budget::rate(1.0, 9.0, 100.0, 1.0, 999.0), 9.0);
    }

    #[test]
    fn rng_is_deterministic_and_in_range() {
        let (mut a, mut b) = (Rng::new(5), Rng::new(5));
        for _ in 0..100 {
            let v = a.unit();
            assert_eq!(v, b.unit());
            assert!((0.0..1.0).contains(&v));
        }
        assert_ne!(Rng::new(1).next_u32(), Rng::new(2).next_u32());
        assert_ne!(Rng::new(0).next_u32(), 0, "a zero seed still runs");
    }
}
