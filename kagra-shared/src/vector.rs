//! 2D vector art for HUDs and illustrations: fills, strokes, gradients, glows and clipping,
//! tessellated on the CPU into [`Mesh2d`] triangles (no GPU needed, so CI and wasm draw the
//! same picture).
//!
//! Every edge gets a thin fringe whose outer vertices are transparent: the GPU blends it into
//! a smooth, anti-aliased edge without MSAA. A wide fringe is a soft edge (drop shadows,
//! glows). Shapes are point lists ([`P`]) in screen pixels, built with the helpers here
//! ([`ellipse`], [`rounded_rect`], [`quad_bez`], ...) or by hand.
//!
//! ```
//! use kagra_shared::scene::Mesh2d;
//! use kagra_shared::vector::{self, Paint};
//! let mut m = Mesh2d::default();
//! let card = vector::rounded_rect(20.0, 20.0, 200.0, 120.0, 16.0);
//! vector::shadow(&mut m, &card, [0.0, 6.0], 14.0, [0, 0, 0, 70]);
//! vector::fill(&mut m, &card, &Paint::vertical(20.0, 140.0, [255, 253, 246, 255], [240, 232, 214, 255]));
//! vector::stroke_closed(&mut m, &card, 2.0, [200, 180, 140, 255]);
//! assert!(m.triangles() > 20);
//! ```

use crate::scene::{Mesh2d, Vert2};

/// A point in pixels.
pub type P = [f32; 2];
pub type Rgba = [u8; 4];

/// Anti-aliasing fringe width in pixels.
pub const AA: f32 = 1.0;

/// How a fill is coloured at each point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Paint {
    Solid(Rgba),
    /// `c0` at `from`, `c1` at `to`, blended along the line between them (clamped).
    Linear {
        from: P,
        to: P,
        c0: Rgba,
        c1: Rgba,
    },
    /// `c0` at `center`, `c1` at `radius` and beyond.
    Radial {
        center: P,
        radius: f32,
        c0: Rgba,
        c1: Rgba,
    },
}

impl Paint {
    /// Top-to-bottom gradient between two heights.
    pub fn vertical(top: f32, bottom: f32, c0: Rgba, c1: Rgba) -> Self {
        Paint::Linear {
            from: [0.0, top],
            to: [0.0, bottom],
            c0,
            c1,
        }
    }

    pub fn at(&self, p: P) -> Rgba {
        match *self {
            Paint::Solid(c) => c,
            Paint::Linear { from, to, c0, c1 } => {
                let d = [to[0] - from[0], to[1] - from[1]];
                let len2 = d[0] * d[0] + d[1] * d[1];
                let k = if len2 > 0.0 {
                    ((p[0] - from[0]) * d[0] + (p[1] - from[1]) * d[1]) / len2
                } else {
                    0.0
                };
                mix(c0, c1, k)
            }
            Paint::Radial {
                center,
                radius,
                c0,
                c1,
            } => {
                let r = ((p[0] - center[0]).powi(2) + (p[1] - center[1]).powi(2)).sqrt();
                mix(c0, c1, if radius > 0.0 { r / radius } else { 1.0 })
            }
        }
    }
}

/// Blend two colours, `k` 0 gives `a`, 1 gives `b` (clamped).
pub fn mix(a: Rgba, b: Rgba, k: f32) -> Rgba {
    let k = k.clamp(0.0, 1.0);
    std::array::from_fn(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * k).round() as u8)
}

/// Lighter (`k` > 0, toward white) or darker (`k` < 0, toward black); alpha kept.
pub fn shade(c: Rgba, k: f32) -> Rgba {
    let t = if k > 0.0 {
        [255, 255, 255, c[3]]
    } else {
        [0, 0, 0, c[3]]
    };
    mix(c, t, k.abs())
}

/// The same colour with alpha `a`.
pub fn alpha(c: Rgba, a: u8) -> Rgba {
    [c[0], c[1], c[2], a]
}

fn clear(c: Rgba) -> Rgba {
    [c[0], c[1], c[2], 0]
}

// ---- shapes ---------------------------------------------------------------------------------

/// Segments for a curve of radius `r` px: smooth on screen without wasting triangles.
pub fn segments(r: f32) -> usize {
    ((r.abs() * 0.75).sqrt() * 4.0).clamp(8.0, 96.0) as usize
}

