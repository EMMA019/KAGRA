//! Free CC0 pictures stamped into the HUD as coloured cells. The phone draws the same
//! mesh as the screenshots (no extra texture bind). Cells stay coarse so a frame stays light.
//!
//! Sources live in `assets/diamond_days/` and the ledger. The pixel stadium is not used:
//! it would sit unmodified next to the illustrated field.

use std::cell::RefCell;
use std::sync::{Arc, OnceLock};

use crate::gltf_load::decode_png;
use crate::hud_art::Painter;
use crate::scene::{Mesh2d, TextAlign, Vert2};
use crate::scene3d::AlbedoRgba;

const COUNTRY: u8 = 0;
const SKY_DAY: u8 = 1;
const SKY_NIGHT: u8 = 2;
const GRASS: u8 = 3;
const BALL: u8 = 4;
const PAPER: u8 = 5;
const BANNER: u8 = 6;
const BTN_GREEN: u8 = 7;
const BTN_GOLD: u8 = 8;
const STAR: u8 = 9;
const MEDAL_GOLD: u8 = 10;
const MEDAL_SILVER: u8 = 11;
const MEDAL_BRONZE: u8 = 12;

fn library() -> &'static [AlbedoRgba] {
    static LIB: OnceLock<Vec<AlbedoRgba>> = OnceLock::new();
    LIB.get_or_init(|| {
        const FILES: [&[u8]; 13] = [
            include_bytes!("../../../assets/diamond_days/country.png"),
            include_bytes!("../../../assets/diamond_days/sky_day.png"),
            include_bytes!("../../../assets/diamond_days/sky_night.png"),
            include_bytes!("../../../assets/diamond_days/grass.png"),
            include_bytes!("../../../assets/diamond_days/ball.png"),
            include_bytes!("../../../assets/diamond_days/paper.png"),
            include_bytes!("../../../assets/diamond_days/banner.png"),
            include_bytes!("../../../assets/diamond_days/btn_green.png"),
            include_bytes!("../../../assets/diamond_days/btn_gold.png"),
            include_bytes!("../../../assets/diamond_days/star.png"),
            include_bytes!("../../../assets/diamond_days/medal_gold.png"),
            include_bytes!("../../../assets/diamond_days/medal_silver.png"),
            include_bytes!("../../../assets/diamond_days/medal_bronze.png"),
        ];
        FILES
            .into_iter()
            .map(|b| {
                decode_png(b).unwrap_or_else(|_| AlbedoRgba {
                    width: 1,
                    height: 1,
                    rgba: Arc::<[u8]>::from(vec![0, 0, 0, 0]),
                })
            })
            .collect()
    })
}

thread_local! {
    static CACHE: RefCell<Vec<(Key, Mesh2d)>> = const { RefCell::new(Vec::new()) };
}

struct Blit {
    pic: u8,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    cell: f32,
    alpha: u8,
    uv: [f32; 4],
    cap: i32,
}

fn sample(img: &AlbedoRgba, u: f32, v: f32) -> [u8; 4] {
    let x = (u.clamp(0.0, 0.999) * img.width as f32) as u32;
    let y = (v.clamp(0.0, 0.999) * img.height as f32) as u32;
    let i = ((y * img.width + x) * 4) as usize;
    [
        img.rgba[i],
        img.rgba[i + 1],
        img.rgba[i + 2],
        img.rgba[i + 3],
    ]
}

fn build(img: &AlbedoRgba, screen: [f32; 4], b: &Blit) -> Mesh2d {
    let [x, y, w, h] = screen;
    let mut mesh = Mesh2d::default();
    if img.width < 2 || w < 2.0 || h < 2.0 {
        return mesh;
    }
    let mut cols = (w / b.cell).ceil().max(1.0) as i32;
    let mut rows = (h / b.cell).ceil().max(1.0) as i32;
    if cols * rows > b.cap {
        let k = ((cols * rows) as f32 / b.cap as f32).sqrt();
        cols = ((cols as f32) / k).ceil().max(1.0) as i32;
        rows = ((rows as f32) / k).ceil().max(1.0) as i32;
    }
    let cw = w / cols as f32;
    let ch = h / rows as f32;
    let (u0, v0, u1, v1) = (b.uv[0], b.uv[1], b.uv[2], b.uv[3]);
    for row in 0..rows {
        for col in 0..cols {
            let c = sample(
                img,
                u0 + (col as f32 + 0.5) / cols as f32 * (u1 - u0),
                v0 + (row as f32 + 0.5) / rows as f32 * (v1 - v0),
            );
            let a = (c[3] as u16 * b.alpha as u16 / 255) as u8;
            if a < 18 {
                continue;
            }
            let color = [c[0], c[1], c[2], a];
            let x0 = x + col as f32 * cw;
            let y0 = y + row as f32 * ch;
            let v = |px: f32, py: f32| Vert2::new(px, py, color);
            mesh.tri(v(x0, y0), v(x0 + cw, y0), v(x0 + cw, y0 + ch));
            mesh.tri(v(x0, y0), v(x0 + cw, y0 + ch), v(x0, y0 + ch));
        }
    }
    mesh
}

type Key = (u8, i32, i32, i32, i32, u16, u8, i16, i16, i16, i16);

