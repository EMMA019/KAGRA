//! The pictures: the sky by season, a ballpark full of people, the board of practice
//! squares, the practice ground with the club at their drills. All drawn with `hud_art` /
//! `vector`, in design units on the stage (400 wide). The game itself is 3D
//! (`hud::stadium`).

use super::board::Square;
use super::team::{Drill, Mood, Player, Role};
use crate::game_core::chibi::{self, Facing, Kit, Pose};
use crate::game_core::portrait::{Expr, Face, Outfit};
use crate::hud_art::Painter;
use crate::scene::{DrawList, Vert2};
use crate::vector::{self, mix, shade, Paint, Rgba, P};

pub const INK: Rgba = [34, 38, 52, 255];
pub const INK_SOFT: Rgba = [96, 104, 124, 255];
pub const WHITE: Rgba = [255, 255, 255, 255];
pub const ORANGE: Rgba = [255, 138, 48, 255];
pub const GOLD: Rgba = [255, 200, 60, 255];
pub const GRASS: Rgba = [84, 160, 80, 255];
pub const GRASS2: Rgba = [96, 172, 88, 255];
pub const DIRT: Rgba = [204, 152, 102, 255];
/// Our school's colour (caps, trim, our side of the scoreboard).
pub const OURS: Rgba = [34, 78, 168, 255];
/// One paper, one ink, one accent. Screens share these instead of one-off greys.
pub const PAPER: Rgba = [255, 253, 246, 255];
pub const PAPER_SHADE: Rgba = [244, 236, 220, 255];
pub const PAPER_EDGE: Rgba = [214, 196, 168, 255];
pub const NAVY: Rgba = [44, 58, 104, 255];
pub const QUIET: Rgba = [110, 116, 140, 255];
pub const RADIUS: f32 = 16.0;
pub const TYPE_TITLE: f32 = 22.0;
pub const TYPE_BODY: f32 = 15.0;
pub const TYPE_CAPTION: f32 = 12.0;
pub const TYPE_BUTTON: f32 = 16.0;

/// A rival school's colour from its name.
pub fn team_color(name: &str) -> Rgba {
    const C: [Rgba; 8] = [
        [176, 40, 52, 255],
        [24, 120, 100, 255],
        [210, 120, 30, 255],
        [110, 60, 150, 255],
        [30, 130, 170, 255],
        [60, 64, 80, 255],
        [150, 110, 40, 255],
        [200, 70, 120, 255],
    ];
    C[(hash(name) % C.len() as u64) as usize]
}

/// Looks only: about one player in four throws and bats left (the 3D figures, the side
/// of the plate, and which corner is inside).
pub fn bats_left(name: &str) -> bool {
    hash(name).is_multiple_of(4)
}

pub fn hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// A face for a name, kept inside a small palette so the club does not look random.
pub fn face_of(name: &str) -> Face {
    restrained(hash(name))
}

pub fn player_face(p: &Player) -> Face {
    match p.role {
        Role::Ace => ace_face(),
        Role::Captain => captain_face(),
        Role::Regular => restrained(hash(&format!("{}{}", p.family, p.given))),
    }
}

/// The founding ace: short black hair, sharp eyes, no glasses.
pub fn ace_face() -> Face {
    Face {
        skin: 1,
        hair_style: 0,
        hair_color: 0,
        eyes: 2,
        brows: 2,
        mouth: 0,
        width: 0.98,
        blush: false,
        glasses: false,
    }
}

/// The founding captain: a slightly wider face, warm eyes, brown hair.
pub fn captain_face() -> Face {
    Face {
        skin: 0,
        hair_style: 3,
        hair_color: 1,
        eyes: 1,
        brows: 0,
        mouth: 1,
        width: 1.02,
        blush: false,
        glasses: false,
    }
}

fn restrained(seed: u64) -> Face {
    let mut f = Face::from_seed(seed);
    f.skin %= 3;
    f.hair_style %= 5;
    f.hair_color = [0, 0, 0, 1, 1, 2][(seed % 6) as usize];
    f.eyes %= 4;
    f.brows %= 3;
    f.mouth %= 3;
    f.width = 0.97 + (seed % 6) as f32 * 0.01;
    f.glasses = seed.is_multiple_of(28);
    f.blush = seed.is_multiple_of(6);
    f
}

pub fn mood_expr(m: Mood) -> Expr {
    match m {
        Mood::Great => Expr::Smile,
        Mood::Good => Expr::Happy,
        Mood::Normal => Expr::Neutral,
        Mood::Poor => Expr::Serious,
        Mood::Awful => Expr::Sad,
    }
}

/// The manager: an original girl in a coral club jacket.
pub fn manager() -> (Face, Outfit) {
    let face = Face {
        skin: 0,
        hair_style: 2,
        hair_color: 1,
        eyes: 1,
        brows: 0,
        mouth: 1,
        width: 0.96,
        blush: true,
        glasses: false,
    };
    let mut o = Outfit::bare([236, 112, 124, 255]);
    o.shirt = [236, 112, 124, 255];
    o.trim = WHITE;
    o.long_hair = true;
    o.ribbon = true;
    (face, o)
}

pub fn drill_color(d: Drill) -> Rgba {
    let c = Square::Train(d).color();
    [c[0] as u8, c[1] as u8, c[2] as u8, 255]
}

pub fn square_color(sq: Square) -> Rgba {
    let c = sq.color();
    [c[0] as u8, c[1] as u8, c[2] as u8, 255]
}

/// Sky colours (top, bottom) for a month.
pub fn sky(month: u8) -> (Rgba, Rgba) {
    match month {
        3..=5 => ([150, 190, 240, 255], [255, 224, 232, 255]),
        6..=8 => ([70, 140, 226, 255], [255, 216, 168, 255]),
        9..=11 => ([120, 120, 196, 255], [255, 192, 140, 255]),
        _ => ([110, 140, 184, 255], [226, 232, 246, 255]),
    }
}

