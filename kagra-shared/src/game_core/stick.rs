//! A floating thumb stick any kit can use: where its base lands, how the base follows a thumb
//! past the rim, the analog push with a dead zone, and its look (base, rim, a jelly neck and a
//! soft knob). Discs and rings are rounded quads (the GPU draws the edge). The jelly neck is
//! the hull of the base and the knob, filled one pixel row at a time with a part-covered quad
//! at each row end, so nothing in it is drawn twice. First used by Moonlit Swarm.

use crate::scene::{DrawList, Quad};

/// Where the base lands for a thumb at (`x`, `y`): there, but kept `rad + margin` off every edge.
pub fn land(x: f32, y: f32, rad: f32, margin: f32, w: f32, h: f32) -> (f32, f32) {
    let m = rad + margin;
    (x.clamp(m, (w - m).max(m)), y.clamp(m, (h - m).max(m)))
}

/// The base after the thumb moved to `finger`: past the rim it is dragged along so the thumb
/// sits on the rim (turning back is instant), inside it stays put.
pub fn follow(base: (f32, f32), finger: (f32, f32), rad: f32) -> (f32, f32) {
    let (dx, dy) = (finger.0 - base.0, finger.1 - base.1);
    let d = dx.hypot(dy);
    if d <= rad {
        return base;
    }
    let k = (d - rad) / d;
    (base.0 + dx * k, base.1 + dy * k)
}

/// Where the knob is drawn: the thumb, clamped to the rim.
pub fn knob(base: (f32, f32), finger: (f32, f32), rad: f32) -> (f32, f32) {
    let (dx, dy) = (finger.0 - base.0, finger.1 - base.1);
    let d = dx.hypot(dy).max(1e-3);
    let k = d.min(rad) / d;
    (base.0 + dx * k, base.1 + dy * k)
}

/// The push, -1..=1 on each axis (screen x right, y down): nothing inside `dead` (a fraction
/// of the radius), then a smooth ramp to full at the rim.
pub fn analog(base: (f32, f32), finger: (f32, f32), rad: f32, dead: f32) -> (f32, f32) {
    let (dx, dy) = ((finger.0 - base.0) / rad, (finger.1 - base.1) / rad);
    let len = dx.hypot(dy);
    if len <= dead {
        return (0.0, 0.0);
    }
    let m = ((len.min(1.0) - dead) / (1.0 - dead)).clamp(0.0, 1.0);
    (dx / len * m, dy / len * m)
}

/// Colours of the stick (RGBA). [`StickLook::default`] is Moonlit Swarm's dark violet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickLook {
    pub base: [u8; 4],
    pub rim: [u8; 4],
    pub neck: [u8; 4],
    pub knob: [u8; 4],
    pub glint: [u8; 4],
    /// The resting hint: base, rim, inner dot.
    pub rest: [[u8; 4]; 3],
}

impl Default for StickLook {
    fn default() -> Self {
        Self {
            base: [10, 6, 22, 120],
            rim: [160, 125, 255, 150],
            neck: [110, 80, 210, 120],
            knob: [205, 185, 255, 215],
            glint: [255, 255, 255, 110],
            rest: [[10, 6, 22, 60], [150, 120, 230, 70], [150, 120, 230, 55]],
        }
    }
}

/// One filled row from `l` to `r` at `y`, its two ends faded by how much of the end pixel is
/// inside.
fn span(list: &mut DrawList, y: f32, l: f32, r: f32, c: [u8; 4]) {
    if r - l < 0.5 {
        return;
    }
    let (li, ri) = (l.ceil(), r.floor());
    if ri > li {
        list.quads.push(Quad::new(li, y, ri - li, 1.0, c));
    }
    let edge = |cov: f32| [c[0], c[1], c[2], (c[3] as f32 * cov.clamp(0.0, 1.0)) as u8];
    if li - l > 0.02 {
        list.quads
            .push(Quad::new(li - 1.0, y, 1.0, 1.0, edge(li - l)));
    }
    if r - ri > 0.02 {
        list.quads.push(Quad::new(ri, y, 1.0, 1.0, edge(r - ri)));
    }
}

/// The left and right edge of a circle on the row through `y`, if the row crosses it.
fn chord(cx: f32, cy: f32, rad: f32, y: f32) -> Option<(f32, f32)> {
    let dy = y - cy;
    (dy.abs() < rad).then(|| {
        let half = (rad * rad - dy * dy).sqrt();
        (cx - half, cx + half)
    })
}

/// A filled circle (the GPU rounds it: `Quad::circle`).
pub fn smooth_disc(list: &mut DrawList, cx: f32, cy: f32, rad: f32, c: [u8; 4]) {
    list.quads.push(Quad::circle(cx, cy, rad, c));
}

/// A ring from `rad - width` to `rad`.
pub fn smooth_ring(list: &mut DrawList, cx: f32, cy: f32, rad: f32, width: f32, c: [u8; 4]) {
    list.quads.push(Quad::ring(cx, cy, rad, width, c));
}