/// An ellipse, clockwise on screen, starting at the right.
pub fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<P> {
    arc(
        cx,
        cy,
        rx,
        ry,
        0.0,
        std::f32::consts::TAU,
        segments(rx.max(ry)),
    )
    .into_iter()
    .take(segments(rx.max(ry)))
    .collect()
}

pub fn circle(cx: f32, cy: f32, r: f32) -> Vec<P> {
    ellipse(cx, cy, r, r)
}

/// Points on an elliptical arc from angle `a0` to `a1` (radians, 0 = right, positive =
/// clockwise on screen), `n` segments (`n + 1` points).
pub fn arc(cx: f32, cy: f32, rx: f32, ry: f32, a0: f32, a1: f32, n: usize) -> Vec<P> {
    let n = n.max(1);
    (0..=n)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            [cx + a.cos() * rx, cy + a.sin() * ry]
        })
        .collect()
}

/// A rectangle with rounded corners (`r` clamped to half the short side).
pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<P> {
    let r = r.max(0.0).min(w * 0.5).min(h * 0.5);
    if r < 0.5 {
        return vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
    }
    use std::f32::consts::{FRAC_PI_2, PI};
    let n = (segments(r) / 2).max(4);
    let mut pts = Vec::with_capacity(4 * (n + 1));
    pts.extend(arc(x + w - r, y + r, r, r, -FRAC_PI_2, 0.0, n));
    pts.extend(arc(x + w - r, y + h - r, r, r, 0.0, FRAC_PI_2, n));
    pts.extend(arc(x + r, y + h - r, r, r, FRAC_PI_2, PI, n));
    pts.extend(arc(x + r, y + r, r, r, PI, PI + FRAC_PI_2, n));
    pts
}

/// A quadratic Bezier from `p0` to `p2` bent toward `p1`, `n` segments.
pub fn quad_bez(p0: P, p1: P, p2: P, n: usize) -> Vec<P> {
    let n = n.max(1);
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let (a, b, c) = ((1.0 - t) * (1.0 - t), 2.0 * (1.0 - t) * t, t * t);
            [
                a * p0[0] + b * p1[0] + c * p2[0],
                a * p0[1] + b * p1[1] + c * p2[1],
            ]
        })
        .collect()
}

/// A cubic Bezier, `n` segments.
pub fn cubic_bez(p0: P, p1: P, p2: P, p3: P, n: usize) -> Vec<P> {
    let n = n.max(1);
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            [
                a * p0[0] + b * p1[0] + c * p2[0] + d * p3[0],
                a * p0[1] + b * p1[1] + c * p2[1] + d * p3[1],
            ]
        })
        .collect()
}

/// A star of `points` tips (`r_in` the inner radius), the first tip straight up.
pub fn star(cx: f32, cy: f32, r_out: f32, r_in: f32, points: usize) -> Vec<P> {
    let n = points.max(2) * 2;
    (0..n)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2
                + i as f32 * std::f32::consts::PI / points.max(2) as f32;
            let r = if i % 2 == 0 { r_out } else { r_in };
            [cx + a.cos() * r, cy + a.sin() * r]
        })
        .collect()
}

/// Map points through `x' = ox + x * s`, `y' = oy + y * s`.
pub fn place(pts: &[P], ox: f32, oy: f32, s: f32) -> Vec<P> {
    pts.iter().map(|p| [ox + p[0] * s, oy + p[1] * s]).collect()
}

/// Move points by (`dx`, `dy`).
pub fn shift(pts: &[P], dx: f32, dy: f32) -> Vec<P> {
    pts.iter().map(|p| [p[0] + dx, p[1] + dy]).collect()
}

