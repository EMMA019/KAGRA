//! Illustrated HUD pieces drawn with [`crate::vector`]: soft-shadowed gradient panels, glossy
//! buttons, ribbons, letter chips, gauges, stars, speech bubbles and portraits, placed on a
//! [`Stage`] (a fixed design canvas fitted to any screen).
//!
//! A kit lays a screen out once in design units (say 400 wide, phone first). [`Stage`] maps
//! them onto the real screen: a phone in portrait fills the width, a desktop shows the
//! canvas centred and the backdrop fills the sides. [`Painter`] draws in design units and
//! keeps the draw order: art and text go on the top layer, [`Painter::layer`] starts a new
//! layer (a popup, a card that overlaps another card).
//!
//! ```
//! use kagra_shared::hud_art::{Painter, Stage};
//! use kagra_shared::scene::DrawList;
//! let mut list = DrawList::default();
//! let st = Stage::fit(390.0, 844.0, 400.0, 720.0);
//! let mut p = Painter::new(&mut list, st);
//! p.panel(20.0, 20.0, 200.0, 120.0, 18.0, [255, 253, 246, 255], [240, 232, 214, 255], 12.0);
//! p.text("白球デイズ", 120.0, 60.0, 24.0, [30, 34, 50, 255], kagra_shared::TextAlign::Center);
//! let ok = p.button(40.0, 160.0, 160.0, 48.0, "OK", 18.0, [255, 138, 48, 255], true);
//! assert!(ok[2] > 100.0);
//! ```

use crate::game_core::portrait::{self, Expr, Face, Outfit};
use crate::scene::{DrawList, Mesh2d, TextAlign, TextFace, TextQuad};
use crate::vector::{self, mix, shade, Paint, Rgba, P};

/// A design canvas fitted to the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stage {
    /// Screen position of the canvas's top-left, and pixels per design unit.
    pub ox: f32,
    pub oy: f32,
    pub k: f32,
    /// The canvas size in design units (the height grows on tall phones).
    pub w: f32,
    pub h: f32,
    /// The whole screen in design units, from the canvas origin (for backdrops).
    pub screen: [f32; 4],
}

impl Stage {
    /// Fit a canvas `design_w` wide and at least `min_h` tall: as wide as the screen when it
    /// is tall enough, else as tall as the screen and centred.
    pub fn fit(sw: f32, sh: f32, design_w: f32, min_h: f32) -> Self {
        let (sw, sh) = (sw.max(1.0), sh.max(1.0));
        let k = (sw / design_w).min(sh / min_h);
        let w = design_w;
        let h = sh / k;
        let ox = (sw - w * k) * 0.5;
        let oy = 0.0;
        Stage {
            ox,
            oy,
            k,
            w,
            h,
            screen: [-ox / k, -oy / k, sw / k, sh / k],
        }
    }

    pub fn x(&self, u: f32) -> f32 {
        self.ox + u * self.k
    }

    pub fn y(&self, u: f32) -> f32 {
        self.oy + u * self.k
    }

    pub fn pt(&self, p: P) -> P {
        [self.x(p[0]), self.y(p[1])]
    }

    /// A rectangle in screen pixels `[x, y, w, h]` (for hit tests).
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> [f32; 4] {
        [self.x(x), self.y(y), w * self.k, h * self.k]
    }

    /// A screen point in design units.
    pub fn to_design(&self, sx: f32, sy: f32) -> (f32, f32) {
        ((sx - self.ox) / self.k, (sy - self.oy) / self.k)
    }

    /// Does the screen point fall in the design rectangle?
    pub fn hit(&self, sx: f32, sy: f32, r: [f32; 4]) -> bool {
        let (x, y) = self.to_design(sx, sy);
        x >= r[0] && y >= r[1] && x <= r[0] + r[2] && y <= r[1] + r[3]
    }
}

/// Draws in design units on a [`Stage`], always on the list's top layer.
pub struct Painter<'a> {
    pub list: &'a mut DrawList,
    pub st: Stage,
}

impl<'a> Painter<'a> {
    pub fn new(list: &'a mut DrawList, st: Stage) -> Self {
        Painter { list, st }
    }

    /// Start a layer: everything after covers everything before (text included).
    pub fn layer(&mut self) {
        self.list.layer();
    }

    pub fn mesh(&mut self) -> &mut Mesh2d {
        &mut self.list.top().mesh
    }

    fn pts(&self, pts: &[P]) -> Vec<P> {
        pts.iter().map(|p| self.st.pt(*p)).collect()
    }

