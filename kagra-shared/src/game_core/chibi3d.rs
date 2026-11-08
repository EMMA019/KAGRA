//! Chibi players in 3D: the [`chibi`](super::chibi) poses and uniforms, built from `toon`
//! primitives (cel shading and a dark outline, `WorldProp::toon`). A big round head under a
//! cap or a helmet, black oval eyes, a short body, arms in the undershirt colour, team socks,
//! a glove, a mitt or a metal bat. No glTF and no textures, so a whole team costs a few
//! hundred instanced primitives and changes colour with a number.
//!
//! The figure faces +Z at `yaw` 0 (positive yaw turns it toward +X, as `WorldProp::yaw`).
//! `height` runs from the cleats to the top of the cap; the head is about two fifths of it.
//! A right-hander throws and bats with the hand on his right (-X when he faces +Z); `lefty`
//! mirrors him.
//!
//! ```
//! use kagra_shared::game_core::chibi::{Kit, Pose};
//! use kagra_shared::game_core::chibi3d::{self, Detail, Figure};
//! use kagra_shared::game_core::portrait::{Expr, Face};
//! let face = Face::from_seed(3);
//! let kit = Kit::home([34, 78, 168, 255]);
//! let mut props = Vec::new();
//! chibi3d::build(
//!     &Figure::new("p1", [0.0, 0.0, -18.4], std::f32::consts::PI, 1.9, &face, &kit)
//!         .pose(Pose::Pitch(0.6))
//!         .expr(Expr::Fiery),
//!     &mut props,
//! );
//! assert!(props.iter().all(|p| p.enabled) && props.len() > 20);
//! ```

use super::chibi::{Kit, Pose};
use super::portrait::{Expr, Face, HAIRS, SKINS};
use crate::vector::{mix, shade, Rgba};
use crate::world_doc::WorldProp;
use glam::{EulerRot, Quat, Vec3};

const GLOVE: Rgba = [196, 126, 58, 255];
const MITT: Rgba = [140, 78, 40, 255];
const SHOE: Rgba = [36, 36, 44, 255];
const BELT: Rgba = [36, 40, 56, 255];
const GEAR: Rgba = [44, 50, 68, 255];
const METAL: Rgba = [206, 212, 222, 255];
const GRIP: Rgba = [30, 30, 36, 255];
const EYE: Rgba = [34, 26, 32, 255];

/// How much of the figure to build: the face and fingers only show up close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detail {
    /// Everything: eyes, highlights, mouth, blush.
    Full,
    /// A fielder across the diamond: body, cap and limbs only.
    Far,
}

/// One player to build.
#[derive(Clone, Copy, Debug)]
pub struct Figure<'a> {
    /// Prefix of the prop ids (`<id>:head`, ...).
    pub id: &'a str,
    /// The point between the feet, on the ground.
    pub at: [f32; 3],
    pub yaw: f32,
    pub height: f32,
    pub face: &'a Face,
    pub expr: Expr,
    pub pose: Pose,
    pub kit: &'a Kit,
    pub lefty: bool,
    pub detail: Detail,
    /// The catcher's mitt instead of a fielder's glove.
    pub mitt: bool,
    /// A bat in the hands (any pose; `Pose::Bat` always has one).
    pub bat: bool,
    /// Reach the glove hand toward this world point (as far as the arm goes): a catcher
    /// framing a pitch, a fielder stretching for a throw.
    pub reach: Option<[f32; 3]>,
}

impl<'a> Figure<'a> {
    pub fn new(
        id: &'a str,
        at: [f32; 3],
        yaw: f32,
        height: f32,
        face: &'a Face,
        kit: &'a Kit,
    ) -> Self {
        Self {
            id,
            at,
            yaw,
            height,
            face,
            expr: Expr::Neutral,
            pose: Pose::Stand,
            kit,
            lefty: false,
            detail: Detail::Full,
            mitt: false,
            bat: false,
            reach: None,
        }
    }

    pub fn pose(mut self, pose: Pose) -> Self {
        self.pose = pose;
        self
    }

    pub fn expr(mut self, expr: Expr) -> Self {
        self.expr = expr;
        self
    }

    pub fn lefty(mut self, lefty: bool) -> Self {
        self.lefty = lefty;
        self
    }

    pub fn detail(mut self, detail: Detail) -> Self {
        self.detail = detail;
        self
    }

    pub fn mitt(mut self, mitt: bool) -> Self {
        self.mitt = mitt;
        self
    }

    pub fn bat(mut self, bat: bool) -> Self {
        self.bat = bat;
        self
    }

