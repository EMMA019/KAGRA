//! Game-agnostic HUD pieces: the 9-slice panel, a coin icon, a counter chip and the
//! screen-size scale (plain quads, a port of `kagra.ui2d.nine_slice`), and smooth rounded
//! pieces on the GPU's rounded quads: [`round_panel`], [`gauge`], [`pill_button`].
//! No kit-specific names live here; a game passes colours from its `HudSpec`.

use crate::scene::{DrawList, Quad, TextAlign, TextQuad};

/// HUD units -> buffer pixels. 1.0 at a 412 px wide phone buffer, so a CSS-pixel
/// size on a 2x phone is `size * 2`. Clamped so a desktop window stays sane.
pub fn ui_scale(width: u32, height: u32, spec_scale: f32) -> f32 {
    (width.min(height) as f32 / 412.0).clamp(0.8, 3.0) * spec_scale.max(0.1)
}

/// `rgba` with its alpha scaled by `k` (0..1).
pub fn fade(rgba: [u8; 4], k: f32) -> [u8; 4] {
    [
        rgba[0],
        rgba[1],
        rgba[2],
        (rgba[3] as f32 * k.clamp(0.0, 1.0)).round() as u8,
    ]
}

/// Nine quads: four corners and four edges in `border`, the centre in `fill`.
/// `slice` is the corner size; `shadow` is `(dx, dy, alpha)`, alpha 0 skips it.
/// The corners are notched by one slice so the panel reads as rounded.
#[allow(clippy::too_many_arguments)]
pub fn nine_slice(
    list: &mut DrawList,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    slice: f32,
    fill: [u8; 4],
    border: [u8; 4],
    shadow: (f32, f32, u8),
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let s = slice.max(0.0).min(w * 0.5).min(h * 0.5);
    let (dx, dy, a) = shadow;
    if a > 0 {
        let k = border[3] as f32 / 255.0;
        list.quads.push(Quad::new(
            x + dx,
            y + dy,
            w,
            h,
            [0, 0, 0, (a as f32 * k).round() as u8],
        ));
    }
    let q = |list: &mut DrawList, qx: f32, qy: f32, qw: f32, qh: f32, c: [u8; 4]| {
        if qw > 0.0 && qh > 0.0 {
            list.quads.push(Quad::new(qx, qy, qw, qh, c));
        }
    };
    // Rim: top and bottom bars lose a corner each side, side bars run full
    // height between them, leaving the corner pixels empty (a notch).
    q(list, x + s, y, w - 2.0 * s, s, border);
    q(list, x + s, y + h - s, w - 2.0 * s, s, border);
    q(list, x, y + s, s, h - 2.0 * s, border);
    q(list, x + w - s, y + s, s, h - 2.0 * s, border);
    // Corners: a smaller square so the rim turns instead of ending.
    let c = s * 0.6;
    q(list, x + s * 0.4, y + s * 0.4, c, c, border);
    q(list, x + w - s * 0.4 - c, y + s * 0.4, c, c, border);
    q(list, x + s * 0.4, y + h - s * 0.4 - c, c, c, border);
    q(list, x + w - s * 0.4 - c, y + h - s * 0.4 - c, c, c, border);
    q(list, x + s, y + s, w - 2.0 * s, h - 2.0 * s, fill);
    // A faint inner highlight on the top edge.
    q(
        list,
        x + s,
        y + s,
        w - 2.0 * s,
        (s * 0.5).max(1.0),
        [255, 255, 255, (fill[3] as f32 * 0.12) as u8],
    );
}

/// A rounded panel: `fill` with a `rim` band `rim_w` wide inside its edge (0 = no rim) and an
/// optional drop shadow `(dx, dy, alpha)`. Smooth corners (rounded quads, drawn on the GPU).
#[allow(clippy::too_many_arguments)]
pub fn round_panel(
    list: &mut DrawList,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    fill: [u8; 4],
    rim: [u8; 4],
    rim_w: f32,
    shadow: (f32, f32, u8),
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let (dx, dy, a) = shadow;
    if a > 0 {
        list.quads
            .push(Quad::round(x + dx, y + dy, w, h, radius, [0, 0, 0, a]));
    }
    list.quads.push(Quad::round(x, y, w, h, radius, fill));
    if rim_w > 0.0 && rim[3] > 0 {
        list.quads.push(Quad {
            stroke: rim_w,
            ..Quad::round(x, y, w, h, radius, rim)
        });
    }
}