    /// Fill a design-unit polygon.
    pub fn fill(&mut self, pts: &[P], paint: Paint) {
        let pts = self.pts(pts);
        let paint = self.paint(paint);
        vector::fill(self.mesh(), &pts, &paint);
    }

    /// Fill and outline (outline `w` design units).
    pub fn fill_ink(&mut self, pts: &[P], c: Rgba, ink: Rgba, w: f32) {
        let pts = self.pts(pts);
        let k = self.st.k;
        vector::fill(self.mesh(), &pts, &Paint::Solid(c));
        vector::stroke_closed(self.mesh(), &pts, (w * k).max(0.8), ink);
    }

    /// An open line `w` design units wide.
    pub fn line(&mut self, pts: &[P], w: f32, c: Rgba) {
        let pts = self.pts(pts);
        let k = self.st.k;
        vector::stroke(self.mesh(), &pts, (w * k).max(0.8), c);
    }

    pub fn outline(&mut self, pts: &[P], w: f32, c: Rgba) {
        let pts = self.pts(pts);
        let k = self.st.k;
        vector::stroke_closed(self.mesh(), &pts, (w * k).max(0.8), c);
    }

    pub fn glow(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        let (p, k) = (self.st.pt([cx, cy]), self.st.k);
        vector::glow(self.mesh(), p[0], p[1], r * k, c);
    }

    pub fn dot(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        let (p, k) = (self.st.pt([cx, cy]), self.st.k);
        vector::dot(self.mesh(), p, r * k, c);
    }

    fn paint(&self, paint: Paint) -> Paint {
        match paint {
            Paint::Solid(c) => Paint::Solid(c),
            Paint::Linear { from, to, c0, c1 } => Paint::Linear {
                from: self.st.pt(from),
                to: self.st.pt(to),
                c0,
                c1,
            },
            Paint::Radial {
                center,
                radius,
                c0,
                c1,
            } => Paint::Radial {
                center: self.st.pt(center),
                radius: radius * self.st.k,
                c0,
                c1,
            },
        }
    }

    /// A soft drop shadow under a rounded rectangle.
    #[allow(clippy::too_many_arguments)]
    pub fn shadow(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, blur: f32, alpha: u8) {
        if blur <= 0.0 || alpha == 0 {
            return;
        }
        let k = self.st.k;
        let rr = vector::rounded_rect(self.st.x(x), self.st.y(y), w * k, h * k, r * k);
        vector::shadow(
            self.mesh(),
            &rr,
            [0.0, blur * 0.35 * k],
            blur * k,
            [0, 0, 0, alpha],
        );
    }

    /// A rounded card: soft shadow, a top-to-bottom gradient.
    #[allow(clippy::too_many_arguments)]
    pub fn panel(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        top: Rgba,
        bottom: Rgba,
        shadow: f32,
    ) {
        self.shadow(x, y, w, h, r, shadow, 70);
        let rr = vector::rounded_rect(x, y, w, h, r);
        self.fill(&rr, Paint::vertical(y, y + h, top, bottom));
    }

    /// [`Painter::panel`] with a border `bw` design units wide.
    #[allow(clippy::too_many_arguments)]
    pub fn panel_bordered(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        top: Rgba,
        bottom: Rgba,
        border: Rgba,
        bw: f32,
        shadow: f32,
    ) {
        self.panel(x, y, w, h, r, top, bottom, shadow);
        let inset = bw * 0.5;
        let rr = vector::rounded_rect(x + inset, y + inset, w - bw, h - bw, (r - inset).max(0.0));
        self.outline(&rr, bw, border);
    }

    /// Text in design units (`y` is the top of the line).
    pub fn text(&mut self, s: &str, x: f32, y: f32, size: f32, c: Rgba, align: TextAlign) {
        let k = self.st.k;
        let (sx, sy) = (self.st.x(x), self.st.y(y));
        self.list
            .top()
            .texts
            .push(TextQuad::new(s, sx, sy, size * k, c).aligned(align));
    }

    /// Text in the display face (numbers, titles).
    pub fn text_display(&mut self, s: &str, x: f32, y: f32, size: f32, c: Rgba, align: TextAlign) {
        let k = self.st.k;
        let (sx, sy) = (self.st.x(x), self.st.y(y));
        self.list.top().texts.push(
            TextQuad::new(s, sx, sy, size * k, c)
                .aligned(align)
                .with_face(TextFace::Display),
        );
    }

