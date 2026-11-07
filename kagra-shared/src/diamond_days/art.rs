//! The pictures: the sky by season, a ballpark full of people, the board of practice
//! squares, the field during a game with little players, the ball in flight. All drawn with
//! `hud_art` / `vector`, in design units on the stage (400 wide).

use super::board::Square;
use super::sim::PlayKind;
use super::team::{Drill, Mood, Player};
use crate::game_core::portrait::{Expr, Face, Outfit};
use crate::hud_art::Painter;
use crate::scene::Vert2;
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

pub fn hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// A face for a name (players, rivals).
pub fn face_of(name: &str) -> Face {
    Face::from_seed(hash(name))
}

pub fn player_face(p: &Player) -> Face {
    face_of(&format!("{}{}", p.family, p.given))
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
pub fn backdrop(p: &mut Painter, month: u8, t: f32) {
    let [x0, y0, w, h] = p.st.screen;
    let (top, bottom) = sky(month);
    let rect = [[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]];
    p.fill(&rect, Paint::vertical(y0, y0 + h, top, bottom));
    for (i, (cy, r, sp)) in [
        (90.0, 90.0, 5.0),
        (210.0, 70.0, 8.0),
        (40.0, 60.0, 3.0),
        (330.0, 80.0, 6.0),
    ]
    .into_iter()
    .enumerate()
    {
        let span = w + r * 4.0;
        let cx = x0 - r * 2.0 + (i as f32 * 260.0 + t * sp).rem_euclid(span);
        for (dx, dy, k) in [(-0.6, 0.1, 0.7), (0.0, -0.1, 1.0), (0.7, 0.12, 0.75)] {
            p.glow(cx + dx * r, cy + dy * r, r * k, [255, 255, 255, 120]);
        }
    }
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

/// Where each of `n` squares sits round the board rectangle (centres): a square in every
/// corner and the rest spaced evenly along the sides. Square 0 is the bottom middle; the ring
/// goes round counter-clockwise on screen (right, up, left, down).
pub fn tile_centers(r: [f32; 4], n: usize, tw: f32, th: f32) -> Vec<P> {
    let (x0, y0) = (r[0] + tw * 0.5, r[1] + th * 0.5);
    let (x1, y1) = (r[0] + r[2] - tw * 0.5, r[1] + r[3] - th * 0.5);
    let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    // nx squares along the top and bottom, ny down the sides, corners shared: 2nx + 2ny - 4 = n.
    let half = n.div_ceil(2) + 2;
    let nx = ((half as f32 * w / (w + h)).round() as usize).clamp(2, half.saturating_sub(2).max(2));
    let ny = half.saturating_sub(nx).max(2);
    let mut pts = Vec::with_capacity(2 * (nx + ny));
    for i in 0..nx - 1 {
        pts.push([x0 + w * i as f32 / (nx - 1) as f32, y1]);
    }
    for i in 0..ny - 1 {
        pts.push([x1, y1 - h * i as f32 / (ny - 1) as f32]);
    }
    for i in 0..nx - 1 {
        pts.push([x1 - w * i as f32 / (nx - 1) as f32, y0]);
    }
    for i in 0..ny - 1 {
        pts.push([x0, y0 + h * i as f32 / (ny - 1) as f32]);
    }
    let start = ((nx - 1) / 2).min(pts.len().saturating_sub(1));
    pts.rotate_left(start);
    pts.truncate(n);
    pts
}

/// One square of the board.
pub fn tile(p: &mut Painter, c: P, tw: f32, th: f32, sq: Square, label: &str, here: bool) {
    let col = square_color(sq);
    let (x, y) = (c[0] - tw * 0.5, c[1] - th * 0.5);
    let light = col[0] as u32 + col[1] as u32 + col[2] as u32 > 600;
    p.shadow(x, y, tw, th, 9.0, 5.0, 60);
    let rr = vector::rounded_rect(x, y, tw, th, 9.0);
    p.fill(
        &rr,
        Paint::vertical(y, y + th, shade(col, 0.25), shade(col, -0.08)),
    );
    p.fill(
        &vector::rounded_rect(x + 3.0, y + 2.0, tw - 6.0, th * 0.4, 7.0),
        Paint::Solid([255, 255, 255, 50]),
    );
    p.outline(
        &rr,
        if here { 3.0 } else { 1.5 },
        if here { WHITE } else { shade(col, -0.32) },
    );
    let tc = if light { INK } else { WHITE };
    let size = if label.chars().count() > 3 {
        10.5
    } else if label.chars().count() > 2 {
        12.0
    } else {
        14.0
    };
    if !light {
        p.text(
            label,
            c[0] + 0.6,
            c[1] - size * 0.58 + 1.0,
            size,
            shade(col, -0.5),
            crate::scene::TextAlign::Center,
        );
    }
    p.text(
        label,
        c[0],
        c[1] - size * 0.58,
        size,
        tc,
        crate::scene::TextAlign::Center,
    );
}

/// The little ballpark in the middle of the board, the club in their places (`players`:
/// the fielding position index into [`FIELDERS`], face, expression).
pub fn mini_field(p: &mut Painter, r: [f32; 4], players: &[(usize, Face, Expr)]) {
    let home = game_field(p, r, true);
    let size = (home.2 * 9.0).clamp(22.0, 34.0);
    for (i, face, expr) in players {
        let (dist, ang) = FIELDERS[*i % FIELDERS.len()];
        let c = field_pt(home, dist.max(4.0), ang);
        p.fill(
            &vector::ellipse(c[0], c[1] + size * 0.42, size * 0.32, size * 0.1),
            Paint::Solid([0, 0, 0, 60]),
        );
        p.portrait(
            c[0] - size * 0.5,
            c[1] - size * 0.6,
            size,
            face,
            *expr,
            &Outfit::cap(OURS),
            1.0,
        );
    }
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

/// The field during a game, fitted in `r`: home plate near the bottom middle. Returns the
/// screen mapping (home x, home y, design units per metre).
pub fn game_field(p: &mut Painter, r: [f32; 4], fit_width: bool) -> (f32, f32, f32) {
    let [x, y, w, h] = r;
    let rr = vector::rounded_rect(x, y, w, h, 16.0);
    p.shadow(x, y, w, h, 16.0, 10.0, 80);
    p.fill(&rr, Paint::Solid([30, 36, 62, 255]));
    // People in the stands all round.
    let st_poly = rr.clone();
    crowd(p, x, y, w, h, &st_poly, 0.5);
    let hx = x + w * 0.5;
    let hy = y + h - 16.0;
    // Design units per metre: the wall (about 110 m) fits the height, or the width too.
    // Fitting the width keeps the corner outfielders (74 m out at 30 degrees) inside.
    let m = if fit_width {
        ((h - 24.0) / 112.0).min((w * 0.5 - 14.0) / 37.0)
    } else {
        (h - 24.0) / 112.0
    };
    let arc = |dist: f32, span: f32| -> Vec<P> {
        let n = 36;
        (0..=n)
            .map(|i| {
                let a = (-span + 2.0 * span * i as f32 / n as f32).to_radians();
                let dd = dist + 5.0 * (2.0 * a).cos();
                [hx + a.sin() * dd * m, hy - a.cos() * dd * m]
            })
            .collect()
    };
    let fan = |dist: f32, span: f32| -> Vec<P> {
        let mut v = vec![[hx - 30.0, y + h], [hx + 30.0, y + h]];
        let mut a = arc(dist, span);
        a.reverse();
        v.extend(a);
        vector::clip_convex(&v, &rr)
    };
    p.fill(&fan(110.0, 62.0), Paint::Solid([24, 78, 58, 255]));
    p.fill(&fan(105.0, 62.0), Paint::Solid([176, 128, 86, 255]));
    p.fill(&fan(100.0, 62.0), Paint::Solid([72, 140, 72, 255]));
    let mut fair = vec![[hx, hy]];
    fair.extend(arc(98.0, 45.0));
    let fair = vector::clip_convex(&fair, &rr);
    p.fill(&fair, Paint::vertical(y, hy, GRASS2, GRASS));
    let mut k = -45.0f32;
    let mut i = 0;
    while k < 45.0 {
        if i % 2 == 0 {
            let a0 = k.to_radians();
            let a1 = (k + 6.0).to_radians();
            let s = [
                [hx, hy],
                [hx + a0.sin() * 98.0 * m, hy - a0.cos() * 98.0 * m],
                [hx + a1.sin() * 98.0 * m, hy - a1.cos() * 98.0 * m],
            ];
            p.fill(
                &vector::clip_convex(&s, &rr),
                Paint::Solid([255, 255, 255, 14]),
            );
        }
        k += 6.0;
        i += 1;
    }
    let inside_r =
        |q: &P| q[0] > x + 2.0 && q[0] < x + w - 2.0 && q[1] > y + 2.0 && q[1] < y + h - 2.0;
    let wall: Vec<P> = arc(110.0, 62.0).into_iter().filter(inside_r).collect();
    p.line(&wall, 1.5, [250, 230, 120, 200]);
    // The infield.
    let mut dirt = vec![[hx - 6.0, hy + 8.0], [hx + 6.0, hy + 8.0]];
    let mut da = arc(28.0, 45.0);
    da.reverse();
    dirt.extend(da);
    p.fill(&dirt, Paint::Solid(DIRT));
    let b = |dist: f32, ang: f32| -> P {
        let a = ang.to_radians();
        [hx + a.sin() * dist * m, hy - a.cos() * dist * m]
    };
    let (b1, b2, b3) = (b(27.4, 45.0), b(38.8, 0.0), b(27.4, -45.0));
    let grass_in = [b(2.5, 0.0), b(23.5, 45.0), b(33.5, 0.0), b(23.5, -45.0)];
    p.fill(&grass_in, Paint::Solid([104, 174, 94, 255]));
    let mound = b(18.4, 0.0);
    p.dot(
        mound[0],
        mound[1],
        4.0 * m.max(0.6) + 3.0,
        shade(DIRT, 0.06),
    );
    for side in [-1.0f32, 1.0] {
        let end = b(98.0, side * 45.0);
        p.line(
            &[
                [hx, hy],
                clip_seg([hx, hy], end, [x + 2.0, y + 2.0, w - 4.0, h - 4.0]),
            ],
            1.4,
            [250, 250, 250, 230],
        );
    }
    for base in [b1, b2, b3] {
        p.fill(
            &[
                [base[0], base[1] - 4.0],
                [base[0] + 4.0, base[1]],
                [base[0], base[1] + 4.0],
                [base[0] - 4.0, base[1]],
            ],
            Paint::Solid(WHITE),
        );
    }
    p.fill(
        &[
            [hx - 3.5, hy - 2.0],
            [hx + 3.5, hy - 2.0],
            [hx + 3.5, hy + 1.0],
            [hx, hy + 4.0],
            [hx - 3.5, hy + 1.0],
        ],
        Paint::Solid(WHITE),
    );
    p.outline(&rr, 2.0, [80, 92, 140, 255]);
    (hx, hy, m)
}

/// The end of the segment from `a` (inside `r`) toward `b`, stopped at the rectangle's edge.
fn clip_seg(a: P, b: P, r: [f32; 4]) -> P {
    let mut t: f32 = 1.0;
    let d = [b[0] - a[0], b[1] - a[1]];
    for (k, lo, hi) in [(0, r[0], r[0] + r[2]), (1, r[1], r[1] + r[3])] {
        if d[k] > 0.0 {
            t = t.min((hi - a[k]) / d[k]);
        } else if d[k] < 0.0 {
            t = t.min((lo - a[k]) / d[k]);
        }
    }
    let t = t.clamp(0.0, 1.0);
    [a[0] + d[0] * t, a[1] + d[1] * t]
}

/// A point on the field: `dist` metres from home toward `ang` degrees (0 = centre field).
pub fn field_pt(home: (f32, f32, f32), dist: f32, ang: f32) -> P {
    let a = ang.to_radians();
    [
        home.0 + a.sin() * dist * home.2,
        home.1 - a.cos() * dist * home.2,
    ]
}

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

/// A little player seen from the stands: shadow, legs, uniform with team trim, cap.
pub fn figure(p: &mut Painter, c: P, color: Rgba, s: f32, bat: bool) {
    let (x, y) = (c[0], c[1]);
    let ink = [30, 30, 40, 255];
    p.fill(
        &vector::ellipse(x, y + s * 0.95, s * 0.6, s * 0.2),
        Paint::Solid([0, 0, 0, 80]),
    );
    for side in [-1.0f32, 1.0] {
        p.line(
            &[
                [x + side * s * 0.18, y + s * 0.45],
                [x + side * s * 0.2, y + s * 0.85],
            ],
            s * 0.26,
            ink,
        );
    }
    let body = vector::rounded_rect(x - s * 0.42, y - s * 0.1, s * 0.84, s * 0.68, s * 0.28);
    p.fill_ink(&body, [246, 246, 250, 255], ink, s * 0.1);
    p.line(
        &[[x - s * 0.3, y + s * 0.04], [x + s * 0.3, y + s * 0.04]],
        s * 0.1,
        color,
    );
    p.fill_ink(
        &vector::circle(x, y - s * 0.38, s * 0.3),
        [250, 214, 186, 255],
        ink,
        s * 0.08,
    );
    p.fill(
        &vector::arc(
            x,
            y - s * 0.42,
            s * 0.31,
            s * 0.3,
            std::f32::consts::PI,
            std::f32::consts::TAU,
            12,
        ),
        Paint::Solid(color),
    );
    p.line(
        &[[x - s * 0.36, y - s * 0.42], [x + s * 0.36, y - s * 0.42]],
        s * 0.1,
        shade(color, -0.3),
    );
    if bat {
        p.line(
            &[[x + s * 0.3, y + s * 0.05], [x + s * 0.75, y - s * 0.75]],
            s * 0.16,
            [214, 170, 104, 255],
        );
    }
}

/// The ball for the latest play, `k` 0..1 through its flight.
pub fn ball_flight(p: &mut Painter, home: (f32, f32, f32), kind: PlayKind, dir: f32, k: f32) {
    let k = k.clamp(0.0, 1.0);
    let (dist, height) = match kind {
        PlayKind::Hit(4) => (115.0, 70.0),
        PlayKind::Hit(3) => (92.0, 40.0),
        PlayKind::Hit(2) => (85.0, 34.0),
        PlayKind::Hit(_) => (55.0, 16.0),
        PlayKind::FlyOut | PlayKind::SacFly => (72.0, 60.0),
        PlayKind::GroundOut | PlayKind::DoublePlay => (32.0, 0.0),
        PlayKind::Bunt => (8.0, 0.0),
        _ => return,
    };
    let e = 1.0 - (1.0 - k).powi(2);
    let ground = field_pt(home, dist * e, dir);
    let lift = (k * std::f32::consts::PI).sin() * height * home.2;
    p.fill(
        &vector::ellipse(ground[0], ground[1], 3.5, 1.6),
        Paint::Solid([0, 0, 0, 90]),
    );
    let r = 3.2 + lift * 0.03;
    p.glow(ground[0], ground[1] - lift, r * 3.0, [255, 255, 255, 90]);
    p.dot(ground[0], ground[1] - lift, r, WHITE);
    p.outline(
        &vector::circle(ground[0], ground[1] - lift, r),
        0.6,
        [200, 60, 60, 255],
    );
}

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
