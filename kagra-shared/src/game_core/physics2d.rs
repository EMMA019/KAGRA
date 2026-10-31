//! Rigid bodies in a side view: boxes and balls that fall, roll, tip over, stack and
//! topple. Gravity is -Y; a kit draws body `(x, y)` at world `(x, y, z)` with the body's
//! angle as the prop's roll.
//!
//! Small and deterministic (a fixed step, no allocation per contact beyond the pair list),
//! made for a few dozen bodies on a phone. Sequential impulses with warm starting and
//! box-box clipping after Erin Catto's Box2D Lite (zlib licence); the code is written for
//! this crate. Bodies at rest fall asleep and wake when something moving touches them.
//!
//! ```text
//! let mut w = World::new();
//! let floor = w.add(Body::fixed(Shape::rect(20.0, 0.5), vec2(0.0, -0.5)));
//! let crate_ = w.add(Body::dynamic(Shape::rect(0.5, 0.5), vec2(0.0, 3.0), 1.0));
//! w.step(1.0 / 60.0);
//! ```

use std::collections::BTreeMap;

use glam::{Mat2, Vec2};

/// A body's outline (half sizes for a box).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Ball { r: f32 },
    Rect { hw: f32, hh: f32 },
}

impl Shape {
    /// A box `2 hw` wide and `2 hh` tall.
    pub fn rect(hw: f32, hh: f32) -> Self {
        Shape::Rect {
            hw: hw.max(0.01),
            hh: hh.max(0.01),
        }
    }

    pub fn ball(r: f32) -> Self {
        Shape::Ball { r: r.max(0.01) }
    }

    fn area(&self) -> f32 {
        match *self {
            Shape::Ball { r } => std::f32::consts::PI * r * r,
            Shape::Rect { hw, hh } => 4.0 * hw * hh,
        }
    }

    /// Moment of inertia per unit mass.
    fn inertia(&self) -> f32 {
        match *self {
            Shape::Ball { r } => 0.5 * r * r,
            Shape::Rect { hw, hh } => (4.0 * hw * hw + 4.0 * hh * hh) / 12.0,
        }
    }