/// The whole screen: a sky gradient, drifting soft clouds, the ground below.
pub fn backdrop(p: &mut Painter, month: u8, _t: f32) {
    let [x0, y0, w, h] = p.st.screen;
    let (top, bottom) = sky(month);
    let rect = [[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]];
    p.fill(&rect, Paint::vertical(y0, y0 + h, top, bottom));
    // Painted countryside behind the week. The board and the cards cover the lower part.
    super::sprites::country(p, x0, y0, w, (h * 0.46).max(160.0));
}

/// A ballpark at dusk across the whole screen (the title, the year's end).
pub fn ballpark(p: &mut Painter, t: f32) {
    let [x0, y0, w, h] = p.st.screen;
    let rect = [[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]];
    let horizon = y0 + h * 0.52;
    p.fill(
        &rect,
        Paint::vertical(y0, horizon, [72, 128, 214, 255], [255, 200, 150, 255]),
    );
    super::sprites::country(p, x0, y0, w, h * 0.48);
    let cx = x0 + w * 0.5;
    p.glow(cx + 90.0, horizon - 40.0, 260.0, [255, 226, 170, 210]);
    p.dot(cx + 90.0, horizon - 40.0, 40.0, [255, 240, 210, 255]);
    for (i, cy) in [90.0f32, 170.0].into_iter().enumerate() {
        let x = x0 + (i as f32 * 300.0 + t * (4.0 + i as f32 * 3.0)).rem_euclid(w + 300.0) - 150.0;
        p.glow(x, y0 + cy, 80.0, [255, 255, 255, 110]);
        p.glow(x + 60.0, y0 + cy + 8.0, 60.0, [255, 255, 255, 100]);
    }
    // Light towers.
    for lx in [cx - 170.0, cx + 170.0] {
        p.fill(
            &vector::rounded_rect(lx - 3.0, horizon - 230.0, 6.0, 200.0, 2.0),
            Paint::Solid([36, 42, 66, 255]),
        );
        p.fill(
            &vector::rounded_rect(lx - 34.0, horizon - 250.0, 68.0, 30.0, 5.0),
            Paint::Solid([48, 54, 80, 255]),
        );
        p.glow(lx, horizon - 236.0, 110.0, [255, 250, 220, 120]);
        for i in 0..4 {
            for j in 0..2 {
                p.dot(
                    lx - 24.0 + i as f32 * 16.0,
                    horizon - 243.0 + j as f32 * 13.0,
                    5.0,
                    [255, 252, 226, 255],
                );
            }
        }
    }
    // The stands with the crowd.
    let stand = [
        [x0, horizon + 30.0],
        [cx - 220.0, horizon - 12.0],
        [cx, horizon - 26.0],
        [cx + 220.0, horizon - 12.0],
        [x0 + w, horizon + 30.0],
        [x0 + w, horizon + 60.0],
        [x0, horizon + 60.0],
    ];
    p.fill(&stand, Paint::Solid([34, 42, 70, 255]));
    crowd(p, x0, horizon - 20.0, w, 70.0, &stand, 0.55);
    p.fill(
        &[
            [x0, horizon + 56.0],
            [x0 + w, horizon + 56.0],
            [x0 + w, horizon + 66.0],
            [x0, horizon + 66.0],
        ],
        Paint::Solid([28, 70, 60, 255]),
    );
    // The field: mowing stripes, the infield, foul lines.
    let ground = [
        [x0, horizon + 64.0],
        [x0 + w, horizon + 64.0],
        [x0 + w, y0 + h],
        [x0, y0 + h],
    ];
    p.fill(
        &ground,
        Paint::vertical(horizon + 64.0, y0 + h, GRASS, shade(GRASS, -0.1)),
    );
    let mut x = cx - 900.0;
    while x < x0 + w + 400.0 {
        let s = [
            [x, horizon + 64.0],
            [x + 60.0, horizon + 64.0],
            [x + 200.0, y0 + h],
            [x + 110.0, y0 + h],
        ];
        p.fill(&s, Paint::Solid([255, 255, 255, 18]));
        x += 140.0;
    }
    let hy = y0 + h * 0.86;
    let d = 120.0;
    p.fill(
        &[
            [cx, hy - d * 1.6],
            [cx + d * 1.6, hy],
            [cx, hy + d * 0.6],
            [cx - d * 1.6, hy],
        ],
        Paint::Solid(DIRT),
    );
    p.fill(
        &[
            [cx, hy - d * 1.2],
            [cx + d * 1.15, hy - d * 0.05],
            [cx, hy + d * 0.32],
            [cx - d * 1.15, hy - d * 0.05],
        ],
        Paint::Solid(GRASS2),
    );
    // Foul lines from home plate to the fence.
    let fence = horizon + 66.0;
    for side in [-1.0f32, 1.0] {
        let run = ((hy + d * 0.5 - fence) / 0.75).min(w);
        p.line(
            &[
                [cx, hy + d * 0.5],
                [cx + side * run, hy + d * 0.5 - run * 0.75],
            ],
            2.0,
            [250, 250, 245, 230],
        );
    }
}

