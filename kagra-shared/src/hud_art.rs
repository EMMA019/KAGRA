//! Illustrated HUD pieces drawn with [`crate::vector`]: soft-shadowed gradient panels, glossy
//! buttons, ribbons, letter chips, gauges, stars, speech bubbles and portraits, placed on a
//! [`Stage`] (a fixed design canvas fitted to any screen). PNG art registered with
//! [`crate::image2d`] draws in the same order ([`Painter::image`], [`Painter::nine_slice`]).
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
use crate::image2d::PicId;
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

/// How a new screen is revealed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wipe {
    /// A colour that fades away.
    Fade,
    /// Two curtains that part to the sides.
    Curtain,
    /// A circle that opens from the middle.
    Iris,
}

/// A screen change that opens on the new screen: the game switches what it draws at once
/// and starts one of these on top, covered, with an optional title band in the middle.
/// It reads as a scene change ("Game day!") instead of a cut.
#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    pub kind: Wipe,
    pub color: Rgba,
    pub title: String,
    t: f32,
    secs: f32,
}

impl Transition {
    /// Start covered; the screen opens over `secs` (the title holds for the first third).
    pub fn open(kind: Wipe, secs: f32, color: Rgba, title: impl Into<String>) -> Self {
        Self {
            kind,
            color,
            title: title.into(),
            t: 0.0,
            secs: secs.max(0.05),
        }
    }

    /// Let time pass. Returns false once the screen is open.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.t += dt.max(0.0);
        !self.done()
    }

    pub fn done(&self) -> bool {
        self.t >= self.secs
    }

    /// How open the screen is, 0 (covered) to 1, eased; it waits a third of the time first.
    pub fn openness(&self) -> f32 {
        let k = ((self.t / self.secs - 0.34) / 0.66).clamp(0.0, 1.0);
        1.0 - (1.0 - k).powi(3)
    }

    /// Draw over everything on a new layer.
    pub fn draw(&self, p: &mut Painter) {
        if self.done() {
            return;
        }
        p.layer();
        let [x0, y0, w, h] = p.st.screen;
        let o = self.openness();
        let c = self.color;
        match self.kind {
            Wipe::Fade => {
                let a = ((1.0 - o) * c[3] as f32) as u8;
                p.fill(
                    &[[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]],
                    Paint::Solid([c[0], c[1], c[2], a]),
                );
            }
            Wipe::Curtain => {
                let half = w * 0.5 * (1.0 - o);
                for (l, r) in [(x0, x0 + half), (x0 + w - half, x0 + w)] {
                    if r - l > 0.5 {
                        p.fill(
                            &[[l, y0], [r, y0], [r, y0 + h], [l, y0 + h]],
                            Paint::vertical(y0, y0 + h, shade(c, 0.12), shade(c, -0.18)),
                        );
                    }
                }
                // The curtains' inner edges.
                for xe in [x0 + half, x0 + w - half] {
                    if half > 1.0 {
                        p.line(&[[xe, y0], [xe, y0 + h]], 3.0, shade(c, 0.35));
                    }
                }
            }
            Wipe::Iris => {
                // A ring of quads around a hole whose radius grows to the screen's corner.
                let (cx, cy) = (x0 + w * 0.5, y0 + h * 0.5);
                let reach = (w * w + h * h).sqrt() * 0.5 + 4.0;
                let r = reach * o;
                let n = 48;
                let outer = reach * 1.5;
                for i in 0..n {
                    let a0 = i as f32 / n as f32 * std::f32::consts::TAU;
                    let a1 = (i + 1) as f32 / n as f32 * std::f32::consts::TAU;
                    let q = [
                        [cx + a0.cos() * r, cy + a0.sin() * r],
                        [cx + a0.cos() * outer, cy + a0.sin() * outer],
                        [cx + a1.cos() * outer, cy + a1.sin() * outer],
                        [cx + a1.cos() * r, cy + a1.sin() * r],
                    ];
                    p.fill(&q, Paint::Solid(c));
                }
            }
        }
        // The title band, while the screen is still mostly covered.
        if !self.title.is_empty() && o < 0.35 {
            let a = (1.0 - o / 0.35).clamp(0.0, 1.0);
            let band = 84.0;
            let cy = p.st.h * 0.45;
            let (top, bot) = (cy - band * 0.5, cy + band * 0.5);
            p.fill(
                &[[x0, top], [x0 + w, top], [x0 + w, bot], [x0, bot]],
                Paint::Solid([255, 255, 255, (236.0 * a) as u8]),
            );
            for yy in [top + 5.0, bot - 5.0] {
                p.line(
                    &[[x0, yy], [x0 + w, yy]],
                    3.0,
                    [232, 184, 60, (255.0 * a) as u8],
                );
            }
            // As big as fits the screen's width, centred on the screen.
            let mut size = 56.0;
            let room = (w - 32.0).max(40.0);
            while size > 18.0 && crate::font::measure_text(&self.title, size) > room {
                size -= 2.0;
            }
            p.text_outlined(
                &self.title,
                x0 + w * 0.5,
                cy - size * 0.54,
                size,
                [c[0], c[1], c[2], (255.0 * a) as u8],
                [255, 255, 255, (255.0 * a) as u8],
                2.5,
                TextAlign::Center,
            );
        }
    }
}