    /// The radius of a circle around it (for the broad phase).
    fn reach(&self) -> f32 {
        match *self {
            Shape::Ball { r } => r,
            Shape::Rect { hw, hh } => hw.hypot(hh),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub shape: Shape,
    pub pos: Vec2,
    /// Radians, counter-clockwise.
    pub angle: f32,
    pub vel: Vec2,
    /// Radians per second.
    pub spin: f32,
    /// 0 for a fixed body.
    pub inv_mass: f32,
    pub inv_inertia: f32,
    /// 0 (ice) .. 1 (rubber); the pair uses the geometric mean.
    pub friction: f32,
    /// 0 (dead) .. 1 (superball); the pair uses the larger.
    pub bounce: f32,
    /// The kit's own number (a kind, an index); the world never reads it.
    pub tag: u32,
    /// Pushed along by the kit each step (wind, a thruster), cleared after the step.
    pub force: Vec2,
    pub asleep: bool,
    /// Seconds it has been nearly still.
    still: f32,
    alive: bool,
}

impl Body {
    /// A moving body of `density` (mass per unit area).
    pub fn dynamic(shape: Shape, pos: Vec2, density: f32) -> Self {
        let mass = (shape.area() * density.max(1e-4)).max(1e-4);
        Self {
            inv_mass: 1.0 / mass,
            inv_inertia: 1.0 / (mass * shape.inertia()),
            ..Self::fixed(shape, pos)
        }
    }

    /// A body that never moves (ground, walls); a kit may still set its `pos` / `angle` by
    /// hand for a moving platform.
    pub fn fixed(shape: Shape, pos: Vec2) -> Self {
        Self {
            shape,
            pos,
            angle: 0.0,
            vel: Vec2::ZERO,
            spin: 0.0,
            inv_mass: 0.0,
            inv_inertia: 0.0,
            friction: 0.6,
            bounce: 0.0,
            tag: 0,
            force: Vec2::ZERO,
            asleep: false,
            still: 0.0,
            alive: true,
        }
    }

    pub fn with_angle(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }

    pub fn with_friction(mut self, f: f32) -> Self {
        self.friction = f.clamp(0.0, 2.0);
        self
    }

    pub fn with_bounce(mut self, b: f32) -> Self {
        self.bounce = b.clamp(0.0, 1.0);
        self
    }

    pub fn with_tag(mut self, tag: u32) -> Self {
        self.tag = tag;
        self
    }

    pub fn is_fixed(&self) -> bool {
        self.inv_mass == 0.0
    }

    pub fn mass(&self) -> f32 {
        if self.inv_mass > 0.0 {
            1.0 / self.inv_mass
        } else {
            f32::INFINITY
        }
    }

    /// A push that changes velocity at once (a kick, an explosion), at a world point.
    pub fn impulse(&mut self, j: Vec2, at: Vec2) {
        if self.is_fixed() {
            return;
        }
        self.vel += j * self.inv_mass;
        self.spin += cross(at - self.pos, j) * self.inv_inertia;
        self.wake();
    }

    pub fn wake(&mut self) {
        self.asleep = false;
        self.still = 0.0;
    }

    /// Is the world point inside the body (a tap on it)?
    pub fn contains(&self, p: Vec2) -> bool {
        let local = Mat2::from_angle(-self.angle) * (p - self.pos);
        match self.shape {
            Shape::Ball { r } => local.length_squared() <= r * r,
            Shape::Rect { hw, hh } => local.x.abs() <= hw && local.y.abs() <= hh,
        }
    }

    /// The corners of a box (or 8 points round a ball), for drawing outlines and tests.
    pub fn outline(&self) -> Vec<Vec2> {
        let rot = Mat2::from_angle(self.angle);
        match self.shape {
            Shape::Rect { hw, hh } => [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)]
                .iter()
                .map(|&(x, y)| self.pos + rot * Vec2::new(x, y))
                .collect(),
            Shape::Ball { r } => (0..8)
                .map(|i| {
                    let a = self.angle + i as f32 * std::f32::consts::TAU / 8.0;
                    self.pos + Vec2::new(a.cos(), a.sin()) * r
                })
                .collect(),
        }
    }
}

/// A handle to a body (stays valid after other bodies are removed).
pub type BodyId = usize;

/// One point where two bodies touch.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Contact {
    pos: Vec2,
    /// From the first body toward the second.
    normal: Vec2,
    /// Negative when overlapping.
    sep: f32,
    /// Which edges made it, to carry impulses from one step to the next.
    feature: u32,
    pn: f32,
    pt: f32,
    mass_n: f32,
    mass_t: f32,
    bias: f32,
}

/// A hit worth a sound or a crack: two bodies met with this normal impulse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub a: BodyId,
    pub b: BodyId,
    pub at: Vec2,
    pub impulse: f32,
}

#[derive(Clone, Debug)]
pub struct World {
    pub gravity: Vec2,
    /// Solver passes per step (more = stiffer stacks).
    pub iterations: u32,
    bodies: Vec<Body>,
    pairs: BTreeMap<(BodyId, BodyId), Vec<Contact>>,
    /// The hits of the last step, strongest first, impulse over [`World::impact_floor`].
    pub impacts: Vec<Impact>,
    pub impact_floor: f32,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

const SLOP: f32 = 0.01;
const BIAS: f32 = 0.2;
const SLEEP_SPEED: f32 = 0.06;
const SLEEP_SECS: f32 = 0.6;

impl World {
    pub fn new() -> Self {
        Self {
            gravity: Vec2::new(0.0, -9.8),
            iterations: 10,
            bodies: Vec::new(),
            pairs: BTreeMap::new(),
            impacts: Vec::new(),
            impact_floor: 0.5,
        }
    }

    pub fn add(&mut self, body: Body) -> BodyId {
        self.bodies.push(body);
        self.bodies.len() - 1
    }

    /// Take a body out (its id is never reused).
    pub fn remove(&mut self, id: BodyId) {
        if let Some(b) = self.bodies.get_mut(id) {
            b.alive = false;
            b.vel = Vec2::ZERO;
            self.pairs.retain(|(a, c), _| *a != id && *c != id);
            for other in self.bodies.iter_mut() {
                other.wake();
            }
        }
    }