/// Rows of little people in shirts of every colour, inside `clip`.
fn crowd(p: &mut Painter, x0: f32, y0: f32, w: f32, h: f32, clip: &[P], light: f32) {
    let shirts: [Rgba; 7] = [
        [240, 240, 240, 255],
        [230, 90, 80, 255],
        [70, 120, 210, 255],
        [250, 200, 80, 255],
        [120, 190, 120, 255],
        [240, 150, 60, 255],
        [200, 120, 200, 255],
    ];
    let base = [34, 42, 70, 255];
    let st = p.st;
    // Raw triangles (no soft edge): thousands of dots, drawn in order with the rest of the art.
    let mesh = &mut p.list.top().mesh;
    let mut seed: u32 = 12345;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed % 1000) as f32 / 1000.0
    };
    let mut y = y0;
    while y < y0 + h {
        let mut x = x0;
        while x < x0 + w {
            let (jx, jy) = (rnd() * 2.0, rnd() * 2.0);
            let pt = [x + jx, y + jy];
            if inside(pt, clip) && rnd() < 0.85 {
                let c = mix(
                    base,
                    shirts[(rnd() * 7.0) as usize % 7],
                    light * (0.4 + rnd() * 0.6),
                );
                let (l, t) = (st.x(pt[0]), st.y(pt[1]));
                let (r, b) = (l + 3.2 * st.k, t + 4.0 * st.k);
                let v = |x: f32, y: f32| Vert2::new(x, y, c);
                mesh.tri(v(l, t), v(r, t), v(r, b));
                mesh.tri(v(l, t), v(r, b), v(l, b));
            }
            x += 5.0;
        }
        y += 6.0;
    }
}

/// Design units one crowd frame covers (it repeats across the stands).
const CROWD_TILE: [f32; 2] = [64.0, 32.0];
/// Pixels per design unit in the baked crowd.
const CROWD_PX: f32 = 3.0;
/// Clear pixels between the two frames, so filtering never borrows the other pose.
const CROWD_GAP: f32 = 4.0;

/// The crowd, baked once into a sheet of two frames (seated, and cheering with arms up):
/// a few dozen little fans in every shirt colour on a clear background, made to repeat.
pub(in crate::diamond_days) fn crowd_sheet() -> Option<crate::image2d::PicId> {
    static SHEET: std::sync::OnceLock<Option<crate::image2d::PicId>> = std::sync::OnceLock::new();
    *SHEET.get_or_init(|| {
        let (fw, fh) = (CROWD_TILE[0] * CROWD_PX, CROWD_TILE[1] * CROWD_PX);
        let shirts: [Rgba; 8] = [
            [242, 242, 244, 255],
            [228, 84, 76, 255],
            [66, 118, 210, 255],
            [250, 200, 72, 255],
            [112, 188, 112, 255],
            [242, 146, 58, 255],
            [196, 112, 200, 255],
            [60, 66, 92, 255],
        ];
        let skins: [Rgba; 3] = [
            [250, 216, 186, 255],
            [232, 190, 150, 255],
            [206, 160, 122, 255],
        ];
        let hairs: [Rgba; 3] = [[40, 34, 40, 255], [80, 56, 44, 255], [30, 30, 34, 255]];
        let mut seed: u32 = 2024;
        let mut rnd = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        // 12 x 4 fans a frame, every other row half a seat over.
        let (cols, rows) = (12usize, 4usize);
        let (cw, ch) = (fw / cols as f32, fh / rows as f32);
        let mut fans = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                let jx = (rnd() % 5) as f32 - 2.0;
                let shirt = shirts[(rnd() % 8) as usize];
                let skin = skins[(rnd() % 3) as usize];
                let hair = hairs[(rnd() % 3) as usize];
                let hop = (rnd() % 3) as f32;
                let x = c as f32 * cw + cw * 0.5 + jx + if r % 2 == 1 { cw * 0.5 } else { 0.0 };
                fans.push((x, r as f32 * ch, shirt, skin, hair, hop));
            }
        }
        let mut list = DrawList::default();
        for frame in 0..2 {
            let ox = frame as f32 * (fw + CROWD_GAP);
            let cheer = frame == 1;
            let cell = [[ox, 0.0], [ox + fw, 0.0], [ox + fw, fh], [ox, fh]];
            let m = &mut list.mesh;
            for &(x, y, shirt, skin, hair, hop) in &fans {
                // A fan cut by the frame's edge shows on the other side, so tiles join.
                for wrap in [0.0, -fw, fw] {
                    let cx = ox + x + wrap;
                    if cx < ox - cw || cx > ox + fw + cw {
                        continue;
                    }
                    let lift = if cheer { 2.0 + hop } else { 0.0 };
                    let top = y + ch * 0.42 - lift;
                    let body = vector::rounded_rect(cx - cw * 0.34, top, cw * 0.68, ch * 0.5, 3.0);
                    vector::fill(m, &vector::clip_convex(&body, &cell), &Paint::Solid(shirt));
                    if cheer {
                        for s in [-1.0f32, 1.0] {
                            let a = [cx + s * cw * 0.28, top + 2.0];
                            let b = [cx + s * cw * 0.42, top - ch * 0.42];
                            let fits = |q: P| q[0] > ox + 1.0 && q[0] < ox + fw - 1.0 && q[1] > 1.0;
                            if fits(a) && fits(b) {
                                vector::stroke(m, &[a, b], 2.4, skin);
                            }
                        }
                    }
                    let head = [cx, top - ch * 0.12];
                    let r = ch * 0.17;
                    let disc = vector::clip_convex(&vector::circle(head[0], head[1], r), &cell);
                    vector::fill(m, &disc, &Paint::Solid(skin));
                    let cap = vector::arc(
                        head[0],
                        head[1] - 0.5,
                        r * 1.05,
                        r,
                        std::f32::consts::PI,
                        std::f32::consts::TAU,
                        8,
                    );
                    vector::fill(m, &vector::clip_convex(&cap, &cell), &Paint::Solid(hair));
                }
            }
        }
        crate::image2d::bake(&list, (fw * 2.0 + CROWD_GAP) as u32, fh as u32).ok()
    })
}

/// The texels of one crowd frame, half a texel in from its edges: filtering stays inside it.
pub(in crate::diamond_days) fn crowd_uv(frame: u32) -> [f32; 4] {
    let (fw, fh) = (CROWD_TILE[0] * CROWD_PX, CROWD_TILE[1] * CROWD_PX);
    let sheet_w = fw * 2.0 + CROWD_GAP;
    let x0 = frame.min(1) as f32 * (fw + CROWD_GAP);
    [
        (x0 + 0.5) / sheet_w,
        0.5 / fh,
        (x0 + fw - 0.5) / sheet_w,
        (fh - 0.5) / fh,
    ]
}

