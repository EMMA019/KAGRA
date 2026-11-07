//! Portraits drawn as line art: an original look for KAGRA games (talk scenes, rosters,
//! result cards). Soft oval faces, big glossy eyes whose lids carry the mood, tapered brows,
//! eight haircuts or a team cap with seams, a jersey with trim. Every part is a filled
//! [`crate::vector`] shape with an ink outline, so a face needs no picture file and stays
//! sharp at any size.
//!
//! A portrait is built for one pixel size (its ink lines stay about a pixel or more wide)
//! and cached; [`draw`] places the cached mesh. Faces come from a seed ([`Face::from_seed`])
//! or a save; [`Expr`] picks the expression.
//!
//! ```
//! use kagra_shared::game_core::portrait::{self, Expr, Face, Outfit};
//! use kagra_shared::scene::DrawList;
//! let mut list = DrawList::default();
//! let face = Face::from_seed(42);
//! portrait::draw(&mut list.mesh, 10.0, 10.0, 96.0, &face, Expr::Happy, &Outfit::cap([28, 72, 160, 255]));
//! assert!(list.mesh.triangles() > 300);
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};
use std::rc::Rc;

use crate::scene::Mesh2d;
use crate::vector::{self, mix, shade, Paint, Rgba, P};

pub const SKINS: [Rgba; 4] = [
    [255, 228, 206, 255],
    [246, 210, 180, 255],
    [230, 190, 152, 255],
    [206, 160, 122, 255],
];
pub const HAIRS: [Rgba; 6] = [
    [44, 38, 46, 255],
    [78, 54, 42, 255],
    [122, 82, 52, 255],
    [46, 52, 82, 255],
    [160, 116, 72, 255],
    [24, 22, 28, 255],
];
pub const EYES: [Rgba; 6] = [
    [92, 60, 44, 255],
    [52, 84, 140, 255],
    [110, 74, 40, 255],
    [44, 44, 54, 255],
    [70, 110, 70, 255],
    [130, 64, 60, 255],
];
/// The ink of every outline.
pub const LINE: Rgba = [58, 38, 40, 255];
pub const HAIR_STYLES: u8 = 8;

/// Who someone looks like. Indices wrap, so any numbers make a face.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Face {
    pub skin: u8,
    pub hair_style: u8,
    pub hair_color: u8,
    pub eyes: u8,
    pub brows: u8,
    pub mouth: u8,
    /// Face width, about 0.94 to 1.06.
    pub width: f32,
    pub blush: bool,
    pub glasses: bool,
}

impl Default for Face {
    fn default() -> Self {
        Face::from_seed(0)
    }
}

impl Face {
    /// A face from a number (the same number, the same face).
    pub fn from_seed(seed: u64) -> Self {
        let mut x = seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0x2545_F491_4F6C_DD1D);
        let mut next = |n: u64| -> u64 {
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) % n
        };
        let hair_color = [0, 0, 0, 0, 0, 0, 1, 1, 1, 2, 2, 3, 4, 5][next(14) as usize];
        Face {
            skin: next(4) as u8,
            hair_style: next(HAIR_STYLES as u64) as u8,
            hair_color,
            eyes: next(6) as u8,
            brows: next(3) as u8,
            mouth: next(4) as u8,
            width: 0.94 + next(13) as f32 * 0.01,
            blush: next(10) < 3,
            glasses: next(100) < 8,
        }
    }

    fn key(&self) -> [u8; 9] {
        [
            self.skin,
            self.hair_style,
            self.hair_color,
            self.eyes,
            self.brows,
            self.mouth,
            (self.width * 100.0).round() as u8,
            self.blush as u8,
            self.glasses as u8,
        ]
    }
}

/// The expression: eyes, brows and mouth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Expr {
    #[default]
    Neutral,
    /// Eyes open, smiling.
    Happy,
    /// Eyes closed in a grin.
    Smile,
    Serious,
    Fiery,
    Sad,
    Surprised,
    Pain,
}

impl Expr {
    /// From a word in data ("smile", "angry", ...); unknown words are neutral.
    pub fn parse(s: &str) -> Self {
        match s {
            "happy" | "glad" => Expr::Happy,
            "smile" | "grin" | "laugh" => Expr::Smile,
            "serious" | "focus" => Expr::Serious,
            "fiery" | "angry" | "determined" => Expr::Fiery,
            "sad" | "worried" | "down" => Expr::Sad,
            "surprised" | "shock" => Expr::Surprised,
            "pain" | "hurt" => Expr::Pain,
            _ => Expr::Neutral,
        }
    }
}

/// What someone wears: a team cap (or hair), the shirt and its trim, long hair, an adult face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outfit {
    pub cap: Option<Rgba>,
    pub shirt: Rgba,
    pub trim: Rgba,
    pub long_hair: bool,
    pub adult: bool,
    /// A ribbon at the collar (a school blouse) instead of the trimmed V.
    pub ribbon: bool,
    /// Shoulders, neck and collar under the head. False draws the head alone, to sit on a
    /// body ([`crate::game_core::chibi`]).
    pub bust: bool,
}

impl Outfit {
    /// A player: team cap, white uniform with the team colour as trim.
    pub fn cap(team: Rgba) -> Self {
        Outfit {
            cap: Some(team),
            shirt: [246, 246, 250, 255],
            trim: team,
            long_hair: false,
            adult: false,
            ribbon: false,
            bust: true,
        }
    }