    pub fn reach(mut self, at: Option<[f32; 3]>) -> Self {
        self.reach = at;
        self
    }

    /// The world to figure space (the inverse of [`Figure::world`]).
    fn local(&self, p: Vec3) -> Vec3 {
        let q = Quat::from_rotation_y(-self.yaw) * (p - Vec3::from_array(self.at))
            / self.height.max(1e-4);
        if self.lefty {
            Vec3::new(-q.x, q.y, q.z)
        } else {
            q
        }
    }

    /// The pose's joints, with the glove hand reaching for [`Figure::reach`].
    fn rig(&self) -> Rig {
        let mut r = rig_for(self.pose);
        if let Some(target) = self.reach {
            let shoulder = r.hips + torso_rot(&r) * v(0.145, 0.215, 0.0);
            let want = self.local(Vec3::from_array(target)) - shoulder;
            let len = want.length().max(1e-4);
            let arm = 0.28;
            let hand = shoulder + want / len * len.min(arm);
            // The elbow bends out and down from the line to the hand.
            let mid = (shoulder + hand) * 0.5;
            let slack = (arm * arm - (len.min(arm) * 0.5).powi(2) * 4.0)
                .max(0.0)
                .sqrt()
                * 0.5;
            r.hand[1] = hand;
            r.elbow[1] = mid + v(0.06, -1.0, 0.0).normalize() * slack.max(0.02);
        }
        r
    }

    /// Figure space (body units, +Z forward) to the world.
    fn world(&self, p: Vec3) -> Vec3 {
        let p = if self.lefty {
            Vec3::new(-p.x, p.y, p.z)
        } else {
            p
        };
        Vec3::from_array(self.at) + Quat::from_rotation_y(self.yaw) * (p * self.height)
    }

    fn world_rot(&self, q: Quat) -> Quat {
        let q = if self.lefty { mirror_x(q) } else { q };
        Quat::from_rotation_y(self.yaw) * q
    }
}

/// A rotation seen in a mirror across the YZ plane (x -> -x).
fn mirror_x(q: Quat) -> Quat {
    Quat::from_xyzw(q.x, -q.y, -q.z, q.w)
}

/// Joint positions in body units (height = 1), feet at y 0, facing +Z. Index 0 is the
/// throwing side (the right hand of a right-hander, -X), 1 the glove side.
#[derive(Clone, Copy, Debug)]
struct Rig {
    hips: Vec3,
    /// Forward lean of the torso (+ tips the chest toward +Z).
    lean: f32,
    /// Turn of the chest about Y (+ turns the chest toward +X).
    twist: f32,
    /// Where the head looks (yaw in figure space) and how far it nods down.
    look: f32,
    nod: f32,
    knee: [Vec3; 2],
    ankle: [Vec3; 2],
    elbow: [Vec3; 2],
    hand: [Vec3; 2],
    /// The bat's direction from the hands, when it is held.
    bat: Vec3,
}

fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

impl Rig {
    fn mix(&self, b: &Rig, k: f32) -> Rig {
        let k = k.clamp(0.0, 1.0);
        let l = |a: f32, b: f32| a + (b - a) * k;
        let lv = |a: Vec3, b: Vec3| a.lerp(b, k);
        let l2 = |a: [Vec3; 2], b: [Vec3; 2]| [lv(a[0], b[0]), lv(a[1], b[1])];
        Rig {
            hips: lv(self.hips, b.hips),
            lean: l(self.lean, b.lean),
            twist: l(self.twist, b.twist),
            look: l(self.look, b.look),
            nod: l(self.nod, b.nod),
            knee: l2(self.knee, b.knee),
            ankle: l2(self.ankle, b.ankle),
            elbow: l2(self.elbow, b.elbow),
            hand: l2(self.hand, b.hand),
            bat: lv(self.bat, b.bat).normalize_or(Vec3::Y),
        }
    }
}

const STAND: Rig = Rig {
    hips: Vec3::new(0.0, 0.27, 0.0),
    lean: 0.05,
    twist: 0.0,
    look: 0.0,
    nod: 0.0,
    knee: [
        Vec3::new(-0.066, 0.145, 0.012),
        Vec3::new(0.066, 0.145, 0.012),
    ],
    ankle: [Vec3::new(-0.07, 0.036, 0.0), Vec3::new(0.07, 0.036, 0.0)],
    elbow: [Vec3::new(-0.19, 0.37, -0.01), Vec3::new(0.19, 0.37, -0.01)],
    hand: [
        Vec3::new(-0.205, 0.265, 0.03),
        Vec3::new(0.205, 0.265, 0.03),
    ],
    bat: Vec3::new(0.0, -1.0, 0.0),
};

