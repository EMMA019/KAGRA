//! A CPU picture of a HUD [`DrawList`]: triangles, rounded quads and text, in the same order
//! and with the same shapes as the GPU path (`render/shader.wgsl`). No GPU, no wasm: tests
//! and agents can look at a kit's screens as PNG files (`examples/hud_shot.rs`).
//!
//! Colours blend in byte space like an unorm swapchain. It is a review picture, not a
//! bit-exact copy of the GPU frame (no sRGB target, no 3D world under the HUD).

use std::collections::HashMap;

use crate::font::{self, Coverage};
use crate::scene::{DrawList, Quad, TextFace, Vert2};

/// An RGBA8 picture, rows top to bottom.
pub struct Canvas {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
    glyphs: HashMap<(char, u32, TextFace), Option<Coverage>>,
}

impl Canvas {
    pub fn new(w: u32, h: u32, clear: [u8; 4]) -> Self {
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..w * h {
            rgba.extend_from_slice(&clear);
        }
        Self {
            w,
            h,
            rgba,
            glyphs: HashMap::new(),
        }
    }

    /// Draw a whole list (and its layers) the way the renderer does.
    pub fn draw(&mut self, list: &DrawList) {
        for l in list.flatten() {
            for t in l.mesh.verts.chunks_exact(3) {
                self.triangle(t[0], t[1], t[2]);
            }
            for q in &l.quads {
                self.quad(q);
            }
            for t in &l.texts {
                self.text(t);
            }
        }
    }

    fn blend(&mut self, x: i32, y: i32, c: [f32; 4]) {
        if x < 0 || y < 0 || x >= self.w as i32 || y >= self.h as i32 || c[3] <= 0.0 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        let a = c[3].clamp(0.0, 1.0);
        for (d, src) in self.rgba[i..i + 3].iter_mut().zip(c) {
            let v = *d as f32;
            *d = (v + (src * 255.0 - v) * a).round().clamp(0.0, 255.0) as u8;
        }
        let da = self.rgba[i + 3] as f32 / 255.0;
        self.rgba[i + 3] = ((a + da * (1.0 - a)) * 255.0).round() as u8;
    }

    fn triangle(&mut self, a: Vert2, b: Vert2, c: Vert2) {
        let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        if area.abs() < 1e-6 {
            return;
        }
        let x0 = a.x.min(b.x).min(c.x).floor().max(0.0) as i32;
        let x1 = a.x.max(b.x).max(c.x).ceil().min(self.w as f32) as i32;
        let y0 = a.y.min(b.y).min(c.y).floor().max(0.0) as i32;
        let y1 = a.y.max(b.y).max(c.y).ceil().min(self.h as f32) as i32;
        let col = |v: Vert2| v.color.map(|x| x as f32 / 255.0);
        let (ca, cb, cc) = (col(a), col(b), col(c));
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((b.x - px) * (c.y - py) - (b.y - py) * (c.x - px)) / area;
                let w1 = ((c.x - px) * (a.y - py) - (c.y - py) * (a.x - px)) / area;
                let w2 = 1.0 - w0 - w1;
                // Top-left-ish rule: include shared edges once (small epsilon).
                if w0 < -1e-5 || w1 < -1e-5 || w2 < -1e-5 {
                    continue;
                }
                let c4: [f32; 4] = std::array::from_fn(|k| ca[k] * w0 + cb[k] * w1 + cc[k] * w2);
                self.blend(x, y, c4);
            }
        }
    }

    fn quad(&mut self, q: &Quad) {
        let (cx, cy) = (q.x + q.w * 0.5, q.y + q.h * 0.5);
        let (hw, hh) = (q.w.abs() * 0.5, q.h.abs() * 0.5);
        let c = q.color.map(|x| x as f32 / 255.0);
        let x0 = (cx - hw).floor().max(0.0) as i32;
        let x1 = (cx + hw).ceil().min(self.w as f32) as i32;
        let y0 = (cy - hh).floor().max(0.0) as i32;
        let y1 = (cy + hh).ceil().min(self.h as f32) as i32;
        let r = q.radius.min(hw).min(hh);
        for y in y0..y1 {
            for x in x0..x1 {
                let (lx, ly) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let a = if q.radius > 0.0 {
                    let (qx, qy) = (lx.abs() - hw + r, ly.abs() - hh + r);
                    let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt()
                        + qx.max(qy).min(0.0)
                        - r;
                    let mut a = (0.5 - d).clamp(0.0, 1.0);
                    if q.stroke > 0.0 {
                        a *= (0.5 + d + q.stroke).clamp(0.0, 1.0);
                    }
                    a
                } else if lx.abs() <= hw && ly.abs() <= hh {
                    1.0
                } else {
                    0.0
                };
                self.blend(x, y, [c[0], c[1], c[2], c[3] * a]);
            }
        }
    }

    fn text(&mut self, t: &crate::scene::TextQuad) {
        let px = font::atlas_px(t.size);
        let k = t.size.max(1.0) / px as f32;
        let c = t.color.map(|x| x as f32 / 255.0);
        for g in font::layout(t) {
            let cov = self
                .glyphs
                .entry((g.ch, px, t.face))
                .or_insert_with(|| font::glyph_coverage(g.ch, px, t.face));
            let Some(cov) = cov.as_ref() else { continue };
            let (l, top) = (g.x + cov.dx * k, g.baseline + cov.dy * k);
            let (gw, gh) = (cov.w as f32 * k, cov.h as f32 * k);
            let (cw, ch) = (cov.w as i32, cov.h as i32);
            let sample = |u: f32, v: f32| -> f32 {
                // Bilinear, like the atlas sampler.
                let (fx, fy) = (u - 0.5, v - 0.5);
                let (x0, y0) = (fx.floor() as i32, fy.floor() as i32);
                let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
                let at = |x: i32, y: i32| -> f32 {
                    if x < 0 || y < 0 || x >= cw || y >= ch {
                        0.0
                    } else {
                        cov.pixels[(y * cw + x) as usize]
                    }
                };
                let top = at(x0, y0) * (1.0 - tx) + at(x0 + 1, y0) * tx;
                let bot = at(x0, y0 + 1) * (1.0 - tx) + at(x0 + 1, y0 + 1) * tx;
                top * (1.0 - ty) + bot * ty
            };
            let mut hits = Vec::new();
            for y in top.floor() as i32..(top + gh).ceil() as i32 {
                for x in l.floor() as i32..(l + gw).ceil() as i32 {
                    let (u, v) = ((x as f32 + 0.5 - l) / k, (y as f32 + 0.5 - top) / k);
                    let a = sample(u, v);
                    if a > 0.0 {
                        hits.push((x, y, a));
                    }
                }
            }
            for (x, y, a) in hits {
                self.blend(x, y, [c[0], c[1], c[2], c[3] * a]);
            }
        }
    }

    /// Save as PNG.
    pub fn save_png(&self, path: &std::path::Path) -> std::io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), self.w, self.h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(std::io::Error::other)?;
        writer
            .write_image_data(&self.rgba)
            .map_err(std::io::Error::other)
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.w + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }
}