/// A pill-shaped gauge: the track in `bg`, the filled share `frac` (0..=1) in `fg`, and a
/// faint highlight on the top half. Returns nothing; HP, XP, charge and timers all use it.
#[allow(clippy::too_many_arguments)]
pub fn gauge(
    list: &mut DrawList,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    frac: f32,
    fg: [u8; 4],
    bg: [u8; 4],
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let r = h * 0.5;
    list.quads.push(Quad::round(x, y, w, h, r, bg));
    let f = frac.clamp(0.0, 1.0);
    if f > 0.0 {
        // Never thinner than the pill's own height, so a nearly empty bar still reads round.
        let fw = (w * f).max(h.min(w));
        list.quads.push(Quad::round(x, y, fw, h, r, fg));
        list.quads.push(Quad::round(
            x + r * 0.5,
            y + h * 0.12,
            (fw - r).max(0.0),
            h * 0.32,
            h * 0.16,
            [255, 255, 255, 60],
        ));
    }
}

/// A pill button with a centred label. Returns its rectangle for hit tests (`Quad::contains`).
#[allow(clippy::too_many_arguments)]
pub fn pill_button(
    list: &mut DrawList,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: &str,
    size: f32,
    fill: [u8; 4],
    text: [u8; 4],
) -> Quad {
    round_panel(
        list,
        x,
        y,
        w,
        h,
        h * 0.5,
        fill,
        [255, 255, 255, 60],
        (h * 0.06).max(1.0),
        (0.0, h * 0.08, 70),
    );
    list.texts.push(
        TextQuad::new(label, x + w * 0.5, y + (h - size) * 0.5, size, text)
            .aligned(TextAlign::Center),
    );
    Quad::new(x, y, w, h, fill)
}

/// A filled circle from horizontal strips (round without a texture).
fn disc(list: &mut DrawList, cx: f32, cy: f32, rad: f32, c: [u8; 4], n: i32) {
    let step = rad / (n as f32 + 0.5);
    for k in -n..=n {
        let dy = k as f32 * step;
        let half = (rad * rad - dy * dy).max(0.0).sqrt();
        list.quads.push(Quad::new(
            cx - half,
            cy + dy - step * 0.5,
            half * 2.0,
            step + 0.3,
            c,
        ));
    }
}

/// A paw print: one big pad and four toes. `(cx, cy)` is the centre of the print.
pub fn paw_icon(list: &mut DrawList, cx: f32, cy: f32, r: f32, color: [u8; 4]) {
    disc(list, cx, cy + r * 0.3, r * 0.58, color, 3);
    for (dx, dy) in [(-0.74, -0.1), (-0.27, -0.58), (0.27, -0.58), (0.74, -0.1)] {
        disc(list, cx + dx * r, cy + dy * r, r * 0.27, color, 2);
    }
}

/// A coin: dark rim, bright face, a slot down the middle, a glint. Drawn from
/// horizontal strips so it is round without a texture. `(cx, cy)` is the centre.
pub fn coin_icon(list: &mut DrawList, cx: f32, cy: f32, r: f32, color: [u8; 4]) {
    let disc = |list: &mut DrawList, rad: f32, c: [u8; 4]| disc(list, cx, cy, rad, c, 7);
    let dark = [
        (color[0] as f32 * 0.55) as u8,
        (color[1] as f32 * 0.45) as u8,
        (color[2] as f32 * 0.3) as u8,
        color[3],
    ];
    disc(list, r, dark);
    disc(list, r * 0.78, color);
    list.quads.push(Quad::new(
        cx - r * 0.1,
        cy - r * 0.42,
        r * 0.2,
        r * 0.84,
        dark,
    ));
    list.quads.push(Quad::new(
        cx - r * 0.5,
        cy - r * 0.5,
        r * 0.22,
        r * 0.22,
        fade([255, 255, 255, 255], color[3] as f32 / 255.0 * 0.7),
    ));
}