/// Keyframes of a motion, and where `k` falls between them.
fn keyed(keys: &[(f32, Rig)], k: f32) -> Rig {
    let k = k.clamp(0.0, 1.0);
    for w in keys.windows(2) {
        let (k0, a) = &w[0];
        let (k1, b) = &w[1];
        if k <= *k1 {
            let t = ((k - k0) / (k1 - k0).max(1e-4)).clamp(0.0, 1.0);
            // Ease in and out between keys.
            return a.mix(b, t * t * (3.0 - 2.0 * t));
        }
    }
    keys.last().map(|(_, r)| *r).unwrap_or(STAND)
}

fn pitch_keys() -> [(f32, Rig); 5] {
    let set = Rig {
        hand: [v(-0.035, 0.43, 0.12), v(0.03, 0.43, 0.13)],
        elbow: [v(-0.14, 0.37, 0.05), v(0.14, 0.37, 0.06)],
        ..STAND
    };
    let lift = Rig {
        hips: v(0.0, 0.29, 0.0),
        lean: -0.05,
        twist: -0.9,
        look: 0.0,
        knee: [v(-0.05, 0.145, 0.0), v(0.05, 0.36, 0.09)],
        ankle: [v(-0.06, 0.036, 0.0), v(0.06, 0.24, 0.01)],
        hand: [v(-0.01, 0.47, 0.03), v(0.01, 0.47, 0.05)],
        elbow: [v(-0.13, 0.42, -0.02), v(0.12, 0.42, 0.0)],
        ..set
    };
    let stride = Rig {
        hips: v(0.0, 0.235, 0.08),
        lean: 0.15,
        twist: -0.7,
        knee: [v(-0.08, 0.125, -0.08), v(0.08, 0.15, 0.22)],
        ankle: [v(-0.09, 0.036, -0.1), v(0.1, 0.036, 0.32)],
        hand: [v(-0.3, 0.52, -0.13), v(0.12, 0.46, 0.29)],
        elbow: [v(-0.23, 0.48, -0.02), v(0.13, 0.45, 0.15)],
        ..lift
    };
    let release = Rig {
        hips: v(0.0, 0.22, 0.14),
        lean: 0.45,
        twist: 0.5,
        look: 0.0,
        knee: [v(-0.08, 0.13, -0.05), v(0.09, 0.14, 0.26)],
        ankle: [v(-0.08, 0.08, -0.16), v(0.1, 0.036, 0.32)],
        hand: [v(-0.08, 0.56, 0.31), v(0.1, 0.36, 0.12)],
        elbow: [v(-0.15, 0.53, 0.16), v(0.17, 0.33, 0.02)],
        ..stride
    };
    let follow = Rig {
        hips: v(0.0, 0.2, 0.18),
        lean: 0.7,
        twist: 0.8,
        nod: -0.3,
        knee: [v(-0.04, 0.16, 0.1), v(0.08, 0.13, 0.27)],
        ankle: [v(-0.06, 0.1, 0.0), v(0.1, 0.036, 0.32)],
        hand: [v(0.14, 0.18, 0.26), v(0.17, 0.3, 0.05)],
        elbow: [v(0.0, 0.3, 0.27), v(0.21, 0.32, -0.02)],
        ..release
    };
    [
        (0.0, set),
        (0.3, lift),
        (0.55, stride),
        (0.7, release),
        (1.0, follow),
    ]
}

