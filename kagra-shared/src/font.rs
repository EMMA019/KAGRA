//! テキスト → 画面 Quad。GPU 非依存なので `render` feature 無しでもテストでき、
//! wasm / Android / iOS / オフスクリーンが同じ画素を出す。
//!
//! 日本語は Zen Maru Gothic Bold（SIL OFL、ゲームが使う字 + かな全部に subset:
//! `tools/subset_fonts.py`）。subset に無い字は PixelMplus（M+ FONT LICENSE、wasm には入れない、
//! JIS 第1・2水準）に落ちる。数字・タイトル用の `TextFace::Display` は Cinzel Bold（OFL）。
//! グリフは `ab_glyph`（純 Rust）で表示サイズの 2 倍にラスタライズし、
//! カバレッジ画素を 0.5px の `Quad` に展開する。1px 四方の画素文字より
//! 縁が細い。テクスチャ / シェーダーの追加は不要で、既存の 2D HUD パスが
//! そのまま文字を描く。

use std::collections::HashMap;

use ab_glyph::{Font as _, ScaleFont}; // as_scaled(Font) / glyph_id・h_advance(ScaleFont)
use once_cell::sync::Lazy;

use crate::scene::{Quad, TextAlign, TextFace, TextQuad};

/// Raster scale. Quads are `1 / TEXT_SUPERSAMPLE` pixels so edges are finer
/// than the old 1px coverage stamps. Still ab_glyph coverage, not an SDF.
const TEXT_SUPERSAMPLE: u32 = 2;

/// 埋め込みフォント（M+ ライセンス: 日本語フォールバック）。
/// Not in the wasm build (1.1 MB): there every game string is in the Zen Maru subset (a test
/// checks), and a phone download is worth more than a fallback for text no game ships.
#[cfg(not(target_arch = "wasm32"))]
pub const DEFAULT_FONT_BYTES: &[u8] = include_bytes!("../assets/PixelMplus10-Regular.ttf");

/// 滑らかな欧文 UI フォント（Public Sans Bold, SIL OFL）。
pub const UI_FONT_BYTES: &[u8] = include_bytes!("../assets/PublicSans-Bold.ttf");

/// 日本語の本文（Zen Maru Gothic Bold, SIL OFL; subset）。
pub const JP_FONT_BYTES: &[u8] = include_bytes!("../assets/ZenMaruGothic-Bold.ttf");

/// 数字・タイトル（Cinzel Bold, SIL OFL; Latin subset）。
pub const DISPLAY_FONT_BYTES: &[u8] = include_bytes!("../assets/Cinzel-Bold.ttf");

static JP_FONT: Lazy<ab_glyph::FontArc> = Lazy::new(|| {
    ab_glyph::FontArc::try_from_slice(JP_FONT_BYTES)
        .expect("embedded ZenMaruGothic-Bold.ttf must parse")
});

static DISPLAY_FONT: Lazy<ab_glyph::FontArc> = Lazy::new(|| {
    ab_glyph::FontArc::try_from_slice(DISPLAY_FONT_BYTES)
        .expect("embedded Cinzel-Bold.ttf must parse")
});

#[cfg(not(target_arch = "wasm32"))]
static FONT: Lazy<ab_glyph::FontArc> = Lazy::new(|| {
    ab_glyph::FontArc::try_from_slice(DEFAULT_FONT_BYTES)
        .expect("embedded PixelMplus10-Regular.ttf must parse")
});

static UI_FONT: Lazy<ab_glyph::FontArc> = Lazy::new(|| {
    ab_glyph::FontArc::try_from_slice(UI_FONT_BYTES)
        .expect("embedded PublicSans-Bold.ttf must parse")
});

/// True when every non-whitespace character has a real glyph (not `.notdef`).
pub fn text_covered(text: &str) -> bool {
    text.chars()
        .filter(|ch| !ch.is_whitespace())
        .all(|ch| select_font(ch).is_some())
}

fn font_has_glyph(font: &ab_glyph::FontArc, ch: char) -> bool {
    // Glyph 0 is `.notdef`. Drawing it paints a missing-glyph box.
    font.glyph_id(ch).0 != 0
}