    /// Text with a hard shadow under it (titles on pictures).
    #[allow(clippy::too_many_arguments)]
    pub fn text_shadow(
        &mut self,
        s: &str,
        x: f32,
        y: f32,
        size: f32,
        c: Rgba,
        shadow: Rgba,
        align: TextAlign,
    ) {
        let d = (size * 0.07).max(1.0);
        self.text(s, x + d * 0.6, y + d, size, shadow, align);
        self.text(s, x, y, size, c, align);
    }

    /// Bold outlined text (a big title): the outline is the text drawn around it.
    #[allow(clippy::too_many_arguments)]
    pub fn text_outlined(
        &mut self,
        s: &str,
        x: f32,
        y: f32,
        size: f32,
        c: Rgba,
        ink: Rgba,
        w: f32,
        align: TextAlign,
    ) {
        for i in 0..12 {
            let a = i as f32 / 12.0 * std::f32::consts::TAU;
            self.text(s, x + a.cos() * w, y + a.sin() * w, size, ink, align);
        }
        self.text(s, x, y, size, c, align);
    }

    /// A glossy button: gradient body, a sheen on the top half, a darker rim, a shadow.
    /// Returns its screen rectangle. A disabled button is grey.
    #[allow(clippy::too_many_arguments)]
    pub fn button(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: &str,
        size: f32,
        color: Rgba,
        enabled: bool,
    ) -> [f32; 4] {
        let c = if enabled {
            color
        } else {
            mix(color, [150, 150, 160, 255], 0.75)
        };
        let r = (h * 0.5).min(18.0);
        self.shadow(x, y, w, h, r, h * 0.22, if enabled { 90 } else { 40 });
        let rr = vector::rounded_rect(x, y, w, h, r);
        self.fill(
            &rr,
            Paint::vertical(y, y + h, shade(c, 0.22), shade(c, -0.12)),
        );
        let sheen = vector::rounded_rect(x + 4.0, y + 3.0, w - 8.0, h * 0.42, (r - 3.0).max(2.0));
        self.fill(
            &sheen,
            Paint::vertical(y, y + h * 0.45, [255, 255, 255, 90], [255, 255, 255, 25]),
        );
        self.outline(&rr, 1.5, shade(c, -0.35));
        let tc = if enabled {
            [255, 255, 255, 255]
        } else {
            [240, 240, 244, 220]
        };
        self.text_shadow(
            label,
            x + w * 0.5,
            y + (h - size) * 0.5 - size * 0.08,
            size,
            tc,
            shade(c, -0.5),
            TextAlign::Center,
        );
        self.st.rect(x, y, w, h)
    }

    /// A ribbon banner with a label (a scene's title).
    #[allow(clippy::too_many_arguments)]
    pub fn ribbon(&mut self, cx: f32, y: f32, w: f32, h: f32, label: &str, size: f32, color: Rgba) {
        let x = cx - w * 0.5;
        for side in [-1.0f32, 1.0] {
            let ex = if side < 0.0 {
                x - h * 0.45
            } else {
                x + w + h * 0.45
            };
            let ix = if side < 0.0 {
                x + h * 0.4
            } else {
                x + w - h * 0.4
            };
            let tail = [
                [ix, y + h * 0.22],
                [ex, y + h * 0.22],
                [ex + side * -h * 0.25, y + h * 0.6],
                [ex, y + h * 0.98],
                [ix, y + h * 0.98],
            ];
            self.fill(&tail, Paint::Solid(shade(color, -0.3)));
        }
        self.panel(
            x,
            y,
            w,
            h * 0.82,
            h * 0.2,
            shade(color, 0.18),
            shade(color, -0.1),
            h * 0.3,
        );
        self.text_shadow(
            label,
            cx,
            y + (h * 0.82 - size) * 0.5 - size * 0.06,
            size,
            [255, 255, 255, 255],
            shade(color, -0.5),
            TextAlign::Center,
        );
    }

    /// A rounded letter chip (a grade, a rank).
    pub fn chip(&mut self, cx: f32, cy: f32, size: f32, letter: &str, color: Rgba) {
        let (x, y) = (cx - size * 0.5, cy - size * 0.5);
        let rr = vector::rounded_rect(x, y, size, size, size * 0.26);
        self.fill(
            &rr,
            Paint::vertical(y, y + size, shade(color, 0.25), shade(color, -0.15)),
        );
        self.outline(&rr, (size * 0.05).max(1.0), shade(color, -0.4));
        self.text_display(
            letter,
            cx,
            cy - size * 0.42,
            size * 0.78,
            [255, 255, 255, 255],
            TextAlign::Center,
        );
    }