fn bat_keys() -> [(f32, Rig); 5] {
    let stance = Rig {
        hips: v(0.0, 0.25, 0.0),
        lean: 0.2,
        twist: -0.35,
        look: 1.3,
        nod: 0.1,
        knee: [v(-0.11, 0.135, 0.04), v(0.11, 0.135, 0.04)],
        ankle: [v(-0.13, 0.036, 0.0), v(0.13, 0.036, 0.0)],
        hand: [v(-0.1, 0.55, -0.02), v(-0.07, 0.52, 0.0)],
        elbow: [v(-0.2, 0.44, -0.05), v(0.0, 0.42, 0.09)],
        bat: v(-0.35, 0.85, -0.3),
    };
    let launch = Rig {
        twist: -0.2,
        knee: [v(-0.11, 0.135, 0.04), v(0.15, 0.13, 0.05)],
        ankle: [v(-0.13, 0.036, 0.0), v(0.2, 0.036, 0.01)],
        hand: [v(-0.07, 0.48, 0.06), v(-0.04, 0.46, 0.07)],
        elbow: [v(-0.18, 0.4, 0.0), v(0.04, 0.4, 0.1)],
        bat: v(-0.75, 0.45, 0.3),
        ..stance
    };
    let contact = Rig {
        twist: 0.5,
        lean: 0.25,
        hips: v(0.02, 0.24, 0.0),
        hand: [v(0.02, 0.4, 0.17), v(0.05, 0.4, 0.17)],
        elbow: [v(-0.1, 0.36, 0.1), v(0.12, 0.4, 0.09)],
        bat: v(-0.1, -0.05, 1.0),
        ..launch
    };
    let extend = Rig {
        twist: 1.0,
        hand: [v(0.14, 0.43, 0.15), v(0.17, 0.43, 0.13)],
        elbow: [v(0.0, 0.4, 0.17), v(0.2, 0.42, 0.04)],
        bat: v(0.9, 0.1, 0.3),
        ..contact
    };
    let finish = Rig {
        twist: 1.3,
        lean: 0.15,
        hand: [v(0.12, 0.58, -0.03), v(0.15, 0.56, -0.02)],
        elbow: [v(0.0, 0.47, 0.12), v(0.19, 0.47, 0.0)],
        bat: v(-0.4, 0.45, -0.8),
        knee: [v(-0.09, 0.14, 0.05), v(0.15, 0.13, 0.05)],
        ..extend
    };
    [
        (0.0, stance),
        (0.3, launch),
        (0.5, contact),
        (0.7, extend),
        (1.0, finish),
    ]
}

/// A leg or an arm hanging from `root`, swung forward by `swing` (radians about X) and
/// bent at the middle joint by `bend` (forward for a knee when negative).
fn limb(root: Vec3, swing: f32, bend: f32, len: [f32; 2]) -> (Vec3, Vec3) {
    let dir = |a: f32| v(0.0, -a.cos(), a.sin());
    let mid = root + dir(swing) * len[0];
    (mid, mid + dir(swing + bend) * len[1])
}

fn rig_for(pose: Pose) -> Rig {
    match pose {
        Pose::Stand => STAND,
        Pose::Ready => Rig {
            hips: v(0.0, 0.21, -0.04),
            lean: 0.45,
            nod: -0.3,
            knee: [v(-0.11, 0.13, 0.07), v(0.11, 0.13, 0.07)],
            ankle: [v(-0.13, 0.036, 0.0), v(0.13, 0.036, 0.0)],
            elbow: [v(-0.15, 0.27, 0.12), v(0.15, 0.27, 0.12)],
            hand: [v(-0.1, 0.17, 0.22), v(0.1, 0.17, 0.22)],
            ..STAND
        },
        Pose::Run(t) => {
            let a = t * std::f32::consts::TAU;
            let s = a.sin();
            let lift = a.cos().abs() * 0.015;
            let hips = v(0.0, 0.265 + lift, 0.02);
            let mut knee = [Vec3::ZERO; 2];
            let mut ankle = [Vec3::ZERO; 2];
            let mut elbow = [Vec3::ZERO; 2];
            let mut hand = [Vec3::ZERO; 2];
            for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
                let phase = if i == 0 { s } else { -s };
                let (k, an) = limb(
                    hips + v(side * 0.07, -0.03, 0.0),
                    phase * 0.75,
                    -0.2 - (-phase).max(0.0) * 1.3,
                    [0.11, 0.105],
                );
                knee[i] = k;
                ankle[i] = an;
                let (e, h) = limb(
                    hips + v(side * 0.14, 0.2, 0.06),
                    -phase * 0.8,
                    1.5,
                    [0.1, 0.09],
                );
                elbow[i] = e + v(side * 0.03, 0.0, 0.0);
                hand[i] = h + v(side * 0.01, 0.0, 0.0);
            }
            Rig {
                hips,
                lean: 0.32,
                twist: s * 0.18,
                nod: -0.25,
                knee,
                ankle,
                elbow,
                hand,
                ..STAND
            }
        }
        Pose::Pitch(k) => keyed(&pitch_keys(), k),
        Pose::Bat(s) => keyed(&bat_keys(), s),
        Pose::Squat => Rig {
            hips: v(0.0, 0.15, -0.03),
            lean: 0.35,
            nod: -0.4,
            knee: [v(-0.15, 0.17, 0.12), v(0.15, 0.17, 0.12)],
            ankle: [v(-0.12, 0.036, 0.0), v(0.12, 0.036, 0.0)],
            elbow: [v(-0.17, 0.26, 0.03), v(0.13, 0.33, 0.12)],
            hand: [v(-0.14, 0.16, 0.08), v(0.04, 0.37, 0.25)],
            ..STAND
        },
        Pose::Cheer => Rig {
            hips: v(0.0, 0.29, 0.0),
            lean: -0.06,
            nod: 0.15,
            knee: [v(-0.08, 0.15, 0.0), v(0.08, 0.15, 0.0)],
            ankle: [v(-0.1, 0.036, 0.0), v(0.1, 0.036, 0.0)],
            elbow: [v(-0.21, 0.7, 0.0), v(0.21, 0.7, 0.0)],
            hand: [v(-0.16, 0.93, 0.03), v(0.16, 0.93, 0.03)],
            ..STAND
        },
    }
}

