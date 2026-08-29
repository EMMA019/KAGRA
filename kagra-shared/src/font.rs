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

/// 埋め込みフォント（M+ ライセンス）。
pub const DEFAULT_FONT_BYTES: &[u8] = include_bytes!("../assets/PixelMplus10-Regular.ttf");

static FONT: Lazy<ab_glyph::FontArc> = Lazy::new(|| {
    ab_glyph::FontArc::try_from_slice(DEFAULT_FONT_BYTES)
        .expect("embedded PixelMplus10-Regular.ttf must parse")
});

/// グリフのカバレッジ画素（0..1）。レイアウト位置は持たない。
#[derive(Debug)]
struct Coverage {
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
    let scaled = FONT.as_scaled(size.max(1.0));
    text.chars().map(|c| scaled.h_advance(scaled.glyph_id(c))).sum()
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
        let scaled = FONT.as_scaled(size.max(1.0));
        let mut max_w = 0.0f32;
        for line in text.split('\n') {
            let w: f32 = line.chars().map(|c| scaled.h_advance(scaled.glyph_id(c))).sum();
            max_w = max_w.max(w);
        }
        max_w
    }

    /// TextQuad を Quad の列に展開する。点灯画素 1 つ = 1px の Quad。
    pub fn text_quads(&mut self, tq: &TextQuad) -> Vec<Quad> {
        let size = tq.size.max(1.0);
        let scaled = FONT.as_scaled(size);
        let baseline = tq.y + scaled.ascent();
        let line_h = scaled.height().max(size);
        let mut out = Vec::new();
        let mut line_start_y = baseline;
        for line in tq.text.split('\n') {
            let line_w: f32 = line.chars().map(|c| scaled.h_advance(scaled.glyph_id(c))).sum();
            let x0 = match tq.align {
                TextAlign::Left => tq.x,
                TextAlign::Center => tq.x - line_w * 0.5,
                TextAlign::Right => tq.x - line_w,
            };
            let mut col = x0;
            for ch in line.chars() {
                let advance = scaled.h_advance(scaled.glyph_id(ch));
                if ch != ' ' {
                    if let Some(cov) = self.coverage(ch, size.round() as u32) {
                        out.extend(coverage_quads(cov, col, line_start_y, tq.color));
                    }
                }
                col += advance;
            }
            line_start_y += line_h;
        }
        out
    }
}

fn coverage_quads(cov: &Coverage, x: f32, y: f32, color: [u8; 4]) -> Vec<Quad> {
    let mut out = Vec::with_capacity(cov.pixels.len());
    for py in 0..cov.h {
        for px in 0..cov.w {
            let a = cov.pixels[(py * cov.w + px) as usize];
            if a <= 0.0 {
                continue;
            }
            let alpha = ((color[3] as f32) * a).round().clamp(0.0, 255.0) as u8;
            if alpha == 0 {
                continue;
            }
            out.push(Quad::new(
                x + px as f32,
                y + py as f32,
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
    let scaled = FONT.as_scaled(size);
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
    Some(Coverage { w, h, pixels })
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
            let qs = r.text_quads(&TextQuad::new(&ch.to_string(), 0.0, 0.0, 16.0, [255, 255, 255, 255]));
            assert!(!qs.is_empty(), "char {ch} must rasterize");
        }
    }

    #[test]
    fn measure_monospace_advance_scales_with_size() {
        let mut r = raster();
        let w10 = r.measure_text_width("abc", 10.0);
        let w20 = r.measure_text_width("abc", 20.0);
        assert!(w10 > 0.0);
        assert!((w20 - w10 * 2.0).abs() < 2.0, "doubling size doubles width: {w10} {w20}");
    }

    #[test]
    fn newline_splits_lines_and_advances_y() {
        let mut r = raster();
        let qs = r.text_quads(&TextQuad::new("A\nB", 0.0, 0.0, 16.0, [255, 255, 255, 255]));
        let ys: Vec<f32> = qs.iter().map(|q| q.y).collect();
        let min_y = ys.iter().cloned().fold(f32::MAX, f32::min);
        let max_y = ys.iter().cloned().fold(f32::MIN, f32::max);
        assert!(max_y - min_y > 10.0, "two lines must be vertically separated");
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
        assert!(c_min < l_min, "center text starts further left: {c_min} < {l_min}");
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