/// Latin prefers Public Sans. Everything else goes to Zen Maru Gothic, then PixelMplus (JIS)
/// for a character outside the subset. If no face has the character, return `None` so the
/// caller skips the glyph instead of rasterizing `.notdef`.
fn select_font(ch: char) -> Option<&'static ab_glyph::FontArc> {
    select_font_for(ch, TextFace::Ui)
}

fn select_font_for(ch: char, face: TextFace) -> Option<&'static ab_glyph::FontArc> {
    if face == TextFace::Display && (ch as u32) <= 0x024F && font_has_glyph(&DISPLAY_FONT, ch) {
        return Some(&DISPLAY_FONT);
    }
    if (ch as u32) <= 0x024F && font_has_glyph(&UI_FONT, ch) {
        return Some(&UI_FONT);
    }
    if font_has_glyph(&JP_FONT, ch) {
        return Some(&JP_FONT);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if font_has_glyph(&FONT, ch) {
        return Some(&FONT);
    }
    if font_has_glyph(&UI_FONT, ch) {
        return Some(&UI_FONT);
    }
    None
}

fn char_advance(ch: char, size: f32) -> f32 {
    char_advance_for(ch, size, TextFace::Ui)
}

fn char_advance_for(ch: char, size: f32, face: TextFace) -> f32 {
    let size = size.max(1.0);
    let Some(font) = select_font_for(ch, face) else {
        return size * 0.5;
    };
    let scaled = font.as_scaled(size);
    scaled.h_advance(scaled.glyph_id(ch))
}

/// グリフのカバレッジ画素（0..1）とベースライン相対オフセット。
#[derive(Debug)]
pub(crate) struct Coverage {
    pub(crate) dx: f32,
    pub(crate) dy: f32,
    pub(crate) w: u32,
    pub(crate) h: u32,
    pub(crate) pixels: Vec<f32>,
}

/// One glyph of laid-out text: the character, its pen x (rounded) and its baseline y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placed {
    pub(crate) ch: char,
    pub(crate) x: f32,
    pub(crate) baseline: f32,
}

/// Where each visible character of `tq` goes (lines, alignment, advances). Spaces only advance.
pub(crate) fn layout(tq: &TextQuad) -> Vec<Placed> {
    let size = tq.size.max(1.0);
    let scaled = UI_FONT.as_scaled(size);
    let line_h = scaled.height().max(size * 1.2).round();
    let mut base = (tq.y + scaled.ascent()).round();
    let mut out = Vec::with_capacity(tq.text.len());
    for line in tq.text.split('\n') {
        let line_w: f32 = line
            .chars()
            .map(|c| char_advance_for(c, size, tq.face))
            .sum();
        let mut col = match tq.align {
            TextAlign::Left => tq.x,
            TextAlign::Center => tq.x - line_w * 0.5,
            TextAlign::Right => tq.x - line_w,
        };
        for ch in line.chars() {
            if ch != ' ' {
                out.push(Placed {
                    ch,
                    x: col.round(),
                    baseline: base,
                });
            }
            col += char_advance_for(ch, size, tq.face);
        }
        base += line_h;
    }
    out
}

/// Raster size for the GPU glyph atlas: twice the drawn size (as the quad path), rounded up to
/// a step, so a size that animates (a popping damage number) reuses a few rasters instead of
/// making a new one every frame. Capped at 160 px; bigger text scales that raster up.
#[cfg_attr(not(feature = "render"), allow(dead_code))]
pub(crate) fn atlas_px(size: f32) -> u32 {
    const STEPS: [u32; 14] = [12, 16, 20, 24, 28, 32, 40, 48, 56, 64, 80, 96, 128, 160];
    let want = (size.max(1.0) * TEXT_SUPERSAMPLE as f32).ceil() as u32;
    STEPS.into_iter().find(|&p| p >= want).unwrap_or(160)
}