/// Signed area (positive when the points run clockwise on screen, y down).
pub fn area(pts: &[P]) -> f32 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut a = 0.0;
    for i in 0..n {
        let (p, q) = (pts[i], pts[(i + 1) % n]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    a * 0.5
}

// ---- fills ----------------------------------------------------------------------------------

/// Fill a polygon (convex or not, not self-crossing) with a smooth 1 px edge.
pub fn fill(m: &mut Mesh2d, pts: &[P], paint: &Paint) {
    fill_soft(m, pts, paint, AA);
}

/// Fill a polygon whose edge fades out over `feather` px beyond it (0 = hard edge).
pub fn fill_soft(m: &mut Mesh2d, pts: &[P], paint: &Paint, feather: f32) {
    let poly = clean(pts);
    if poly.len() < 3 {
        return;
    }
    let orient = if area(&poly) >= 0.0 { 1.0 } else { -1.0 };
    let v = |p: P| Vert2::new(p[0], p[1], paint.at(p));
    for [a, b, c] in triangulate(&poly, orient) {
        m.tri(v(poly[a]), v(poly[b]), v(poly[c]));
    }
    if feather > 0.0 {
        let outs = offsets(&poly, orient, feather, true);
        let n = poly.len();
        for i in 0..n {
            let j = (i + 1) % n;
            let (a, b) = (poly[i], poly[j]);
            let (ao, bo) = (outs[i], outs[j]);
            let (ca, cb) = (paint.at(a), paint.at(b));
            m.tri(v(a), v(b), Vert2::new(bo[0], bo[1], clear(cb)));
            m.tri(
                v(a),
                Vert2::new(bo[0], bo[1], clear(cb)),
                Vert2::new(ao[0], ao[1], clear(ca)),
            );
        }
    }
}

/// A soft drop shadow of `pts` moved by `offset`, blurred over `blur` px.
pub fn shadow(m: &mut Mesh2d, pts: &[P], offset: P, blur: f32, color: Rgba) {
    let inset = blur * 0.5;
    let b = bbox(pts);
    let (cx, cy) = ((b[0] + b[2]) * 0.5, (b[1] + b[3]) * 0.5);
    let (w, h) = ((b[2] - b[0]).max(1.0), (b[3] - b[1]).max(1.0));
    // Shrink toward the centre so the soft edge straddles the outline.
    let (kx, ky) = (
        ((w - 2.0 * inset) / w).max(0.2),
        ((h - 2.0 * inset) / h).max(0.2),
    );
    let core: Vec<P> = pts
        .iter()
        .map(|p| {
            [
                cx + (p[0] - cx) * kx + offset[0],
                cy + (p[1] - cy) * ky + offset[1],
            ]
        })
        .collect();
    fill_soft(m, &core, &Paint::Solid(color), blur);
}

/// A round glow: `color` at the centre fading to nothing at `r`.
pub fn glow(m: &mut Mesh2d, cx: f32, cy: f32, r: f32, color: Rgba) {
    let n = segments(r).max(24);
    let rings = [0.0f32, 0.25, 0.5, 0.75, 1.0];
    let a_at = |k: f32| ((1.0 - k).powi(2) * color[3] as f32).round() as u8;
    let ring = |k: f32| -> Vec<P> {
        (0..n)
            .map(|i| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                [cx + a.cos() * r * k, cy + a.sin() * r * k]
            })
            .collect()
    };
    for w in rings.windows(2) {
        let (k0, k1) = (w[0], w[1]);
        let (inner, outer) = (ring(k0), ring(k1));
        let (c0, c1) = (alpha(color, a_at(k0)), alpha(color, a_at(k1)));
        for i in 0..n {
            let j = (i + 1) % n;
            let (o_i, o_j) = (
                Vert2::new(outer[i][0], outer[i][1], c1),
                Vert2::new(outer[j][0], outer[j][1], c1),
            );
            if k0 == 0.0 {
                m.tri(Vert2::new(cx, cy, c0), o_i, o_j);
            } else {
                let (i_i, i_j) = (
                    Vert2::new(inner[i][0], inner[i][1], c0),
                    Vert2::new(inner[j][0], inner[j][1], c0),
                );
                m.tri(i_i, o_i, o_j);
                m.tri(i_i, o_j, i_j);
            }
        }
    }
}

// ---- strokes --------------------------------------------------------------------------------

/// An open polyline `width` px wide with round ends.
pub fn stroke(m: &mut Mesh2d, pts: &[P], width: f32, color: Rgba) {
    stroke_taper(m, pts, width, width, color);
}

/// A closed outline `width` px wide, centred on the polygon's edge.
pub fn stroke_closed(m: &mut Mesh2d, pts: &[P], width: f32, color: Rgba) {
    let poly = clean(pts);
    if poly.len() < 3 {
        return;
    }
    let hw = vec![width * 0.5; poly.len()];
    ribbon(m, &poly, &hw, color, true);
}

