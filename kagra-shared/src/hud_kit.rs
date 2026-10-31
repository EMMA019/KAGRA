//! Game-agnostic HUD pieces: the 9-slice panel, a coin icon, a counter chip and the
//! screen-size scale (plain quads, a port of `kagra.ui2d.nine_slice`), and smooth rounded
//! pieces on the GPU's rounded quads: [`round_panel`], [`gauge`], [`pill_button`].
//! No kit-specific names live here; a game passes colours from its `HudSpec`.

use crate::scene::{DrawList, Quad, TextAlign, TextFace, TextQuad};

/// HUD units -> buffer pixels. 1.0 at a 412 px wide phone buffer, so a CSS-pixel
/// size on a 2x phone is `size * 2`. Clamped so a desktop window stays sane.
pub fn ui_scale(width: u32, height: u32, spec_scale: f32) -> f32 {
    (width.min(height) as f32 / 412.0).clamp(0.8, 3.0) * spec_scale.max(0.1)
}

/// `rgba` with its alpha scaled by `k` (0..1).
pub fn fade(rgba: [u8; 4], k: f32) -> [u8; 4] {
    [
        rgba[0],
        rgba[1],
        rgba[2],
        (rgba[3] as f32 * k.clamp(0.0, 1.0)).round() as u8,
    ]
}

/// Nine quads: four corners and four edges in `border`, the centre in `fill`.
/// `slice` is the corner size; `shadow` is `(dx, dy, alpha)`, alpha 0 skips it.
/// The corners are notched by one slice so the panel reads as rounded.
#[allow(clippy::too_many_arguments)]
pub fn nine_slice(
    list: &mut DrawList,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    slice: f32,
    fill: [u8; 4],
    border: [u8; 4],
    shadow: (f32, f32, u8),
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let s = slice.max(0.0).min(w * 0.5).min(h * 0.5);
    let (dx, dy, a) = shadow;
    if a > 0 {
        let k = border[3] as f32 / 255.0;
        list.quads.push(Quad::new(
            x + dx,
            y + dy,
            w,
            h,
            [0, 0, 0, (a as f32 * k).round() as u8],
        ));
    }
    let q = |list: &mut DrawList, qx: f32, qy: f32, qw: f32, qh: f32, c: [u8; 4]| {
        if qw > 0.0 && qh > 0.0 {
            list.quads.push(Quad::new(qx, qy, qw, qh, c));
        }
    };
    // Rim: top and bottom bars lose a corner each side, side bars run full
    // height between them, leaving the corner pixels empty (a notch).
    q(list, x + s, y, w - 2.0 * s, s, border);
    q(list, x + s, y + h - s, w - 2.0 * s, s, border);
    q(list, x, y + s, s, h - 2.0 * s, border);
    q(list, x + w - s, y + s, s, h - 2.0 * s, border);
    // Corners: a smaller square so the rim turns instead of ending.
    let c = s * 0.6;
    q(list, x + s * 0.4, y + s * 0.4, c, c, border);
    q(list, x + w - s * 0.4 - c, y + s * 0.4, c, c, border);
    q(list, x + s * 0.4, y + h - s * 0.4 - c, c, c, border);
    q(list, x + w - s * 0.4 - c, y + h - s * 0.4 - c, c, c, border);
    q(list, x + s, y + s, w - 2.0 * s, h - 2.0 * s, fill);
    // A faint inner highlight on the top edge.
    q(
        list,
        x + s,
        y + s,
        w - 2.0 * s,
        (s * 0.5).max(1.0),
        [255, 255, 255, (fill[3] as f32 * 0.12) as u8],
    );
}

/// A rounded panel: `fill` with a `rim` band `rim_w` wide inside its edge (0 = no rim) and an
/// optional drop shadow `(dx, dy, alpha)`. Smooth corners (rounded quads, drawn on the GPU).
#[allow(clippy::too_many_arguments)]
pub fn round_panel(
    list: &mut DrawList,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    fill: [u8; 4],
    rim: [u8; 4],
    rim_w: f32,
    shadow: (f32, f32, u8),
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let (dx, dy, a) = shadow;
    if a > 0 {
        list.quads
            .push(Quad::round(x + dx, y + dy, w, h, radius, [0, 0, 0, a]));
    }
    list.quads.push(Quad::round(x, y, w, h, radius, fill));
    if rim_w > 0.0 && rim[3] > 0 {
        list.quads.push(Quad {
            stroke: rim_w,
            ..Quad::round(x, y, w, h, radius, rim)
        });
    }
}