    pub fn get(&self, id: BodyId) -> Option<&Body> {
        self.bodies.get(id).filter(|b| b.alive)
    }

    pub fn get_mut(&mut self, id: BodyId) -> Option<&mut Body> {
        self.bodies.get_mut(id).filter(|b| b.alive)
    }

    /// Every body still in the world, with its id.
    pub fn bodies(&self) -> impl Iterator<Item = (BodyId, &Body)> {
        self.bodies.iter().enumerate().filter(|(_, b)| b.alive)
    }

    /// The top-most body under a world point (the last added wins).
    pub fn body_at(&self, p: Vec2) -> Option<BodyId> {
        self.bodies()
            .filter(|(_, b)| b.contains(p))
            .map(|(i, _)| i)
            .last()
    }

    /// Are these two touching now?
    pub fn touching(&self, a: BodyId, b: BodyId) -> bool {
        self.pairs.contains_key(&(a.min(b), a.max(b)))
    }

    /// Bodies moving (not asleep, not fixed).
    pub fn awake(&self) -> usize {
        self.bodies()
            .filter(|(_, b)| !b.is_fixed() && !b.asleep)
            .count()
    }

    /// Push everything within `radius` of `at` away (an explosion), stronger near it.
    pub fn blast(&mut self, at: Vec2, radius: f32, strength: f32) {
        for b in self.bodies.iter_mut().filter(|b| b.alive && !b.is_fixed()) {
            let d = b.pos - at;
            let dist = d.length();
            if dist < radius {
                let k = strength * (1.0 - dist / radius);
                let dir = if dist > 1e-4 { d / dist } else { Vec2::Y };
                let p = b.pos;
                b.impulse(dir * k * b.mass().min(1e6), p);
            }
        }
    }

    /// One fixed step.
    pub fn step(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let inv_dt = 1.0 / dt;
        self.collide_all();
        // Forces.
        let g = self.gravity;
        for b in self.bodies.iter_mut().filter(|b| b.alive) {
            if b.is_fixed() || b.asleep {
                b.force = Vec2::ZERO;
                continue;
            }
            b.vel += dt * (g + b.force * b.inv_mass);
            b.force = Vec2::ZERO;
        }
        // Prepare and warm-start each contact. A sleeping body takes no push (it acts as
        // fixed until something moving wakes it).
        let keys: Vec<(BodyId, BodyId)> = self.pairs.keys().copied().collect();
        for &(ia, ib) in &keys {
            let (a, b) = two(&mut self.bodies, ia, ib);
            if resting(a) && resting(b) {
                continue;
            }
            let ((ima, iia), (imb, iib)) = (masses(a), masses(b));
            let bounce = a.bounce.max(b.bounce);
            for c in self.pairs.get_mut(&(ia, ib)).unwrap() {
                let r1 = c.pos - a.pos;
                let r2 = c.pos - b.pos;
                let k = |r: Vec2, d: Vec2, im: f32, ii: f32| im + ii * cross(r, d).powi(2);
                let t = tangent(c.normal);
                c.mass_n = 1.0 / (k(r1, c.normal, ima, iia) + k(r2, c.normal, imb, iib)).max(1e-9);
                c.mass_t = 1.0 / (k(r1, t, ima, iia) + k(r2, t, imb, iib)).max(1e-9);
                c.bias = -BIAS * inv_dt * (c.sep + SLOP).min(0.0);
                let vn =
                    (b.vel + cross_sv(b.spin, r2) - a.vel - cross_sv(a.spin, r1)).dot(c.normal);
                if vn < -1.0 && bounce > 0.0 {
                    c.bias = c.bias.max(-bounce * vn);
                }
                let p = c.normal * c.pn + t * c.pt;
                push(a, b, r1, r2, p);
            }
        }
        // Solve.
        for _ in 0..self.iterations {
            for &(ia, ib) in &keys {
                let (a, b) = two(&mut self.bodies, ia, ib);
                if resting(a) && resting(b) {
                    continue;
                }
                let friction = (a.friction * b.friction).sqrt();
                for c in self.pairs.get_mut(&(ia, ib)).unwrap() {
                    let r1 = c.pos - a.pos;
                    let r2 = c.pos - b.pos;
                    let rel = |a: &Body, b: &Body| {
                        b.vel + cross_sv(b.spin, r2) - a.vel - cross_sv(a.spin, r1)
                    };
                    let vn = rel(a, b).dot(c.normal);
                    let old = c.pn;
                    c.pn = (old + c.mass_n * (-vn + c.bias)).max(0.0);
                    push(a, b, r1, r2, c.normal * (c.pn - old));
                    let t = tangent(c.normal);
                    let vt = rel(a, b).dot(t);
                    let max = friction * c.pn;
                    let old = c.pt;
                    c.pt = (old - c.mass_t * vt).clamp(-max, max);
                    push(a, b, r1, r2, t * (c.pt - old));
                }
            }
        }
        // Hits worth a sound.
        self.impacts.clear();
        for (&(a, b), cs) in &self.pairs {
            if let Some(c) = cs.iter().max_by(|x, y| x.pn.total_cmp(&y.pn)) {
                let total: f32 = cs.iter().map(|c| c.pn).sum();
                if total > self.impact_floor {
                    self.impacts.push(Impact {
                        a,
                        b,
                        at: c.pos,
                        impulse: total,
                    });
                }
            }
        }
        self.impacts.sort_by(|x, y| y.impulse.total_cmp(&x.impulse));
        // Move, and let still bodies sleep.
        for b in self.bodies.iter_mut().filter(|b| b.alive && !b.is_fixed()) {
            if b.asleep {
                continue;
            }
            b.spin *= 0.998;
            b.pos += b.vel * dt;
            b.angle += b.spin * dt;
            if b.vel.length() < SLEEP_SPEED && b.spin.abs() < SLEEP_SPEED * 2.0 {
                b.still += dt;
                if b.still > SLEEP_SECS {
                    b.asleep = true;
                    b.vel = Vec2::ZERO;
                    b.spin = 0.0;
                }
            } else {
                b.still = 0.0;
            }
        }
    }