/// Rasterize one glyph at `px` (for the GPU atlas).
#[cfg_attr(not(feature = "render"), allow(dead_code))]
pub(crate) fn glyph_coverage(ch: char, px: u32, face: TextFace) -> Option<Coverage> {
    rasterize(ch, px, face)
}

/// (文字, ピクセル高) → カバレッジ。1 回ラスタライズしたら使い回す。
#[derive(Debug, Default)]
pub struct TextRaster {
    cache: HashMap<(char, u32, TextFace), Coverage>,
}

/// 行幅（ピクセル）。キャッシュ不要: h_advance だけで出るので
/// Python 側のテキスト折り返し（UI パネル）でも気軽に呼べる。
pub fn measure_text(text: &str, size: f32) -> f32 {
    let s = size.max(1.0);
    text.chars().map(|c| char_advance(c, s)).sum()
}

impl TextRaster {
    pub fn new() -> Self {
        Self::default()
    }

    fn coverage(&mut self, ch: char, px: u32, face: TextFace) -> Option<&Coverage> {
        let key = (ch, px, face);
        match self.cache.entry(key) {
            std::collections::hash_map::Entry::Occupied(e) => Some(e.into_mut()),
            std::collections::hash_map::Entry::Vacant(e) => {
                let cov = rasterize(ch, px, face)?;
                Some(e.insert(cov))
            }
        }
    }

    /// このサイズでの行幅（ピクセル）。`\n` は区切りで、最長行を返す。
    pub fn measure_text_width(&mut self, text: &str, size: f32) -> f32 {
        let s = size.max(1.0);
        let mut max_w = 0.0f32;
        for line in text.split('\n') {
            let w: f32 = line.chars().map(|c| char_advance(c, s)).sum();
            max_w = max_w.max(w);
        }
        max_w
    }

    /// TextQuad を Quad の列に展開する。点灯画素 1 つ = 0.5px の Quad。
    pub fn text_quads(&mut self, tq: &TextQuad) -> Vec<Quad> {
        let size = tq.size.max(1.0);
        let raster_px = (size.round() as u32)
            .saturating_mul(TEXT_SUPERSAMPLE)
            .max(TEXT_SUPERSAMPLE);
        let step = 1.0 / TEXT_SUPERSAMPLE as f32;
        let mut out = Vec::new();
        for g in layout(tq) {
            if let Some(cov) = self.coverage(g.ch, raster_px, tq.face) {
                out.extend(coverage_quads(cov, g.x, g.baseline, tq.color, step));
            }
        }
        out
    }
}

fn coverage_quads(cov: &Coverage, x: f32, y: f32, color: [u8; 4], step: f32) -> Vec<Quad> {
    let mut out = Vec::with_capacity(cov.pixels.len());
    for py in 0..cov.h {
        for px in 0..cov.w {
            let a = cov.pixels[(py * cov.w + px) as usize];
            if a <= 0.02 {
                continue;
            }
            let alpha = ((color[3] as f32) * a).round().clamp(0.0, 255.0) as u8;
            if alpha == 0 {
                continue;
            }
            out.push(Quad::new(
                x + (cov.dx + px as f32) * step,
                y + (cov.dy + py as f32) * step,
                step,
                step,
                [color[0], color[1], color[2], alpha],
            ));
        }
    }
    out
}