/// A brush stroke whose width goes from `w0` at the start to `w1` at the end (ink lines,
/// eyelashes, brows). Round ends where the width is above 2 px.
pub fn stroke_taper(m: &mut Mesh2d, pts: &[P], w0: f32, w1: f32, color: Rgba) {
    let line = dedup(pts);
    if line.len() < 2 {
        if let Some(p) = line.first() {
            dot(m, *p, w0.max(w1) * 0.5, color);
        }
        return;
    }
    let n = line.len();
    let hw: Vec<f32> = (0..n)
        .map(|i| (w0 + (w1 - w0) * i as f32 / (n - 1) as f32).max(0.0) * 0.5)
        .collect();
    ribbon(m, &line, &hw, color, false);
    for (p, r) in [(line[0], hw[0]), (line[n - 1], hw[n - 1])] {
        if r >= 1.0 {
            dot(m, p, r, color);
        }
    }
}

/// A filled circle with a smooth edge.
pub fn dot(m: &mut Mesh2d, p: P, r: f32, color: Rgba) {
    fill(m, &circle(p[0], p[1], r), &Paint::Solid(color));
}

/// The ribbon along a line: a solid core between the two offsets, a transparent fringe outside.
fn ribbon(m: &mut Mesh2d, line: &[P], hw: &[f32], color: Rgba, closed: bool) {
    let n = line.len();
    let normals: Vec<P> = (0..n)
        .map(|i| {
            let prev = if i == 0 {
                if closed {
                    line[n - 1]
                } else {
                    line[0]
                }
            } else {
                line[i - 1]
            };
            let next = if i + 1 == n {
                if closed {
                    line[0]
                } else {
                    line[n - 1]
                }
            } else {
                line[i + 1]
            };
            let n_in = edge_normal(prev, line[i]);
            let n_out = edge_normal(line[i], next);
            let (a, b) = match (n_in, n_out) {
                (Some(a), Some(b)) => (a, b),
                (Some(a), None) => (a, a),
                (None, Some(b)) => (b, b),
                (None, None) => ([0.0, 1.0], [0.0, 1.0]),
            };
            miter(a, b)
        })
        .collect();
    let at = |i: usize, d: f32| -> P {
        [
            line[i][0] + normals[i][0] * d,
            line[i][1] + normals[i][1] * d,
        ]
    };
    let solid = color;
    let fade = clear(color);
    let segs = if closed { n } else { n - 1 };
    for s in 0..segs {
        let (i, j) = (s, (s + 1) % n);
        let (ri, rj) = (hw[i].max(0.0), hw[j].max(0.0));
        if ri < 0.6 && rj < 0.6 {
            // A hairline: two fringes meeting at the centre, fainter the thinner it is.
            let a = ((ri + rj + 0.3).clamp(0.3, 1.0) * color[3] as f32) as u8;
            let mid = [color[0], color[1], color[2], a];
            let li = [at(i, ri + AA), line[i], at(i, -ri - AA)];
            let lj = [at(j, rj + AA), line[j], at(j, -rj - AA)];
            let cols = [fade, mid, fade];
            for k in 0..2 {
                let (a, b) = (
                    Vert2::new(li[k][0], li[k][1], cols[k]),
                    Vert2::new(li[k + 1][0], li[k + 1][1], cols[k + 1]),
                );
                let (c, d) = (
                    Vert2::new(lj[k][0], lj[k][1], cols[k]),
                    Vert2::new(lj[k + 1][0], lj[k + 1][1], cols[k + 1]),
                );
                m.tri(a, b, d);
                m.tri(a, d, c);
            }
            continue;
        }
        // Lanes: outer left (clear), left (solid), right (solid), outer right (clear).
        let li = [at(i, ri + AA), at(i, ri), at(i, -ri), at(i, -ri - AA)];
        let lj = [at(j, rj + AA), at(j, rj), at(j, -rj), at(j, -rj - AA)];
        let cols = [fade, solid, solid, fade];
        for k in 0..3 {
            let (a, b) = (
                Vert2::new(li[k][0], li[k][1], cols[k]),
                Vert2::new(li[k + 1][0], li[k + 1][1], cols[k + 1]),
            );
            let (c, d) = (
                Vert2::new(lj[k][0], lj[k][1], cols[k]),
                Vert2::new(lj[k + 1][0], lj[k + 1][1], cols[k + 1]),
            );
            m.tri(a, b, d);
            m.tri(a, d, c);
        }
    }
}