    /// Bare head, white uniform.
    pub fn bare(team: Rgba) -> Self {
        Outfit {
            cap: None,
            ..Outfit::cap(team)
        }
    }

    fn key(&self) -> [u8; 15] {
        let c = self.cap.unwrap_or([0, 0, 0, 0]);
        [
            c[0],
            c[1],
            c[2],
            self.cap.is_some() as u8,
            self.shirt[0],
            self.shirt[1],
            self.shirt[2],
            self.trim[0],
            self.trim[1],
            self.trim[2],
            self.long_hair as u8,
            self.adult as u8,
            self.ribbon as u8,
            self.bust as u8,
            0,
        ]
    }
}

type Key = ([u8; 9], Expr, [u8; 15], u16);

thread_local! {
    static CACHE: RefCell<HashMap<Key, Rc<Mesh2d>>> = RefCell::new(HashMap::new());
}

/// The portrait mesh for a `size` px square (origin at its top-left), cached.
pub fn mesh(size: f32, face: &Face, expr: Expr, outfit: &Outfit) -> Rc<Mesh2d> {
    let px = size.round().clamp(8.0, 2048.0) as u16;
    let key = (face.key(), expr, outfit.key(), px);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(m) = c.get(&key) {
            return m.clone();
        }
        if c.len() > 512 {
            c.clear();
        }
        let m = Rc::new(build(px as f32, face, expr, outfit));
        c.insert(key, m.clone());
        m
    })
}

/// Draw a portrait `size` px square with its top-left at (`x`, `y`).
pub fn draw(out: &mut Mesh2d, x: f32, y: f32, size: f32, face: &Face, expr: Expr, outfit: &Outfit) {
    draw_faded(out, x, y, size, face, expr, outfit, 1.0);
}

/// [`draw`] at an opacity (0..1), for faces fading in or listening in the background.
#[allow(clippy::too_many_arguments)]
pub fn draw_faded(
    out: &mut Mesh2d,
    x: f32,
    y: f32,
    size: f32,
    face: &Face,
    expr: Expr,
    outfit: &Outfit,
    alpha: f32,
) {
    let m = mesh(size, face, expr, outfit);
    let s = size / size.round().clamp(8.0, 2048.0);
    out.append_placed(&m, x, y, s, alpha);
}

// ---- building -------------------------------------------------------------------------------

struct Pen<'a> {
    m: &'a mut Mesh2d,
    lw: f32,
    /// Big enough for the small touches (iris shading, lower lids, seams, sheen). A face in a
    /// list row skips them: a third of the triangles, the same face at a glance.
    fine: bool,
}

thread_local! {
    /// Curve segments as a share of the full count (smaller faces need fewer).
    static DETAIL: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}

impl Pen<'_> {
    fn fill(&mut self, pts: &[P], c: Rgba) {
        vector::fill(self.m, pts, &Paint::Solid(c));
    }
    /// Fill and outline in ink.
    fn inked(&mut self, pts: &[P], c: Rgba, ink: Rgba) {
        self.fill(pts, c);
        vector::stroke_closed(self.m, pts, self.lw * 1.8, ink);
    }
    fn line(&mut self, pts: &[P], w0: f32, w1: f32, c: Rgba) {
        vector::stroke_taper(self.m, pts, w0, w1, c);
    }
    /// `shape` kept inside the convex `clip`, filled.
    fn inside(&mut self, shape: &[P], clip: &[P], c: Rgba) {
        let part = vector::clip_convex(shape, clip);
        if part.len() >= 3 {
            self.fill(&part, c);
        }
    }
}

fn ell(cx: f32, cy: f32, w: f32, h: f32) -> Vec<P> {
    vector::ellipse(cx, cy, w * 0.5, h * 0.5)
}

fn bez(a: P, b: P, c: P, n: usize) -> Vec<P> {
    let k = DETAIL.with(|d| d.get());
    vector::quad_bez(a, b, c, ((n as f32 * k).round() as usize).max(3))
}

fn face_pts(cx: f32, cy: f32, w: f32, h: f32) -> Vec<P> {
    let n = (64.0 * DETAIL.with(|d| d.get())).round().max(24.0) as usize;
    (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * TAU;
            let (mut x, mut y) = (a.cos(), a.sin());
            if y > 0.0 {
                x *= 1.0 - 0.36 * y.powf(2.2);
                y *= 0.56;
            } else {
                y *= 0.5;
            }
            [cx + x * w * 0.5, cy + y * h]
        })
        .collect()
}