/// A pill-shaped gauge: the track in `bg`, the filled share `frac` (0..=1) in `fg`, and a
/// faint highlight on the top half. Returns nothing; HP, XP, charge and timers all use it.
#[allow(clippy::too_many_arguments)]
pub fn gauge(
    list: &mut DrawList,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    frac: f32,
    fg: [u8; 4],
    bg: [u8; 4],
) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let r = h * 0.5;
    list.quads.push(Quad::round(x, y, w, h, r, bg));
    let f = frac.clamp(0.0, 1.0);
    if f > 0.0 {
        // Never thinner than the pill's own height, so a nearly empty bar still reads round.
        let fw = (w * f).max(h.min(w));
        list.quads.push(Quad::round(x, y, fw, h, r, fg));
        list.quads.push(Quad::round(
            x + r * 0.5,
            y + h * 0.12,
            (fw - r).max(0.0),
            h * 0.32,
            h * 0.16,
            [255, 255, 255, 60],
        ));
    }
}

/// A pill button with a centred label. Returns its rectangle for hit tests (`Quad::contains`).
#[allow(clippy::too_many_arguments)]
pub fn pill_button(
    list: &mut DrawList,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: &str,
    size: f32,
    fill: [u8; 4],
    text: [u8; 4],
) -> Quad {
    round_panel(
        list,
        x,
        y,
        w,
        h,
        h * 0.5,
        fill,
        [255, 255, 255, 60],
        (h * 0.06).max(1.0),
        (0.0, h * 0.08, 70),
    );
    list.texts.push(
        TextQuad::new(label, x + w * 0.5, y + (h - size) * 0.5, size, text)
            .aligned(TextAlign::Center),
    );
    Quad::new(x, y, w, h, fill)
}

/// A filled circle from horizontal strips (round without a texture).
fn disc(list: &mut DrawList, cx: f32, cy: f32, rad: f32, c: [u8; 4], n: i32) {
    let step = rad / (n as f32 + 0.5);
    for k in -n..=n {
        let dy = k as f32 * step;
        let half = (rad * rad - dy * dy).max(0.0).sqrt();
        list.quads.push(Quad::new(
            cx - half,
            cy + dy - step * 0.5,
            half * 2.0,
            step + 0.3,
            c,
        ));
    }
}

/// A paw print: one big pad and four toes. `(cx, cy)` is the centre of the print.
pub fn paw_icon(list: &mut DrawList, cx: f32, cy: f32, r: f32, color: [u8; 4]) {
    disc(list, cx, cy + r * 0.3, r * 0.58, color, 3);
    for (dx, dy) in [(-0.74, -0.1), (-0.27, -0.58), (0.27, -0.58), (0.74, -0.1)] {
        disc(list, cx + dx * r, cy + dy * r, r * 0.27, color, 2);
    }
}

/// A coin: dark rim, bright face, a slot down the middle, a glint. Drawn from
/// horizontal strips so it is round without a texture. `(cx, cy)` is the centre.
pub fn coin_icon(list: &mut DrawList, cx: f32, cy: f32, r: f32, color: [u8; 4]) {
    let disc = |list: &mut DrawList, rad: f32, c: [u8; 4]| disc(list, cx, cy, rad, c, 7);
    let dark = [
        (color[0] as f32 * 0.55) as u8,
        (color[1] as f32 * 0.45) as u8,
        (color[2] as f32 * 0.3) as u8,
        color[3],
    ];
    disc(list, r, dark);
    disc(list, r * 0.78, color);
    list.quads.push(Quad::new(
        cx - r * 0.1,
        cy - r * 0.42,
        r * 0.2,
        r * 0.84,
        dark,
    ));
    list.quads.push(Quad::new(
        cx - r * 0.5,
        cy - r * 0.5,
        r * 0.22,
        r * 0.22,
        fade([255, 255, 255, 255], color[3] as f32 / 255.0 * 0.7),
    ));
}