fn rasterize(ch: char, px: u32, face: TextFace) -> Option<Coverage> {
    let size = px.max(1) as f32;
    let font = select_font_for(ch, face)?;
    let scaled = font.as_scaled(size);
    let glyph = scaled.scaled_glyph(ch);
    let outline = scaled.outline_glyph(glyph)?;
    let bounds = outline.px_bounds();
    let w = bounds.width().ceil().max(0.0) as u32;
    let h = bounds.height().ceil().max(0.0) as u32;
    if w == 0 || h == 0 {
        return None;
    }
    let mut pixels = vec![0.0f32; (w * h) as usize];
    // ab_glyph の draw は px_bounds の左上を (0,0) とした相対座標で呼ぶ。
    outline.draw(|x, y, coverage| {
        let xi = x as usize;
        let yi = y as usize;
        if xi < w as usize && yi < h as usize {
            pixels[yi * w as usize + xi] = coverage.clamp(0.0, 1.0);
        }
    });
    Some(Coverage {
        dx: bounds.min.x.floor(),
        dy: bounds.min.y.floor(),
        w,
        h,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raster() -> TextRaster {
        TextRaster::new()
    }

    #[test]
    fn ascii_letter_rasterizes_to_some_quads() {
        let mut r = raster();
        let qs = r.text_quads(&TextQuad::new("A", 0.0, 0.0, 16.0, [255, 255, 255, 255]));
        assert!(!qs.is_empty(), "A must produce pixel quads");
        for q in &qs {
            assert!((q.w - 0.5).abs() < 1e-4, "coverage quads are half a pixel");
            assert!((q.h - 0.5).abs() < 1e-4);
        }
    }

    #[test]
    fn japanese_kana_and_kanji_rasterize() {
        let mut r = raster();
        for ch in ['あ', 'カ', 'ト', 'ル', 'ネ', 'コ', '食'] {
            let qs = r.text_quads(&TextQuad::new(
                &ch.to_string(),
                0.0,
                0.0,
                16.0,
                [255, 255, 255, 255],
            ));
            assert!(!qs.is_empty(), "char {ch} must rasterize");
            assert!(
                !is_hollow_box(&qs),
                "char {ch} must not be a missing-glyph box"
            );
        }
    }

    #[test]
    fn every_japanese_character_in_the_games_is_in_the_subset() {
        // Zen Maru Gothic is cut down to the characters the games use. A new kanji outside it
        // would quietly fall back to the pixel face: rerun `python tools/subset_fonts.py`.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = Vec::new();
        let mut dirs = vec![root.join("src")];
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    files.push(p);
                }
            }
        }
        for d in [root.join("../games"), root.join("tests/fixtures")] {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                if e.path().extension().is_some_and(|x| x == "json") {
                    files.push(e.path());
                }
            }
        }
        let mut missing = std::collections::BTreeSet::new();
        for f in &files {
            for ch in std::fs::read_to_string(f).unwrap_or_default().chars() {
                let jp = matches!(ch as u32, 0x3041..=0x30FF | 0x4E00..=0x9FFF);
                if jp && !font_has_glyph(&JP_FONT, ch) {
                    missing.insert(ch);
                }
            }
        }
        assert!(
            missing.is_empty(),
            "not in ZenMaruGothic-Bold.ttf, run tools/subset_fonts.py: {missing:?}"
        );
    }

    #[test]
    fn japanese_uses_zen_maru_and_digits_can_use_the_display_face() {
        let jp = select_font('弾').unwrap();
        assert_ne!(jp.glyph_id('弾').0, 0);
        assert!(std::ptr::eq(jp, &*JP_FONT), "kanji draw in Zen Maru Gothic");
        assert!(std::ptr::eq(
            select_font_for('7', TextFace::Display).unwrap(),
            &*DISPLAY_FONT
        ));
        assert!(std::ptr::eq(
            select_font_for('7', TextFace::Ui).unwrap(),
            &*UI_FONT
        ));
        // The display face has no kana: it falls back to the Japanese face.
        assert!(std::ptr::eq(
            select_font_for('あ', TextFace::Display).unwrap(),
            &*JP_FONT
        ));
        let mut r = raster();
        let ui = r.text_quads(&TextQuad::new("1:57", 0.0, 0.0, 24.0, [255; 4]));
        let disp = r.text_quads(
            &TextQuad::new("1:57", 0.0, 0.0, 24.0, [255; 4]).with_face(TextFace::Display),
        );
        assert!(
            !disp.is_empty() && ui != disp,
            "a different face draws different pixels"
        );
    }

    #[test]
    fn japanese_falls_back_off_the_latin_ui_face() {
        assert_eq!(UI_FONT.glyph_id('あ').0, 0, "Public Sans has no hiragana");
        assert_ne!(FONT.glyph_id('あ').0, 0, "PixelMplus covers あ");
        assert!(select_font('あ').is_some());
        assert!(select_font('A').is_some());
        assert_eq!(
            select_font('A').map(|f| f.glyph_id('A').0),
            Some(UI_FONT.glyph_id('A').0)
        );
    }

    #[test]
    fn missing_glyph_draws_no_notdef_box() {
        let ch = '\u{E000}';
        assert_eq!(UI_FONT.glyph_id(ch).0, 0);
        assert_eq!(FONT.glyph_id(ch).0, 0);
        assert!(select_font(ch).is_none());
        let mut r = raster();
        let qs = r.text_quads(&TextQuad::new(
            &ch.to_string(),
            0.0,
            0.0,
            16.0,
            [255, 255, 255, 255],
        ));
        assert!(qs.is_empty(), "missing glyph must not paint a tofu box");
    }

    /// Hollow rectangle (`.notdef`): every coverage pixel sits on the bbox edge.
    fn is_hollow_box(qs: &[Quad]) -> bool {
        if qs.len() < 8 {
            return false;
        }
        let min_x = qs.iter().map(|q| q.x).fold(f32::MAX, f32::min);
        let max_x = qs.iter().map(|q| q.x).fold(f32::MIN, f32::max);
        let min_y = qs.iter().map(|q| q.y).fold(f32::MAX, f32::min);
        let max_y = qs.iter().map(|q| q.y).fold(f32::MIN, f32::max);
        let w = max_x - min_x;
        let h = max_y - min_y;
        if w < 4.0 || h < 4.0 {
            return false;
        }
        qs.iter().all(|q| {
            let on_x = (q.x - min_x).abs() < 0.5 || (q.x - max_x).abs() < 0.5;
            let on_y = (q.y - min_y).abs() < 0.5 || (q.y - max_y).abs() < 0.5;
            on_x || on_y
        })
    }

    #[test]
    fn measure_monospace_advance_scales_with_size() {
        let mut r = raster();
        let w10 = r.measure_text_width("abc", 10.0);
        let w20 = r.measure_text_width("abc", 20.0);
        assert!(w10 > 0.0);
        assert!(
            (w20 - w10 * 2.0).abs() < 2.0,
            "doubling size doubles width: {w10} {w20}"
        );
    }

    #[test]
    fn newline_splits_lines_and_advances_y() {
        let mut r = raster();
        let qs = r.text_quads(&TextQuad::new("A\nB", 0.0, 0.0, 16.0, [255, 255, 255, 255]));
        let ys: Vec<f32> = qs.iter().map(|q| q.y).collect();
        let min_y = ys.iter().cloned().fold(f32::MAX, f32::min);
        let max_y = ys.iter().cloned().fold(f32::MIN, f32::max);
        assert!(
            max_y - min_y > 10.0,
            "two lines must be vertically separated"
        );
    }

    #[test]
    fn center_align_shifts_x_left() {
        let mut r = raster();
        let left = r.text_quads(&TextQuad::new("AA", 50.0, 0.0, 16.0, [255, 255, 255, 255]));
        let center = r.text_quads(
            &TextQuad::new("AA", 50.0, 0.0, 16.0, [255, 255, 255, 255]).aligned(TextAlign::Center),
        );
        let l_min = left.iter().map(|q| q.x).fold(f32::MAX, f32::min);
        let c_min = center.iter().map(|q| q.x).fold(f32::MAX, f32::min);
        assert!(
            c_min < l_min,
            "center text starts further left: {c_min} < {l_min}"
        );
    }

    #[test]
    fn space_advances_without_pixels() {
        let mut r = raster();
        let space = r.text_quads(&TextQuad::new(" ", 0.0, 0.0, 16.0, [255, 255, 255, 255]));
        assert!(space.is_empty());
    }

    #[test]
    fn alpha_scales_with_coverage() {
        let mut r = raster();
        let qs = r.text_quads(&TextQuad::new("A", 0.0, 0.0, 16.0, [255, 0, 0, 128]));
        assert!(!qs.is_empty());
        for q in &qs {
            assert!(q.color[3] <= 128, "alpha must not exceed source alpha");
            assert_eq!((q.color[0], q.color[1], q.color[2]), (255, 0, 0));
        }
    }
}