/// Words that pop up on the stage and rise as they fade ("UP!", "+1", a new grade), in
/// design units. The game pushes them where something happened, ticks them, and draws them
/// last so they sit on top.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pops {
    items: Vec<Pop>,
}

#[derive(Clone, Debug, PartialEq)]
struct Pop {
    at: P,
    text: String,
    color: Rgba,
    size: f32,
    delay: f32,
    age: f32,
}

/// Seconds a pop lives.
pub const POP_SECS: f32 = 1.2;

impl Pops {
    /// A word at `at` (its centre), after `delay` seconds.
    pub fn push(&mut self, at: P, text: impl Into<String>, color: Rgba, size: f32, delay: f32) {
        self.items.push(Pop {
            at,
            text: text.into(),
            color,
            size,
            delay: delay.max(0.0),
            age: 0.0,
        });
    }

    pub fn tick(&mut self, dt: f32) {
        for p in &mut self.items {
            if p.delay > 0.0 {
                p.delay -= dt;
            } else {
                p.age += dt;
            }
        }
        self.items.retain(|p| p.age < POP_SECS);
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Draw on the painter's top layer: a quick overshoot, a rise, a fade at the end.
    pub fn draw(&self, p: &mut Painter) {
        for it in self.items.iter().filter(|it| it.delay <= 0.0) {
            let k = (it.age / POP_SECS).clamp(0.0, 1.0);
            let grow = (k / 0.18).min(1.0);
            let scale =
                0.6 + 0.4 * grow + (1.0 - grow) * 0.0 + (grow * std::f32::consts::PI).sin() * 0.25;
            let fade = if k > 0.65 {
                1.0 - (k - 0.65) / 0.35
            } else {
                1.0
            };
            let a = |c: Rgba| [c[0], c[1], c[2], (c[3] as f32 * fade) as u8];
            let size = it.size * scale;
            let y = it.at[1] - 26.0 * (1.0 - (1.0 - k).powi(2)) - size * 0.55;
            p.text_outlined(
                &it.text,
                it.at[0],
                y,
                size,
                a(it.color),
                a([24, 20, 40, 255]),
                (size * 0.09).max(1.0),
                TextAlign::Center,
            );
        }
    }
}

/// A list that follows a finger, in design units: drag it, fling it, and it settles back
/// inside its ends. The game feeds pointer positions (`press`, `drag`, `release`) and time
/// (`step`), draws its rows at `-offset`, and treats a release that did not move as a tap.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scroll {
    /// How far the content is scrolled (0: the top row at the top).
    pub offset: f32,
    /// Height of everything in the list and of the window onto it.
    pub content: f32,
    pub view: f32,
    vel: f32,
    /// Where a drag started (y), the offset then, the last y and the distance moved.
    drag: Option<[f32; 4]>,
}

/// A drag shorter than this (design units) is a tap.
pub const TAP_SLOP: f32 = 9.0;