/// A rounded counter chip: panel, icon on the left, number to its right.
#[allow(clippy::too_many_arguments)]
pub fn counter_chip(
    list: &mut DrawList,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    ui: f32,
    fill: [u8; 4],
    border: [u8; 4],
    color: [u8; 4],
    value: &str,
) {
    nine_slice(
        list,
        x,
        y,
        w,
        h,
        3.0 * ui,
        fill,
        border,
        (2.0 * ui, 3.0 * ui, 90),
    );
    coin_icon(list, x + h * 0.5 + 2.0 * ui, y + h * 0.5, h * 0.3, color);
    list.texts.push(
        TextQuad::new(
            value,
            x + w - 10.0 * ui,
            y + h * 0.5 - 11.0 * ui,
            22.0 * ui,
            fade(color, 1.0),
        )
        .aligned(TextAlign::Right),
    );
}

/// Lines of text inside `width` pixels (a story card, a hint): Japanese breaks anywhere,
/// English at spaces. A glyph is taken as 0.55 of the size wide in English and 1.0 in
/// Japanese. At most four lines.
#[allow(clippy::too_many_arguments)]
pub fn wrap_text(
    list: &mut DrawList,
    text: &str,
    x: f32,
    y: f32,
    width: f32,
    size: f32,
    color: [u8; 4],
    ja: bool,
) {
    let per = if ja { size } else { size * 0.55 };
    let max = ((width / per) as usize).max(4);
    let mut lines: Vec<String> = Vec::new();
    if ja {
        let chars: Vec<char> = text.chars().collect();
        for chunk in chars.chunks(max) {
            lines.push(chunk.iter().collect());
        }
    } else {
        let mut line = String::new();
        for word in text.split_whitespace() {
            if !line.is_empty() && line.len() + 1 + word.len() > max {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    for (i, l) in lines.iter().take(4).enumerate() {
        list.texts
            .push(TextQuad::new(l, x, y + i as f32 * size * 1.45, size, color));
    }
}

/// The first button whose rectangle `[x, y, w, h]` holds the pixel (HUD buttons a kit lays
/// out once and both draws and hit-tests).
pub fn hit<T: Copy>(buttons: &[(T, [f32; 4])], x: f32, y: f32) -> Option<T> {
    buttons.iter().find_map(|&(b, r)| {
        (x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3]).then_some(b)
    })
}

/// The colours of a [`Card`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardStyle {
    /// Over the whole screen behind the card.
    pub dim: [u8; 4],
    pub fill: [u8; 4],
    pub edge: [u8; 4],
    /// The small line on top ("STAGE 2").
    pub kicker: [u8; 4],
    pub title: [u8; 4],
    pub body: [u8; 4],
    /// Row labels and values on a tally.
    pub label: [u8; 4],
    pub value: [u8; 4],
}

impl Default for CardStyle {
    /// Night blue with warm titles.
    fn default() -> Self {
        Self {
            dim: [0, 0, 10, 115],
            fill: [12, 16, 34, 236],
            edge: [120, 170, 255, 120],
            kicker: [140, 190, 255, 255],
            title: [255, 244, 225, 255],
            body: [215, 222, 240, 255],
            label: [190, 200, 225, 255],
            value: [255, 244, 225, 255],
        }
    }
}

/// A card in the middle of a dimmed screen: a story card before a stage, a clear card with a
/// tally, a defeat. Open it with its height, then place lines by their distance from its top.
/// Every size is in HUD units times `s`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Card {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub s: f32,
    pub style: CardStyle,
}

impl Card {
    /// Dim the `w` x `h` screen and draw a card `tall` high (at most 420 wide).
    pub fn open(list: &mut DrawList, w: f32, h: f32, s: f32, tall: f32, style: CardStyle) -> Self {
        let cw = (w - 32.0 * s).min(420.0 * s);
        let (x, y) = ((w - cw) * 0.5, h * 0.5 - tall * 0.5);
        list.quads.push(Quad::new(0.0, 0.0, w, h, style.dim));
        round_panel(
            list,
            x,
            y,
            cw,
            tall,
            16.0 * s,
            style.fill,
            style.edge,
            1.5 * s,
            (0.0, 6.0 * s, 90),
        );
        Self {
            x,
            y,
            w: cw,
            h: tall,
            s,
            style,
        }
    }

    fn mid(&self) -> f32 {
        self.x + self.w * 0.5
    }

