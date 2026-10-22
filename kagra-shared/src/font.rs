//! テキスト → 画面 Quad。GPU 非依存なので `render` feature 無しでもテストでき、
//! wasm / Android / iOS / オフスクリーンが同じ画素を出す。
//!
//! 埋め込みフォントは PixelMplus（M+ FONT LICENSE、自由に使用・再配布可、
//! JIS 第1・2水準の漢字 + Latin-1 収録）: `assets/PixelMplus10-Regular.ttf`。
//! グリフは `ab_glyph`（純 Rust）でラスタライズし、カバレッジ画素を 1px の
//! `Quad` に展開する。テクスチャ / シェーダーの追加は不要で、既存の
//! 2D HUD パスがそのまま文字を描く。

use std::collections::HashMap;

use ab_glyph::{Font as _, ScaleFont}; // as_scaled(Font) / glyph_id・h_advance(ScaleFont)
use once_cell::sync::Lazy;

use crate::scene::{Quad, TextAlign, TextQuad};

/// 埋め込みフォント（M+ ライセンス: 日本語フォールバック）。
pub const DEFAULT_FONT_BYTES: &[u8] = include_bytes!("../assets/PixelMplus10-Regular.ttf");

/// 滑らかな欧文 UI フォント（Public Sans Bold, SIL OFL）。
pub const UI_FONT_BYTES: &[u8] = include_bytes!("../assets/PublicSans-Bold.ttf");

static FONT: Lazy<ab_glyph::FontArc> = Lazy::new(|| {
    ab_glyph::FontArc::try_from_slice(DEFAULT_FONT_BYTES)
        .expect("embedded PixelMplus10-Regular.ttf must parse")
});

static UI_FONT: Lazy<ab_glyph::FontArc> = Lazy::new(|| {
    ab_glyph::FontArc::try_from_slice(UI_FONT_BYTES)
        .expect("embedded PublicSans-Bold.ttf must parse")
});

fn font_has_glyph(font: &ab_glyph::FontArc, ch: char) -> bool {
    // Glyph 0 is `.notdef`. Drawing it paints a missing-glyph box.
    font.glyph_id(ch).0 != 0
}

/// Latin prefers Public Sans. Everything else, and any Latin the UI face
/// lacks, falls back to PixelMplus (JIS). If neither face has the character,
/// return `None` so the caller skips the glyph instead of rasterizing `.notdef`.
fn select_font(ch: char) -> Option<&'static ab_glyph::FontArc> {
    if (ch as u32) <= 0x024F && font_has_glyph(&UI_FONT, ch) {
        return Some(&UI_FONT);
    }
    if font_has_glyph(&FONT, ch) {
        return Some(&FONT);
    }
    if font_has_glyph(&UI_FONT, ch) {
        return Some(&UI_FONT);
    }
    None
}

fn char_advance(ch: char, size: f32) -> f32 {
    let size = size.max(1.0);
    let Some(font) = select_font(ch) else {
        return size * 0.5;
    };
    let scaled = font.as_scaled(size);
    scaled.h_advance(scaled.glyph_id(ch))
}

/// グリフのカバレッジ画素（0..1）とベースライン相対オフセット。
#[derive(Debug)]
struct Coverage {
    dx: f32,
    dy: f32,
    w: u32,
    h: u32,
    pixels: Vec<f32>,
}

/// (文字, ピクセル高) → カバレッジ。1 回ラスタライズしたら使い回す。
#[derive(Debug, Default)]
pub struct TextRaster {
    cache: HashMap<(char, u32), Coverage>,
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

    fn coverage(&mut self, ch: char, px: u32) -> Option<&Coverage> {
        let key = (ch, px);
        match self.cache.entry(key) {
            std::collections::hash_map::Entry::Occupied(e) => Some(e.into_mut()),
            std::collections::hash_map::Entry::Vacant(e) => {
                let cov = rasterize(ch, px)?;
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

    /// TextQuad を Quad の列に展開する。点灯画素 1 つ = 1px の Quad。
    pub fn text_quads(&mut self, tq: &TextQuad) -> Vec<Quad> {
        let size = tq.size.max(1.0);
        let scaled = UI_FONT.as_scaled(size);
        let baseline = (tq.y + scaled.ascent()).round();
        let line_h = scaled.height().max(size * 1.2).round();
        let mut out = Vec::new();
        let mut line_base_y = baseline;
        for line in tq.text.split('\n') {
            let line_w: f32 = line.chars().map(|c| char_advance(c, size)).sum();
            let x0 = match tq.align {
                TextAlign::Left => tq.x,
                TextAlign::Center => tq.x - line_w * 0.5,
                TextAlign::Right => tq.x - line_w,
            };
            let mut col = x0;
            for ch in line.chars() {
                let advance = char_advance(ch, size);
                if ch != ' ' {
                    if let Some(cov) = self.coverage(ch, size.round() as u32) {
                        out.extend(coverage_quads(cov, col.round(), line_base_y, tq.color));
                    }
                }
                col += advance;
            }
            line_base_y += line_h;
        }
        out
    }
}

fn coverage_quads(cov: &Coverage, x: f32, y: f32, color: [u8; 4]) -> Vec<Quad> {
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
                x + cov.dx + px as f32,
                y + cov.dy + py as f32,
                1.0,
                1.0,
                [color[0], color[1], color[2], alpha],
            ));
        }
    }
    out
}

fn rasterize(ch: char, px: u32) -> Option<Coverage> {
    let size = px.max(1) as f32;
    let font = select_font(ch)?;
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
            assert_eq!(q.w, 1.0);
            assert_eq!(q.h, 1.0);
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