// ---- clipping -------------------------------------------------------------------------------

/// The part of `subject` inside the convex polygon `clip` (Sutherland-Hodgman). Use it to
/// keep shading inside a face, highlights inside a cap.
pub fn clip_convex(subject: &[P], clip: &[P]) -> Vec<P> {
    let clip = clean(clip);
    if clip.len() < 3 {
        return Vec::new();
    }
    let orient = if area(&clip) >= 0.0 { 1.0 } else { -1.0 };
    let mut out: Vec<P> = subject.to_vec();
    for i in 0..clip.len() {
        if out.is_empty() {
            break;
        }
        let (a, b) = (clip[i], clip[(i + 1) % clip.len()]);
        // Inside = on the same side as the polygon's interior.
        let side = |p: P| ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])) * orient;
        let input = std::mem::take(&mut out);
        for k in 0..input.len() {
            let (cur, prev) = (input[k], input[(k + input.len() - 1) % input.len()]);
            let (sc, sp) = (side(cur), side(prev));
            if sc >= 0.0 {
                if sp < 0.0 {
                    out.push(cross_point(prev, cur, sp, sc));
                }
                out.push(cur);
            } else if sp >= 0.0 {
                out.push(cross_point(prev, cur, sp, sc));
            }
        }
    }
    out
}

fn cross_point(p: P, q: P, sp: f32, sq: f32) -> P {
    let t = sp / (sp - sq);
    [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t]
}

// ---- internals ------------------------------------------------------------------------------

pub fn bbox(pts: &[P]) -> [f32; 4] {
    pts.iter()
        .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |b, p| {
            [
                b[0].min(p[0]),
                b[1].min(p[1]),
                b[2].max(p[0]),
                b[3].max(p[1]),
            ]
        })
}

fn dedup(pts: &[P]) -> Vec<P> {
    let mut out: Vec<P> = Vec::with_capacity(pts.len());
    for p in pts {
        if !p[0].is_finite() || !p[1].is_finite() {
            continue;
        }
        if out
            .last()
            .is_none_or(|q| (q[0] - p[0]).abs() + (q[1] - p[1]).abs() > 1e-3)
        {
            out.push(*p);
        }
    }
    out
}

/// Drop repeated and collinear points (and the closing duplicate).
fn clean(pts: &[P]) -> Vec<P> {
    let mut out = dedup(pts);
    while out.len() > 1 {
        let (f, l) = (out[0], out[out.len() - 1]);
        if (f[0] - l[0]).abs() + (f[1] - l[1]).abs() <= 1e-3 {
            out.pop();
        } else {
            break;
        }
    }
    let mut changed = true;
    while changed && out.len() >= 3 {
        changed = false;
        let n = out.len();
        for i in 0..n {
            let (a, b, c) = (out[(i + n - 1) % n], out[i], out[(i + 1) % n]);
            let cr = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
            let len = ((c[0] - a[0]).powi(2) + (c[1] - a[1]).powi(2))
                .sqrt()
                .max(1e-6);
            if (cr / len).abs() < 1e-4 {
                out.remove(i);
                changed = true;
                break;
            }
        }
    }
    out
}

fn edge_normal(a: P, b: P) -> Option<P> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt();
    (len > 1e-5).then(|| [dy / len, -dx / len])
}

/// The offset direction at a corner between two edge normals, lengthened so the offset edges
/// stay parallel (limited at sharp corners).
fn miter(a: P, b: P) -> P {
    let s = [a[0] + b[0], a[1] + b[1]];
    let len = (s[0] * s[0] + s[1] * s[1]).sqrt();
    if len < 1e-4 {
        return a;
    }
    let d = [s[0] / len, s[1] / len];
    let cos = (d[0] * a[0] + d[1] * a[1]).max(0.35);
    [d[0] / cos, d[1] / cos]
}