    /// A centred line `dy` below the card's top.
    pub fn line(&self, list: &mut DrawList, text: &str, dy: f32, size: f32, color: [u8; 4]) {
        list.texts.push(
            TextQuad::new(text, self.mid(), self.y + dy * self.s, size * self.s, color)
                .aligned(TextAlign::Center),
        );
    }

    /// The small display-face line on top ("STAGE 2", "MAP 1").
    pub fn kicker(&self, list: &mut DrawList, text: &str) {
        list.texts.push(
            TextQuad::new(
                text,
                self.mid(),
                self.y + 18.0 * self.s,
                16.0 * self.s,
                self.style.kicker,
            )
            .aligned(TextAlign::Center)
            .with_face(TextFace::Display),
        );
    }

    /// The big name under the kicker.
    pub fn title(&self, list: &mut DrawList, text: &str) {
        self.line(list, text, 42.0, 27.0, self.style.title);
    }

    /// A paragraph, wrapped to the card (`ja`: break anywhere).
    pub fn body(&self, list: &mut DrawList, text: &str, dy: f32, size: f32, ja: bool) {
        wrap_text(
            list,
            text,
            self.x + 20.0 * self.s,
            self.y + dy * self.s,
            self.w - 40.0 * self.s,
            size * self.s,
            self.style.body,
            ja,
        );
    }

    /// A tally: labels on the left, values on the right, from `dy` down, `step` apart.
    pub fn rows(&self, list: &mut DrawList, rows: &[(&str, String)], dy: f32, step: f32) {
        let s = self.s;
        for (i, (k, v)) in rows.iter().enumerate() {
            let ry = self.y + (dy + i as f32 * step) * s;
            list.texts.push(TextQuad::new(
                k,
                self.x + 28.0 * s,
                ry,
                17.0 * s,
                self.style.label,
            ));
            list.texts.push(
                TextQuad::new(
                    v,
                    self.x + self.w - 28.0 * s,
                    ry,
                    19.0 * s,
                    self.style.value,
                )
                .aligned(TextAlign::Right)
                .with_face(TextFace::Display),
            );
        }
    }

    /// The breathing "tap to go on" line, `from_bottom` above the card's bottom edge.
    pub fn prompt(&self, list: &mut DrawList, text: &str, clock: f32, from_bottom: f32) {
        let a = (255.0 * crate::game_core::tween::pulse(clock, 0.48, 0.27)) as u8;
        list.texts.push(
            TextQuad::new(
                text,
                self.mid(),
                self.y + self.h - from_bottom * self.s,
                16.0 * self.s,
                [255, 255, 255, a],
            )
            .aligned(TextAlign::Center),
        );
    }

    /// ◀ ▶ at the card's sides, `dy` below its top: a stage select (taps on the outer
    /// quarters of the screen pick, see [`pick_side`]). Drawn as shapes: the bundled fonts
    /// have no triangle glyphs.
    pub fn arrows(&self, list: &mut DrawList, dy: f32) {
        let s = self.s;
        for dir in [-1.0, 1.0] {
            let cx = self.mid() + dir * (self.w * 0.5 - 22.0 * s);
            let cy = self.y + (dy + 14.0) * s;
            let r = 15.0 * s;
            round_panel(
                list,
                cx - r,
                cy - r,
                2.0 * r,
                2.0 * r,
                r,
                [60, 90, 160, 150],
                [170, 200, 255, 140],
                1.0 * s,
                (0.0, 0.0, 0),
            );
            arrow(list, cx, cy, 13.0 * s, dir, [220, 232, 255, 240]);
        }
    }
}

/// A solid triangle `size` across centred on `(cx, cy)`, pointing right (`dir` > 0) or left,
/// from thin columns (the fonts have no ◀ ▶).
pub fn arrow(list: &mut DrawList, cx: f32, cy: f32, size: f32, dir: f32, color: [u8; 4]) {
    let n = 10;
    let col = size / n as f32;
    // A little toward the tip, so the triangle's weight sits in the middle.
    let x0 = cx - size * 0.5 + dir.signum() * size * 0.1;
    for i in 0..n {
        // The tall side at the back, the tip in front.
        let k = if dir > 0.0 { i } else { n - 1 - i };
        let tall = size * (1.0 - (k as f32 + 0.5) / n as f32);
        list.quads.push(Quad::new(
            x0 + i as f32 * col,
            cy - tall * 0.5,
            col + 0.3,
            tall,
            color,
        ));
    }
}

