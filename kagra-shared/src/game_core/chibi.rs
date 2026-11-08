//! Chibi players: a [`portrait`] head on a small body drawn with [`crate::vector`] (uniform
//! with team trim, belt, socks, cleats, arms, a glove or a bat). Poses for a ball game and a
//! practice ground: standing, a fielder's crouch, running, a pitch, a swing, a catcher's
//! squat, a cheer. A figure faces the viewer or shows its back (a batter or a catcher seen
//! from behind the plate).
//!
//! Sizes are in the [`Painter`]'s design units: `height` is from the cleats to the top of
//! the cap. The head is about two fifths of it, so a figure reads at 40 units and still
//! looks right at 200.
//!
//! ```
//! use kagra_shared::game_core::chibi::{self, Facing, Kit, Pose};
//! use kagra_shared::game_core::portrait::{Expr, Face};
//! use kagra_shared::hud_art::{Painter, Stage};
//! use kagra_shared::scene::DrawList;
//! let mut list = DrawList::default();
//! let mut p = Painter::new(&mut list, Stage::fit(400.0, 700.0, 400.0, 700.0));
//! let kit = Kit::home([34, 78, 168, 255]);
//! chibi::draw(&mut p, [200.0, 400.0], 120.0, &Face::from_seed(7), Expr::Fiery,
//!             Facing::Front, Pose::Pitch(0.5), &kit);
//! assert!(list.mesh.triangles() > 200);
//! ```

use super::portrait::{Expr, Face, Outfit, HAIRS, LINE, SKINS};
use crate::hud_art::Painter;
use crate::scene::TextAlign;
use crate::vector::{self, mix, shade, Paint, Rgba, P};

/// Which way the figure looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    /// Toward the viewer: the face shows.
    Front,
    /// Away from the viewer: the back of the cap or helmet, and the number.
    Back,
}

/// What the body is doing. The numbers run 0..1 through the motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pose {
    Stand,
    /// A fielder's crouch, glove out.
    Ready,
    /// A running stride (loop the number for a cycle).
    Run(f32),
    /// A pitch: set and leg lift, stride, release and follow-through.
    Pitch(f32),
    /// A swing: bat cocked at 0, through the zone at 0.5, follow-through at 1.
    Bat(f32),
    /// A catcher's squat, mitt up.
    Squat,
    /// Both arms up.
    Cheer,
    /// A bow from the waist, hands at the sides: 0 upright, 1 the deepest point.
    Bow(f32),
}

/// The uniform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kit {
    /// The cap or helmet.
    pub cap: Rgba,
    pub shirt: Rgba,
    /// Collar, sleeve ends, the stripe down the pants, the number.
    pub trim: Rgba,
    pub pants: Rgba,
    pub socks: Rgba,
    /// The number on the back.
    pub number: Option<u8>,
    /// A batting helmet (ear flap) instead of a cap.
    pub helmet: bool,
    /// Catcher's gear: helmet, chest protector, shin guards.
    pub gear: bool,
}

impl Kit {
    /// A home uniform: white with the team colour.
    pub fn home(team: Rgba) -> Self {
        Kit {
            cap: team,
            shirt: [248, 248, 250, 255],
            trim: team,
            pants: [244, 244, 246, 255],
            socks: team,
            number: None,
            helmet: false,
            gear: false,
        }
    }

    /// A grey road uniform with the team colour.
    pub fn road(team: Rgba) -> Self {
        Kit {
            shirt: [206, 210, 218, 255],
            pants: [200, 204, 212, 255],
            ..Kit::home(team)
        }
    }

    /// Practice: a T-shirt in the team colour over white pants.
    pub fn practice(team: Rgba) -> Self {
        Kit {
            shirt: mix(team, [255, 255, 255, 255], 0.15),
            trim: [250, 250, 252, 255],
            ..Kit::home(team)
        }
    }
}

const GLOVE: Rgba = [200, 132, 62, 255];
const MITT: Rgba = [150, 84, 44, 255];
const SHOE: Rgba = [40, 40, 48, 255];
const BAT: Rgba = [222, 180, 112, 255];
const BELT: Rgba = [40, 44, 60, 255];
const GEAR: Rgba = [46, 52, 70, 255];