/// Draw a list onto a fresh canvas cleared to its `clear` colour (opaque).
pub fn picture(list: &DrawList, w: u32, h: u32) -> Canvas {
    let mut c = Canvas::new(w, h, [list.clear[0], list.clear[1], list.clear[2], 255]);
    c.draw(list);
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Mesh2d, TextQuad};
    use crate::vector::{self, Paint};

    #[test]
    fn layers_cover_the_text_under_them() {
        let mut list = DrawList {
            clear: [0, 0, 0, 255],
            ..Default::default()
        };
        list.texts
            .push(TextQuad::new("OK", 10.0, 10.0, 30.0, [255, 255, 255, 255]));
        let top = list.layer();
        top.quads
            .push(Quad::new(0.0, 0.0, 100.0, 60.0, [200, 0, 0, 255]));
        let c = picture(&list, 100, 60);
        // The red layer hides the white text everywhere.
        assert!(c.rgba.chunks(4).all(|p| p[0] == 200 && p[1] == 0));
    }

    #[test]
    fn meshes_draw_under_quads_and_blend_gradients() {
        let mut list = DrawList::default();
        let mut m = Mesh2d::default();
        vector::fill(
            &mut m,
            &[[0.0, 0.0], [40.0, 0.0], [40.0, 40.0], [0.0, 40.0]],
            &Paint::vertical(0.0, 40.0, [0, 0, 0, 255], [0, 0, 200, 255]),
        );
        list.mesh = m;
        list.quads
            .push(Quad::new(30.0, 30.0, 10.0, 10.0, [0, 255, 0, 255]));
        let c = picture(&list, 40, 40);
        assert_eq!(c.pixel(35, 35), [0, 255, 0, 255]);
        let (top, bottom) = (c.pixel(5, 1)[2], c.pixel(5, 25)[2]);
        assert!(bottom > top + 80, "{top} {bottom}");
        assert_eq!(c.pixel(0, 0)[3], 255);
    }

    #[test]
    fn text_leaves_ink() {
        let mut list = DrawList::default();
        list.texts
            .push(TextQuad::new("白球", 4.0, 4.0, 24.0, [255, 255, 255, 255]));
        let c = picture(&list, 64, 40);
        assert!(c.rgba.chunks(4).filter(|p| p[0] > 128).count() > 40);
    }
}