fn paint(p: &mut Painter, mut b: Blit) {
    let img = &library()[b.pic as usize];
    let k = p.st.k.max(0.2);
    let sx = p.st.x(b.x);
    let sy = p.st.y(b.y);
    let sw = (b.w * k).max(1.0);
    let sh = (b.h * k).max(1.0);
    b.cell = (b.cell * k).clamp(1.6, 12.0);
    let key: Key = (
        b.pic,
        sx.round() as i32,
        sy.round() as i32,
        sw.round() as i32,
        sh.round() as i32,
        (b.cell * 10.0).round() as u16,
        b.alpha,
        (b.uv[0] * 1000.0) as i16,
        (b.uv[1] * 1000.0) as i16,
        (b.uv[2] * 1000.0) as i16,
        (b.uv[3] * 1000.0) as i16,
    );
    let verts = CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((_, mesh)) = c.iter().find(|(k, _)| *k == key) {
            return mesh.verts.clone();
        }
        let mesh = build(img, [sx, sy, sw, sh], &b);
        let verts = mesh.verts.clone();
        if c.len() > 96 {
            c.remove(0);
        }
        c.push((key, mesh));
        verts
    });
    p.mesh().verts.extend(verts);
}

fn full(pic: u8, rect: [f32; 4], cell: f32, alpha: u8, cap: i32) -> Blit {
    Blit {
        pic,
        x: rect[0],
        y: rect[1],
        w: rect[2],
        h: rect[3],
        cell,
        alpha,
        uv: [0.0, 0.0, 1.0, 1.0],
        cap,
    }
}

/// Keep the picture's border and repeat its middle, so a small panel can grow.
fn nineslice(p: &mut Painter, pic: u8, x: f32, y: f32, w: f32, h: f32, cell: f32) {
    let b = 0.22;
    let m = 16.0_f32.min(w * 0.28).min(h * 0.28);
    let parts = [
        (x, y, m, m, [0.0, 0.0, b, b]),
        (x + w - m, y, m, m, [1.0 - b, 0.0, 1.0, b]),
        (x, y + h - m, m, m, [0.0, 1.0 - b, b, 1.0]),
        (x + w - m, y + h - m, m, m, [1.0 - b, 1.0 - b, 1.0, 1.0]),
        (x + m, y, w - 2.0 * m, m, [b, 0.0, 1.0 - b, b]),
        (x + m, y + h - m, w - 2.0 * m, m, [b, 1.0 - b, 1.0 - b, 1.0]),
        (x, y + m, m, h - 2.0 * m, [0.0, b, b, 1.0 - b]),
        (x + w - m, y + m, m, h - 2.0 * m, [1.0 - b, b, 1.0, 1.0 - b]),
    ];
    for (px, py, pw, ph, uv) in parts {
        paint(
            p,
            Blit {
                pic,
                x: px,
                y: py,
                w: pw,
                h: ph,
                cell,
                alpha: 255,
                uv,
                cap: 400,
            },
        );
    }
    let tile = 36.0_f32;
    let mut ty = y + m;
    while ty < y + h - m - 0.5 {
        let th = tile.min(y + h - m - ty);
        let mut tx = x + m;
        while tx < x + w - m - 0.5 {
            let tw = tile.min(x + w - m - tx);
            paint(
                p,
                Blit {
                    pic,
                    x: tx,
                    y: ty,
                    w: tw,
                    h: th,
                    cell,
                    alpha: 255,
                    uv: [b, b, 1.0 - b, 1.0 - b],
                    cap: 500,
                },
            );
            tx += tw;
        }
        ty += th;
    }
}

/// Cartoon countryside (Segel) behind the week's sky. Not a stadium.
pub fn country(p: &mut Painter, x: f32, y: f32, w: f32, h: f32) {
    paint(p, full(COUNTRY, [x, y, w, h], 3.2, 255, 7000));
}

/// A graded sky plate. `night` is the moonless plate; day is the blue-hour one.
pub fn sky(p: &mut Painter, night: bool, x: f32, y: f32, w: f32, h: f32) {
    let pic = if night { SKY_NIGHT } else { SKY_DAY };
    paint(p, full(pic, [x, y, w, h], 8.0, 255, 2200));
}

/// Mown-grass stripes over the outfield, kept translucent so the diamond still reads.
pub fn grass(p: &mut Painter, x: f32, y: f32, w: f32, h: f32) {
    paint(p, full(GRASS, [x, y, w, h], 3.5, 200, 2200));
}

pub fn ball(p: &mut Painter, x: f32, y: f32, size: f32) {
    paint(p, full(BALL, [x, y, size, size], 1.5, 255, 1400));
}

pub fn paper(p: &mut Painter, x: f32, y: f32, w: f32, h: f32) {
    nineslice(p, PAPER, x, y, w, h, 2.6);
}

pub fn banner(p: &mut Painter, x: f32, y: f32, w: f32, h: f32) {
    nineslice(p, BANNER, x, y, w, h, 2.2);
}

pub fn kenney_star(p: &mut Painter, cx: f32, cy: f32, size: f32) {
    paint(
        p,
        full(
            STAR,
            [cx - size * 0.5, cy - size * 0.5, size, size],
            2.5,
            255,
            800,
        ),
    );
}

/// 0 gold, 1 silver, 2 bronze.
pub fn medal(p: &mut Painter, x: f32, y: f32, which: u8) {
    let pic = match which {
        0 => MEDAL_GOLD,
        1 => MEDAL_SILVER,
        _ => MEDAL_BRONZE,
    };
    paint(p, full(pic, [x, y, 22.0, 44.0], 2.5, 255, 800));
}

/// A Kenney depth button with the label in ink. Gold is the primary action.
pub fn button(p: &mut Painter, rect: [f32; 4], label: &str, size: f32, gold: bool) {
    let pic = if gold { BTN_GOLD } else { BTN_GREEN };
    paint(p, full(pic, rect, 3.5, 255, 1200));
    let [x, y, w, h] = rect;
    let ink = if gold {
        [96, 62, 18, 255]
    } else {
        [16, 72, 48, 255]
    };
    p.text(
        label,
        x + w * 0.5,
        y + (h - size) * 0.42,
        size,
        ink,
        TextAlign::Center,
    );
}