/// Joint positions in body units (u = height / 100), origin at the feet, y up.
#[derive(Clone, Copy)]
struct Rig {
    /// Hips and shoulders move with the torso.
    tx: f32,
    ty: f32,
    /// Lean of the torso (radians, + to the viewer's right).
    lean: f32,
    /// Head offset.
    head: [f32; 2],
    /// [left, right] as the viewer sees them: knee, ankle.
    knee: [[f32; 2]; 2],
    ankle: [[f32; 2]; 2],
    /// Elbow and hand.
    elbow: [[f32; 2]; 2],
    hand: [[f32; 2]; 2],
}

fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}

fn lerp2(a: [f32; 2], b: [f32; 2], k: f32) -> [f32; 2] {
    [lerp(a[0], b[0], k), lerp(a[1], b[1], k)]
}

fn blend(a: &Rig, b: &Rig, k: f32) -> Rig {
    let k = k.clamp(0.0, 1.0);
    let e = k * k * (3.0 - 2.0 * k);
    let pair = |x: [[f32; 2]; 2], y: [[f32; 2]; 2]| [lerp2(x[0], y[0], e), lerp2(x[1], y[1], e)];
    Rig {
        tx: lerp(a.tx, b.tx, e),
        ty: lerp(a.ty, b.ty, e),
        lean: lerp(a.lean, b.lean, e),
        head: lerp2(a.head, b.head, e),
        knee: pair(a.knee, b.knee),
        ankle: pair(a.ankle, b.ankle),
        elbow: pair(a.elbow, b.elbow),
        hand: pair(a.hand, b.hand),
    }
}

const STAND: Rig = Rig {
    tx: 0.0,
    ty: 0.0,
    lean: 0.0,
    head: [0.0, 0.0],
    knee: [[-7.0, 14.0], [7.0, 14.0]],
    ankle: [[-8.0, 3.0], [8.0, 3.0]],
    elbow: [[-21.0, 37.0], [21.0, 37.0]],
    hand: [[-22.0, 26.0], [22.0, 26.0]],
};