/// One row of a [`Menu`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    /// A glyph or short word on the left ("♥", "⚔").
    pub icon: String,
    pub icon_color: [u8; 4],
    pub label: String,
    /// On the right: a count, a price, "E" for equipped.
    pub right: String,
    /// Greyed out (too dear, cannot use).
    pub dim: bool,
}

/// A titled list panel: a bag, a shop, an equipment screen. Lay it out with
/// [`Menu::layout`], draw it with [`Menu::draw`], hit-test rows with [`Menu::row_at`].
/// It shows `visible` rows from `scroll`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Menu {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub s: f32,
    pub row_h: f32,
    pub visible: usize,
}

impl Menu {
    /// A panel `w_max` wide at most, from `top` down to `bottom` pixels on a `w`-wide screen.
    pub fn layout(w: f32, top: f32, bottom: f32, w_max: f32, s: f32) -> Self {
        let mw = (w - 24.0 * s).min(w_max * s);
        let row_h = 44.0 * s;
        let h = (bottom - top).max(row_h * 2.0 + 50.0 * s);
        let visible = (((h - 50.0 * s) / row_h).floor() as usize).max(1);
        Self {
            x: (w - mw) * 0.5,
            y: top,
            w: mw,
            h,
            s,
            row_h,
            visible,
        }
    }

    fn row_y(&self, i: usize) -> f32 {
        self.y + 44.0 * self.s + i as f32 * self.row_h
    }

    /// Draw the panel, the title (and a note on its right: coins), rows from `scroll`, with
    /// `selected` (an index into `rows`) lit.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        list: &mut DrawList,
        title: &str,
        note: &str,
        rows: &[Row],
        scroll: usize,
        selected: Option<usize>,
    ) {
        let s = self.s;
        round_panel(
            list,
            self.x,
            self.y,
            self.w,
            self.h,
            14.0 * s,
            [12, 16, 34, 238],
            [140, 175, 255, 120],
            1.5 * s,
            (0.0, 5.0 * s, 90),
        );
        list.texts.push(TextQuad::new(
            title,
            self.x + 16.0 * s,
            self.y + 12.0 * s,
            19.0 * s,
            [255, 236, 200, 255],
        ));
        if !note.is_empty() {
            list.texts.push(
                TextQuad::new(
                    note,
                    self.x + self.w - 16.0 * s,
                    self.y + 13.0 * s,
                    17.0 * s,
                    [255, 220, 120, 255],
                )
                .aligned(TextAlign::Right),
            );
        }
        for (k, row) in rows.iter().enumerate().skip(scroll).take(self.visible) {
            let ry = self.row_y(k - scroll);
            if selected == Some(k) {
                let mut q = Quad::new(
                    self.x + 8.0 * s,
                    ry,
                    self.w - 16.0 * s,
                    self.row_h - 4.0 * s,
                    [70, 110, 200, 200],
                );
                q.radius = 10.0 * s;
                list.quads.push(q);
            }
            let a = if row.dim { 110 } else { 255 };
            let ic = row.icon_color;
            list.texts.push(
                TextQuad::new(
                    &row.icon,
                    self.x + 30.0 * s,
                    ry + 10.0 * s,
                    20.0 * s,
                    [ic[0], ic[1], ic[2], ic[3].min(a)],
                )
                .aligned(TextAlign::Center),
            );
            list.texts.push(TextQuad::new(
                &row.label,
                self.x + 52.0 * s,
                ry + 11.0 * s,
                17.0 * s,
                [235, 238, 250, a],
            ));
            list.texts.push(
                TextQuad::new(
                    &row.right,
                    self.x + self.w - 18.0 * s,
                    ry + 11.0 * s,
                    17.0 * s,
                    [255, 220, 140, a],
                )
                .aligned(TextAlign::Right),
            );
        }
        // More above / below.
        if scroll > 0 {
            list.texts.push(
                TextQuad::new(
                    "…",
                    self.x + self.w * 0.5,
                    self.y + 30.0 * s,
                    14.0 * s,
                    [200, 210, 255, 200],
                )
                .aligned(TextAlign::Center),
            );
        }
        if scroll + self.visible < rows.len() {
            list.texts.push(
                TextQuad::new(
                    "…",
                    self.x + self.w * 0.5,
                    self.y + self.h - 18.0 * s,
                    14.0 * s,
                    [200, 210, 255, 200],
                )
                .aligned(TextAlign::Center),
            );
        }
    }

    /// Which row (an index into the full list) a tap hit, with the list scrolled to `scroll`.
    pub fn row_at(&self, x: f32, y: f32, rows: usize, scroll: usize) -> Option<usize> {
        if x < self.x || x > self.x + self.w {
            return None;
        }
        (0..self.visible.min(rows.saturating_sub(scroll))).find_map(|i| {
            let ry = self.row_y(i);
            (y >= ry && y < ry + self.row_h).then_some(i + scroll)
        })
    }

    /// The scroll that keeps `selected` in view.
    pub fn follow(&self, selected: usize, scroll: usize) -> usize {
        if selected < scroll {
            selected
        } else if selected >= scroll + self.visible {
            selected + 1 - self.visible
        } else {
            scroll
        }
    }
}