/// A rounded counter chip: panel, icon on the left, number to its right.
#[allow(clippy::too_many_arguments)]
pub fn counter_chip(
    list: &mut DrawList,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    ui: f32,
    fill: [u8; 4],
    border: [u8; 4],
    color: [u8; 4],
    value: &str,
) {
    nine_slice(
        list,
        x,
        y,
        w,
        h,
        3.0 * ui,
        fill,
        border,
        (2.0 * ui, 3.0 * ui, 90),
    );
    coin_icon(list, x + h * 0.5 + 2.0 * ui, y + h * 0.5, h * 0.3, color);
    list.texts.push(
        TextQuad::new(
            value,
            x + w - 10.0 * ui,
            y + h * 0.5 - 11.0 * ui,
            22.0 * ui,
            fade(color, 1.0),
        )
        .aligned(TextAlign::Right),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_pieces_are_a_few_rounded_quads() {
        let mut l = DrawList::default();
        round_panel(
            &mut l,
            10.0,
            10.0,
            200.0,
            80.0,
            16.0,
            [20, 20, 40, 220],
            [255, 255, 255, 80],
            2.0,
            (0.0, 4.0, 90),
        );
        assert_eq!(l.quads.len(), 3, "shadow, fill, rim");
        assert!(l.quads.iter().all(|q| q.radius == 16.0));
        assert_eq!(l.quads[2].stroke, 2.0);
        let mut g = DrawList::default();
        gauge(
            &mut g,
            0.0,
            0.0,
            100.0,
            10.0,
            0.5,
            [0, 255, 0, 255],
            [0, 0, 0, 160],
        );
        assert_eq!(g.quads[1].w, 50.0);
        let mut e = DrawList::default();
        gauge(
            &mut e,
            0.0,
            0.0,
            100.0,
            10.0,
            0.01,
            [0, 255, 0, 255],
            [0, 0, 0, 160],
        );
        assert_eq!(e.quads[1].w, 10.0, "a nearly empty bar stays a round dot");
        let mut b = DrawList::default();
        let r = pill_button(
            &mut b,
            20.0,
            30.0,
            160.0,
            48.0,
            "START",
            20.0,
            [90, 60, 200, 255],
            [255; 4],
        );
        assert!(r.contains(100.0, 54.0) && !r.contains(10.0, 54.0));
        assert_eq!(b.texts[0].text, "START");
    }

    fn empty() -> DrawList {
        DrawList {
            clear: [0; 4],
            quads: Vec::new(),
            texts: Vec::new(),
            images: Vec::new(),
        }
    }

    #[test]
    fn scale_follows_the_short_side() {
        assert_eq!(ui_scale(824, 1800, 1.0), 2.0);
        assert_eq!(ui_scale(1800, 824, 1.0), 2.0);
        assert_eq!(ui_scale(100, 100, 1.0), 0.8);
        assert_eq!(ui_scale(824, 1800, 1.5), 3.0);
    }

    #[test]
    fn panel_stays_inside_its_rectangle() {
        let mut l = empty();
        nine_slice(
            &mut l,
            10.0,
            20.0,
            100.0,
            40.0,
            4.0,
            [1; 4],
            [2; 4],
            (0.0, 0.0, 0),
        );
        for q in &l.quads {
            assert!(q.x >= 10.0 - 1e-3 && q.x + q.w <= 110.0 + 1e-3, "{q:?}");
            assert!(q.y >= 20.0 - 1e-3 && q.y + q.h <= 60.0 + 1e-3, "{q:?}");
        }
        assert!(l.quads.iter().any(|q| q.color == [1; 4]), "has a fill");
        assert!(l.quads.iter().any(|q| q.color == [2; 4]), "has a rim");
    }

    #[test]
    fn a_tiny_panel_does_not_invert() {
        let mut l = empty();
        nine_slice(
            &mut l,
            0.0,
            0.0,
            6.0,
            6.0,
            20.0,
            [1; 4],
            [2; 4],
            (1.0, 1.0, 80),
        );
        assert!(l.quads.iter().all(|q| q.w >= 0.0 && q.h >= 0.0));
    }

    #[test]
    fn coin_is_round_and_centred() {
        let mut l = empty();
        coin_icon(&mut l, 50.0, 50.0, 10.0, [255, 205, 70, 255]);
        let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for q in &l.quads {
            x0 = x0.min(q.x);
            x1 = x1.max(q.x + q.w);
            y0 = y0.min(q.y);
            y1 = y1.max(q.y + q.h);
        }
        assert!(((x0 + x1) * 0.5 - 50.0).abs() < 0.2 && ((y0 + y1) * 0.5 - 50.0).abs() < 0.2);
        assert!((x1 - x0 - 20.0).abs() < 0.8 && (y1 - y0 - 20.0).abs() < 0.8);
    }

    #[test]
    fn paw_is_centred_and_fits_its_cell() {
        let mut l = empty();
        paw_icon(&mut l, 50.0, 50.0, 10.0, [255, 150, 205, 255]);
        let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for q in &l.quads {
            x0 = x0.min(q.x);
            x1 = x1.max(q.x + q.w);
            y0 = y0.min(q.y);
            y1 = y1.max(q.y + q.h);
        }
        assert!(((x0 + x1) * 0.5 - 50.0).abs() < 0.6 && ((y0 + y1) * 0.5 - 50.0).abs() < 1.2);
        // Wider than tall, and inside 2.2 radii so nine of them keep a gap.
        assert!(
            x1 - x0 < 22.0 && y1 - y0 < 19.0,
            "{} x {}",
            x1 - x0,
            y1 - y0
        );
        assert!(l.quads.len() > 12, "pad plus four toes");
    }
}