/// Keyframes along a pose's motion.
fn rig(pose: Pose) -> Rig {
    match pose {
        Pose::Stand => STAND,
        Pose::Ready => Rig {
            ty: -6.0,
            head: [0.0, -5.0],
            knee: [[-15.0, 12.0], [15.0, 12.0]],
            ankle: [[-16.0, 3.0], [16.0, 3.0]],
            elbow: [[-20.0, 32.0], [20.0, 32.0]],
            hand: [[-12.0, 22.0], [13.0, 22.0]],
            ..STAND
        },
        Pose::Run(t) => {
            let s = (t * std::f32::consts::TAU).sin();
            Rig {
                ty: -2.0 + s.abs() * 2.0,
                lean: 0.08,
                head: [2.0, -1.0],
                knee: [
                    [-6.0 - s * 7.0, 15.0 + s.max(0.0) * 6.0],
                    [6.0 + s * 7.0, 15.0 + (-s).max(0.0) * 6.0],
                ],
                ankle: [
                    [-8.0 - s * 10.0, 3.0 + s.max(0.0) * 8.0],
                    [8.0 + s * 10.0, 3.0 + (-s).max(0.0) * 8.0],
                ],
                elbow: [[-20.0 + s * 4.0, 38.0], [20.0 - s * 4.0, 38.0]],
                hand: [
                    [-16.0 + s * 9.0, 30.0 + s * 6.0],
                    [16.0 - s * 9.0, 30.0 - s * 6.0],
                ],
                tx: 0.0,
            }
        }
        Pose::Pitch(k) => {
            // The thrower's glove is on the viewer's right.
            let set = Rig {
                head: [0.0, 1.0],
                knee: [[-7.0, 14.0], [10.0, 30.0]],
                ankle: [[-8.0, 3.0], [6.0, 18.0]],
                elbow: [[-16.0, 36.0], [16.0, 36.0]],
                hand: [[-4.0, 40.0], [4.0, 40.0]],
                ..STAND
            };
            let stride = Rig {
                tx: 3.0,
                ty: -4.0,
                lean: -0.08,
                head: [-2.0, 0.0],
                knee: [[-14.0, 13.0], [17.0, 12.0]],
                ankle: [[-19.0, 3.0], [22.0, 3.0]],
                elbow: [[-28.0, 48.0], [26.0, 42.0]],
                hand: [[-30.0, 62.0], [32.0, 40.0]],
            };
            let release = Rig {
                tx: 6.0,
                ty: -7.0,
                lean: 0.22,
                head: [4.0, -6.0],
                knee: [[-12.0, 15.0], [16.0, 11.0]],
                ankle: [[-20.0, 6.0], [21.0, 3.0]],
                elbow: [[-6.0, 40.0], [18.0, 36.0]],
                hand: [[14.0, 22.0], [12.0, 34.0]],
            };
            if k < 0.35 {
                blend(&STAND, &set, k / 0.35)
            } else if k < 0.65 {
                blend(&set, &stride, (k - 0.35) / 0.3)
            } else {
                blend(&stride, &release, (k - 0.65) / 0.35)
            }
        }
        Pose::Bat(s) => {
            let cocked = Rig {
                ty: -3.0,
                head: [0.0, -1.0],
                knee: [[-12.0, 13.0], [11.0, 13.0]],
                ankle: [[-15.0, 3.0], [14.0, 3.0]],
                elbow: [[-18.0, 40.0], [-4.0, 36.0]],
                hand: [[-10.0, 46.0], [-8.0, 44.0]],
                ..STAND
            };
            let contact = Rig {
                tx: 2.0,
                ty: -5.0,
                lean: 0.1,
                head: [1.0, -3.0],
                knee: [[-14.0, 12.0], [13.0, 12.0]],
                ankle: [[-17.0, 3.0], [17.0, 3.0]],
                elbow: [[-6.0, 30.0], [12.0, 32.0]],
                hand: [[10.0, 30.0], [12.0, 30.0]],
            };
            let follow = Rig {
                tx: 3.0,
                ty: -4.0,
                lean: 0.14,
                head: [2.0, -2.0],
                knee: [[-12.0, 13.0], [12.0, 13.0]],
                ankle: [[-17.0, 4.0], [16.0, 3.0]],
                elbow: [[4.0, 44.0], [18.0, 40.0]],
                hand: [[14.0, 50.0], [16.0, 50.0]],
            };
            if s < 0.5 {
                blend(&cocked, &contact, s / 0.5)
            } else {
                blend(&contact, &follow, (s - 0.5) / 0.5)
            }
        }
        Pose::Squat => Rig {
            ty: -18.0,
            lean: 0.0,
            head: [0.0, -2.0],
            knee: [[-22.0, 14.0], [22.0, 14.0]],
            ankle: [[-13.0, 3.0], [13.0, 3.0]],
            elbow: [[-24.0, 30.0], [22.0, 28.0]],
            hand: [[-26.0, 40.0], [14.0, 22.0]],
            tx: 0.0,
        },
        Pose::Cheer => Rig {
            ty: 2.0,
            head: [0.0, 2.0],
            knee: [[-9.0, 15.0], [9.0, 15.0]],
            ankle: [[-11.0, 4.0], [11.0, 4.0]],
            elbow: [[-24.0, 54.0], [24.0, 54.0]],
            hand: [[-28.0, 70.0], [28.0, 70.0]],
            ..STAND
        },
        Pose::Bow(d) => {
            // Heels together, hands flat at the sides; seen from the front the head dips.
            let upright = Rig {
                knee: [[-6.0, 14.0], [6.0, 14.0]],
                ankle: [[-6.0, 3.0], [6.0, 3.0]],
                elbow: [[-18.0, 37.0], [18.0, 37.0]],
                hand: [[-16.0, 24.0], [16.0, 24.0]],
                ..STAND
            };
            let low = Rig {
                ty: -2.0,
                head: [0.0, -15.0],
                elbow: [[-17.0, 34.0], [17.0, 34.0]],
                hand: [[-12.0, 20.0], [12.0, 20.0]],
                ..upright
            };
            blend(&upright, &low, d)
        }
    }
}

/// A limb: an ink stroke under a coloured one, with round joints.
fn limb(p: &mut Painter, pts: &[P], w: f32, color: Rgba, ink: f32) {
    p.line(pts, w + ink * 2.0, LINE);
    for q in pts {
        p.dot(q[0], q[1], w * 0.5 + ink, LINE);
    }
    p.line(pts, w, color);
    for q in pts {
        p.dot(q[0], q[1], w * 0.5, color);
    }
}