/// The convex hull of a chain of circles, one span per row (left-most to right-most edge of
/// any circle the row crosses). Exact for a chain that tapers evenly between two circles.
fn blob(list: &mut DrawList, circles: &[(f32, f32, f32)], c: [u8; 4]) {
    let top = circles
        .iter()
        .map(|&(_, y, r)| y - r)
        .fold(f32::MAX, f32::min);
    let bottom = circles
        .iter()
        .map(|&(_, y, r)| y + r)
        .fold(f32::MIN, f32::max);
    let mut y = top.floor();
    while y < bottom {
        let mid = y + 0.5;
        let (mut l, mut r) = (f32::MAX, f32::MIN);
        for &(cx, cy, rad) in circles {
            if let Some((a, b)) = chord(cx, cy, rad, mid) {
                l = l.min(a);
                r = r.max(b);
            }
        }
        if l < r {
            span(list, y, l, r, c);
        }
        y += 1.0;
    }
}

/// Draw the stick: base at `b` with radius `rad`, knob at `k` (use [`knob`]); `s` is the HUD
/// scale. `live` false draws the resting hint where a thumb would land.
pub fn draw(
    list: &mut DrawList,
    b: (f32, f32),
    k: (f32, f32),
    rad: f32,
    s: f32,
    live: bool,
    look: &StickLook,
) {
    let (bx, by) = b;
    if !live {
        smooth_disc(list, bx, by, rad, look.rest[0]);
        smooth_ring(list, bx, by, rad, 2.0 * s, look.rest[1]);
        smooth_disc(list, bx, by, rad * 0.4, look.rest[2]);
        return;
    }
    let (kx, ky) = k;
    smooth_disc(list, bx, by, rad, look.base);
    smooth_ring(list, bx, by, rad, 2.5 * s, look.rim);
    // The jelly: thick at the base, thinner as it stretches, as wide as the knob at the end.
    let stretch = ((kx - bx).hypot(ky - by) / rad).min(1.0);
    let (r0, r1) = (rad * (0.44 - 0.16 * stretch), rad * 0.42);
    let n = 8;
    let chain: Vec<(f32, f32, f32)> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            (bx + (kx - bx) * t, by + (ky - by) * t, r0 + (r1 - r0) * t)
        })
        .collect();
    blob(list, &chain, look.neck);
    smooth_disc(list, kx, ky, rad * 0.42, look.knob);
    // A glint up and to the left, so the knob reads as a soft ball.
    smooth_disc(
        list,
        kx - rad * 0.13,
        ky - rad * 0.14,
        rad * 0.13,
        look.glint,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(list: &DrawList, x: f32, y: f32) -> u32 {
        list.quads
            .iter()
            .filter(|q| q.contains(x, y))
            .map(|q| q.color[3] as u32)
            .sum()
    }

    #[test]
    fn the_base_lands_on_screen_follows_past_the_rim_and_pushes_analog() {
        assert_eq!(land(5.0, 900.0, 60.0, 4.0, 390.0, 844.0), (64.0, 780.0));
        let base = (100.0, 500.0);
        assert_eq!(
            follow(base, (130.0, 500.0), 60.0),
            base,
            "inside the rim it stays"
        );
        assert_eq!(
            follow(base, (280.0, 500.0), 60.0),
            (220.0, 500.0),
            "past it, dragged along"
        );
        assert_eq!(knob(base, (280.0, 500.0), 60.0), (160.0, 500.0));
        assert_eq!(
            analog(base, (103.0, 500.0), 60.0, 0.12),
            (0.0, 0.0),
            "dead zone"
        );
        let (x, y) = analog(base, (160.0, 500.0), 60.0, 0.12);
        assert!((x - 1.0).abs() < 1e-5 && y == 0.0, "full at the rim");
        let (half, _) = analog(base, (130.0, 500.0), 60.0, 0.12);
        assert!(half > 0.3 && half < 0.6, "a ramp: {half}");
    }

    #[test]
    fn discs_and_rings_are_single_rounded_quads() {
        let mut l = DrawList::default();
        smooth_disc(&mut l, 100.0, 100.0, 40.0, [255, 255, 255, 200]);
        smooth_ring(&mut l, 100.0, 100.0, 40.0, 3.0, [255, 255, 255, 200]);
        assert_eq!(l.quads.len(), 2);
        let (d, r) = (l.quads[0], l.quads[1]);
        assert_eq!(
            (d.x, d.y, d.w, d.radius, d.stroke),
            (60.0, 60.0, 80.0, 40.0, 0.0)
        );
        assert_eq!((r.radius, r.stroke), (40.0, 3.0));
    }

    #[test]
    fn the_jelly_is_drawn_once_per_row() {
        let mut l = DrawList::default();
        blob(
            &mut l,
            &[
                (100.0, 100.0, 20.0),
                (130.0, 100.0, 20.0),
                (160.0, 100.0, 20.0),
            ],
            [255, 255, 255, 100],
        );
        // Overlapping circles do not stack alpha.
        assert_eq!(alpha_at(&l, 130.5, 100.5), 100);
        assert_eq!(alpha_at(&l, 115.5, 100.5), 100);
    }
}