/// A tap on a stage-select card: -1 on the left quarter of the screen, +1 on the right, 0 in
/// the middle (start).
pub fn pick_side(x: f32, w: f32) -> i32 {
    if x < w * 0.25 {
        -1
    } else if x > w * 0.75 {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_pieces_are_a_few_rounded_quads() {
        let mut l = DrawList::default();
        round_panel(
            &mut l,
            10.0,
            10.0,
            200.0,
            80.0,
            16.0,
            [20, 20, 40, 220],
            [255, 255, 255, 80],
            2.0,
            (0.0, 4.0, 90),
        );
        assert_eq!(l.quads.len(), 3, "shadow, fill, rim");
        assert!(l.quads.iter().all(|q| q.radius == 16.0));
        assert_eq!(l.quads[2].stroke, 2.0);
        let mut g = DrawList::default();
        gauge(
            &mut g,
            0.0,
            0.0,
            100.0,
            10.0,
            0.5,
            [0, 255, 0, 255],
            [0, 0, 0, 160],
        );
        assert_eq!(g.quads[1].w, 50.0);
        let mut e = DrawList::default();
        gauge(
            &mut e,
            0.0,
            0.0,
            100.0,
            10.0,
            0.01,
            [0, 255, 0, 255],
            [0, 0, 0, 160],
        );
        assert_eq!(e.quads[1].w, 10.0, "a nearly empty bar stays a round dot");
        let mut b = DrawList::default();
        let r = pill_button(
            &mut b,
            20.0,
            30.0,
            160.0,
            48.0,
            "START",
            20.0,
            [90, 60, 200, 255],
            [255; 4],
        );
        assert!(r.contains(100.0, 54.0) && !r.contains(10.0, 54.0));
        assert_eq!(b.texts[0].text, "START");
    }

    fn empty() -> DrawList {
        DrawList {
            clear: [0; 4],
            quads: Vec::new(),
            texts: Vec::new(),
            images: Vec::new(),
        }
    }

    #[test]
    fn scale_follows_the_short_side() {
        assert_eq!(ui_scale(824, 1800, 1.0), 2.0);
        assert_eq!(ui_scale(1800, 824, 1.0), 2.0);
        assert_eq!(ui_scale(100, 100, 1.0), 0.8);
        assert_eq!(ui_scale(824, 1800, 1.5), 3.0);
    }

    #[test]
    fn panel_stays_inside_its_rectangle() {
        let mut l = empty();
        nine_slice(
            &mut l,
            10.0,
            20.0,
            100.0,
            40.0,
            4.0,
            [1; 4],
            [2; 4],
            (0.0, 0.0, 0),
        );
        for q in &l.quads {
            assert!(q.x >= 10.0 - 1e-3 && q.x + q.w <= 110.0 + 1e-3, "{q:?}");
            assert!(q.y >= 20.0 - 1e-3 && q.y + q.h <= 60.0 + 1e-3, "{q:?}");
        }
        assert!(l.quads.iter().any(|q| q.color == [1; 4]), "has a fill");
        assert!(l.quads.iter().any(|q| q.color == [2; 4]), "has a rim");
    }

    #[test]
    fn a_tiny_panel_does_not_invert() {
        let mut l = empty();
        nine_slice(
            &mut l,
            0.0,
            0.0,
            6.0,
            6.0,
            20.0,
            [1; 4],
            [2; 4],
            (1.0, 1.0, 80),
        );
        assert!(l.quads.iter().all(|q| q.w >= 0.0 && q.h >= 0.0));
    }

    #[test]
    fn coin_is_round_and_centred() {
        let mut l = empty();
        coin_icon(&mut l, 50.0, 50.0, 10.0, [255, 205, 70, 255]);
        let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for q in &l.quads {
            x0 = x0.min(q.x);
            x1 = x1.max(q.x + q.w);
            y0 = y0.min(q.y);
            y1 = y1.max(q.y + q.h);
        }
        assert!(((x0 + x1) * 0.5 - 50.0).abs() < 0.2 && ((y0 + y1) * 0.5 - 50.0).abs() < 0.2);
        assert!((x1 - x0 - 20.0).abs() < 0.8 && (y1 - y0 - 20.0).abs() < 0.8);
    }

    #[test]
    fn paw_is_centred_and_fits_its_cell() {
        let mut l = empty();
        paw_icon(&mut l, 50.0, 50.0, 10.0, [255, 150, 205, 255]);
        let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for q in &l.quads {
            x0 = x0.min(q.x);
            x1 = x1.max(q.x + q.w);
            y0 = y0.min(q.y);
            y1 = y1.max(q.y + q.h);
        }
        assert!(((x0 + x1) * 0.5 - 50.0).abs() < 0.6 && ((y0 + y1) * 0.5 - 50.0).abs() < 1.2);
        // Wider than tall, and inside 2.2 radii so nine of them keep a gap.
        assert!(
            x1 - x0 < 22.0 && y1 - y0 < 19.0,
            "{} x {}",
            x1 - x0,
            y1 - y0
        );
        assert!(l.quads.len() > 12, "pad plus four toes");
    }

    #[test]
    fn a_card_dims_the_screen_and_buttons_hit_test() {
        let mut l = DrawList::default();
        let c = Card::open(&mut l, 400.0, 800.0, 1.0, 200.0, CardStyle::default());
        assert_eq!((c.x, c.y, c.w), (16.0, 300.0, 368.0));
        assert_eq!(l.quads[0].w, 400.0, "the dim covers the screen");
        c.kicker(&mut l, "STAGE 1");
        c.title(&mut l, "Docking Bay");
        c.rows(&mut l, &[("Time", "0:41".into())], 60.0, 28.0);
        c.prompt(&mut l, "Tap", 0.0, 30.0);
        c.arrows(&mut l, 40.0);
        assert_eq!(l.texts.len(), 5);
        let btns = [(1, [0.0, 0.0, 10.0, 10.0]), (2, [20.0, 0.0, 10.0, 10.0])];
        assert_eq!(hit(&btns, 25.0, 5.0), Some(2));
        assert_eq!(hit(&btns, 15.0, 5.0), None);
        assert_eq!(
            (
                pick_side(10.0, 400.0),
                pick_side(200.0, 400.0),
                pick_side(390.0, 400.0)
            ),
            (-1, 0, 1)
        );
    }

    #[test]
    fn a_menu_draws_rows_scrolls_and_hit_tests() {
        let m = Menu::layout(390.0, 100.0, 400.0, 420.0, 1.0);
        assert_eq!(m.visible, 5);
        let rows: Vec<Row> = (0..8)
            .map(|i| Row {
                icon: "♥".into(),
                label: format!("Potion {i}"),
                right: format!("x{i}"),
                ..Row::default()
            })
            .collect();
        let mut l = DrawList::default();
        m.draw(&mut l, "Bag", "120 G", &rows, 2, Some(3));
        assert!(l.texts.iter().any(|t| t.text == "Potion 2"));
        assert!(
            !l.texts.iter().any(|t| t.text == "Potion 1"),
            "scrolled past"
        );
        assert!(l.texts.iter().any(|t| t.text == "120 G"));
        let y = m.y + 44.0 + 1.5 * m.row_h;
        assert_eq!(m.row_at(m.x + 20.0, y, rows.len(), 2), Some(3));
        assert_eq!(m.row_at(2.0, y, rows.len(), 2), None);
        assert_eq!(m.follow(7, 0), 3);
        assert_eq!(m.follow(1, 3), 1);
    }
}