/// The top of a head of hair, left to right: a rounded crown, with spikes (`jag`) or a quiff.
#[allow(clippy::too_many_arguments)]
fn dome(cx: f32, top: f32, w: f32, h: f32, spread: f32, n: usize, jag: f32, quiff: f32) -> Vec<P> {
    (0..=n)
        .map(|i| {
            let a = PI + i as f32 / n as f32 * PI;
            let r = 1.0 + if i % 2 == 1 { jag } else { 0.0 };
            let x = cx + a.cos() * w * spread * r;
            let mut y = top + h * 0.32 + a.sin() * h * 0.36 * r;
            y -= quiff * h * (-a.sin()).max(0.0) * (a - PI * 1.6).cos().max(0.0);
            [x, y]
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn eye(
    pen: &mut Pen,
    x: f32,
    y: f32,
    ew: f32,
    eh: f32,
    iris: Rgba,
    expr: Expr,
    side: f32,
    skin: Rgba,
) {
    let lw = pen.lw;
    match expr {
        Expr::Smile => {
            let arc = bez(
                [x - ew * 0.5, y + eh * 0.15],
                [x, y - eh * 0.6],
                [x + ew * 0.5, y + eh * 0.15],
                12,
            );
            pen.line(&arc, lw * 2.4, lw * 2.4, LINE);
            return;
        }
        Expr::Pain => {
            let inner = -side;
            let pts = [
                [x - inner * ew * 0.42, y - eh * 0.4],
                [x + inner * ew * 0.36, y],
                [x - inner * ew * 0.42, y + eh * 0.4],
            ];
            pen.line(&pts, lw * 2.3, lw * 2.3, LINE);
            return;
        }
        _ => {}
    }
    let k = if expr == Expr::Surprised { 1.12 } else { 1.0 };
    let (w2, h2) = (ew * k, eh * k);
    // The upper lid: its slant carries the mood.
    let mut inner = [x - side * w2 * 0.54, y - h2 * 0.04];
    let mut outer = [x + side * w2 * 0.58, y - h2 * 0.1];
    let mut ctrl_y = y - h2 * 0.72;
    match expr {
        Expr::Serious | Expr::Fiery => {
            let d = if expr == Expr::Fiery { 0.3 } else { 0.2 };
            inner[1] += -h2 * 0.05 + h2 * d * 0.4;
            ctrl_y += h2 * d;
        }
        Expr::Sad => {
            inner[1] -= h2 * 0.28;
            outer[1] += h2 * 0.12;
            ctrl_y += h2 * 0.1;
        }
        Expr::Surprised => ctrl_y -= h2 * 0.12,
        _ => {}
    }
    let lid = bez(inner, [x, ctrl_y], outer, 14);
    // Everything below the lid (a convex region: the lid is a parabola opening downward).
    let mut below = lid.clone();
    below.push([outer[0] + side * w2 * 0.4, y + h2 * 2.0]);
    below.push([inner[0] - side * w2 * 0.4, y + h2 * 2.0]);
    let white = vector::clip_convex(&ell(x, y, w2, h2), &below);
    if white.len() >= 3 {
        pen.fill(&white, [255, 255, 255, 255]);
        let irw = w2 * if expr == Expr::Surprised { 0.46 } else { 0.62 };
        let irh = h2 * 0.92;
        let (ix, iy) = (x - side * w2 * 0.05, y + h2 * 0.06);
        pen.inside(&ell(ix, iy, irw, irh), &white, iris);
        if pen.fine {
            pen.inside(
                &ell(ix, iy - irh * 0.25, irw, irh * 0.5),
                &white,
                shade(iris, -0.5),
            );
            pen.inside(
                &ell(ix, iy + irh * 0.28, irw * 0.6, irh * 0.32),
                &white,
                shade(iris, 0.4),
            );
        }
        pen.inside(
            &ell(ix, iy + irh * 0.01, irw * 0.4, irh * 0.42),
            &white,
            [16, 12, 18, 255],
        );
        pen.inside(
            &ell(ix - irw * 0.26, iy - irh * 0.18, irw * 0.4, irw * 0.4),
            &white,
            [255, 255, 255, 255],
        );
        if pen.fine {
            pen.inside(
                &ell(ix + irw * 0.18, iy + irh * 0.26, irw * 0.2, irw * 0.2),
                &white,
                [255, 255, 255, 255],
            );
        }
    }
    pen.line(&lid, lw * 1.4, lw * 2.6, LINE);
    if !pen.fine {
        return;
    }
    pen.line(
        &[outer, [outer[0] + side * w2 * 0.14, outer[1] - h2 * 0.12]],
        lw * 2.2,
        lw * 0.6,
        LINE,
    );
    let low = bez(
        [x - side * w2 * 0.1, y + h2 * 0.5],
        [x + side * w2 * 0.3, y + h2 * 0.52],
        [x + side * w2 * 0.5, y + h2 * 0.18],
        8,
    );
    pen.line(&low, lw * 0.6, lw * 1.2, shade(skin, -0.45));
}

#[allow(clippy::too_many_arguments)]
fn brows(pen: &mut Pen, cx: f32, y: f32, gap: f32, bw: f32, color: Rgba, expr: Expr, kind: u8) {
    let tilt = match expr {
        Expr::Serious => 0.22,
        Expr::Fiery => 0.34,
        Expr::Sad => -0.26,
        Expr::Surprised => -0.1,
        Expr::Pain => 0.24,
        _ => 0.0,
    };
    let lift = if expr == Expr::Surprised {
        bw * 0.18
    } else {
        0.0
    };
    let thick = pen.lw * (2.6 + (kind % 3) as f32 * 0.45);
    for side in [-1.0f32, 1.0] {
        let (x0, x1) = (cx + side * gap, cx + side * (gap + bw));
        let (y0, y1) = (y + bw * tilt - lift, y - bw * 0.04 - lift);
        let mid = [(x0 + x1) * 0.5, y0.min(y1) - bw * 0.12];
        pen.line(&bez([x0, y0], mid, [x1, y1], 8), thick, thick * 0.35, color);
    }
}

fn mouth(pen: &mut Pen, cx: f32, y: f32, mw: f32, expr: Expr, kind: u8) {
    let lw = pen.lw;
    let ink = shade(LINE, 0.1);
    let red = [150, 48, 62, 255];
    match expr {
        Expr::Smile | Expr::Happy => {
            let depth = mw * if expr == Expr::Smile { 0.5 } else { 0.38 };
            let curve = bez(
                [cx - mw * 0.5, y],
                [cx, y + depth * 2.0],
                [cx + mw * 0.5, y],
                12,
            );
            pen.fill(&curve, red);
            let tongue = ell(cx, y + depth * 0.72, mw * 0.48, depth * 0.55);
            pen.inside(&tongue, &curve, [236, 118, 126, 255]);
            vector::stroke_closed(pen.m, &curve, lw * 1.1, ink);
        }
        Expr::Surprised => {
            pen.fill(&ell(cx, y + mw * 0.17, mw * 0.4, mw * 0.46), ink);
            pen.fill(&ell(cx, y + mw * 0.17, mw * 0.32, mw * 0.38), red);
        }
        Expr::Sad => {
            let c = bez(
                [cx - mw * 0.3, y + mw * 0.16],
                [cx, y - mw * 0.14],
                [cx + mw * 0.3, y + mw * 0.16],
                8,
            );
            pen.line(&c, lw * 1.3, lw * 1.3, ink);
        }
        Expr::Pain => {
            let (w, h) = (mw * 0.62, mw * 0.24);
            let r = vector::rounded_rect(cx - w * 0.5, y + mw * 0.06 - h * 0.5, w, h, h * 0.4);
            pen.fill(&r, [250, 250, 250, 255]);
            vector::stroke_closed(pen.m, &r, lw, ink);
            pen.line(
                &[[cx - w * 0.5, y + mw * 0.06], [cx + w * 0.5, y + mw * 0.06]],
                lw * 0.7,
                lw * 0.7,
                ink,
            );
        }
        Expr::Fiery => {
            let pts = [
                [cx - mw * 0.44, y],
                [cx + mw * 0.44, y],
                [cx + mw * 0.3, y + mw * 0.28],
                [cx - mw * 0.3, y + mw * 0.28],
            ];
            pen.fill(&pts, red);
            let teeth = [
                [cx - mw * 0.4, y + 0.5],
                [cx + mw * 0.4, y + 0.5],
                [cx + mw * 0.37, y + mw * 0.09],
                [cx - mw * 0.37, y + mw * 0.09],
            ];
            pen.fill(&teeth, [252, 252, 252, 255]);
            vector::stroke_closed(pen.m, &pts, lw * 1.1, ink);
        }
        Expr::Serious => pen.line(
            &[
                [cx - mw * 0.22, y + mw * 0.06],
                [cx + mw * 0.22, y + mw * 0.06],
            ],
            lw * 1.3,
            lw * 1.3,
            ink,
        ),
        Expr::Neutral => {
            let c = if kind % 2 == 1 {
                bez(
                    [cx - mw * 0.26, y],
                    [cx, y + mw * 0.2],
                    [cx + mw * 0.26, y],
                    8,
                )
            } else {
                bez(
                    [cx - mw * 0.2, y + mw * 0.04],
                    [cx, y + mw * 0.12],
                    [cx + mw * 0.2, y + mw * 0.04],
                    6,
                )
            };
            pen.line(&c, lw * 1.25, lw * 1.25, ink);
        }
    }
}

fn hair_front(style: u8, cx: f32, cy: f32, w: f32, h: f32) -> Vec<Vec<P>> {
    let top = cy - h * 0.58;
    let with = |mut a: Vec<P>, b: &[P]| {
        a.extend_from_slice(b);
        vec![a]
    };
    match style % HAIR_STYLES {
        0 => with(
            dome(cx, top, w, h, 0.57, 14, 0.12, 0.0),
            &[
                [cx + w * 0.56, cy - h * 0.04],
                [cx + w * 0.42, cy - h * 0.22],
                [cx + w * 0.15, cy - h * 0.3],
                [cx - w * 0.12, cy - h * 0.24],
                [cx - w * 0.4, cy - h * 0.2],
                [cx - w * 0.56, cy - h * 0.04],
            ],
        ),
        1 => with(
            dome(cx, top, w, h, 0.57, 18, 0.0, 0.0),
            &[
                [cx + w * 0.57, cy - h * 0.06],
                [cx + w * 0.3, cy - h * 0.2],
                [cx + w * 0.05, cy - h * 0.3],
                [cx - w * 0.3, cy - h * 0.18],
                [cx - w * 0.57, cy + h * 0.02],
            ],
        ),
        2 => with(
            dome(cx, top, w, h, 0.58, 18, 0.0, 0.0),
            &[
                [cx + w * 0.58, cy + h * 0.06],
                [cx + w * 0.36, cy - h * 0.14],
                [cx + w * 0.22, cy - h * 0.08],
                [cx + w * 0.1, cy - h * 0.18],
                [cx - w * 0.06, cy - h * 0.1],
                [cx - w * 0.2, cy - h * 0.2],
                [cx - w * 0.38, cy - h * 0.1],
                [cx - w * 0.58, cy + h * 0.06],
            ],
        ),
        3 => with(
            dome(cx, top + h * 0.04, w, h, 0.53, 18, 0.0, 0.0),
            &[
                [cx + w * 0.53, cy - h * 0.18],
                [cx, cy - h * 0.3],
                [cx - w * 0.53, cy - h * 0.18],
            ],
        ),
        4 => with(
            dome(cx, top, w, h, 0.57, 18, 0.0, 0.1),
            &[
                [cx + w * 0.57, cy - h * 0.08],
                [cx + w * 0.1, cy - h * 0.3],
                [cx - w * 0.2, cy - h * 0.26],
                [cx - w * 0.57, cy - h * 0.06],
            ],
        ),
        5 => with(
            dome(cx, top, w, h, 0.6, 16, 0.08, 0.0),
            &[
                [cx + w * 0.6, cy + h * 0.16],
                [cx + w * 0.38, cy - h * 0.12],
                [cx + w * 0.14, cy - h * 0.2],
                [cx - w * 0.1, cy - h * 0.12],
                [cx - w * 0.36, cy - h * 0.16],
                [cx - w * 0.6, cy + h * 0.16],
            ],
        ),
        6 => with(
            dome(cx, top, w, h, 0.57, 18, 0.0, 0.0),
            &[
                [cx + w * 0.57, cy + h * 0.02],
                [cx + w * 0.3, cy - h * 0.24],
                [cx + w * 0.04, cy - h * 0.36],
                [cx - w * 0.04, cy - h * 0.36],
                [cx - w * 0.3, cy - h * 0.24],
                [cx - w * 0.57, cy + h * 0.02],
            ],
        ),
        _ => {
            // Curls: a band of round tufts.
            let mut out = vec![ell(cx, top + h * 0.16, w * 1.04, h * 0.24)];
            for i in 0..9 {
                let x = cx - w * 0.5 + i as f32 * w / 8.0;
                out.push(ell(
                    x,
                    top + h * 0.08 + (i % 2) as f32 * h * 0.05,
                    w * 0.28,
                    h * 0.28,
                ));
            }
            out
        }
    }
}

fn long_back(cx: f32, cy: f32, w: f32, h: f32) -> Vec<P> {
    let mut pts = dome(cx, cy - h * 0.62, w, h, 0.64, 20, 0.0, 0.0);
    let right = bez(
        [cx + w * 0.64, cy - h * 0.3],
        [cx + w * 0.8, cy + h * 0.3],
        [cx + w * 0.7, cy + h * 0.82],
        12,
    );
    pts.extend_from_slice(&right[1..]);
    pts.extend_from_slice(&[
        [cx + w * 0.5, cy + h * 0.74],
        [cx + w * 0.3, cy + h * 0.8],
        [cx, cy + h * 0.74],
        [cx - w * 0.3, cy + h * 0.8],
        [cx - w * 0.5, cy + h * 0.74],
    ]);
    pts.extend(bez(
        [cx - w * 0.7, cy + h * 0.82],
        [cx - w * 0.8, cy + h * 0.3],
        [cx - w * 0.64, cy - h * 0.3],
        12,
    ));
    pts
}

fn long_bangs(cx: f32, cy: f32, w: f32, h: f32) -> Vec<P> {
    let mut pts = dome(cx, cy - h * 0.6, w, h, 0.59, 20, 0.0, 0.0);
    let tips = [
        [cx + w * 0.58, cy + h * 0.02],
        [cx + w * 0.4, cy - h * 0.08],
        [cx + w * 0.18, cy - h * 0.1],
        [cx - w * 0.04, cy - h * 0.06],
        [cx - w * 0.26, cy - h * 0.1],
        [cx - w * 0.46, cy - h * 0.04],
        [cx - w * 0.59, cy + h * 0.04],
    ];
    for pair in tips.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let mid = [(a[0] + b[0]) * 0.5, a[1].min(b[1]) - h * 0.16];
        let seg = bez(a, mid, b, 6);
        pts.extend_from_slice(&seg[..seg.len() - 1]);
    }
    pts.push(tips[tips.len() - 1]);
    pts
}

fn cap(pen: &mut Pen, cx: f32, cy: f32, w: f32, h: f32, color: Rgba) {
    let lw = pen.lw;
    let band = cy - h * 0.26;
    let crown_top = cy - h * 0.8;
    let cw = w * 0.56;
    let mut crown = vec![[cx - cw, band]];
    let n = if pen.fine { 24 } else { 12 };
    for i in 1..n {
        let a = PI + i as f32 / n as f32 * PI;
        crown.push([
            cx + a.cos() * cw,
            band + a.sin() * (band - crown_top) * (0.92 + 0.08 * a.cos().abs()),
        ]);
    }
    crown.push([cx + cw, band]);
    pen.fill(&crown, color);
    // Light from the upper left: the right side in shadow, a soft sheen on the left.
    pen.inside(
        &ell(
            cx + cw * 0.7,
            crown_top + (band - crown_top) * 0.7,
            cw * 1.2,
            (band - crown_top) * 1.4,
        ),
        &crown,
        shade(color, -0.22),
    );
    if pen.fine {
        pen.inside(
            &ell(
                cx - cw * 0.45,
                crown_top + h * 0.06 + (band - crown_top) * 0.27,
                cw * 0.7,
                (band - crown_top) * 0.55,
            ),
            &crown,
            mix(color, [255, 255, 255, 255], 0.24),
        );
    }
    let seam = shade(color, -0.35);
    pen.line(
        &bez(
            [cx, crown_top + 1.0],
            [cx - cw * 0.02, crown_top + h * 0.1],
            [cx, band - h * 0.02],
            8,
        ),
        lw * 0.9,
        lw * 0.9,
        seam,
    );
    for side in [-1.0f32, 1.0] {
        if !pen.fine {
            break;
        }
        let s = bez(
            [cx, crown_top + 1.0],
            [cx + side * cw * 0.62, crown_top + h * 0.1],
            [cx + side * cw * 0.86, band - h * 0.02],
            10,
        );
        pen.line(&s, lw * 0.7, lw * 0.7, seam);
    }
    vector::stroke_closed(pen.m, &crown, lw * 1.8, LINE);
    if pen.fine {
        pen.inked(
            &ell(cx, crown_top, lw * 4.8, lw * 3.2),
            shade(color, -0.2),
            LINE,
        );
    }
    // The brim, curving toward the viewer.
    let mut brim = bez(
        [cx - cw * 1.08, band - h * 0.01],
        [cx, band + h * 0.2],
        [cx + cw * 1.08, band - h * 0.01],
        16,
    );
    let under = bez(
        [cx + cw * 1.08, band - h * 0.01],
        [cx, band + h * 0.03],
        [cx - cw * 1.08, band - h * 0.01],
        16,
    );
    brim.extend_from_slice(&under[1..under.len() - 1]);
    pen.inked(&brim, shade(color, -0.28), LINE);
    if !pen.fine {
        return;
    }
    pen.line(
        &bez(
            [cx - cw * 0.9, band + h * 0.02],
            [cx, band + h * 0.08],
            [cx + cw * 0.9, band + h * 0.02],
            10,
        ),
        lw * 0.8,
        lw * 0.8,
        shade(color, 0.18),
    );
}

/// Build a portrait for a `size` px square (origin top-left).
pub fn build(size: f32, face: &Face, expr: Expr, outfit: &Outfit) -> Mesh2d {
    let mut m = Mesh2d::default();
    let big = size.max(8.0);
    let fine = big >= 64.0;
    DETAIL.with(|d| d.set((big / 96.0).clamp(0.4, 1.0)));
    let mut pen = Pen {
        m: &mut m,
        lw: (big * 0.011).max(0.6),
        fine,
    };
    let lw = pen.lw;
    let skin = SKINS[face.skin as usize % SKINS.len()];
    let hair = HAIRS[face.hair_color as usize % HAIRS.len()];
    let hair_ink = shade(hair, -0.55);
    let iris = EYES[face.eyes as usize % EYES.len()];
    let (cx, cy) = (big * 0.5, big * 0.47);
    let (w, h) = (big * 0.5 * face.width.clamp(0.8, 1.2), big * 0.57);
    let square = [[0.0, 0.0], [big, 0.0], [big, big], [0.0, big]];
    let capped = outfit.cap.is_some() && !outfit.long_hair;

    if outfit.long_hair {
        pen.inked(&long_back(cx, cy, w, h), hair, hair_ink);
    }
    if outfit.bust {
        // Shoulders: the shirt, clipped to the square, shaded on the right, raglan seams.
        let body = ell(cx, big * 1.08, big * 0.94, big * 0.56);
        let shirt = vector::clip_convex(&body, &square);
        pen.fill(&shirt, outfit.shirt);
        let dark = if outfit.shirt.iter().take(3).all(|&c| c > 200) {
            -0.1
        } else {
            -0.25
        };
        pen.inside(
            &ell(cx + big * 0.3, big * 1.08, big * 0.5, big * 0.56),
            &shirt,
            shade(outfit.shirt, dark),
        );
        for side in [-1.0f32, 1.0] {
            let seam: Vec<P> = bez(
                [cx + side * w * 0.3, big * 0.83],
                [cx + side * big * 0.26, big * 0.86],
                [cx + side * big * 0.36, big * 1.0],
                8,
            );
            pen.line(&seam, lw * 1.8, lw * 1.8, outfit.trim);
        }
        let rim: Vec<P> = body.iter().copied().filter(|p| p[1] <= big - 0.5).collect();
        let rim = order_by(rim, 0);
        pen.line(&rim, lw * 1.8, lw * 1.8, LINE);
        // The neck with the chin's shadow.
        let neck = [
            [cx - w * 0.17, cy + h * 0.3],
            [cx + w * 0.17, cy + h * 0.3],
            [cx + w * 0.17, big * 0.86 + h * 0.08],
            [cx - w * 0.17, big * 0.86 + h * 0.08],
        ];
        pen.fill(&neck, skin);
        pen.inside(
            &ell(cx, cy + h * 0.36, w * 0.6, h * 0.36),
            &neck,
            shade(skin, -0.16),
        );
        vector::stroke_closed(pen.m, &neck, lw * 1.8, LINE);
        // The collar.
        let (v_top, v_bot) = (big * 0.82, big * 0.93);
        if outfit.ribbon {
            let col = [
                [cx - w * 0.42, v_top - big * 0.01],
                [cx - w * 0.2, v_top + big * 0.06],
                [cx, v_bot - big * 0.04],
                [cx + w * 0.2, v_top + big * 0.06],
                [cx + w * 0.42, v_top - big * 0.01],
                [cx, v_bot],
            ];
            pen.inked(&col, [250, 250, 252, 255], LINE);
            let bow = [
                [cx - w * 0.1, v_bot - big * 0.035],
                [cx + w * 0.1, v_bot - big * 0.035],
                [cx + w * 0.14, v_bot + big * 0.03],
                [cx, v_bot],
                [cx - w * 0.14, v_bot + big * 0.03],
            ];
            pen.inked(&bow, [220, 60, 76, 255], LINE);
        } else {
            let v = [
                [cx - w * 0.38, v_top],
                [cx - w * 0.3, v_top - big * 0.01],
                [cx, v_bot - big * 0.045],
                [cx + w * 0.3, v_top - big * 0.01],
                [cx + w * 0.38, v_top],
                [cx, v_bot],
            ];
            pen.inked(&v, outfit.trim, LINE);
        }
    }
    if outfit.cap.is_none() && !outfit.long_hair {
        let back = if matches!(face.hair_style % HAIR_STYLES, 2 | 5) {
            ell(cx, cy - h * 0.62 + h * 0.54, w * 1.2, h * 1.08)
        } else {
            ell(cx, cy - h * 0.62 + h * 0.43, w * 1.14, h * 0.86)
        };
        pen.inked(&back, hair, hair_ink);
    }
    // Ears, then the face over them.
    for side in [-1.0f32, 1.0] {
        let ex = cx + side * w * 0.48;
        pen.inked(&ell(ex, cy + h * 0.06, w * 0.2, h * 0.24), skin, LINE);
        let fold = bez(
            [ex + side * w * 0.03, cy - h * 0.01],
            [ex + side * w * 0.05, cy + h * 0.06],
            [ex - side * w * 0.01, cy + h * 0.12],
            6,
        );
        if fine {
            pen.line(&fold, lw * 0.9, lw * 0.9, shade(skin, -0.25));
        }
    }
    let fpts = face_pts(cx, cy, w, h);
    pen.fill(&fpts, skin);
    // Shadow under the cap or the fringe, and a soft crescent down the right cheek.
    let shadow_top = if capped { cy - h * 0.22 } else { cy - h * 0.3 };
    let band = [
        [cx - w, cy - h],
        [cx + w, cy - h],
        [cx + w, shadow_top],
        [cx - w, shadow_top],
    ];
    pen.inside(&band, &fpts, shade(skin, -0.12));
    let right: Vec<P> = fpts
        .iter()
        .copied()
        .filter(|p| p[0] > cx + w * 0.12 && p[1] > cy - h * 0.3)
        .collect();
    if fine && right.len() > 3 {
        let mut cres = order_by(right, 1);
        let inner: Vec<P> = cres.iter().rev().map(|p| [p[0] - w * 0.08, p[1]]).collect();
        cres.extend(inner);
        pen.fill(&cres, mix(skin, [236, 196, 196, 255], 0.45));
    }
    vector::stroke_closed(pen.m, &fpts, lw * 1.8, LINE);
    if face.blush || outfit.long_hair {
        for side in [-1.0f32, 1.0] {
            let bx = cx + side * w * 0.27;
            pen.fill(
                &ell(bx, cy + h * 0.19, w * 0.22, h * 0.08),
                [255, 120, 132, 110],
            );
            for k in 0..if fine { 3 } else { 0 } {
                let x = bx - w * 0.05 + k as f32 * w * 0.045;
                pen.line(
                    &[[x + w * 0.02, cy + h * 0.165], [x, cy + h * 0.205]],
                    lw * 0.6,
                    lw * 0.6,
                    [232, 110, 120, 150],
                );
            }
        }
    }
    // Eyes, brows, nose, mouth.
    let ey = cy + h * 0.06;
    let (ew, eh) = (w * 0.24, h * 0.19);
    for side in [-1.0f32, 1.0] {
        eye(
            &mut pen,
            cx + side * w * 0.21,
            ey,
            ew,
            eh,
            iris,
            expr,
            side,
            skin,
        );
    }
    brows(
        &mut pen,
        cx,
        ey - eh * 1.02,
        w * 0.07,
        w * 0.25,
        shade(hair, -0.15),
        expr,
        face.brows,
    );
    pen.line(
        &[
            [cx + w * 0.025, cy + h * 0.17],
            [cx - w * 0.012, cy + h * 0.22],
        ],
        lw * 1.1,
        lw * 0.5,
        shade(skin, -0.3),
    );
    mouth(&mut pen, cx, cy + h * 0.31, w * 0.3, expr, face.mouth);
    if outfit.adult && fine {
        for side in [-1.0f32, 1.0] {
            pen.line(
                &[
                    [cx + side * w * 0.16, cy + h * 0.24],
                    [cx + side * w * 0.2, cy + h * 0.32],
                ],
                lw * 0.9,
                lw * 0.3,
                shade(skin, -0.25),
            );
            pen.line(
                &[
                    [cx + side * w * 0.14, ey + eh * 0.62],
                    [cx + side * w * 0.28, ey + eh * 0.58],
                ],
                lw * 0.7,
                lw * 0.3,
                shade(skin, -0.2),
            );
        }
    }
    if face.glasses {
        let gl = [52, 52, 66, 255];
        for side in [-1.0f32, 1.0] {
            let (gw, gh) = (ew * 1.5, eh * 1.4);
            let (gx, gy) = (cx + side * w * 0.21 - gw * 0.5, ey + eh * 0.05 - gh * 0.5);
            let frame = vector::rounded_rect(gx, gy, gw, gh, eh * 0.45);
            vector::stroke_closed(pen.m, &frame, lw * 1.1, gl);
            if !fine {
                continue;
            }
            pen.line(
                &[
                    [gx + gw * 0.62, gy + gh * 0.2],
                    [gx + gw * 0.4, gy + gh * 0.62],
                ],
                lw * 0.8,
                lw * 0.8,
                [255, 255, 255, 200],
            );
        }
        pen.line(
            &[
                [cx - w * 0.07, ey - eh * 0.1],
                [cx + w * 0.07, ey - eh * 0.1],
            ],
            lw,
            lw,
            gl,
        );
    }
    // Hair in front, or the cap.
    if capped {
        for side in [-1.0f32, 1.0] {
            let burn = [
                [cx + side * w * 0.53, cy - h * 0.26],
                [cx + side * w * 0.3, cy - h * 0.24],
                [cx + side * w * 0.42, cy - h * 0.12],
                [cx + side * w * 0.46, cy + h * 0.02],
                [cx + side * w * 0.53, cy - h * 0.08],
            ];
            pen.inked(&burn, hair, hair_ink);
        }
        let fringe = [
            [cx - w * 0.3, cy - h * 0.27],
            [cx - w * 0.18, cy - h * 0.16],
            [cx - w * 0.08, cy - h * 0.24],
            [cx + w * 0.06, cy - h * 0.17],
            [cx + w * 0.18, cy - h * 0.25],
            [cx + w * 0.3, cy - h * 0.27],
        ];
        pen.inked(&fringe, hair, hair_ink);
        cap(
            &mut pen,
            cx,
            cy,
            w,
            h,
            outfit.cap.unwrap_or([28, 72, 160, 255]),
        );
    } else {
        let parts = if outfit.long_hair {
            vec![long_bangs(cx, cy, w, h)]
        } else {
            hair_front(face.hair_style, cx, cy, w, h)
        };
        // Fill, ink, fill again: the second fill hides the inner half of the ink, so curls
        // that overlap read as one outline.
        for p in &parts {
            pen.fill(p, hair);
        }
        for p in &parts {
            vector::stroke_closed(pen.m, p, lw * 1.8, hair_ink);
        }
        for p in &parts {
            pen.fill(p, hair);
        }
        if fine && (outfit.long_hair || face.hair_style % HAIR_STYLES != 3) {
            let top = cy - h * 0.58;
            let sheen = bez(
                [cx - w * 0.34, top + h * 0.16],
                [cx, top + h * 0.04],
                [cx + w * 0.34, top + h * 0.16],
                10,
            );
            pen.line(&sheen, lw * 2.2, lw * 0.6, shade(hair, 0.32));
        }
        if outfit.long_hair {
            for side in [-1.0f32, 1.0] {
                let mut lock = bez(
                    [cx + side * w * 0.52, cy - h * 0.34],
                    [cx + side * w * 0.66, cy + h * 0.1],
                    [cx + side * w * 0.5, cy + h * 0.5],
                    10,
                );
                let inner = bez(
                    [cx + side * w * 0.5, cy + h * 0.5],
                    [cx + side * w * 0.4, cy + h * 0.12],
                    [cx + side * w * 0.38, cy - h * 0.2],
                    10,
                );
                lock.extend_from_slice(&inner[1..]);
                pen.inked(&lock, hair, hair_ink);
            }
            let clip = vector::star(cx + w * 0.3, cy - h * 0.34, w * 0.09, w * 0.04, 5);
            pen.fill(&clip, [255, 206, 72, 255]);
            vector::stroke_closed(pen.m, &clip, lw * 0.9, [190, 120, 30, 255]);
        }
    }
    m
}

/// Points sorted along x (`axis` 0) or y (1): one side of a convex shape as a path.
fn order_by(mut pts: Vec<P>, axis: usize) -> Vec<P> {
    pts.sort_by(|a, b| a[axis].total_cmp(&b[axis]));
    pts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_from_seeds_are_stable_and_varied() {
        assert_eq!(Face::from_seed(7), Face::from_seed(7));
        let styles: std::collections::HashSet<u8> =
            (0..64).map(|s| Face::from_seed(s).hair_style).collect();
        assert!(styles.len() >= 6);
        let f = Face::from_seed(3);
        assert!((0.94..=1.06).contains(&f.width));
    }

    #[test]
    fn a_portrait_stays_in_its_square_and_caches() {
        let face = Face::from_seed(11);
        for expr in [
            Expr::Neutral,
            Expr::Happy,
            Expr::Smile,
            Expr::Serious,
            Expr::Fiery,
            Expr::Sad,
            Expr::Surprised,
            Expr::Pain,
        ] {
            for outfit in [
                Outfit::cap([200, 40, 50, 255]),
                Outfit::bare([28, 72, 160, 255]),
            ] {
                let m = build(120.0, &face, expr, &outfit);
                let b = m.bounds().unwrap();
                assert!(
                    b[0] > -4.0 && b[1] > -4.0 && b[2] < 124.0 && b[3] < 123.0,
                    "{expr:?} {b:?}"
                );
                assert!(m.triangles() > 200);
            }
        }
        let a = mesh(64.0, &face, Expr::Happy, &Outfit::cap([1, 2, 3, 255]));
        let b = mesh(64.0, &face, Expr::Happy, &Outfit::cap([1, 2, 3, 255]));
        assert!(Rc::ptr_eq(&a, &b));
    }

    #[test]
    fn expressions_change_the_face() {
        let face = Face::from_seed(5);
        let o = Outfit::cap([28, 72, 160, 255]);
        assert_ne!(
            build(96.0, &face, Expr::Neutral, &o),
            build(96.0, &face, Expr::Smile, &o)
        );
        assert_eq!(Expr::parse("angry"), Expr::Fiery);
        assert_eq!(Expr::parse("??"), Expr::Neutral);
    }

    #[test]
    fn draw_places_the_cached_mesh() {
        let mut m = Mesh2d::default();
        let mut long = Outfit::bare([236, 112, 124, 255]);
        long.long_hair = true;
        long.ribbon = true;
        draw(
            &mut m,
            100.0,
            50.0,
            80.0,
            &Face::from_seed(1),
            Expr::Happy,
            &long,
        );
        let b = m.bounds().unwrap();
        assert!(
            b[0] >= 96.0 && b[1] >= 46.0 && b[2] <= 184.0 && b[3] <= 134.0,
            "{b:?}"
        );
    }
}