/// Draw a figure standing at `foot` (the point between its cleats on the ground).
#[allow(clippy::too_many_arguments)]
pub fn draw(
    p: &mut Painter,
    foot: P,
    height: f32,
    face: &Face,
    expr: Expr,
    facing: Facing,
    pose: Pose,
    kit: &Kit,
) {
    let u = height / 100.0;
    let r = rig(pose);
    let back = facing == Facing::Back;
    // Seen from behind, left and right swap.
    let flip = if back { -1.0 } else { 1.0 };
    let at = |q: [f32; 2]| -> P { [foot[0] + q[0] * u * flip, foot[1] - q[1] * u] };
    let ink = (1.3 * u).max(0.7);
    let skin = SKINS[face.skin as usize % SKINS.len()];
    // Hips and shoulders follow the torso and its lean.
    let waist_y = 30.0 + r.ty;
    let shoulder_y = 50.0 + r.ty;
    let lean = r.lean * flip;
    let hip = |side: f32| -> [f32; 2] { [r.tx + side * 8.0, waist_y - 3.0] };
    let shoulder = |side: f32| -> [f32; 2] {
        [
            r.tx + side * 16.0 + lean * (shoulder_y - waist_y),
            shoulder_y - 4.0,
        ]
    };

    // Shadow.
    p.fill(
        &vector::ellipse(foot[0], foot[1], 20.0 * u, 5.0 * u),
        Paint::Solid([0, 0, 0, 64]),
    );

    // Legs: thigh in the pants, shin in the socks (or shin guards), a cleat.
    for (k, side) in [-1.0f32, 1.0].into_iter().enumerate() {
        let (h, kn, an) = (at(hip(side)), at(r.knee[k]), at(r.ankle[k]));
        let shin = if kit.gear { GEAR } else { kit.socks };
        limb(p, &[kn, an], 8.0 * u, shin, ink);
        limb(p, &[h, kn], 11.0 * u, kit.pants, ink);
        // The pants stripe.
        if !kit.gear {
            p.line(
                &[
                    [h[0] + side * flip * 4.2 * u, h[1] + 1.0 * u],
                    [kn[0] + side * flip * 4.2 * u, kn[1] - 1.0 * u],
                ],
                1.4 * u,
                kit.trim,
            );
        }
        let toe = if back { 0.0 } else { side * flip * 2.0 * u };
        p.fill_ink(
            &vector::ellipse(an[0] + toe, an[1] + 1.2 * u, 6.5 * u, 3.4 * u),
            SHOE,
            LINE,
            ink,
        );
    }

    // The arm behind the body first (the glove arm when pitching from the front is in front).
    let arm = |p: &mut Painter, k: usize| {
        let side = if k == 0 { -1.0 } else { 1.0 };
        let s = at(shoulder(side));
        let e = at(r.elbow[k]);
        let h = at(r.hand[k]);
        let mid = [s[0] + (e[0] - s[0]) * 0.55, s[1] + (e[1] - s[1]) * 0.55];
        limb(p, &[e, h], 6.4 * u, skin, ink);
        limb(p, &[s, mid], 9.0 * u, kit.shirt, ink);
        p.line(
            &[
                mid,
                [mid[0] + (e[0] - s[0]) * 0.08, mid[1] + (e[1] - s[1]) * 0.08],
            ],
            9.0 * u,
            kit.trim,
        );
        limb(p, &[mid, e], 6.4 * u, skin, ink);
        p.fill_ink(&vector::circle(h[0], h[1], 4.0 * u), skin, LINE, ink);
    };

    // Torso: shirt with a belt.
    let tw = 34.0 * u;
    let top = at([r.tx + lean * (shoulder_y - waist_y), shoulder_y + 2.0]);
    let bottom = at([r.tx, waist_y - 3.0]);
    let torso = vec![
        [top[0] - tw * 0.5, top[1] + 3.0 * u],
        [top[0] - tw * 0.38, top[1] - 1.0 * u],
        [top[0] + tw * 0.38, top[1] - 1.0 * u],
        [top[0] + tw * 0.5, top[1] + 3.0 * u],
        [bottom[0] + tw * 0.46, bottom[1]],
        [bottom[0] - tw * 0.46, bottom[1]],
    ];
    // A batter seen from behind keeps his top hand behind the shirt.
    let behind = |k: usize| back && k == 0 && matches!(pose, Pose::Bat(_));
    for k in 0..2 {
        if behind(k) {
            arm(p, k);
        }
    }
    p.fill_ink(&torso, kit.shirt, LINE, ink);
    // Shade the far side.
    let shade_side = [
        [top[0] + tw * 0.18, top[1]],
        [top[0] + tw * 0.5, top[1] + 3.0 * u],
        [bottom[0] + tw * 0.46, bottom[1]],
        [bottom[0] + tw * 0.2, bottom[1]],
    ];
    p.fill(&shade_side, Paint::Solid(shade(kit.shirt, -0.1)));
    let belt = vec![
        [bottom[0] - tw * 0.47, bottom[1] - 3.2 * u],
        [bottom[0] + tw * 0.47, bottom[1] - 3.2 * u],
        [bottom[0] + tw * 0.47, bottom[1] + 0.6 * u],
        [bottom[0] - tw * 0.47, bottom[1] + 0.6 * u],
    ];
    p.fill_ink(&belt, BELT, LINE, ink * 0.8);
    if back {
        if kit.gear {
            // Chest protector straps across the back.
            for s in [-1.0f32, 1.0] {
                p.line(
                    &[
                        [top[0] - s * tw * 0.36, top[1] + 2.0 * u],
                        [bottom[0] + s * tw * 0.36, bottom[1] - 5.0 * u],
                    ],
                    2.6 * u,
                    GEAR,
                );
            }
        } else if let Some(n) = kit.number {
            p.text_outlined(
                &n.to_string(),
                (top[0] + bottom[0]) * 0.5,
                top[1] + 3.0 * u,
                13.0 * u,
                kit.trim,
                [255, 255, 255, 255],
                1.2 * u,
                TextAlign::Center,
            );
        }
    } else {
        // Collar V and the placket.
        let v = [
            [top[0] - 6.0 * u, top[1] - 0.5 * u],
            [top[0], top[1] + 6.0 * u],
            [top[0] + 6.0 * u, top[1] - 0.5 * u],
        ];
        p.line(&v, 2.0 * u, kit.trim);
        p.line(
            &[[top[0], top[1] + 6.0 * u], [bottom[0], bottom[1] - 3.6 * u]],
            1.0 * u,
            shade(kit.shirt, -0.22),
        );
        if kit.gear {
            let plate = vector::rounded_rect(
                top[0] - tw * 0.36,
                top[1] + 1.0 * u,
                tw * 0.72,
                (bottom[1] - top[1]) - 5.0 * u,
                5.0 * u,
            );
            p.fill_ink(&plate, GEAR, LINE, ink);
        }
    }

    // Front arms.
    for k in 0..2 {
        if !behind(k) {
            arm(p, k);
        }
    }

    // What the hands hold.
    let gl = if back { 0 } else { 1 };
    let hand_g = at(r.hand[gl]);
    match pose {
        Pose::Bat(_) => {
            let h0 = at(r.hand[0]);
            let h1 = at(r.hand[1]);
            let grip = [(h0[0] + h1[0]) * 0.5, (h0[1] + h1[1]) * 0.5];
            let s = if let Pose::Bat(s) = pose { s } else { 0.0 };
            // The bat turns from up-and-back through level to around the shoulders.
            let a = if s < 0.5 {
                lerp(2.2, 0.05, (s / 0.5).powf(1.6))
            } else {
                lerp(0.05, 1.25, (s - 0.5) / 0.5)
            };
            let len = 46.0 * u;
            let dir = [a.cos() * flip, -a.sin()];
            let tip = [grip[0] + dir[0] * len, grip[1] + dir[1] * len];
            let knob = [grip[0] - dir[0] * 4.0 * u, grip[1] - dir[1] * 4.0 * u];
            p.line(&[knob, tip], 6.4 * u, LINE);
            let taper: Vec<P> = vec![knob, tip];
            p.line(&taper, 4.4 * u, BAT);
            p.dot(tip[0], tip[1], 3.0 * u, BAT);
            p.fill_ink(&vector::circle(grip[0], grip[1], 4.2 * u), skin, LINE, ink);
        }
        Pose::Squat => {
            let m = at(r.hand[if back { 0 } else { 1 }]);
            p.fill_ink(&vector::circle(m[0], m[1], 9.0 * u), MITT, LINE, ink);
            p.dot(m[0], m[1], 4.0 * u, shade(MITT, -0.2));
        }
        Pose::Pitch(_) | Pose::Ready | Pose::Stand | Pose::Run(_) => {
            p.fill_ink(
                &vector::ellipse(hand_g[0], hand_g[1], 6.0 * u, 6.8 * u),
                GLOVE,
                LINE,
                ink,
            );
            p.line(
                &[
                    [hand_g[0] - 2.5 * u, hand_g[1] - 3.0 * u],
                    [hand_g[0] + 2.5 * u, hand_g[1] + 3.0 * u],
                ],
                1.0 * u,
                shade(GLOVE, -0.3),
            );
        }
        Pose::Cheer | Pose::Bow(_) => {}
    }

    // The head.
    let head_s = 66.0 * u;
    let chin = at([r.tx + lean * 20.0 + r.head[0], 56.0 + r.ty + r.head[1]]);
    if back {
        // The same head as the portrait's, from behind: its middle sits a quarter of the
        // portrait square above the chin.
        let cx = chin[0];
        let cy = chin[1] - head_s * 0.27;
        let hair = HAIRS[face.hair_color as usize % HAIRS.len()];
        let (w, h) = (head_s * 0.27 * face.width.clamp(0.9, 1.1), head_s * 0.29);
        p.fill_ink(
            &vector::rounded_rect(cx - w * 0.3, chin[1] - 7.0 * u, w * 0.6, 9.0 * u, 1.0 * u),
            skin,
            LINE,
            ink,
        );
        for s in [-1.0f32, 1.0] {
            p.fill_ink(
                &vector::ellipse(cx + s * w * 0.98, cy + h * 0.22, w * 0.17, h * 0.24),
                skin,
                LINE,
                ink,
            );
        }
        p.fill_ink(
            &vector::ellipse(cx, cy + h * 0.1, w, h * 0.92),
            hair,
            LINE,
            ink,
        );
        let cap = if kit.gear { GEAR } else { kit.cap };
        let dome = vector::arc(
            cx,
            cy + h * 0.02,
            w * 1.06,
            h * 1.02,
            std::f32::consts::PI,
            std::f32::consts::TAU,
            24,
        );
        p.fill_ink(&dome, cap, LINE, ink);
        p.fill(
            &vector::ellipse(cx - w * 0.35, cy - h * 0.55, w * 0.32, h * 0.18),
            Paint::Solid([255, 255, 255, 70]),
        );
        p.line(
            &[
                [cx - w * 1.02, cy + h * 0.08],
                [cx + w * 1.02, cy + h * 0.08],
            ],
            2.0 * u,
            shade(cap, -0.35),
        );
        if kit.helmet || kit.gear {
            // Ear flap on the side away from the pitcher.
            p.fill_ink(
                &vector::ellipse(cx + flip * w * 0.86, cy + h * 0.36, w * 0.28, h * 0.36),
                cap,
                LINE,
                ink,
            );
        }
    } else {
        let mut o = Outfit::cap(kit.cap);
        o.shirt = kit.shirt;
        o.trim = kit.trim;
        o.bust = false;
        p.portrait(
            chin[0] - head_s * 0.5,
            chin[1] - head_s * 0.70,
            head_s,
            face,
            expr,
            &o,
            1.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud_art::Stage;
    use crate::scene::DrawList;

    fn one(facing: Facing, pose: Pose) -> DrawList {
        let mut list = DrawList::default();
        let mut p = Painter::new(&mut list, Stage::fit(400.0, 700.0, 400.0, 700.0));
        draw(
            &mut p,
            [200.0, 400.0],
            100.0,
            &Face::from_seed(3),
            Expr::Happy,
            facing,
            pose,
            &Kit::home([30, 80, 170, 255]),
        );
        list
    }

    #[test]
    fn every_pose_draws_a_whole_figure_standing_on_its_feet() {
        for pose in [
            Pose::Stand,
            Pose::Ready,
            Pose::Run(0.25),
            Pose::Pitch(0.2),
            Pose::Pitch(0.5),
            Pose::Pitch(0.9),
            Pose::Bat(0.0),
            Pose::Bat(0.5),
            Pose::Bat(1.0),
            Pose::Squat,
            Pose::Cheer,
            Pose::Bow(0.0),
            Pose::Bow(1.0),
        ] {
            for facing in [Facing::Front, Facing::Back] {
                let list = one(facing, pose);
                let b = list.mesh.bounds().unwrap();
                // About as tall as asked, feet on the ground point.
                assert!(b[3] > 398.0 && b[3] < 410.0, "{pose:?} {facing:?} {b:?}");
                assert!(b[1] < 330.0 && b[1] > 260.0, "{pose:?} {facing:?} {b:?}");
                assert!(list.mesh.triangles() > 150);
            }
        }
    }

    #[test]
    fn a_pitch_and_a_swing_move_the_hands() {
        let hands = |pose| one(Facing::Front, pose).mesh.bounds().unwrap();
        assert_ne!(hands(Pose::Pitch(0.0)), hands(Pose::Pitch(0.55)));
        assert_ne!(hands(Pose::Bat(0.0)), hands(Pose::Bat(0.5)));
        // From behind there is no face, so far fewer triangles than a front view.
        let front = one(Facing::Front, Pose::Stand).mesh.triangles();
        let back = one(Facing::Back, Pose::Stand).mesh.triangles();
        assert!(back < front, "{back} {front}");
    }
}
