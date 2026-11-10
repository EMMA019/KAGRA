//! Symbols the fonts do not have, drawn by the engine.
//!
//! The HUD fonts are cut down to what the games write, and neither Public Sans nor Zen Maru
//! Gothic has a skull, a sun, a turning arrow, a slanted arrow, a check mark or a warning
//! sign. A game that writes one in a label used to get a blank box (or nothing). The
//! letters here are shapes instead: [`ink`] says whether a point of the letter's square is
//! inked, `font` turns that into the same coverage a font glyph gives, and the text
//! renderers (GPU atlas, CPU picture) draw it like any other letter, in the text's colour.
//!
//! ```text
//! ↖ ↗ ↘ ↙ ↔ ↕   arrows the fonts lack (← ↑ → ↓ are in the font)
//! ↺ ↻           turn back, turn on
//! ☀ ☠ ⚠         sun, skull, warning
//! ✓ ✔ ✕ ✖       check and cross
//! ―             the long dash
//! ```

use std::f32::consts::PI;

/// Is `ch` one of the engine's own symbols?
pub fn has(ch: char) -> bool {
    matches!(
        ch,
        '↖' | '↗'
            | '↘'
            | '↙'
            | '↔'
            | '↕'
            | '↺'
            | '↻'
            | '☀'
            | '☠'
            | '⚠'
            | '✓'
            | '✔'
            | '✕'
            | '✖'
            | '―'
    )
}

/// Every symbol [`has`] knows (for tests and font sheets).
pub const ALL: &str = "↖↗↘↙↔↕↺↻☀☠⚠✓✔✕✖―";

fn dist_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (vx, vy) = (b[0] - a[0], b[1] - a[1]);
    let (wx, wy) = (p[0] - a[0], p[1] - a[1]);
    let len2 = (vx * vx + vy * vy).max(1e-9);
    let t = ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0);
    let (dx, dy) = (wx - vx * t, wy - vy * t);
    (dx * dx + dy * dy).sqrt()
}