impl Scroll {
    /// The furthest the list scrolls.
    pub fn max(&self) -> f32 {
        (self.content - self.view).max(0.0)
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// A finger comes down at `y`; a fling in progress stops.
    pub fn press(&mut self, y: f32) {
        self.vel = 0.0;
        self.drag = Some([y, self.offset, y, 0.0]);
    }

    /// The finger moves to `y`. Past either end the list moves at a third of the finger.
    pub fn drag(&mut self, y: f32) {
        let Some([y0, off0, last, moved]) = self.drag else {
            return;
        };
        let want = off0 + (y0 - y);
        let over = if want < 0.0 {
            want
        } else if want > self.max() {
            want - self.max()
        } else {
            0.0
        };
        self.offset = want - over * (2.0 / 3.0);
        let step = last - y;
        // Units per second at 60 Hz pointer rate, smoothed.
        self.vel = self.vel * 0.4 + step * 60.0 * 0.6;
        self.drag = Some([y0, off0, y, moved.max((y - y0).abs())]);
    }

    /// The finger lifts. True if it was a tap (it hardly moved).
    pub fn release(&mut self) -> bool {
        match self.drag.take() {
            Some([.., moved]) if moved < TAP_SLOP => {
                self.vel = 0.0;
                true
            }
            _ => false,
        }
    }

    /// Scroll by `d` (buttons, a wheel): a short glide there.
    pub fn nudge(&mut self, d: f32) {
        self.vel = d * 4.0;
    }

    /// Let time pass: a fling slows down, and a list past an end springs back.
    pub fn step(&mut self, dt: f32) {
        if self.drag.is_some() {
            return;
        }
        let dt = dt.clamp(0.0, 0.1);
        self.offset += self.vel * dt;
        self.vel *= (-4.5 * dt).exp();
        let max = self.max();
        let target = self.offset.clamp(0.0, max);
        if (self.offset - target).abs() > 0.01 {
            self.vel *= (-12.0 * dt).exp();
            self.offset += (target - self.offset) * (1.0 - (-14.0 * dt).exp());
        } else {
            self.offset = target;
        }
        if self.vel.abs() < 1.0 {
            self.vel = 0.0;
        }
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

    /// Start a layer that only draws inside the design rectangle `r` (a scrolling list, a
    /// window). [`Painter::layer`] ends it.
    pub fn clip_layer(&mut self, r: [f32; 4]) {
        let s = self.st.rect(r[0], r[1], r[2], r[3]);
        self.list.layer().clip = Some(s);
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

    /// A registered picture stretched over the design rectangle.
    pub fn image(&mut self, pic: PicId, x: f32, y: f32, w: f32, h: f32) {
        self.image_part(pic, [x, y, w, h], [0.0, 0.0, 1.0, 1.0], [255; 4]);
    }

    /// Part `uv` (u0, v0, u1, v1) of a picture over the design rectangle `rect`, tinted:
    /// white keeps it as drawn, a lower alpha fades it.
    pub fn image_part(&mut self, pic: PicId, rect: [f32; 4], uv: [f32; 4], tint: Rgba) {
        let r = self.st.rect(rect[0], rect[1], rect[2], rect[3]);
        self.mesh().sprite(pic, r, uv, tint);
    }

    /// Frame `frame` of a sprite sheet of `cols` x `rows` equal cells (left to right, top to
    /// bottom; the number wraps) over `rect`. Leave a clear pixel between cells so filtering
    /// does not pick up the neighbour.
    #[allow(clippy::too_many_arguments)]
    pub fn image_frame(
        &mut self,
        pic: PicId,
        cols: u32,
        rows: u32,
        frame: u32,
        rect: [f32; 4],
        tint: Rgba,
    ) {
        let (c, r) = (cols.max(1), rows.max(1));
        let f = frame % (c * r);
        let (fx, fy) = ((f % c) as f32, (f / c) as f32);
        let (cw, ch) = (1.0 / c as f32, 1.0 / r as f32);
        // Half a texel in from the cell's edges, so filtering never reads the neighbour.
        let (hx, hy) = crate::image2d::size(pic)
            .map(|(w, h)| (0.5 / w.max(1) as f32, 0.5 / h.max(1) as f32))
            .unwrap_or((0.0, 0.0));
        let uv = [
            fx * cw + hx,
            fy * ch + hy,
            (fx + 1.0) * cw - hx,
            (fy + 1.0) * ch - hy,
        ];
        self.image_part(pic, rect, uv, tint);
    }

    /// Fill the rectangle with the picture at its own aspect, cropping what overflows
    /// (anchored at `focus`, 0..1 on each axis: 0.5 keeps the centre).
    pub fn image_cover(&mut self, pic: PicId, rect: [f32; 4], focus: [f32; 2], tint: Rgba) {
        let Some((pw, ph)) = crate::image2d::size(pic) else {
            return;
        };
        let (ra, pa) = (rect[2] / rect[3].max(1e-3), pw as f32 / ph as f32);
        let uv = if pa > ra {
            let f = ra / pa;
            let u0 = (1.0 - f) * focus[0].clamp(0.0, 1.0);
            [u0, 0.0, u0 + f, 1.0]
        } else {
            let f = pa / ra;
            let v0 = (1.0 - f) * focus[1].clamp(0.0, 1.0);
            [0.0, v0, 1.0, v0 + f]
        };
        self.image_part(pic, rect, uv, tint);
    }

    /// A framed picture (a paper card, a banner) grown to any size: the corners keep their
    /// size, `border` design units on screen and `edge` of the picture (0..0.5) per side; the
    /// edges and the middle stretch, or repeat in `tile` design-unit steps when `tile` > 0.
    #[allow(clippy::too_many_arguments)]
    pub fn nine_slice(
        &mut self,
        pic: PicId,
        rect: [f32; 4],
        border: f32,
        edge: f32,
        tile: f32,
        tint: Rgba,
    ) {
        let [x, y, w, h] = rect;
        let m = border.min(w * 0.5).min(h * 0.5).max(0.0);
        let e = edge.clamp(0.0, 0.5);
        // Columns and rows: (design start, design size, uv start, uv end).
        let spans = |start: f32, size: f32| {
            [
                (start, m, 0.0, e),
                (start + m, size - 2.0 * m, e, 1.0 - e),
                (start + size - m, m, 1.0 - e, 1.0),
            ]
        };
        for (ri, (py, ph, v0, v1)) in spans(y, h).into_iter().enumerate() {
            for (ci, (px, pw, u0, u1)) in spans(x, w).into_iter().enumerate() {
                if pw <= 0.0 || ph <= 0.0 {
                    continue;
                }
                // Repeat the middle parts along their stretched axes.
                let (step_x, step_y) = (
                    if ci == 1 && tile > 0.0 { tile } else { pw },
                    if ri == 1 && tile > 0.0 { tile } else { ph },
                );
                let mut ty = py;
                while ty < py + ph - 1e-3 {
                    let th = step_y.min(py + ph - ty);
                    let tv1 = v0 + (v1 - v0) * th / step_y;
                    let mut tx = px;
                    while tx < px + pw - 1e-3 {
                        let tw = step_x.min(px + pw - tx);
                        let tu1 = u0 + (u1 - u0) * tw / step_x;
                        self.image_part(pic, [tx, ty, tw, th], [u0, v0, tu1, tv1], tint);
                        tx += tw;
                    }
                    ty += th;
                }
            }
        }
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

    /// A radar chart: one axis per label (the first straight up, then clockwise), rings at
    /// quarters, and each series as a translucent shape with its outline. Values are 0..1.
    pub fn radar(&mut self, cx: f32, cy: f32, r: f32, labels: &[&str], series: &[(&[f32], Rgba)]) {
        let n = labels.len().max(3);
        let at = |i: usize, v: f32| -> P {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
            [cx + a.cos() * r * v, cy + a.sin() * r * v]
        };
        let ring = |v: f32| -> Vec<P> { (0..n).map(|i| at(i, v)).collect() };
        self.fill(&ring(1.0), Paint::Solid([255, 255, 255, 40]));
        for q in [0.25, 0.5, 0.75, 1.0] {
            self.outline(
                &ring(q),
                if q == 1.0 { 1.4 } else { 0.8 },
                [255, 255, 255, 90],
            );
        }
        for i in 0..n {
            self.line(&[[cx, cy], at(i, 1.0)], 0.8, [255, 255, 255, 80]);
        }
        for (vals, color) in series {
            let shape: Vec<P> = (0..n)
                .map(|i| at(i, vals.get(i).copied().unwrap_or(0.0).clamp(0.02, 1.0)))
                .collect();
            self.fill(&shape, Paint::Solid([color[0], color[1], color[2], 90]));
            self.outline(&shape, 2.0, *color);
            for q in &shape {
                self.dot(q[0], q[1], 2.6, *color);
            }
        }
        for (i, label) in labels.iter().enumerate() {
            let q = at(i, 1.22);
            self.text_outlined(
                label,
                q[0],
                q[1] - 7.0,
                12.0,
                [255, 255, 255, 255],
                [16, 20, 40, 255],
                1.4,
                TextAlign::Center,
            );
        }
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

    #[test]
    fn a_scroll_follows_the_finger_flings_and_settles_inside() {
        let mut s = Scroll {
            content: 1000.0,
            view: 400.0,
            ..Default::default()
        };
        s.press(300.0);
        for k in 1..=10 {
            s.drag(300.0 - k as f32 * 10.0);
        }
        assert!((s.offset - 100.0).abs() < 1e-3);
        assert!(!s.release(), "a drag is not a tap");
        let before = s.offset;
        for _ in 0..30 {
            s.step(1.0 / 60.0);
        }
        assert!(
            s.offset > before + 50.0,
            "the fling carries on: {}",
            s.offset
        );
        for _ in 0..600 {
            s.step(1.0 / 60.0);
        }
        assert!(s.offset <= s.max() + 0.01 && s.offset >= 0.0);
        // Pulled past the top it gives at a third, then springs back.
        s.offset = 0.0;
        s.press(100.0);
        s.drag(190.0);
        assert!((s.offset + 30.0).abs() < 1e-3, "{}", s.offset);
        s.release();
        for _ in 0..120 {
            s.step(1.0 / 60.0);
        }
        assert!(s.offset.abs() < 0.05, "{}", s.offset);
        // A press that does not move is a tap.
        s.press(50.0);
        s.drag(53.0);
        assert!(s.release());
    }

    #[test]
    fn a_radar_draws_rings_shapes_and_labels() {
        let mut list = DrawList::default();
        let st = Stage::fit(400.0, 700.0, 400.0, 700.0);
        let mut p = Painter::new(&mut list, st);
        p.radar(
            200.0,
            300.0,
            60.0,
            &["A", "B", "C", "D", "E"],
            &[
                (&[0.5, 0.6, 0.7, 0.8, 0.9], [40, 80, 200, 255]),
                (&[1.0, 0.2, 0.3, 0.4, 0.5], [200, 60, 60, 255]),
            ],
        );
        let labels: Vec<_> = list.texts.iter().map(|t| t.text.clone()).collect();
        for l in ["A", "B", "C", "D", "E"] {
            assert!(labels.iter().any(|x| x == l));
        }
        let b = list.mesh.bounds().unwrap();
        // The first axis points up: the chart reaches its full radius above the centre.
        assert!(b[1] <= 300.0 - 60.0 + 1.0, "{b:?}");
    }

    #[test]
    fn a_transition_opens_and_ends() {
        let st = Stage::fit(400.0, 700.0, 400.0, 700.0);
        for kind in [Wipe::Fade, Wipe::Curtain, Wipe::Iris] {
            let mut t = Transition::open(kind, 1.0, [30, 60, 160, 255], "GAME DAY");
            // A long title shrinks to fit a phone's width.
            let long = Transition::open(kind, 1.0, [30, 60, 160, 255], "BACK TO PRACTICE AT LAST");
            let mut l2 = DrawList::default();
            long.draw(&mut Painter::new(&mut l2, st));
            let title = l2.layers[0]
                .texts
                .iter()
                .find(|x| x.text.starts_with("BACK"))
                .unwrap();
            assert!(crate::font::measure_text(&title.text, title.size) <= 400.0 * st.k);
            assert_eq!(t.openness(), 0.0, "covered at first");
            let mut list = DrawList::default();
            t.draw(&mut Painter::new(&mut list, st));
            assert!(list.layers[0].mesh.triangles() > 0);
            assert!(list.layers[0].texts.iter().any(|x| x.text == "GAME DAY"));
            t.tick(0.6);
            assert!(t.openness() > 0.0 && t.openness() < 1.0);
            assert!(!t.tick(0.5), "open after its time");
            let mut list = DrawList::default();
            t.draw(&mut Painter::new(&mut list, st));
            assert!(list.layers.is_empty(), "nothing once open");
        }
    }

    #[test]
    fn pops_wait_rise_and_go() {
        let mut pops = Pops::default();
        pops.push([100.0, 200.0], "UP!", [255, 200, 60, 255], 20.0, 0.5);
        let mut list = DrawList::default();
        let st = Stage::fit(400.0, 700.0, 400.0, 700.0);
        Painter::new(&mut list, st).text("", 0.0, 0.0, 1.0, [0; 4], TextAlign::Left);
        pops.draw(&mut Painter::new(&mut list, st));
        let before = list.texts.len();
        assert_eq!(before, 1, "still waiting");
        pops.tick(0.6);
        pops.draw(&mut Painter::new(&mut list, st));
        assert!(list.texts.len() > before);
        for _ in 0..100 {
            pops.tick(0.05);
        }
        assert!(pops.is_empty());
    }

    #[test]
    fn a_clip_layer_is_in_screen_pixels() {
        let mut list = DrawList::default();
        let st = Stage::fit(800.0, 1400.0, 400.0, 700.0);
        let mut p = Painter::new(&mut list, st);
        p.clip_layer([10.0, 20.0, 100.0, 50.0]);
        assert_eq!(list.layers[0].clip, Some([20.0, 40.0, 200.0, 100.0]));
    }

    #[test]
    fn pictures_place_crop_and_nine_slice_in_design_units() {
        let pic = crate::image2d::register_rgba(8, 4, vec![255; 8 * 4 * 4]).unwrap();
        let mut list = DrawList::default();
        let st = Stage::fit(800.0, 1400.0, 400.0, 700.0);
        let mut p = Painter::new(&mut list, st);
        p.image(pic, 10.0, 20.0, 40.0, 20.0);
        assert_eq!(list.mesh.sprites[0].rect, [20.0, 40.0, 80.0, 40.0]);
        let mut p = Painter::new(&mut list, st);
        // A square box shows the middle half of a 2:1 picture.
        p.image_cover(pic, [0.0, 0.0, 10.0, 10.0], [0.5, 0.5], [255; 4]);
        assert_eq!(list.mesh.sprites[1].uv, [0.25, 0.0, 0.75, 1.0]);
        // Frame 5 of a 4 x 2 sheet is the second cell of the second row; 9 wraps to 1.
        let mut p = Painter::new(&mut list, st);
        p.image_frame(pic, 4, 2, 5, [0.0, 0.0, 10.0, 10.0], [255; 4]);
        p.image_frame(pic, 4, 2, 9, [0.0, 0.0, 10.0, 10.0], [255; 4]);
        let n = list.mesh.sprites.len();
        // The 8 x 4 picture: half a texel is 1/16 across and 1/8 down.
        let (hx, hy) = (0.5 / 8.0, 0.5 / 4.0);
        assert_eq!(
            list.mesh.sprites[n - 2].uv,
            [0.25 + hx, 0.5 + hy, 0.5 - hx, 1.0 - hy]
        );
        assert_eq!(
            list.mesh.sprites[n - 1].uv,
            [0.25 + hx, 0.0 + hy, 0.5 - hx, 0.5 - hy]
        );
        list.mesh.sprites.truncate(2);
        let mut p = Painter::new(&mut list, st);
        p.nine_slice(pic, [0.0, 0.0, 100.0, 60.0], 10.0, 0.25, 0.0, [255; 4]);
        assert_eq!(list.mesh.sprites.len(), 2 + 9);
        let mut p = Painter::new(&mut list, st);
        // Tiled: the 80 x 40 middle in 20-unit steps is 4 x 2 tiles; edges tile along.
        p.nine_slice(pic, [0.0, 0.0, 100.0, 60.0], 10.0, 0.25, 20.0, [255; 4]);
        assert_eq!(list.mesh.sprites.len(), 11 + 4 + 8 + 4 + 4 + 2 + 2);
        let last = list.mesh.sprites.last().unwrap();
        assert_eq!(last.rect, [180.0, 100.0, 20.0, 20.0]);
    }
}