    /// Find every touching pair, keep the impulses of contacts that carried over.
    fn collide_all(&mut self) {
        let n = self.bodies.len();
        let mut next: BTreeMap<(BodyId, BodyId), Vec<Contact>> = BTreeMap::new();
        let mut wake: Vec<BodyId> = Vec::new();
        for i in 0..n {
            for j in i + 1..n {
                let (a, b) = (&self.bodies[i], &self.bodies[j]);
                if !a.alive || !b.alive || (a.is_fixed() && b.is_fixed()) {
                    continue;
                }
                let sleepy = |x: &Body| x.is_fixed() || x.asleep;
                if sleepy(a) && sleepy(b) {
                    // Keep the pair as it was: nothing moves.
                    if let Some(old) = self.pairs.get(&(i, j)) {
                        next.insert((i, j), old.clone());
                    }
                    continue;
                }
                let reach = a.shape.reach() + b.shape.reach() + 0.05;
                if (a.pos - b.pos).length_squared() > reach * reach {
                    continue;
                }
                let mut cs = collide(a, b);
                if cs.is_empty() {
                    continue;
                }
                if let Some(old) = self.pairs.get(&(i, j)) {
                    for c in &mut cs {
                        if let Some(o) = old.iter().find(|o| o.feature == c.feature) {
                            c.pn = o.pn;
                            c.pt = o.pt;
                        }
                    }
                }
                // Something moving wakes what it touches.
                let moving = |x: &Body| !x.is_fixed() && !x.asleep && x.still == 0.0;
                if a.asleep && moving(b) {
                    wake.push(i);
                }
                if b.asleep && moving(a) {
                    wake.push(j);
                }
                next.insert((i, j), cs);
            }
        }
        for i in wake {
            self.bodies[i].wake();
        }
        self.pairs = next;
    }
}

/// Fixed or asleep: takes no push from a contact.
fn resting(b: &Body) -> bool {
    b.is_fixed() || b.asleep
}

fn masses(b: &Body) -> (f32, f32) {
    if b.asleep {
        (0.0, 0.0)
    } else {
        (b.inv_mass, b.inv_inertia)
    }
}

/// Impulse `p` at the contact: `-p` on `a`, `+p` on `b`.
fn push(a: &mut Body, b: &mut Body, r1: Vec2, r2: Vec2, p: Vec2) {
    let ((ima, iia), (imb, iib)) = (masses(a), masses(b));
    a.vel -= p * ima;
    a.spin -= cross(r1, p) * iia;
    b.vel += p * imb;
    b.spin += cross(r2, p) * iib;
}

fn two(v: &mut [Body], i: usize, j: usize) -> (&mut Body, &mut Body) {
    debug_assert!(i < j);
    let (lo, hi) = v.split_at_mut(j);
    (&mut lo[i], &mut hi[0])
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// Spin times a lever: the velocity of a point `r` from the centre.
fn cross_sv(w: f32, r: Vec2) -> Vec2 {
    Vec2::new(-w * r.y, w * r.x)
}

fn tangent(n: Vec2) -> Vec2 {
    Vec2::new(n.y, -n.x)
}

/// The contacts between two bodies, normals from `a` toward `b`.
fn collide(a: &Body, b: &Body) -> Vec<Contact> {
    match (a.shape, b.shape) {
        (Shape::Ball { r: ra }, Shape::Ball { r: rb }) => {
            let d = b.pos - a.pos;
            let dist = d.length();
            if dist >= ra + rb {
                return Vec::new();
            }
            let n = if dist > 1e-6 { d / dist } else { Vec2::Y };
            vec![Contact {
                pos: a.pos + n * ra,
                normal: n,
                sep: dist - ra - rb,
                ..Contact::default()
            }]
        }
        (Shape::Rect { hw, hh }, Shape::Ball { r }) => rect_ball(a, Vec2::new(hw, hh), b, r),
        (Shape::Ball { r }, Shape::Rect { hw, hh }) => {
            let mut cs = rect_ball(b, Vec2::new(hw, hh), a, r);
            for c in &mut cs {
                c.normal = -c.normal;
            }
            cs
        }
        (Shape::Rect { hw: aw, hh: ah }, Shape::Rect { hw: bw, hh: bh }) => {
            rect_rect(a, Vec2::new(aw, ah), b, Vec2::new(bw, bh))
        }
    }
}

fn rect_ball(a: &Body, h: Vec2, b: &Body, r: f32) -> Vec<Contact> {
    let rot = Mat2::from_angle(a.angle);
    let local = rot.transpose() * (b.pos - a.pos);
    let clamped = local.clamp(-h, h);
    let (n_local, sep, at_local) = if clamped == local {
        // The centre is inside: out through the nearest face.
        let dx = h.x - local.x.abs();
        let dy = h.y - local.y.abs();
        if dx < dy {
            let s = local.x.signum();
            (Vec2::new(s, 0.0), -dx - r, Vec2::new(s * h.x, local.y))
        } else {
            let s = local.y.signum();
            (Vec2::new(0.0, s), -dy - r, Vec2::new(local.x, s * h.y))
        }
    } else {
        let d = local - clamped;
        let dist = d.length();
        if dist >= r {
            return Vec::new();
        }
        (d / dist.max(1e-6), dist - r, clamped)
    };
    vec![Contact {
        pos: a.pos + rot * at_local,
        normal: rot * n_local,
        sep,
        ..Contact::default()
    }]
}

// Box-box: find the axis of least overlap, then clip the other box's most facing edge
// against the reference face's sides (Box2D Lite's method).

const NO_EDGE: u8 = 0;
const EDGE1: u8 = 1;
const EDGE2: u8 = 2;
const EDGE3: u8 = 3;
const EDGE4: u8 = 4;

#[derive(Clone, Copy, Debug, Default)]
struct Feature {
    in1: u8,
    out1: u8,
    in2: u8,
    out2: u8,
}

impl Feature {
    fn key(self) -> u32 {
        u32::from_le_bytes([self.in1, self.out1, self.in2, self.out2])
    }

    fn flipped(self) -> Self {
        Self {
            in1: self.in2,
            out1: self.out2,
            in2: self.in1,
            out2: self.out1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ClipVertex {
    v: Vec2,
    fp: Feature,
}

fn incident_edge(h: Vec2, pos: Vec2, rot: Mat2, normal: Vec2) -> [ClipVertex; 2] {
    let n = -(rot.transpose() * normal);
    let v = |x: f32, y: f32, i: u8, o: u8| ClipVertex {
        v: Vec2::new(x, y),
        fp: Feature {
            in2: i,
            out2: o,
            ..Feature::default()
        },
    };
    let mut c = if n.x.abs() > n.y.abs() {
        if n.x > 0.0 {
            [v(h.x, -h.y, EDGE3, EDGE4), v(h.x, h.y, EDGE4, EDGE1)]
        } else {
            [v(-h.x, h.y, EDGE1, EDGE2), v(-h.x, -h.y, EDGE2, EDGE3)]
        }
    } else if n.y > 0.0 {
        [v(h.x, h.y, EDGE4, EDGE1), v(-h.x, h.y, EDGE1, EDGE2)]
    } else {
        [v(-h.x, -h.y, EDGE2, EDGE3), v(h.x, -h.y, EDGE3, EDGE4)]
    };
    for cv in &mut c {
        cv.v = pos + rot * cv.v;
    }
    c
}

fn clip_segment(input: &[ClipVertex; 2], normal: Vec2, offset: f32, edge: u8) -> Vec<ClipVertex> {
    let mut out = Vec::with_capacity(2);
    let d0 = normal.dot(input[0].v) - offset;
    let d1 = normal.dot(input[1].v) - offset;
    if d0 <= 0.0 {
        out.push(input[0]);
    }
    if d1 <= 0.0 {
        out.push(input[1]);
    }
    if d0 * d1 < 0.0 {
        let k = d0 / (d0 - d1);
        let mut cv = ClipVertex {
            v: input[0].v + k * (input[1].v - input[0].v),
            fp: Feature::default(),
        };
        if d0 > 0.0 {
            cv.fp = input[0].fp;
            cv.fp.in1 = edge;
            cv.fp.in2 = NO_EDGE;
        } else {
            cv.fp = input[1].fp;
            cv.fp.out1 = edge;
            cv.fp.out2 = NO_EDGE;
        }
        out.push(cv);
    }
    out
}

fn rect_rect(a: &Body, ha: Vec2, b: &Body, hb: Vec2) -> Vec<Contact> {
    let rot_a = Mat2::from_angle(a.angle);
    let rot_b = Mat2::from_angle(b.angle);
    let (rot_at, rot_bt) = (rot_a.transpose(), rot_b.transpose());
    let dp = b.pos - a.pos;
    let da = rot_at * dp;
    let db = rot_bt * dp;
    let c = rot_at * rot_b;
    let abs_c = c.abs();
    let abs_ct = abs_c.transpose();
    let face_a = da.abs() - ha - abs_c * hb;
    if face_a.x > 0.0 || face_a.y > 0.0 {
        return Vec::new();
    }
    let face_b = db.abs() - abs_ct * ha - hb;
    if face_b.x > 0.0 || face_b.y > 0.0 {
        return Vec::new();
    }
    #[derive(PartialEq, Clone, Copy)]
    enum Axis {
        AX,
        AY,
        BX,
        BY,
    }
    let (rel, abs) = (0.95, 0.01);
    let flip = |d: f32, v: Vec2| if d > 0.0 { v } else { -v };
    let mut axis = Axis::AX;
    let mut sep = face_a.x;
    let mut normal = flip(da.x, rot_a.x_axis);
    if face_a.y > rel * sep + abs * ha.y {
        axis = Axis::AY;
        sep = face_a.y;
        normal = flip(da.y, rot_a.y_axis);
    }
    if face_b.x > rel * sep + abs * hb.x {
        axis = Axis::BX;
        sep = face_b.x;
        normal = flip(db.x, rot_b.x_axis);
    }
    if face_b.y > rel * sep + abs * hb.y {
        axis = Axis::BY;
        normal = flip(db.y, rot_b.y_axis);
    }
    let (front_n, front, side_n, neg_side, pos_side, neg_edge, pos_edge, incident) = match axis {
        Axis::AX => {
            let side_n = rot_a.y_axis;
            let side = a.pos.dot(side_n);
            (
                normal,
                a.pos.dot(normal) + ha.x,
                side_n,
                -side + ha.y,
                side + ha.y,
                EDGE3,
                EDGE1,
                incident_edge(hb, b.pos, rot_b, normal),
            )
        }
        Axis::AY => {
            let side_n = rot_a.x_axis;
            let side = a.pos.dot(side_n);
            (
                normal,
                a.pos.dot(normal) + ha.y,
                side_n,
                -side + ha.x,
                side + ha.x,
                EDGE2,
                EDGE4,
                incident_edge(hb, b.pos, rot_b, normal),
            )
        }
        Axis::BX => {
            let fn_ = -normal;
            let side_n = rot_b.y_axis;
            let side = b.pos.dot(side_n);
            (
                fn_,
                b.pos.dot(fn_) + hb.x,
                side_n,
                -side + hb.y,
                side + hb.y,
                EDGE3,
                EDGE1,
                incident_edge(ha, a.pos, rot_a, fn_),
            )
        }
        Axis::BY => {
            let fn_ = -normal;
            let side_n = rot_b.x_axis;
            let side = b.pos.dot(side_n);
            (
                fn_,
                b.pos.dot(fn_) + hb.y,
                side_n,
                -side + hb.x,
                side + hb.x,
                EDGE2,
                EDGE4,
                incident_edge(ha, a.pos, rot_a, fn_),
            )
        }
    };
    let c1 = clip_segment(&incident, -side_n, neg_side, neg_edge);
    if c1.len() < 2 {
        return Vec::new();
    }
    let c2 = clip_segment(&[c1[0], c1[1]], side_n, pos_side, pos_edge);
    if c2.len() < 2 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(2);
    for cv in c2 {
        let s = front_n.dot(cv.v) - front;
        if s <= 0.0 {
            let fp = if matches!(axis, Axis::BX | Axis::BY) {
                cv.fp.flipped()
            } else {
                cv.fp
            };
            out.push(Contact {
                pos: cv.v - s * front_n,
                normal,
                sep: s,
                feature: fp.key(),
                ..Contact::default()
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn floor(w: &mut World) -> BodyId {
        w.add(Body::fixed(Shape::rect(20.0, 0.5), Vec2::new(0.0, -0.5)))
    }

    fn run(w: &mut World, secs: f32) {
        for _ in 0..(secs / DT) as usize {
            w.step(DT);
        }
    }

    #[test]
    fn a_crate_falls_lands_and_sleeps_on_the_floor() {
        let mut w = World::new();
        floor(&mut w);
        let c = w.add(Body::dynamic(
            Shape::rect(0.5, 0.5),
            Vec2::new(0.0, 3.0),
            1.0,
        ));
        run(&mut w, 3.0);
        let b = w.get(c).unwrap();
        assert!(
            (b.pos.y - 0.5).abs() < 0.03,
            "rests on the floor: {}",
            b.pos.y
        );
        assert!(b.angle.abs() < 0.01);
        assert!(b.asleep, "a crate at rest sleeps");
        assert_eq!(w.awake(), 0);
    }

    #[test]
    fn a_stack_of_crates_stands_and_a_ball_knocks_it_down() {
        let mut w = World::new();
        floor(&mut w);
        let crates: Vec<BodyId> = (0..6)
            .map(|i| {
                w.add(Body::dynamic(
                    Shape::rect(0.5, 0.5),
                    Vec2::new(0.0, 0.5 + i as f32 * 1.0),
                    1.0,
                ))
            })
            .collect();
        run(&mut w, 4.0);
        let top = w.get(*crates.last().unwrap()).unwrap();
        assert!(top.pos.x.abs() < 0.05, "the stack stands: {:?}", top.pos);
        assert!((top.pos.y - 5.5).abs() < 0.1, "{:?}", top.pos);
        // A heavy ball rolls in fast and the tower comes down.
        let ball = w.add(Body::dynamic(Shape::ball(0.6), Vec2::new(-8.0, 3.0), 6.0));
        w.get_mut(ball).unwrap().vel = Vec2::new(14.0, 0.0);
        run(&mut w, 4.0);
        let top = w.get(*crates.last().unwrap()).unwrap();
        assert!(top.pos.y < 3.0, "the top crate fell: {:?}", top.pos);
        assert!(!w.impacts.is_empty() || w.awake() < 7);
    }

    #[test]
    fn a_ball_rolls_down_a_slope_and_a_tilted_crate_tips_over() {
        let mut w = World::new();
        floor(&mut w);
        w.add(Body::fixed(Shape::rect(4.0, 0.2), Vec2::new(-4.0, 2.0)).with_angle(-0.35));
        let ball = w.add(Body::dynamic(Shape::ball(0.4), Vec2::new(-6.5, 3.6), 1.0));
        run(&mut w, 2.5);
        let b = w.get(ball).unwrap();
        assert!(b.pos.x > -2.0, "rolled down and away: {:?}", b.pos);
        assert!(
            b.spin.abs() > 0.5 || b.vel.x > 0.5,
            "it rolls (spins), not slides"
        );
        // A tall plank set past its tipping point falls flat.
        let mut w = World::new();
        floor(&mut w);
        let plank =
            w.add(Body::dynamic(Shape::rect(0.25, 1.5), Vec2::new(0.0, 1.6), 1.0).with_angle(0.4));
        run(&mut w, 4.0);
        let p = w.get(plank).unwrap();
        let flat = (p.angle.abs() - std::f32::consts::FRAC_PI_2).abs();
        assert!(flat < 0.1, "lies flat: angle {}", p.angle);
        assert!((p.pos.y - 0.25).abs() < 0.08, "{:?}", p.pos);
    }

    #[test]
    fn the_same_start_gives_the_same_end() {
        let make = || {
            let mut w = World::new();
            floor(&mut w);
            for i in 0..8 {
                w.add(
                    Body::dynamic(
                        Shape::rect(0.4, 0.3),
                        Vec2::new((i % 3) as f32 * 0.3, 1.0 + i as f32 * 0.7),
                        1.0,
                    )
                    .with_angle(i as f32 * 0.2),
                );
            }
            run(&mut w, 3.0);
            w.bodies()
                .map(|(_, b)| (b.pos.x, b.pos.y, b.angle))
                .collect::<Vec<_>>()
        };
        assert_eq!(make(), make());
    }

    #[test]
    fn blasts_wake_sleepers_taps_find_bodies_and_removal_wakes_the_rest() {
        let mut w = World::new();
        floor(&mut w);
        let a = w.add(Body::dynamic(
            Shape::rect(0.5, 0.5),
            Vec2::new(0.0, 0.5),
            1.0,
        ));
        let b = w.add(Body::dynamic(
            Shape::rect(0.5, 0.5),
            Vec2::new(0.0, 1.5),
            1.0,
        ));
        run(&mut w, 2.0);
        assert!(w.get(b).unwrap().asleep);
        assert_eq!(w.body_at(Vec2::new(0.1, 1.4)), Some(b));
        assert_eq!(w.body_at(Vec2::new(5.0, 5.0)), None);
        w.remove(a);
        assert!(w.get(a).is_none());
        run(&mut w, 1.0);
        assert!(
            (w.get(b).unwrap().pos.y - 0.5).abs() < 0.05,
            "the top one dropped"
        );
        w.blast(Vec2::new(-1.0, 0.0), 3.0, 8.0);
        w.step(DT);
        assert!(w.get(b).unwrap().vel.x > 1.0);
    }

    #[test]
    fn a_hard_landing_is_an_impact_and_bouncy_balls_bounce() {
        let mut w = World::new();
        floor(&mut w);
        let c = w.add(Body::dynamic(
            Shape::rect(0.5, 0.5),
            Vec2::new(0.0, 6.0),
            1.0,
        ));
        let mut hit = 0.0_f32;
        for _ in 0..120 {
            w.step(DT);
            if let Some(i) = w.impacts.iter().find(|i| i.b == c || i.a == c) {
                hit = hit.max(i.impulse);
            }
        }
        assert!(hit > 2.0, "landing impact {hit}");
        let mut w = World::new();
        floor(&mut w);
        let ball =
            w.add(Body::dynamic(Shape::ball(0.3), Vec2::new(0.0, 4.0), 1.0).with_bounce(0.8));
        let mut top_after = 0.0_f32;
        let mut landed = false;
        for _ in 0..200 {
            w.step(DT);
            let b = w.get(ball).unwrap();
            if b.pos.y < 0.35 {
                landed = true;
            }
            if landed {
                top_after = top_after.max(b.pos.y);
            }
        }
        assert!(top_after > 1.5, "bounced back up to {top_after}");
    }
}