fn inside(p: P, poly: &[P]) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            c = !c;
        }
    }
    c
}

// ---- the board -------------------------------------------------------------------------------

/// A square's picture: a bat for batting, a ball for pitching, a glove, running
/// chevrons, a dumbbell, a moon for rest, "!" for an event, a star for spirit, a lens for
/// scouting. `s` is the icon's size (design units).
pub fn square_icon(p: &mut Painter, c: P, s: f32, sq: Square, ink: Rgba) {
    let [x, y] = c;
    match sq {
        Square::Train(Drill::Batting) => {
            p.line(
                &[[x - s * 0.42, y + s * 0.38], [x - s * 0.12, y + s * 0.08]],
                s * 0.12,
                ink,
            );
            p.line(
                &[[x - s * 0.12, y + s * 0.08], [x + s * 0.36, y - s * 0.4]],
                s * 0.24,
                ink,
            );
            p.dot(x + s * 0.3, y + s * 0.28, s * 0.15, WHITE);
            p.outline(
                &vector::circle(x + s * 0.3, y + s * 0.28, s * 0.15),
                s * 0.05,
                ink,
            );
        }
        Square::Train(Drill::Pitching) => {
            let r = s * 0.42;
            p.dot(x, y, r, WHITE);
            p.outline(&vector::circle(x, y, r), s * 0.06, ink);
            // The seams: two arcs bowing in from either side.
            let pi = std::f32::consts::PI;
            for (cx, a) in [(x - r * 1.3, 0.0f32), (x + r * 1.3, pi)] {
                let seam = vector::arc(cx, y, r * 0.95, r * 0.95, a - 0.62, a + 0.62, 10);
                p.line(&seam, s * 0.06, [220, 60, 60, 255]);
            }
        }
        Square::Train(Drill::Fielding) => {
            p.fill_ink(
                &vector::ellipse(x, y + s * 0.05, s * 0.38, s * 0.42),
                [196, 126, 58, 255],
                ink,
                s * 0.05,
            );
            for k in 0..3 {
                let fx = x - s * 0.16 + k as f32 * s * 0.16;
                p.line(
                    &[[fx, y - s * 0.3], [fx, y + s * 0.05]],
                    s * 0.05,
                    shade([196, 126, 58, 255], -0.4),
                );
            }
            p.fill_ink(
                &vector::ellipse(x + s * 0.36, y + s * 0.02, s * 0.12, s * 0.2),
                [196, 126, 58, 255],
                ink,
                s * 0.05,
            );
        }
        Square::Train(Drill::Running) => {
            for k in 0..3 {
                let ox = x - s * 0.36 + k as f32 * s * 0.28;
                p.line(
                    &[[ox, y - s * 0.3], [ox + s * 0.22, y], [ox, y + s * 0.3]],
                    s * 0.12,
                    ink,
                );
            }
        }
        Square::Train(Drill::Strength) => {
            p.line(&[[x - s * 0.42, y], [x + s * 0.42, y]], s * 0.1, ink);
            for d in [-1.0f32, 1.0] {
                p.fill(
                    &vector::rounded_rect(
                        x + d * s * 0.3 - s * 0.08,
                        y - s * 0.28,
                        s * 0.16,
                        s * 0.56,
                        s * 0.04,
                    ),
                    Paint::Solid(ink),
                );
                p.fill(
                    &vector::rounded_rect(
                        x + d * s * 0.44 - s * 0.05,
                        y - s * 0.18,
                        s * 0.1,
                        s * 0.36,
                        s * 0.03,
                    ),
                    Paint::Solid(ink),
                );
            }
        }
        Square::Rest => {
            let moon = vector::circle(x - s * 0.06, y + s * 0.04, s * 0.34);
            p.fill(&moon, Paint::Solid([255, 230, 140, 255]));
            p.fill(
                &vector::circle(x + s * 0.08, y - s * 0.06, s * 0.28),
                Paint::Solid([150, 220, 230, 255]),
            );
            p.text(
                "z",
                x + s * 0.3,
                y - s * 0.62,
                s * 0.42,
                ink,
                crate::scene::TextAlign::Center,
            );
        }
        Square::Event => {
            p.fill_ink(
                &vector::circle(x, y, s * 0.42),
                [255, 214, 90, 255],
                ink,
                s * 0.05,
            );
            p.text(
                "!",
                x,
                y - s * 0.34,
                s * 0.62,
                ink,
                crate::scene::TextAlign::Center,
            );
        }
        Square::Spirit => {
            p.fill_ink(
                &vector::star(x, y, s * 0.48, s * 0.22, 5),
                [255, 236, 120, 255],
                ink,
                s * 0.05,
            );
        }
        Square::Scout => {
            p.outline(
                &vector::circle(x - s * 0.08, y - s * 0.08, s * 0.26),
                s * 0.1,
                ink,
            );
            p.line(
                &[[x + s * 0.12, y + s * 0.12], [x + s * 0.4, y + s * 0.4]],
                s * 0.14,
                ink,
            );
        }
    }
}