/// Outward (or inward) offsets of every vertex by `dist`.
fn offsets(poly: &[P], orient: f32, dist: f32, outward: bool) -> Vec<P> {
    let n = poly.len();
    let sign = if outward { orient } else { -orient };
    (0..n)
        .map(|i| {
            let (p, c, q) = (poly[(i + n - 1) % n], poly[i], poly[(i + 1) % n]);
            let a = edge_normal(p, c).unwrap_or([0.0, 0.0]);
            let b = edge_normal(c, q).unwrap_or(a);
            let d = miter(a, b);
            [c[0] + d[0] * dist * sign, c[1] + d[1] * dist * sign]
        })
        .collect()
}

/// Triangles (indices into `poly`) covering a simple polygon: a fan when convex, ear clipping
/// otherwise.
fn triangulate(poly: &[P], orient: f32) -> Vec<[usize; 3]> {
    let n = poly.len();
    let turn =
        |a: P, b: P, c: P| ((b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0])) * orient;
    let convex = (0..n).all(|i| turn(poly[(i + n - 1) % n], poly[i], poly[(i + 1) % n]) >= -1e-3);
    if convex {
        return (1..n - 1).map(|i| [0, i, i + 1]).collect();
    }
    let mut idx: Vec<usize> = (0..n).collect();
    let mut out = Vec::with_capacity(n - 2);
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for k in 0..m {
            let (ia, ib, ic) = (idx[(k + m - 1) % m], idx[k], idx[(k + 1) % m]);
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            if turn(a, b, c) <= 1e-6 {
                continue;
            }
            let inside = idx
                .iter()
                .any(|&j| j != ia && j != ib && j != ic && in_triangle(poly[j], a, b, c, orient));
            if inside {
                continue;
            }
            out.push([ia, ib, ic]);
            idx.remove(k);
            clipped = true;
            break;
        }
        if !clipped {
            // Degenerate leftovers (self-touching input): fan what is left.
            for k in 1..idx.len() - 1 {
                out.push([idx[0], idx[k], idx[k + 1]]);
            }
            return out;
        }
    }
    if idx.len() == 3 {
        out.push([idx[0], idx[1], idx[2]]);
    }
    out
}