/// The torso's rotation: turn, then lean.
fn torso_rot(r: &Rig) -> Quat {
    Quat::from_rotation_y(r.twist) * Quat::from_rotation_x(r.lean)
}

/// Where the head sits (figure space) and how it turns.
fn head_frame(r: &Rig) -> (Vec3, Quat) {
    let centre = r.hips + torso_rot(r) * v(0.0, 0.47, 0.0);
    let look = if r.look == 0.0 { r.twist * 0.5 } else { r.look };
    let q = Quat::from_rotation_y(look) * Quat::from_rotation_x(r.lean * 0.45 + r.nod);
    (centre, q)
}

/// The throwing hand in the world (the ball leaves from here on a pitch or a throw).
pub fn throwing_hand(f: &Figure) -> [f32; 3] {
    f.world(f.rig().hand[0]).to_array()
}

/// The glove hand in the world (where a caught ball sits).
pub fn glove_hand(f: &Figure) -> [f32; 3] {
    f.world(f.rig().hand[1]).to_array()
}

/// The far end of the bat in the world, while the figure holds one.
pub fn bat_tip(f: &Figure) -> [f32; 3] {
    let r = f.rig();
    let grip = (r.hand[0] + r.hand[1]) * 0.5;
    f.world(grip + r.bat * BAT_LEN).to_array()
}

const BAT_LEN: f32 = 0.47;

struct Out<'a, 'f> {
    f: &'a Figure<'f>,
    props: &'a mut Vec<WorldProp>,
    n: usize,
}

impl Out<'_, '_> {
    /// One primitive: `centre` and `rot` in figure space, `size` the full extents in body
    /// units (a sphere's diameter).
    fn put(&mut self, model: &str, centre: Vec3, rot: Quat, size: Vec3, col: Rgba, toon: bool) {
        let f = self.f;
        let at = f.world(centre);
        let q = f.world_rot(rot);
        let (yaw, x, roll) = q.to_euler(EulerRot::YXZ);
        self.n += 1;
        self.props.push(WorldProp {
            id: format!("{}:{}", f.id, self.n),
            kind: "prop".into(),
            name: "chibi".into(),
            model: model.into(),
            position: at.to_array(),
            yaw,
            pitch: -x,
            roll,
            scale: (size * f.height).to_array(),
            enabled: true,
            color: Some([col[0] as u32, col[1] as u32, col[2] as u32]),
            roughness: 0.7,
            is_static: true,
            collider: Some("none".into()),
            toon,
            ..WorldProp::default()
        });
    }

    fn ball(&mut self, centre: Vec3, rot: Quat, radii: Vec3, col: Rgba) {
        self.put("sphere", centre, rot, radii * 2.0, col, true);
    }

    /// An ellipsoid from `a` to `b`, `r` thick: a thigh, a forearm.
    fn bone(&mut self, a: Vec3, b: Vec3, r: f32, col: Rgba) {
        let d = b - a;
        let len = d.length().max(1e-4);
        let rot = Quat::from_rotation_arc(Vec3::Y, d / len);
        self.ball((a + b) * 0.5, rot, v(r, len * 0.5 + r * 0.6, r), col);
    }

    /// A cylinder from `a` along `dir` (unit) for `len`, `r` thick.
    fn rod(&mut self, a: Vec3, dir: Vec3, len: f32, r: f32, col: Rgba) {
        let rot = Quat::from_rotation_arc(Vec3::Y, dir.normalize_or(Vec3::Y));
        self.put("cylinder", a, rot, v(r * 2.0, len, r * 2.0), col, true);
    }
}

