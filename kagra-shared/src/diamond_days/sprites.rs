//! Free CC0 pictures drawn as engine textures ([`crate::image2d`]): registered once, packed
//! into the HUD picture atlas, and drawn in order with the vector art around them.
//!
//! Sources live in `assets/diamond_days/` and the ledger. The pixel stadium is not used:
//! it would sit unmodified next to the illustrated field.

use std::sync::OnceLock;

use crate::hud_art::Painter;
use crate::image2d::PicId;
use crate::scene::TextAlign;

const COUNTRY: usize = 0;
const SKY_DAY: usize = 1;
const SKY_NIGHT: usize = 2;
const BALL: usize = 3;
const PAPER: usize = 4;
const BANNER: usize = 5;
const BTN_GREEN: usize = 6;
const BTN_GOLD: usize = 7;
const STAR: usize = 8;
const MEDAL_GOLD: usize = 9;
const MEDAL_SILVER: usize = 10;
const MEDAL_BRONZE: usize = 11;

/// The game's pictures, registered on first use. A file that fails to decode draws nothing.
fn pics() -> &'static [Option<PicId>; 12] {
    static PICS: OnceLock<[Option<PicId>; 12]> = OnceLock::new();
    PICS.get_or_init(|| {
        const FILES: [&[u8]; 12] = [
            include_bytes!("../../../assets/diamond_days/country.png"),
            include_bytes!("../../../assets/diamond_days/sky_day.png"),
            include_bytes!("../../../assets/diamond_days/sky_night.png"),
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
        FILES.map(|b| crate::image2d::register_png(b).ok())
    })
}

fn image(p: &mut Painter, which: usize, rect: [f32; 4]) {
    if let Some(pic) = pics()[which] {
        p.image_part(pic, rect, [0.0, 0.0, 1.0, 1.0], [255; 4]);
    }
}

fn cover(p: &mut Painter, which: usize, rect: [f32; 4], focus: [f32; 2]) {
    if let Some(pic) = pics()[which] {
        p.image_cover(pic, rect, focus, [255; 4]);
    }
}

fn framed(p: &mut Painter, which: usize, rect: [f32; 4], border: f32, tile: f32) {
    if let Some(pic) = pics()[which] {
        p.nine_slice(pic, rect, border, 0.22, tile, [255; 4]);
    }
}

/// Cartoon countryside (Segel) behind the week's sky. Not a stadium.
/// Covers the rectangle at the picture's own aspect (wide screens crop the sky).
pub fn country(p: &mut Painter, x: f32, y: f32, w: f32, h: f32) {
    cover(p, COUNTRY, [x, y, w, h], [0.5, 1.0]);
}

/// A graded sky plate. `night` is the moonless plate; day is the blue-hour one.
pub fn sky(p: &mut Painter, night: bool, x: f32, y: f32, w: f32, h: f32) {
    cover(
        p,
        if night { SKY_NIGHT } else { SKY_DAY },
        [x, y, w, h],
        [0.5, 0.5],
    );
}

pub fn ball(p: &mut Painter, x: f32, y: f32, size: f32) {
    image(p, BALL, [x, y, size, size]);
}

/// A paper card: the rim keeps its size, the grain repeats so a tall card stays sharp.
pub fn paper(p: &mut Painter, x: f32, y: f32, w: f32, h: f32) {
    framed(p, PAPER, [x, y, w, h], 14.0, 40.0);
}

pub fn banner(p: &mut Painter, x: f32, y: f32, w: f32, h: f32) {
    framed(p, BANNER, [x, y, w, h], 16.0, 0.0);
}

pub fn kenney_star(p: &mut Painter, cx: f32, cy: f32, size: f32) {
    image(p, STAR, [cx - size * 0.5, cy - size * 0.5, size, size]);
}

/// 0 gold, 1 silver, 2 bronze.
pub fn medal(p: &mut Painter, x: f32, y: f32, which: u8) {
    let pic = match which {
        0 => MEDAL_GOLD,
        1 => MEDAL_SILVER,
        _ => MEDAL_BRONZE,
    };
    image(p, pic, [x, y, 22.0, 44.0]);
}

/// A Kenney depth button with the label in ink. Gold is the primary action.
pub fn button(p: &mut Painter, rect: [f32; 4], label: &str, size: f32, gold: bool) {
    let pic = if gold { BTN_GOLD } else { BTN_GREEN };
    framed(p, pic, rect, 12.0, 0.0);
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
