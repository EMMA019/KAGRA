//! The thumb stick, drawn smooth. Discs and rings are rounded quads (the GPU draws the edge).
//! The jelly neck is the hull of the base and the knob, filled one pixel row at a time with a
//! part-covered quad at each row end, so nothing in it is drawn twice.

use super::*;

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
pub(super) fn smooth_disc(list: &mut DrawList, cx: f32, cy: f32, rad: f32, c: [u8; 4]) {
    list.quads.push(Quad::circle(cx, cy, rad, c));
}

/// A ring from `rad - width` to `rad`.
pub(super) fn smooth_ring(list: &mut DrawList, cx: f32, cy: f32, rad: f32, width: f32, c: [u8; 4]) {
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

/// Base at `(bx, by)` with radius `rad`, knob at `(kx, ky)` (already inside the rim). `live`
/// is false for the resting hint where a thumb would land.
pub(super) fn draw_stick(
    list: &mut DrawList,
    b: (f32, f32),
    k: (f32, f32),
    rad: f32,
    s: f32,
    live: bool,
) {
    let (bx, by) = b;
    if !live {
        smooth_disc(list, bx, by, rad, [10, 6, 22, 60]);
        smooth_ring(list, bx, by, rad, 2.0 * s, [150, 120, 230, 70]);
        smooth_disc(list, bx, by, rad * 0.4, [150, 120, 230, 55]);
        return;
    }
    let (kx, ky) = k;
    smooth_disc(list, bx, by, rad, [10, 6, 22, 120]);
    smooth_ring(list, bx, by, rad, 2.5 * s, [160, 125, 255, 150]);
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
    blob(list, &chain, [110, 80, 210, 120]);
    smooth_disc(list, kx, ky, rad * 0.42, [205, 185, 255, 215]);
    // A glint up and to the left, so the knob reads as a soft ball.
    smooth_disc(
        list,
        kx - rad * 0.13,
        ky - rad * 0.14,
        rad * 0.13,
        [255, 255, 255, 110],
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