    /// A pill gauge with a sheen.
    #[allow(clippy::too_many_arguments)]
    pub fn gauge(&mut self, x: f32, y: f32, w: f32, h: f32, frac: f32, color: Rgba, back: Rgba) {
        let rr = vector::rounded_rect(x, y, w, h, h * 0.5);
        self.fill(&rr, Paint::Solid(back));
        let f = frac.clamp(0.0, 1.0);
        if f > 0.0 {
            let fw = (w * f).max(h);
            let bar = vector::rounded_rect(x, y, fw, h, h * 0.5);
            self.fill(
                &bar,
                Paint::vertical(y, y + h, shade(color, 0.3), shade(color, -0.1)),
            );
        }
    }

    /// A five-point star.
    pub fn star(&mut self, cx: f32, cy: f32, r: f32, color: Rgba) {
        let s = vector::star(cx, cy, r, r * 0.46, 5);
        self.fill(&s, Paint::Solid(color));
    }

    /// A speech bubble with a tail pointing at (`tx`, `ty`).
    #[allow(clippy::too_many_arguments)]
    pub fn bubble(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tx: f32,
        ty: f32,
        fill: Rgba,
        border: Rgba,
    ) {
        self.shadow(x, y, w, h, 14.0, 8.0, 40);
        let rr = vector::rounded_rect(x, y, w, h, 14.0);
        let base_x = tx.clamp(x + 20.0, x + w - 20.0);
        let (by0, by1) = if ty < y {
            (y + 1.5, y + 1.5)
        } else {
            (y + h - 1.5, y + h - 1.5)
        };
        let tail = [[base_x - 9.0, by0], [tx, ty], [base_x + 9.0, by1]];
        self.fill_ink(&tail, fill, border, 2.0);
        self.fill(&rr, Paint::Solid(fill));
        self.outline(&rr, 2.0, border);
        // Cover the tail's ink where it meets the bubble.
        let seam = [
            [base_x - 7.5, by0],
            [base_x + 7.5, by1],
            [(base_x + tx) * 0.5, (by0 + ty) * 0.5],
        ];
        self.fill(&seam, Paint::Solid(fill));
    }

    /// A portrait `size` design units square.
    #[allow(clippy::too_many_arguments)]
    pub fn portrait(
        &mut self,
        x: f32,
        y: f32,
        size: f32,
        face: &Face,
        expr: Expr,
        outfit: &Outfit,
        alpha: f32,
    ) {
        let (sx, sy, px) = (self.st.x(x), self.st.y(y), size * self.st.k);
        portrait::draw_faded(self.mesh(), sx, sy, px, face, expr, outfit, alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_phone_fills_the_width_and_a_desktop_centres_the_canvas() {
        let phone = Stage::fit(390.0, 844.0, 400.0, 720.0);
        assert!((phone.k - 0.975).abs() < 1e-3);
        assert!(phone.ox.abs() < 1e-3 && phone.h > 860.0);
        let desk = Stage::fit(1280.0, 720.0, 400.0, 720.0);
        assert!((desk.k - 1.0).abs() < 1e-3);
        assert!((desk.ox - 440.0).abs() < 1e-3);
        assert!(desk.screen[0] < -400.0 && desk.screen[2] > 1200.0);
        let (x, y) = desk.to_design(640.0, 360.0);
        assert!((x - 200.0).abs() < 1e-3 && (y - 360.0).abs() < 1e-3);
        assert!(desk.hit(640.0, 360.0, [150.0, 300.0, 100.0, 100.0]));
    }

    #[test]
    fn a_popup_layer_covers_what_came_before() {
        let mut list = DrawList::default();
        let st = Stage::fit(400.0, 800.0, 400.0, 720.0);
        let mut p = Painter::new(&mut list, st);
        p.text("under", 10.0, 10.0, 20.0, [0, 0, 0, 255], TextAlign::Left);
        p.layer();
        p.panel(
            0.0,
            0.0,
            200.0,
            100.0,
            12.0,
            [255, 255, 255, 255],
            [250, 250, 250, 255],
            8.0,
        );
        let r = p.button(
            10.0,
            120.0,
            120.0,
            40.0,
            "GO",
            16.0,
            [255, 120, 40, 255],
            true,
        );
        assert!((r[0] - 10.0).abs() < 1e-3 && (r[2] - 120.0).abs() < 1e-3);
        assert_eq!(list.layers.len(), 1);
        assert_eq!(list.texts.len(), 1);
        assert!(list.layers[0].mesh.triangles() > 40);
        assert!(!list.layers[0].texts.is_empty());
    }
}