/// One square on the road: a rounded stone in its colour with its picture and its name,
/// lit when the coach stands on it.
pub fn road_tile(p: &mut Painter, c: P, tw: f32, th: f32, sq: Square, label: &str, here: bool) {
    let col = square_color(sq);
    let (x, y) = (c[0] - tw * 0.5, c[1] - th * 0.5);
    if here {
        p.glow(c[0], c[1], tw * 0.95, [255, 236, 150, 170]);
    }
    p.shadow(x, y + 2.0, tw, th, 10.0, 5.0, 70);
    let rr = vector::rounded_rect(x, y, tw, th, 10.0);
    p.fill(
        &rr,
        Paint::vertical(y, y + th, shade(col, 0.3), shade(col, -0.06)),
    );
    // The name on a pale strip at the foot.
    p.fill(
        &vector::rounded_rect(x + 3.0, y + th - 19.0, tw - 6.0, 16.0, 6.0),
        Paint::Solid([255, 255, 255, 215]),
    );
    p.fill(
        &vector::rounded_rect(x + 4.0, y + 3.0, tw - 8.0, th * 0.28, 7.0),
        Paint::Solid([255, 255, 255, 60]),
    );
    p.outline(
        &rr,
        if here { 3.0 } else { 1.6 },
        if here {
            [255, 214, 90, 255]
        } else {
            shade(col, -0.35)
        },
    );
    let ink = shade(col, -0.62);
    square_icon(
        p,
        [c[0], y + (th - 19.0) * 0.5 + 1.0],
        (th - 19.0) * 0.62,
        sq,
        ink,
    );
    let size = if label.chars().count() > 3 { 9.5 } else { 11.0 };
    p.text(
        label,
        c[0],
        y + th - 17.5 + (14.0 - size) * 0.5,
        size,
        INK,
        crate::scene::TextAlign::Center,
    );
}

/// The coach's token: a cap on a little stand.
pub fn coach_token(p: &mut Painter, c: P, size: f32, lift: f32) {
    let face = Face::from_seed(9001);
    let mut o = Outfit::cap([60, 64, 80, 255]);
    o.shirt = [58, 62, 80, 255];
    o.trim = [200, 204, 214, 255];
    o.adult = true;
    p.fill(
        &vector::ellipse(c[0], c[1] + size * 0.3, size * 0.36, size * 0.12),
        Paint::Solid([0, 0, 0, 70]),
    );
    let expr = if lift > 0.5 { Expr::Smile } else { Expr::Happy };
    p.portrait(
        c[0] - size * 0.5,
        c[1] - size * 0.72 - lift,
        size,
        &face,
        expr,
        &o,
        1.0,
    );
}

// ---- the game --------------------------------------------------------------------------------

/// Where the fielders stand (metres, degrees).
pub const FIELDERS: [(f32, f32); 9] = [
    (18.4, 0.0),
    (-1.5, 0.0),
    (28.0, 40.0),
    (38.0, 14.0),
    (28.0, -40.0),
    (38.0, -14.0),
    (74.0, -30.0),
    (84.0, 0.0),
    (74.0, 30.0),
];

/// A round mood face: the colour and arrow say how a player feels.
pub fn mood_icon(p: &mut Painter, c: P, r: f32, m: Mood) {
    let (col, ang) = match m {
        Mood::Great => ([240, 80, 80, 255], 60.0f32),
        Mood::Good => ([255, 150, 60, 255], 30.0),
        Mood::Normal => ([240, 200, 70, 255], 0.0),
        Mood::Poor => ([110, 170, 220, 255], -30.0),
        Mood::Awful => ([90, 110, 200, 255], -60.0),
    };
    p.dot(c[0], c[1], r, col);
    let a = (-ang).to_radians();
    let (dx, dy) = (a.cos() * r * 0.55, a.sin() * r * 0.55);
    p.line(
        &[[c[0] - dx, c[1] - dy], [c[0] + dx, c[1] + dy]],
        r * 0.28,
        WHITE,
    );
    for s in [-1.0f32, 1.0] {
        let b = a + std::f32::consts::PI + s * 0.65;
        p.line(
            &[
                [c[0] + dx, c[1] + dy],
                [
                    c[0] + dx + b.cos() * r * 0.42,
                    c[1] + dy + b.sin() * r * 0.42,
                ],
            ],
            r * 0.28,
            WHITE,
        );
    }
}

// ---- the practice ground ---------------------------------------------------------------------