/// Build one player into `out` as toon primitives (about 30 close up, 17 far away).
pub fn build(f: &Figure, out: &mut Vec<WorldProp>) {
    let r = f.rig();
    let kit = f.kit;
    let skin = SKINS[f.face.skin as usize % SKINS.len()];
    let hair = HAIRS[f.face.hair_color as usize % HAIRS.len()];
    // The long-sleeved undershirt, a deeper shade of the team colour.
    let sleeve = shade(kit.trim, -0.22);
    let socks = if kit.gear { GEAR } else { kit.socks };
    let far = f.detail == Detail::Far;
    let mut o = Out {
        f,
        props: out,
        n: 0,
    };
    let tq = torso_rot(&r);
    let pelvis_q = Quat::from_rotation_y(r.twist * 0.35);

    // Legs: thigh, shin (socks), shoe.
    for i in 0..2 {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let hip = r.hips + pelvis_q * v(side * 0.068, -0.03, 0.0);
        o.bone(hip, r.knee[i], 0.054, kit.pants);
        o.bone(r.knee[i], r.ankle[i], 0.045, socks);
        let foot = Quat::from_rotation_y(r.twist * 0.3 + side * 0.12);
        o.ball(
            r.ankle[i] + foot * v(0.0, -0.006, 0.028),
            foot,
            v(0.05, 0.034, 0.082),
            SHOE,
        );
    }
    // Pelvis, belt, torso.
    o.ball(r.hips, pelvis_q, v(0.138, 0.082, 0.104), kit.pants);
    if !far {
        o.ball(
            r.hips + tq * v(0.0, 0.05, 0.0),
            tq,
            v(0.142, 0.022, 0.108),
            BELT,
        );
    }
    o.ball(
        r.hips + tq * v(0.0, 0.14, 0.0),
        tq,
        v(0.15, 0.14, 0.114),
        kit.shirt,
    );
    if kit.gear {
        // The catcher's chest protector.
        o.ball(
            r.hips + tq * v(0.0, 0.15, 0.06),
            tq,
            v(0.13, 0.13, 0.07),
            GEAR,
        );
    }
    // Arms: sleeve, undershirt, hand; a glove or a mitt on the glove hand.
    for i in 0..2 {
        let side = if i == 0 { -1.0 } else { 1.0 };
        let shoulder = r.hips + tq * v(side * 0.145, 0.215, 0.0);
        if far {
            o.bone(shoulder, r.elbow[i], 0.046, kit.shirt);
        } else {
            let mid = shoulder.lerp(r.elbow[i], 0.55);
            o.bone(shoulder, mid, 0.046, kit.shirt);
            o.bone(mid, r.elbow[i], 0.04, sleeve);
        }
        o.bone(r.elbow[i], r.hand[i], 0.036, sleeve);
        o.ball(r.hand[i], Quat::IDENTITY, v(0.042, 0.042, 0.042), skin);
    }
    let glove_q = Quat::from_rotation_y(r.twist * 0.5);
    if f.mitt {
        o.ball(
            r.hand[1] + v(0.0, 0.01, 0.03),
            glove_q,
            v(0.085, 0.085, 0.05),
            MITT,
        );
    } else if !matches!(f.pose, Pose::Bat(_)) && !f.bat {
        o.ball(
            r.hand[1] + v(0.0, 0.01, 0.02),
            glove_q,
            v(0.068, 0.075, 0.045),
            GLOVE,
        );
    }
    if f.bat || matches!(f.pose, Pose::Bat(_)) {
        let grip = (r.hand[0] + r.hand[1]) * 0.5;
        let dir = r.bat.normalize_or(Vec3::Y);
        o.rod(grip - dir * 0.05, dir, 0.2, 0.019, GRIP);
        o.rod(grip + dir * 0.15, dir, BAT_LEN - 0.15, 0.03, METAL);
    }

    // The head.
    let (hc, hq) = head_frame(&r);
    let at = |p: Vec3| hc + hq * p;
    let w = f.face.width.clamp(0.9, 1.1);
    o.ball(hc, hq, v(0.205 * w, 0.2, 0.198), skin);
    // Hair below the cap at the back and sides (a helmet covers it).
    if !far && !kit.helmet && !kit.gear {
        o.ball(at(v(0.0, -0.005, -0.03)), hq, v(0.207 * w, 0.16, 0.2), hair);
    }
    if kit.helmet || kit.gear {
        let shell = if kit.gear { GEAR } else { kit.cap };
        o.ball(
            at(v(0.0, 0.045, -0.012)),
            hq,
            v(0.224 * w, 0.168, 0.224),
            shell,
        );
        if kit.helmet {
            // Ear flap on the side facing the pitcher, and a short brim.
            o.ball(at(v(0.178, -0.04, -0.01)), hq, v(0.055, 0.085, 0.09), shell);
            o.ball(
                at(v(0.0, 0.03, 0.18)),
                hq * Quat::from_rotation_x(0.25),
                v(0.13, 0.012, 0.07),
                shell,
            );
        }
        if kit.gear {
            // The mask: a dark frame over the face.
            o.put(
                "box",
                at(v(0.0, -0.05, 0.2)),
                hq,
                v(0.25, 0.27, 0.04),
                GEAR,
                true,
            );
        }
    } else {
        o.ball(
            at(v(0.0, 0.06, -0.006)),
            hq,
            v(0.213 * w, 0.152, 0.213),
            kit.cap,
        );
        // The brim, tipped down a little.
        let bq = hq * Quat::from_rotation_x(0.2);
        o.ball(at(v(0.0, 0.03, 0.2)), bq, v(0.14, 0.012, 0.11), kit.cap);
    }
    if far || kit.gear {
        return;
    }
    // Eyes: dark ovals with a highlight; squeezed shut when happy.
    let (eh, ew) = match f.expr {
        Expr::Happy | Expr::Smile => (0.012, 0.03),
        Expr::Surprised => (0.04, 0.03),
        Expr::Sad => (0.03, 0.024),
        _ => (0.036, 0.023),
    };
    for side in [-1.0f32, 1.0] {
        let ex = side * 0.07 * w;
        let ey = -0.035;
        let ez = (0.198f32.powi(2) - ex * ex - ey * ey).max(0.0).sqrt();
        let face_q = hq * Quat::from_rotation_y((ex / ez).atan());
        let tilt = match f.expr {
            Expr::Fiery | Expr::Serious => Quat::from_rotation_z(side * 0.18),
            Expr::Sad => Quat::from_rotation_z(-side * 0.25),
            _ => Quat::IDENTITY,
        };
        o.ball(
            at(v(ex, ey, ez - 0.004)),
            face_q * tilt,
            v(ew, eh, 0.012),
            EYE,
        );
        if eh > 0.02 {
            o.put(
                "sphere",
                at(v(ex - side * 0.007, ey + eh * 0.35, ez + 0.007)),
                Quat::IDENTITY,
                v(0.016, 0.016, 0.01),
                [255, 255, 255, 255],
                false,
            );
        }
        if f.face.blush {
            o.put(
                "sphere",
                at(v(side * 0.115 * w, -0.085, 0.158)),
                face_q,
                v(0.06, 0.026, 0.012),
                [255, 160, 160, 255],
                false,
            );
        }
    }
    // The mouth.
    let (mw, mh, mc) = match f.expr {
        Expr::Happy | Expr::Smile => (0.06, 0.024, [196, 70, 76, 255]),
        Expr::Surprised => (0.03, 0.03, [150, 52, 58, 255]),
        _ => (0.042, 0.009, mix(EYE, skin, 0.35)),
    };
    o.put(
        "sphere",
        at(v(0.0, -0.115, 0.17)),
        hq * Quat::from_rotation_x(-0.5),
        v(mw, mh, 0.01),
        mc,
        false,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_core::chibi::Kit;

    fn figure<'a>(face: &'a Face, kit: &'a Kit, pose: Pose) -> Figure<'a> {
        Figure::new("t", [0.0, 0.0, 0.0], 0.0, 2.0, face, kit).pose(pose)
    }

    #[test]
    fn every_pose_stands_on_the_ground_at_its_height() {
        let face = Face::from_seed(1);
        let kit = Kit::home([30, 80, 170, 255]);
        for pose in [
            Pose::Stand,
            Pose::Ready,
            Pose::Run(0.0),
            Pose::Run(0.3),
            Pose::Pitch(0.0),
            Pose::Pitch(0.3),
            Pose::Pitch(0.7),
            Pose::Pitch(1.0),
            Pose::Bat(0.0),
            Pose::Bat(0.5),
            Pose::Bat(1.0),
            Pose::Squat,
            Pose::Cheer,
        ] {
            let mut props = Vec::new();
            build(&figure(&face, &kit, pose), &mut props);
            let low = props
                .iter()
                .map(|p| p.position[1] - p.scale[1] * 0.5)
                .fold(f32::MAX, f32::min);
            let high = props
                .iter()
                .filter(|p| p.model == "sphere")
                .map(|p| p.position[1] + p.scale[1] * 0.5)
                .fold(f32::MIN, f32::max);
            assert!(low > -0.12 && low < 0.15, "{pose:?} feet at {low}");
            assert!(high > 1.4 && high < 2.3, "{pose:?} top at {high}");
            assert!(props
                .iter()
                .all(|p| p.enabled && p.scale.iter().all(|s| *s > 0.0)));
            let mut ids: Vec<_> = props.iter().map(|p| p.id.clone()).collect();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), props.len(), "ids are unique");
        }
    }

    #[test]
    fn a_far_fielder_is_cheaper_and_a_lefty_is_a_mirror() {
        let face = Face::from_seed(2);
        let kit = Kit::road([170, 40, 50, 255]);
        let mut near = Vec::new();
        let mut far = Vec::new();
        build(&figure(&face, &kit, Pose::Ready), &mut near);
        build(
            &figure(&face, &kit, Pose::Ready).detail(Detail::Far),
            &mut far,
        );
        assert!(
            far.len() + 6 <= near.len(),
            "{} vs {}",
            far.len(),
            near.len()
        );
        // The throwing hand of a lefty is on the other side.
        let righty = figure(&face, &kit, Pose::Pitch(0.7));
        let lefty = righty.lefty(true);
        let (a, b) = (throwing_hand(&righty), throwing_hand(&lefty));
        assert!(a[0] < 0.0 && (a[0] + b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4);
    }

    #[test]
    fn the_release_is_out_front_and_the_swing_goes_round() {
        let face = Face::from_seed(3);
        let kit = Kit::home([30, 80, 170, 255]);
        // Facing +Z: the ball leaves high and in front of the body.
        let h = throwing_hand(&figure(&face, &kit, Pose::Pitch(0.7)));
        assert!(h[2] > 0.4 && h[1] > 0.9, "{h:?}");
        // The bat starts up behind the head and crosses the plate side (+Z) at contact.
        let tip = |s| bat_tip(&figure(&face, &kit, Pose::Bat(s)));
        assert!(tip(0.0)[1] > 1.3, "{:?}", tip(0.0));
        assert!(tip(0.5)[2] > 0.9, "{:?}", tip(0.5));
        assert!(tip(0.7)[0] > 0.6, "{:?}", tip(0.7));
        // Turning the figure turns everything with it.
        let turned = figure(&face, &kit, Pose::Pitch(0.7));
        let turned = Figure {
            yaw: std::f32::consts::PI,
            ..turned
        };
        let t = throwing_hand(&turned);
        assert!((t[2] + h[2]).abs() < 1e-3 && (t[0] + h[0]).abs() < 1e-3);
    }

    #[test]
    fn a_catcher_reaches_for_the_pitch() {
        let face = Face::from_seed(5);
        let kit = Kit::home([30, 80, 170, 255]);
        let base = Figure::new("c", [0.0, 0.0, 1.4], std::f32::consts::PI, 1.9, &face, &kit)
            .pose(Pose::Squat)
            .mitt(true);
        let rest = glove_hand(&base);
        // Low on his glove side: the mitt follows, but no farther than the arm goes.
        let target = [-0.45, 0.45, 0.95];
        let reached = glove_hand(&base.reach(Some(target)));
        let d = |a: [f32; 3], b: [f32; 3]| {
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        };
        assert!(
            d(reached, target) < d(rest, target),
            "{rest:?} -> {reached:?}"
        );
        assert!(d(reached, target) < 0.15, "{reached:?}");
        let far = glove_hand(&base.reach(Some([5.0, 3.0, -4.0])));
        let shoulder_ish = [0.0, 0.8, 1.2];
        assert!(d(far, shoulder_ish) < 0.9, "{far:?}");
        // A lefty reaches with the other hand, to the mirrored point.
        let target = [0.45, 0.45, 0.95];
        let lefty = glove_hand(&base.lefty(true).reach(Some(target)));
        assert!(d(lefty, target) < 0.15, "{lefty:?}");
    }

    #[test]
    fn toon_props_draw_as_toon_meshes_with_outlines() {
        let face = Face::from_seed(4);
        let kit = Kit::home([30, 80, 170, 255]);
        let mut doc = crate::world_doc::WorldDoc {
            version: 1,
            ..Default::default()
        };
        build(&figure(&face, &kit, Pose::Stand), &mut doc.props);
        assert!(doc.has_toon());
        let meshes = doc.compile_meshes();
        let toon: Vec<_> = meshes
            .iter()
            .filter(|(_, m)| m.mtoon.is_some_and(|t| t.outline_width > 0.0))
            .collect();
        assert_eq!(toon.len(), 4, "sphere, box, cylinder, cone");
        let scene = doc.compile_scene(0.5);
        let toon_inst = scene
            .batches
            .iter()
            .flat_map(|b| b.instances.iter())
            .filter(|i| i.material == crate::scene3d::Material::Toon)
            .count();
        let plain = doc.props.iter().filter(|p| !p.toon).count();
        assert_eq!(toon_inst, doc.props.len() - plain);
        assert!(doc.gpu_mesh_key().contains("toon:"));
    }
}