fn in_triangle(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let side = |p: [f32; 2], q: [f32; 2], r: [f32; 2]| {
        (p[0] - r[0]) * (q[1] - r[1]) - (q[0] - r[0]) * (p[1] - r[1])
    };
    let (d1, d2, d3) = (side(p, a, b), side(p, b, c), side(p, c, a));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

fn in_circle(p: [f32; 2], c: [f32; 2], r: f32) -> bool {
    let (dx, dy) = (p[0] - c[0], p[1] - c[1]);
    dx * dx + dy * dy <= r * r
}

/// An arrow from the middle of the square pointing along `angle` (0 = right, a quarter turn
/// = down, as the screen's y grows downward). `both`: a head at each end.
fn arrow(p: [f32; 2], angle: f32, both: bool) -> bool {
    let (s, c) = angle.sin_cos();
    // The point in the arrow's own frame: u along it, v across.
    let (x, y) = (p[0] - 0.5, p[1] - 0.5);
    let (u, v) = (x * c + y * s, -x * s + y * c);
    let head = |u: f32| (0.12..=0.44).contains(&u) && v.abs() <= 0.24 * (0.44 - u) / 0.32;
    let shaft_from = if both { -0.2 } else { -0.4 };
    (v.abs() <= 0.065 && (shaft_from..=0.2).contains(&u)) || head(u) || (both && head(-u))
}

/// A ring with a gap and an arrow head where it ends. `clockwise`: which way it turns.
fn turn(p: [f32; 2], clockwise: bool) -> bool {
    // Drawn turning clockwise (up the left side, the head pointing right across the top);
    // the anticlockwise one is its mirror image.
    let x = if clockwise { p[0] } else { 1.0 - p[0] };
    let (dx, dy) = (x - 0.5, p[1] - 0.52);
    let r = (dx * dx + dy * dy).sqrt();
    // Angle from straight up, growing anticlockwise (to the left on screen).
    let a = (-dx).atan2(-dy).rem_euclid(2.0 * PI);
    let (radius, half) = (0.3, 0.055);
    // The ring runs from just left of the top round to the right side; the gap is the
    // top right, where the head sits.
    let in_ring = (r - radius).abs() <= half && (0.25..=2.0 * PI - 1.25).contains(&a);
    // The head: at the ring's start (left of the top), pointing right along the turn.
    let tip = [0.5 + 0.14, 0.52 - radius];
    let base = [0.5 - 0.1, 0.52 - radius];
    in_ring
        || in_triangle(
            [x, p[1]],
            tip,
            [base[0], base[1] - 0.17],
            [base[0], base[1] + 0.17],
        )
}

fn sun(p: [f32; 2]) -> bool {
    let c = [0.5, 0.5];
    if in_circle(p, c, 0.19) {
        return true;
    }
    (0..8).any(|k| {
        let a = k as f32 * PI / 4.0;
        let (s, co) = a.sin_cos();
        dist_to_segment(
            p,
            [0.5 + co * 0.28, 0.5 + s * 0.28],
            [0.5 + co * 0.43, 0.5 + s * 0.43],
        ) <= 0.045
    })
}

fn skull(p: [f32; 2]) -> bool {
    let [x, y] = p;
    let head = in_circle(p, [0.5, 0.42], 0.32);
    let jaw = (0.33..=0.67).contains(&x) && (0.55..=0.86).contains(&y);
    if !(head || jaw) {
        return false;
    }
    let eye = in_circle(p, [0.37, 0.44], 0.095) || in_circle(p, [0.63, 0.44], 0.095);
    let nose = in_triangle(p, [0.5, 0.52], [0.45, 0.62], [0.55, 0.62]);
    // The gaps between the teeth.
    let teeth = y >= 0.74
        && [0.42f32, 0.5, 0.58]
            .iter()
            .any(|gx| (x - gx).abs() <= 0.016);
    !(eye || nose || teeth)
}

fn warning(p: [f32; 2]) -> bool {
    let tri = in_triangle(p, [0.5, 0.1], [0.94, 0.88], [0.06, 0.88]);
    if !tri {
        return false;
    }
    let bar = (p[0] - 0.5).abs() <= 0.045 && (0.38..=0.64).contains(&p[1]);
    let dot = in_circle(p, [0.5, 0.75], 0.052);
    !(bar || dot)
}

fn strokes(p: [f32; 2], pts: &[[f32; 2]], half: f32) -> bool {
    pts.windows(2)
        .any(|w| dist_to_segment(p, w[0], w[1]) <= half)
}

/// Is the point (`x`, `y`) of `ch`'s unit square inked? (0, 0) is the top left corner of the
/// square a full-width letter fills. False everywhere for a letter [`has`] does not know.
pub fn ink(ch: char, x: f32, y: f32) -> bool {
    let p = [x, y];
    match ch {
        '↗' => arrow(p, -PI * 0.25, false),
        '↘' => arrow(p, PI * 0.25, false),
        '↙' => arrow(p, PI * 0.75, false),
        '↖' => arrow(p, -PI * 0.75, false),
        '↔' => arrow(p, 0.0, true),
        '↕' => arrow(p, PI * 0.5, true),
        '↺' => turn(p, false),
        '↻' => turn(p, true),
        '☀' => sun(p),
        '☠' => skull(p),
        '⚠' => warning(p),
        '✓' => strokes(p, &[[0.16, 0.54], [0.4, 0.78], [0.86, 0.22]], 0.055),
        '✔' => strokes(p, &[[0.16, 0.54], [0.4, 0.78], [0.86, 0.22]], 0.085),
        '✕' => {
            strokes(p, &[[0.2, 0.2], [0.8, 0.8]], 0.055)
                || strokes(p, &[[0.8, 0.2], [0.2, 0.8]], 0.055)
        }
        '✖' => {
            strokes(p, &[[0.2, 0.2], [0.8, 0.8]], 0.085)
                || strokes(p, &[[0.8, 0.2], [0.2, 0.8]], 0.085)
        }
        '―' => (y - 0.52).abs() <= 0.035,
        _ => false,
    }
}

/// `ch` as coverage (0..1 a pixel, rows top to bottom) on a `w` x `h` pixel box, each pixel
/// sampled 4 x 4 for smooth edges.
pub fn coverage(ch: char, w: u32, h: u32) -> Vec<f32> {
    const N: u32 = 4;
    let mut out = Vec::with_capacity((w * h) as usize);
    for j in 0..h {
        for i in 0..w {
            let mut hit = 0u32;
            for sj in 0..N {
                for si in 0..N {
                    let x = (i as f32 + (si as f32 + 0.5) / N as f32) / w.max(1) as f32;
                    let y = (j as f32 + (sj as f32 + 0.5) / N as f32) / h.max(1) as f32;
                    hit += ink(ch, x, y) as u32;
                }
            }
            out.push(hit as f32 / (N * N) as f32);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share(ch: char) -> f32 {
        let c = coverage(ch, 48, 48);
        c.iter().sum::<f32>() / c.len() as f32
    }

    #[test]
    fn every_symbol_has_ink_and_leaves_room() {
        for ch in ALL.chars() {
            assert!(has(ch), "{ch}");
            let s = share(ch);
            assert!(s > 0.03 && s < 0.7, "'{ch}' inks {s:.2} of its square");
        }
        assert!(!has('A') && !has('→'));
        assert_eq!(share('A'), 0.0);
    }

    /// The four slanted arrows are one shape turned: the same ink, each with its head in
    /// its own corner.
    #[test]
    fn slanted_arrows_point_into_their_corners() {
        let corner = |ch: char, right: bool, down: bool| {
            let c = coverage(ch, 40, 40);
            let mut sum = 0.0;
            for j in 0..40 {
                for i in 0..40 {
                    if (i >= 20) == right && (j >= 20) == down {
                        sum += c[j * 40 + i];
                    }
                }
            }
            sum
        };
        for (ch, right, down) in [
            ('↗', true, false),
            ('↘', true, true),
            ('↙', false, true),
            ('↖', false, false),
        ] {
            let head = corner(ch, right, down);
            let tail = corner(ch, !right, !down);
            assert!(head > tail * 1.3, "'{ch}': head {head:.1}, tail {tail:.1}");
            assert!(
                (share(ch) - share('↗')).abs() < 0.01,
                "'{ch}' is '↗' turned"
            );
        }
    }

    #[test]
    fn the_two_turns_mirror_each_other_and_bold_marks_are_heavier() {
        let (a, b) = (coverage('↺', 40, 40), coverage('↻', 40, 40));
        for j in 0..40 {
            for i in 0..40 {
                assert!((a[j * 40 + i] - b[j * 40 + (39 - i)]).abs() < 1e-5);
            }
        }
        assert!(share('✔') > share('✓') * 1.2);
        assert!(share('✖') > share('✕') * 1.2);
    }

    /// A skull has two eyes: holes in the upper half, left and right of the middle.
    #[test]
    fn the_skull_has_eyes() {
        assert!(ink('☠', 0.5, 0.25), "forehead");
        assert!(!ink('☠', 0.37, 0.44) && !ink('☠', 0.63, 0.44), "eyes");
        assert!(ink('☠', 0.5, 0.44), "between the eyes");
        assert!(!ink('☠', 0.05, 0.05), "outside");
        assert!(
            ink('⚠', 0.2, 0.8) && !ink('⚠', 0.5, 0.5),
            "the mark is cut out"
        );
    }
}

#[cfg(test)]
mod sheet {
    /// `KAGRA_SHOTS=dir`: the symbols next to the font's own marks, large and at HUD size.
    #[test]
    fn symbols_to_png() {
        let Ok(dir) = std::env::var("KAGRA_SHOTS") else {
            return;
        };
        use crate::scene::{DrawList, Quad, TextQuad};
        let mut list = DrawList {
            clear: [24, 30, 48, 255],
            ..Default::default()
        };
        list.quads
            .push(Quad::new(0.0, 150.0, 760.0, 110.0, [246, 240, 228, 255]));
        let row = format!("{} ◆●★→♥", super::ALL);
        list.texts
            .push(TextQuad::new(&row, 16.0, 14.0, 44.0, [255, 255, 255, 255]));
        list.texts
            .push(TextQuad::new(&row, 16.0, 84.0, 18.0, [255, 220, 110, 255]));
        list.texts.push(TextQuad::new(
            "☠ ばんにん  -6 ↺-2  ☀ 晴れ  ✓ クリア  ⚠ 注意  ↗ 出口",
            16.0,
            166.0,
            26.0,
            [34, 38, 52, 255],
        ));
        list.texts.push(TextQuad::new(
            "☠ WARDEN  -6 ↺-2  ☀ Sunny  ✔ Clear  ✖ Miss  ↙ Exit ↔ ↕",
            16.0,
            212.0,
            16.0,
            [34, 38, 52, 255],
        ));
        std::fs::create_dir_all(&dir).unwrap();
        crate::raster2d::picture(&list, 760, 260)
            .save_png(&std::path::Path::new(&dir).join("symbols.png"))
            .unwrap();
    }
}