/// The school's practice ground in the middle of the board: the school building, the net,
/// a dirt field with chalk lines, a batting cage, a bullpen, the running lane, and the club
/// at their drills (`crew`: the drill, face, expression).
pub fn school_ground(p: &mut Painter, r: [f32; 4], crew: &[(Drill, Face, Expr)], t: f32) {
    let [x, y, w, h] = r;
    let rr = vector::rounded_rect(x, y, w, h, 14.0);
    p.shadow(x, y, w, h, 14.0, 8.0, 70);
    // Sky and the school.
    let sky_b = y + h * 0.3;
    p.fill(
        &rr,
        Paint::vertical(y, sky_b, [150, 200, 244, 255], [214, 236, 250, 255]),
    );
    let bx0 = x + w * 0.16;
    let bw = w * 0.68;
    let by = y + h * 0.06;
    let bh = sky_b - by;
    p.fill_ink(
        &vector::rounded_rect(bx0, by, bw, bh, 3.0),
        [246, 240, 226, 255],
        [150, 140, 128, 255],
        1.2,
    );
    p.fill(
        &vector::rounded_rect(bx0 - 3.0, by - 3.0, bw + 6.0, 5.0, 2.0),
        Paint::Solid([186, 120, 96, 255]),
    );
    let (cols, rows) = (9, 3);
    for j in 0..rows {
        for i in 0..cols {
            let wx = bx0 + 8.0 + i as f32 * (bw - 16.0) / cols as f32;
            let wy = by + 6.0 + j as f32 * (bh - 10.0) / rows as f32;
            if i == cols / 2 && j == 0 {
                continue;
            }
            p.fill(
                &vector::rounded_rect(
                    wx,
                    wy,
                    (bw - 16.0) / cols as f32 - 5.0,
                    (bh - 10.0) / rows as f32 - 5.0,
                    1.5,
                ),
                Paint::vertical(wy, wy + 8.0, [150, 196, 230, 255], [112, 160, 206, 255]),
            );
        }
    }
    let clock = [bx0 + bw * 0.5, by + 8.0];
    p.fill_ink(
        &vector::circle(clock[0], clock[1], 6.0),
        WHITE,
        [90, 90, 100, 255],
        1.0,
    );
    p.line(&[clock, [clock[0], clock[1] - 4.0]], 1.0, INK);
    p.line(&[clock, [clock[0] + 3.0, clock[1]]], 1.0, INK);
    for (tx, s) in [(x + w * 0.07, 1.0), (x + w * 0.93, 1.1)] {
        p.fill(
            &vector::rounded_rect(tx - 3.0, sky_b - 26.0 * s, 6.0, 26.0 * s, 2.0),
            Paint::Solid([120, 84, 60, 255]),
        );
        for (dx, dy, rr2) in [
            (0.0, -34.0, 16.0),
            (-11.0, -26.0, 12.0),
            (11.0, -26.0, 12.0),
        ] {
            p.fill_ink(
                &vector::circle(tx + dx * s, sky_b + dy * s, rr2 * s),
                [92, 168, 92, 255],
                [52, 110, 60, 255],
                1.0,
            );
        }
    }
    // The ground.
    let ground = vector::clip_convex(
        &[[x, sky_b], [x + w, sky_b], [x + w, y + h], [x, y + h]],
        &rr,
    );
    p.fill(
        &ground,
        Paint::vertical(sky_b, y + h, [222, 190, 146, 255], [204, 168, 122, 255]),
    );
    let mut ly = sky_b + 14.0;
    while ly < y + h - 6.0 {
        p.line(
            &[[x + 8.0, ly], [x + w - 8.0, ly + 2.0]],
            0.8,
            [190, 154, 110, 90],
        );
        ly += 11.0;
    }
    // The back net along the school side.
    let ny = sky_b - 2.0;
    p.fill(
        &[
            [x, ny - 16.0],
            [x + w, ny - 16.0],
            [x + w, ny + 2.0],
            [x, ny + 2.0],
        ],
        Paint::Solid([60, 70, 70, 40]),
    );
    let mut nx = x + 4.0;
    while nx < x + w {
        p.line(&[[nx, ny - 16.0], [nx, ny + 2.0]], 0.6, [70, 80, 80, 120]);
        nx += 6.0;
    }
    for k in 0..3 {
        let yy = ny - 16.0 + k as f32 * 6.0;
        p.line(&[[x, yy], [x + w, yy]], 0.6, [70, 80, 80, 120]);
    }
    // Chalk: a small diamond in the middle.
    let dc = [x + w * 0.5, y + h * 0.66];
    let (dw, dh) = (w * 0.2, h * 0.13);
    p.outline(
        &[
            [dc[0], dc[1] - dh],
            [dc[0] + dw, dc[1]],
            [dc[0], dc[1] + dh],
            [dc[0] - dw, dc[1]],
        ],
        1.6,
        [255, 255, 255, 200],
    );
    // The batting cage (left) and the bullpen (right).
    let cage = [x + w * 0.04, y + h * 0.36, w * 0.24, h * 0.34];
    p.fill(
        &vector::rounded_rect(cage[0], cage[1], cage[2], cage[3], 3.0),
        Paint::Solid([40, 60, 60, 40]),
    );
    let mut gx = cage[0];
    while gx <= cage[0] + cage[2] + 0.1 {
        p.line(
            &[[gx, cage[1]], [gx, cage[1] + cage[3]]],
            0.6,
            [60, 70, 70, 150],
        );
        gx += 7.0;
    }
    let mut gy = cage[1];
    while gy <= cage[1] + cage[3] + 0.1 {
        p.line(
            &[[cage[0], gy], [cage[0] + cage[2], gy]],
            0.6,
            [60, 70, 70, 150],
        );
        gy += 7.0;
    }
    p.outline(
        &vector::rounded_rect(cage[0], cage[1], cage[2], cage[3], 3.0),
        1.4,
        [70, 80, 84, 220],
    );
    let pen = [x + w * 0.84, y + h * 0.5];
    p.fill(
        &vector::ellipse(pen[0], pen[1], w * 0.08, h * 0.035),
        Paint::Solid([214, 168, 116, 255]),
    );
    p.fill(
        &vector::rounded_rect(pen[0] - 5.0, pen[1] - 1.5, 10.0, 3.0, 1.0),
        Paint::Solid(WHITE),
    );
    // Cones along the running lane.
    for i in 0..5 {
        let c = [x + w * (0.18 + i as f32 * 0.16), y + h * 0.95];
        p.fill_ink(
            &[[c[0], c[1] - 7.0], [c[0] + 4.0, c[1]], [c[0] - 4.0, c[1]]],
            [255, 140, 50, 255],
            [160, 80, 30, 255],
            0.8,
        );
    }
    // The club at their drills, back to front. Each drill has a few spots; anyone past
    // them waits nearby for a turn.
    let size = (h * 0.22).clamp(28.0, 60.0);
    let mut slots: Vec<(P, Pose, Facing)> = Vec::new();
    let mut used = [0usize; 5];
    let spot = |d: Drill, n: usize| -> Option<(P, Pose)> {
        let tt = t + n as f32 * 0.37;
        match (d, n) {
            (Drill::Batting, 0) => Some((
                [cage[0] + cage[2] * 0.5, cage[1] + cage[3] * 0.8],
                Pose::Bat((tt * 0.8 % 1.6).min(1.0)),
            )),
            (Drill::Batting, 1) => Some((
                [cage[0] + cage[2] * 0.62, cage[1] + cage[3] + 30.0],
                Pose::Bat((tt * 0.7 % 1.8).min(1.0)),
            )),
            (Drill::Batting, 2) => Some((
                [cage[0] + cage[2] * 0.22, cage[1] + cage[3] + 44.0],
                Pose::Bat((tt * 0.75 % 1.7).min(1.0)),
            )),
            (Drill::Pitching, 0) => Some((
                [pen[0], pen[1] + 4.0],
                Pose::Pitch((tt * 0.6 % 1.4).min(1.0)),
            )),
            (Drill::Pitching, 1) => Some((
                [pen[0] - 18.0, pen[1] + 40.0],
                Pose::Pitch((tt * 0.55 % 1.5).min(1.0)),
            )),
            (Drill::Fielding, k) if k < 4 => Some((
                [
                    dc[0] + [-0.75, 0.75, 0.0, 0.0][k] * dw,
                    dc[1] + [0.0, 0.0, -0.8, 0.85][k] * dh,
                ],
                Pose::Ready,
            )),
            (Drill::Running, k) if k < 3 => Some((
                [
                    x + w * (0.1 + ((t * 0.07 + k as f32 * 0.27) % 0.8)),
                    y + h * 0.9,
                ],
                Pose::Run(t * 1.6 + k as f32 * 0.3),
            )),
            (Drill::Strength, k) if k < 2 => Some((
                [x + w * (0.4 + k as f32 * 0.22), y + h * 0.44],
                if ((t * 0.7 + k as f32) as i32) % 2 == 0 {
                    Pose::Cheer
                } else {
                    Pose::Stand
                },
            )),
            _ => None,
        }
    };
    let mut waiting = 0usize;
    for (d, _, _) in crew {
        let n = used[d.index()];
        used[d.index()] += 1;
        let (at, pose) = spot(*d, n).unwrap_or_else(|| {
            // In line along the side, bat or glove in hand.
            let k = waiting;
            waiting += 1;
            (
                [
                    x + w * (0.36 + (k % 4) as f32 * 0.1),
                    y + h * (0.47 + (k / 4) as f32 * 0.1),
                ],
                Pose::Stand,
            )
        });
        slots.push((at, pose, Facing::Front));
    }
    let mut order: Vec<usize> = (0..crew.len()).collect();
    order.sort_by(|a, b| slots[*a].0[1].total_cmp(&slots[*b].0[1]));
    let k = Kit::practice(OURS);
    for i in order {
        let (at, pose, facing) = slots[i];
        let (_, face, expr) = &crew[i];
        chibi::draw(p, at, size, face, *expr, facing, pose, &k);
    }
    p.outline(&rr, 2.0, [255, 255, 255, 170]);
}