fn in_triangle(p: P, a: P, b: P, c: P, orient: f32) -> bool {
    let s = |u: P, v: P| ((v[0] - u[0]) * (p[1] - u[1]) - (v[1] - u[1]) * (p[0] - u[0])) * orient;
    s(a, b) >= -1e-6 && s(b, c) >= -1e-6 && s(c, a) >= -1e-6
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgba = [255, 0, 0, 255];

    /// Area covered by fully opaque triangles (the fill without its fringe).
    fn solid_area(m: &Mesh2d) -> f32 {
        m.verts
            .chunks(3)
            .filter(|t| t.iter().all(|v| v.color[3] == 255))
            .map(|t| area(&[[t[0].x, t[0].y], [t[1].x, t[1].y], [t[2].x, t[2].y]]).abs())
            .sum()
    }

    #[test]
    fn square_fill_covers_its_area_with_a_clear_fringe() {
        let mut m = Mesh2d::default();
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        fill(&mut m, &sq, &Paint::Solid(RED));
        assert!((solid_area(&m) - 100.0).abs() < 1e-3);
        let b = m.bounds().unwrap();
        assert!((b[0] + AA).abs() < 0.01 && (b[2] - 10.0 - AA).abs() < 0.01);
        // Outer fringe corners are transparent.
        assert!(m.verts.iter().any(|v| v.color[3] == 0));
    }

    #[test]
    fn concave_shapes_triangulate_to_their_area() {
        for pts in [
            vec![
                [0.0, 0.0],
                [20.0, 0.0],
                [20.0, 5.0],
                [5.0, 5.0],
                [5.0, 20.0],
                [0.0, 20.0],
            ],
            star(50.0, 50.0, 40.0, 16.0, 5),
            // Counter-clockwise input.
            vec![
                [0.0, 20.0],
                [5.0, 20.0],
                [5.0, 5.0],
                [20.0, 5.0],
                [20.0, 0.0],
                [0.0, 0.0],
            ],
        ] {
            let mut m = Mesh2d::default();
            fill_soft(&mut m, &pts, &Paint::Solid(RED), 0.0);
            assert!((solid_area(&m) - area(&pts).abs()).abs() < 0.05, "{pts:?}");
        }
    }

    #[test]
    fn stroke_covers_length_times_width() {
        let mut m = Mesh2d::default();
        let line = [[0.0, 0.0], [100.0, 0.0]];
        ribbon(&mut m, &line, &[2.0, 2.0], RED, false);
        assert!((solid_area(&m) - 400.0).abs() < 0.5);
        let mut t = Mesh2d::default();
        stroke_taper(
            &mut t,
            &quad_bez([0.0, 0.0], [50.0, -40.0], [100.0, 0.0], 12),
            6.0,
            0.0,
            RED,
        );
        assert!(t.triangles() > 30);
    }

    #[test]
    fn closed_outline_rings_the_shape() {
        let mut m = Mesh2d::default();
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        stroke_closed(&mut m, &sq, 2.0, RED);
        // A 2 px band centred on a 10 px square: 12^2 - 8^2 = 80.
        assert!((solid_area(&m) - 80.0).abs() < 0.5, "{}", solid_area(&m));
    }

    #[test]
    fn clipping_keeps_the_inside() {
        let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let right = [[5.0, -5.0], [20.0, -5.0], [20.0, 20.0], [5.0, 20.0]];
        let c = clip_convex(&sq, &right);
        assert!((area(&c).abs() - 50.0).abs() < 1e-3);
        let none = clip_convex(&sq, &shift(&right, 20.0, 0.0));
        assert!(area(&none).abs() < 1e-3);
    }

    #[test]
    fn paints_blend_between_their_ends() {
        let p = Paint::vertical(0.0, 10.0, [0, 0, 0, 255], [200, 100, 50, 255]);
        assert_eq!(p.at([3.0, 0.0]), [0, 0, 0, 255]);
        assert_eq!(p.at([3.0, 10.0]), [200, 100, 50, 255]);
        assert_eq!(p.at([3.0, 5.0]), [100, 50, 25, 255]);
        let r = Paint::Radial {
            center: [0.0, 0.0],
            radius: 10.0,
            c0: RED,
            c1: [0, 0, 255, 0],
        };
        assert_eq!(r.at([20.0, 0.0]), [0, 0, 255, 0]);
    }

    #[test]
    fn glow_fades_to_nothing() {
        let mut m = Mesh2d::default();
        glow(&mut m, 0.0, 0.0, 50.0, [255, 240, 200, 200]);
        let far = m
            .verts
            .iter()
            .filter(|v| (v.x * v.x + v.y * v.y).sqrt() > 49.0);
        assert!(far.into_iter().all(|v| v.color[3] == 0));
        assert!(m.verts.iter().any(|v| v.color[3] == 200));
    }

    #[test]
    fn shapes_close_and_curve() {
        assert_eq!(rounded_rect(0.0, 0.0, 10.0, 10.0, 0.0).len(), 4);
        let rr = rounded_rect(0.0, 0.0, 100.0, 50.0, 10.0);
        let a = area(&rr).abs();
        let expect = 100.0 * 50.0 - (4.0 - std::f32::consts::PI) * 100.0;
        assert!((a - expect).abs() < 6.0, "{a} vs {expect}");
        let e = ellipse(0.0, 0.0, 30.0, 10.0);
        // Inscribed polygon: within 3% of the true ellipse.
        assert!(
            (area(&e).abs() - std::f32::consts::PI * 300.0).abs()
                < 0.03 * std::f32::consts::PI * 300.0
        );
        let q = quad_bez([0.0, 0.0], [5.0, 10.0], [10.0, 0.0], 4);
        assert_eq!(q.len(), 5);
        assert_eq!(q[2], [5.0, 5.0]);
    }

    #[test]
    fn placed_meshes_scale_and_fade() {
        let mut unit = Mesh2d::default();
        fill(
            &mut unit,
            &[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
            &Paint::Solid(RED),
        );
        let mut m = Mesh2d::default();
        m.append_placed(&unit, 10.0, 20.0, 100.0, 0.5);
        let b = m.bounds().unwrap();
        assert!(b[0] < 10.0 && b[2] > 109.0 && b[1] < 20.0);
        assert!(m.verts.iter().all(|v| v.color[3] <= 128));
    }
}