/// A school crest: a shield in the school colour with its first letter.
pub fn emblem(p: &mut Painter, cx: f32, cy: f32, r: f32, color: Rgba, letter: &str) {
    let shield = vec![
        [cx - r, cy - r * 0.9],
        [cx - r * 0.35, cy - r * 1.05],
        [cx, cy - r * 0.92],
        [cx + r * 0.35, cy - r * 1.05],
        [cx + r, cy - r * 0.9],
        [cx + r * 0.96, cy + r * 0.2],
        [cx + r * 0.6, cy + r * 0.75],
        [cx, cy + r * 1.1],
        [cx - r * 0.6, cy + r * 0.75],
        [cx - r * 0.96, cy + r * 0.2],
    ];
    p.shadow(cx - r, cy - r, r * 2.0, r * 2.1, r * 0.4, r * 0.25, 90);
    p.fill_ink(&shield, shade(color, 0.08), shade(color, -0.5), r * 0.06);
    let inner: Vec<P> = shield
        .iter()
        .map(|q| [cx + (q[0] - cx) * 0.84, cy + (q[1] - cy) * 0.84 + r * 0.02])
        .collect();
    p.outline(&inner, r * 0.05, [255, 255, 255, 220]);
    p.fill(
        &[
            [cx - r * 0.9, cy - r * 0.85],
            [cx + r * 0.9, cy - r * 0.85],
            [cx + r * 0.9, cy - r * 0.2],
            [cx - r * 0.9, cy - r * 0.2],
        ],
        Paint::Solid([255, 255, 255, 30]),
    );
    p.text_outlined(
        letter,
        cx,
        cy - r * 0.55,
        r * 1.0,
        WHITE,
        shade(color, -0.55),
        r * 0.06,
        crate::scene::TextAlign::Center,
    );
}

#[cfg(test)]
mod crowd_tests {
    use super::*;

    #[test]
    fn crowd_frames_never_sample_each_other() {
        let (a, b) = (crowd_uv(0), crowd_uv(1));
        assert!(a[2] < b[0], "a clear gap between the frames: {a:?} {b:?}");
        let sheet = crowd_sheet().unwrap();
        let pic = crate::image2d::get(sheet).unwrap();
        let fw = (CROWD_TILE[0] * CROWD_PX) as u32;
        // The middle of the gutter is clear all the way down (its edge texels may hold
        // the anti-aliased rim of a frame; the half-texel uv never reaches them).
        for y in 0..pic.height {
            for x in fw + 1..fw + CROWD_GAP as u32 - 1 {
                assert_eq!(pic.pixel(x, y)[3], 0, "gutter texel {x},{y}");
            }
        }
    }
}

/// The app icon (`games/diamond_days_minimal.json`: `icon`, `icon_layers`), baked by its
/// test with `KAGRA_ICON=assets/icons`.
#[cfg(test)]
mod icon {
    use super::*;

    /// The app icon is drawn on a square this many units wide (`Stage::fit(px, px, ICON, ICON)`).
    pub const ICON: f32 = 512.0;

    /// The radius around the middle that every launcher mask leaves visible: an adaptive icon
    /// shows 72 of its 108 units and some masks cut a circle of 66 inside that.
    pub const ICON_SAFE: f32 = ICON * 33.0 / 108.0;

    /// The app icon's back layer: a summer sky, light from the middle, the field below.
    pub fn icon_background(p: &mut Painter) {
        let s = ICON;
        let square = [[0.0, 0.0], [s, 0.0], [s, s], [0.0, s]];
        p.fill(
            &square,
            Paint::vertical(0.0, s, [30, 96, 200, 255], [120, 190, 246, 255]),
        );
        p.sunburst(s * 0.5, s * 0.47, s * 0.8, 18, 0.09, [255, 255, 255, 64]);
        p.glow(s * 0.5, s * 0.47, s * 0.36, [255, 250, 220, 90]);
        // The outfield grass and the dirt of the infield's edge.
        let grass = vector::ellipse(s * 0.5, s * 1.32, s * 0.95, s * 0.55);
        p.fill(
            &grass,
            Paint::vertical(s * 0.77, s, [86, 178, 84, 255], [44, 126, 62, 255]),
        );
        let dirt = vector::ellipse(s * 0.5, s * 1.36, s * 0.52, s * 0.44);
        p.fill(
            &dirt,
            Paint::vertical(s * 0.92, s, [214, 160, 104, 255], [176, 120, 76, 255]),
        );
        p.outline(&grass, s * 0.012, [255, 255, 255, 120]);
    }

    /// The app icon's front layer: a white ball with red stitches and the game's name on a
    /// ribbon, all inside [`ICON_SAFE`] of the middle so no launcher's mask cuts it.
    pub fn icon_foreground(p: &mut Painter) {
        let pi = std::f32::consts::PI;
        let (cx, cy) = (ICON * 0.5, ICON * 0.44);
        let r = ICON * 0.215;
        // The ball: a soft shadow, white with a cool shade away from the light.
        p.fill(
            &vector::circle(cx + 7.0, cy + 11.0, r),
            Paint::Solid([0, 30, 70, 70]),
        );
        let ball = vector::circle(cx, cy, r);
        p.fill(
            &ball,
            Paint::Radial {
                center: [cx - r * 0.3, cy - r * 0.45],
                radius: r * 1.8,
                c0: WHITE,
                c1: [204, 214, 230, 255],
            },
        );
        // The seams bow in from either side, stitched in red.
        let red = [214, 44, 50, 255];
        for (sx, a) in [(cx - r * 1.3, 0.0f32), (cx + r * 1.3, pi)] {
            let seam = vector::arc(sx, cy, r * 0.95, r * 0.95, a - 0.84, a + 0.84, 28);
            p.line(&seam, r * 0.04, red);
            for k in (2..seam.len() - 2).step_by(3) {
                let (q0, q1) = (seam[k - 1], seam[k + 1]);
                let (tx, ty) = (q1[0] - q0[0], q1[1] - q0[1]);
                let n = (tx * tx + ty * ty).sqrt().max(1e-3);
                let (tx, ty) = (tx / n, ty / n);
                let (nx, ny) = (-ty, tx);
                let q = seam[k];
                let w = r * 0.1;
                // A small V across the seam.
                p.line(
                    &[
                        [q[0] - nx * w - tx * w * 0.45, q[1] - ny * w - ty * w * 0.45],
                        q,
                        [q[0] + nx * w - tx * w * 0.45, q[1] + ny * w - ty * w * 0.45],
                    ],
                    r * 0.038,
                    red,
                );
            }
        }
        p.outline(&ball, r * 0.04, [44, 56, 84, 255]);
        // The shine, between the seams.
        p.fill(
            &vector::ellipse(cx - r * 0.02, cy - r * 0.64, r * 0.2, r * 0.1),
            Paint::Solid([255, 255, 255, 230]),
        );
        // The name on a red ribbon across the bottom of the ball.
        let (rw, rh) = (ICON * 0.36, ICON * 0.13);
        p.ribbon(
            cx,
            cy + r * 0.45,
            rw,
            rh,
            "白球",
            rh * 0.7,
            [222, 48, 56, 255],
        );
    }

    /// The whole icon at `px` square (the store picture, the page): both layers.
    pub fn icon_picture(px: u32) -> crate::raster2d::Canvas {
        let mut list = DrawList::default();
        {
            let st = crate::hud_art::Stage::fit(px as f32, px as f32, ICON, ICON);
            let mut p = Painter::new(&mut list, st);
            icon_background(&mut p);
            icon_foreground(&mut p);
        }
        crate::raster2d::picture(&list, px, px)
    }

    /// One adaptive-icon layer at `px` square; the front layer keeps its see-through
    /// background.
    pub fn icon_layer(px: u32, front: bool) -> crate::raster2d::Canvas {
        let mut list = DrawList::default();
        {
            let st = crate::hud_art::Stage::fit(px as f32, px as f32, ICON, ICON);
            let mut p = Painter::new(&mut list, st);
            if front {
                icon_foreground(&mut p);
            } else {
                icon_background(&mut p);
            }
        }
        let mut c = crate::raster2d::Canvas::new(px, px, [0, 0, 0, if front { 0 } else { 255 }]);
        c.draw(&list);
        c
    }

    /// The app icon (`KAGRA_ICON=dir` writes the files `games/diamond_days_minimal.json`
    /// names). The front layer stays inside the circle every launcher mask keeps, the back
    /// layer fills the square.
    #[test]
    fn the_app_icon_keeps_inside_the_mask() {
        let px = 432u32;
        let front = icon_layer(px, true);
        let back = icon_layer(px, false);
        let k = px as f32 / ICON;
        let mut inside = 0;
        for y in 0..px {
            for x in 0..px {
                let i = ((y * px + x) * 4) as usize;
                let (dx, dy) = (
                    x as f32 + 0.5 - px as f32 * 0.5,
                    y as f32 + 0.5 - px as f32 * 0.5,
                );
                let d = (dx * dx + dy * dy).sqrt() / k;
                if d > ICON_SAFE {
                    assert_eq!(
                        front.rgba[i + 3],
                        0,
                        "front layer at {x},{y} ({d:.0} units out)"
                    );
                } else if front.rgba[i + 3] > 0 {
                    inside += 1;
                }
                assert_eq!(back.rgba[i + 3], 255, "back layer at {x},{y}");
            }
        }
        assert!(
            inside > (px * px / 8) as usize,
            "the ball and the ribbon are there: {inside}"
        );
        if let Ok(dir) = std::env::var("KAGRA_ICON") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            icon_picture(512)
                .save_png(&dir.join("diamond_days.png"))
                .unwrap();
            front.save_png(&dir.join("diamond_days_front.png")).unwrap();
            back.save_png(&dir.join("diamond_days_back.png")).unwrap();
        }
    }
}
