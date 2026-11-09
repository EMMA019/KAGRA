//! 画面内容の記述。GPU に依存しないので `render` feature 無しでもテストできる。
//!
//! シーンは「入力とフレーム数から矩形の列を作る純関数」に近い形にしてある。
//! これにより Android / iOS / Web / オフスクリーンで同じ絵が出ることを、
//! GPU 無しの CI でも検証できる。

/// 画面座標系（左上原点、y は下向き、単位はピクセル）の矩形。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// 直線 RGBA（0..255）。
    pub color: [u8; 4],
    /// Corner radius in pixels. 0 is a sharp rectangle; half the short side or more is a
    /// circle or a pill. The GPU draws the rounded edge smooth (one-pixel soft edge).
    pub radius: f32,
    /// Outline width in pixels: 0 fills the shape, more draws only a band that wide inside
    /// its edge (a ring when the quad is a circle). Only for rounded quads (`radius` > 0).
    pub stroke: f32,
}

impl Quad {
    pub fn new(x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) -> Self {
        Self {
            x,
            y,
            w,
            h,
            color,
            radius: 0.0,
            stroke: 0.0,
        }
    }

    /// A rectangle with rounded corners.
    pub fn round(x: f32, y: f32, w: f32, h: f32, radius: f32, color: [u8; 4]) -> Self {
        Self {
            radius: radius.max(0.0),
            ..Self::new(x, y, w, h, color)
        }
    }

    /// A filled circle around (`cx`, `cy`).
    pub fn circle(cx: f32, cy: f32, r: f32, color: [u8; 4]) -> Self {
        Self::round(cx - r, cy - r, 2.0 * r, 2.0 * r, r, color)
    }

    /// A ring around (`cx`, `cy`): outer radius `r`, `width` pixels thick.
    pub fn ring(cx: f32, cy: f32, r: f32, width: f32, color: [u8; 4]) -> Self {
        Self {
            stroke: width.max(0.0),
            ..Self::circle(cx, cy, r, color)
        }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

/// テキスト行の水平揃え。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// 画面座標系（左上原点）のテキスト。`size` は em のピクセル高。
///
/// GPU には依存しない: グリフは `font.rs` がカバレッジ画素 → `Quad` に
/// 展開するので、CI も wasm も同じ絵を出す。
#[derive(Clone, Debug, PartialEq)]
pub struct TextQuad {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    /// RGBA（0..255）。カバレッジでアルファを掛ける。
    pub color: [u8; 4],
    pub align: TextAlign,
    /// Which face draws it. `Ui` everywhere unless a game asks for `Display` (numbers, titles).
    pub face: TextFace,
}

/// Text face. `Ui`: Public Sans for Latin, Zen Maru Gothic for Japanese. `Display`: Cinzel for
/// Latin and digits (timers, damage numbers, titles), falling back to `Ui` for the rest.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextFace {
    #[default]
    Ui,
    Display,
}

impl TextQuad {
    pub fn new(text: &str, x: f32, y: f32, size: f32, color: [u8; 4]) -> Self {
        Self {
            text: text.to_string(),
            x,
            y,
            size,
            color,
            align: TextAlign::Left,
            face: TextFace::Ui,
        }
    }

    pub fn aligned(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn with_face(mut self, face: TextFace) -> Self {
        self.face = face;
        self
    }
}

/// Screen-space image blit (left-top origin). Decoded after GPU readback.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageQuad {
    pub path: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl ImageQuad {
    pub fn new(path: &str, x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            path: path.to_string(),
            x,
            y,
            w,
            h,
        }
    }
}

/// One corner of a 2D triangle: screen pixels (left-top origin) and an RGBA colour (0..255,
/// the same space as [`Quad::color`]). Colours interpolate across the triangle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vert2 {
    pub x: f32,
    pub y: f32,
    pub color: [u8; 4],
}

impl Vert2 {
    pub fn new(x: f32, y: f32, color: [u8; 4]) -> Self {
        Self { x, y, color }
    }
}

/// A picture ([`crate::image2d`]) stretched over a screen rectangle, drawn inside a
/// [`Mesh2d`] at the point it was added: after the triangles before it, under the ones after.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprite2d {
    pub pic: crate::image2d::PicId,
    /// Drawn before the mesh vertex with this index (`verts.len()` when it was added).
    pub at: usize,
    /// Left, top, width, height in screen pixels.
    pub rect: [f32; 4],
    /// The part of the picture shown: u0, v0, u1, v1 (0..1, top-left origin).
    pub uv: [f32; 4],
    /// Multiplies the picture (same space as [`Quad::color`]); white keeps it as drawn.
    pub tint: [u8; 4],
}

/// One step of a mesh in draw order: a run of triangle vertices, or a picture.
#[derive(Clone, Copy, Debug)]
pub enum MeshRun<'a> {
    Tris(&'a [Vert2]),
    Sprite(&'a Sprite2d),
}

/// Coloured triangles in screen pixels: three vertices each, drawn in order, alpha blended.
/// Per-vertex colours make gradients; a vertex with alpha 0 fades an edge, which is how
/// [`crate::vector`] anti-aliases fills and strokes and draws soft shadows and glows.
/// `sprites` are pictures drawn in between, at their `at` vertex.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh2d {
    pub verts: Vec<Vert2>,
    pub sprites: Vec<Sprite2d>,
}

impl Mesh2d {
    pub fn tri(&mut self, a: Vert2, b: Vert2, c: Vert2) {
        self.verts.extend([a, b, c]);
    }

    /// Draw `uv` of picture `pic` over `rect` (x, y, w, h), on top of everything added so far.
    pub fn sprite(
        &mut self,
        pic: crate::image2d::PicId,
        rect: [f32; 4],
        uv: [f32; 4],
        tint: [u8; 4],
    ) {
        if rect[2] <= 0.0 || rect[3] <= 0.0 || tint[3] == 0 {
            return;
        }
        // Keep `at` on a triangle boundary so a run never splits a triangle.
        let at = self.verts.len() - self.verts.len() % 3;
        self.sprites.push(Sprite2d {
            pic,
            at,
            rect,
            uv,
            tint,
        });
    }

    /// Triangles, counting each picture as the two it draws.
    pub fn triangles(&self) -> usize {
        self.verts.len() / 3 + self.sprites.len() * 2
    }

    pub fn is_empty(&self) -> bool {
        self.verts.is_empty() && self.sprites.is_empty()
    }

    /// Triangle runs and pictures in draw order.
    pub fn runs(&self) -> Vec<MeshRun<'_>> {
        let mut out = Vec::with_capacity(self.sprites.len() * 2 + 1);
        let mut from = 0;
        for s in &self.sprites {
            let at = s.at.clamp(from, self.verts.len());
            if at > from {
                out.push(MeshRun::Tris(&self.verts[from..at]));
            }
            out.push(MeshRun::Sprite(s));
            from = at;
        }
        if from < self.verts.len() {
            out.push(MeshRun::Tris(&self.verts[from..]));
        }
        out
    }

    /// Append `other` scaled by `scale` and moved by (`dx`, `dy`), its alpha times `alpha`
    /// (0..1). Art built once in unit space (a portrait) is placed this way every frame.
    pub fn append_placed(&mut self, other: &Mesh2d, dx: f32, dy: f32, scale: f32, alpha: f32) {
        let a = alpha.clamp(0.0, 1.0);
        let base = self.verts.len();
        let fade = |c: [u8; 4]| [c[0], c[1], c[2], (c[3] as f32 * a).round() as u8];
        self.verts.extend(other.verts.iter().map(|v| Vert2 {
            x: dx + v.x * scale,
            y: dy + v.y * scale,
            color: fade(v.color),
        }));
        self.sprites.extend(other.sprites.iter().map(|s| Sprite2d {
            at: base + s.at,
            rect: [
                dx + s.rect[0] * scale,
                dy + s.rect[1] * scale,
                s.rect[2] * scale,
                s.rect[3] * scale,
            ],
            tint: fade(s.tint),
            ..*s
        }));
    }

    /// Bounding box (min x, min y, max x, max y), or None when empty.
    pub fn bounds(&self) -> Option<[f32; 4]> {
        let corners = self
            .verts
            .iter()
            .map(|v| [v.x, v.y])
            .chain(self.sprites.iter().flat_map(|s| {
                [
                    [s.rect[0], s.rect[1]],
                    [s.rect[0] + s.rect[2], s.rect[1] + s.rect[3]],
                ]
            }));
        corners.fold(None, |b, [x, y]| {
            Some(match b {
                None => [x, y, x, y],
                Some(b) => [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)],
            })
        })
    }
}

/// The overlap of two clip rectangles (x, y, w, h); None means unclipped. Rectangles that
/// do not meet give an empty one (zero size).
pub fn clip_within(a: Option<[f32; 4]>, b: Option<[f32; 4]>) -> Option<[f32; 4]> {
    match (a, b) {
        (None, r) | (r, None) => r,
        (Some(a), Some(b)) => {
            let x0 = a[0].max(b[0]);
            let y0 = a[1].max(b[1]);
            let x1 = (a[0] + a[2]).min(b[0] + b[2]);
            let y1 = (a[1] + a[3]).min(b[1] + b[3]);
            Some([x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)])
        }
    }
}

/// 1 フレームぶんの描画内容。
///
/// Draw order: `mesh`, then `quads`, then `texts`, then each of `layers` the same way
/// (depth first). A layer is a whole `DrawList` on top of everything before it, so a popup
/// drawn into a layer covers the text under it, and every `hud_kit` helper draws into a
/// layer as it does into the base list. A layer's `clear` is ignored.
///
/// `clip` keeps a list (and its layers) inside a screen rectangle: a scrolling list, a
/// window onto a bigger picture.
#[derive(Clone, Debug, Default)]
pub struct DrawList {
    pub clear: [u8; 4],
    /// Draw only inside this screen rectangle (x, y, w, h), within any parent's.
    pub clip: Option<[f32; 4]>,
    pub mesh: Mesh2d,
    pub quads: Vec<Quad>,
    pub texts: Vec<TextQuad>,
    pub images: Vec<ImageQuad>,
    pub layers: Vec<DrawList>,
}

impl DrawList {
    /// A new layer on top of everything drawn so far (and of earlier layers).
    pub fn layer(&mut self) -> &mut DrawList {
        self.layers.push(DrawList::default());
        self.layers.last_mut().expect("just pushed")
    }

    /// The topmost layer: the last layer's topmost layer, or this list when it has none.
    pub fn top(&mut self) -> &mut DrawList {
        if self.layers.is_empty() {
            return self;
        }
        self.layers.last_mut().expect("not empty").top()
    }

    /// Every list in draw order: this one, then its layers depth first.
    pub fn flatten(&self) -> Vec<&DrawList> {
        let mut out = vec![self];
        for l in &self.layers {
            out.extend(l.flatten());
        }
        out
    }

    /// [`DrawList::flatten`] with the rectangle each list is drawn in: its own `clip` within
    /// its parents' (None: the whole screen).
    pub fn flatten_clipped(&self) -> Vec<(&DrawList, Option<[f32; 4]>)> {
        fn walk<'a>(
            l: &'a DrawList,
            outer: Option<[f32; 4]>,
            out: &mut Vec<(&'a DrawList, Option<[f32; 4]>)>,
        ) {
            let clip = clip_within(outer, l.clip);
            out.push((l, clip));
            for c in &l.layers {
                walk(c, clip, out);
            }
        }
        let mut out = Vec::new();
        walk(self, None, &mut out);
        out
    }

    /// Quads, triangles and texts in this list and all its layers (for budgets and tests).
    pub fn counts(&self) -> (usize, usize, usize) {
        self.flatten().iter().fold((0, 0, 0), |(q, t, x), l| {
            (q + l.quads.len(), t + l.mesh.triangles(), x + l.texts.len())
        })
    }
}

/// シェルの実フレームレートに関係なく同じ絵を出すため固定ステップで進める。
/// `std::time::Instant` は wasm32-unknown-unknown で使えないので時計に触らない。
pub const FIXED_DT: f32 = 1.0 / 60.0;

const PLAYER_SIZE: f32 = 72.0;
const PLAYER_SPEED: f32 = 420.0;
const RIPPLE_LIFE: f32 = 0.6;

#[derive(Clone, Copy, Debug)]
struct Ripple {
    x: f32,
    y: f32,
    age: f32,
}

/// パッドで四角を動かし、タッチに波紋を出すだけの参照シーン。
///
/// 「共有コアが実際に絵を出している」ことを全プラットフォームで示すための最小実装。
#[derive(Clone, Debug)]
pub struct DemoScene {
    pub player_x: f32,
    pub player_y: f32,
    elapsed: f32,
    ripples: Vec<Ripple>,
}

impl Default for DemoScene {
    fn default() -> Self {
        Self {
            player_x: 0.5,
            player_y: 0.5,
            elapsed: 0.0,
            ripples: Vec::new(),
        }
    }
}

impl DemoScene {
    /// 入力を取り込んで 1 ステップ進める。`taps` はこのフレームで新しく触れた
    /// 座標（ピクセル）。位置は 0..1 の正規化値で保持するので、画面回転や
    /// リサイズでプレイヤーが飛ばない。
    pub fn update(&mut self, width: u32, height: u32, pad: (f32, f32), taps: &[(f32, f32)]) {
        self.elapsed += FIXED_DT;

        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let (px, py) = pad;
        self.player_x = (self.player_x + px * PLAYER_SPEED * FIXED_DT / w).clamp(0.0, 1.0);
        self.player_y = (self.player_y + py * PLAYER_SPEED * FIXED_DT / h).clamp(0.0, 1.0);

        for (x, y) in taps {
            self.ripples.push(Ripple {
                x: x / w,
                y: y / h,
                age: 0.0,
            });
        }
        for r in &mut self.ripples {
            r.age += FIXED_DT;
        }
        self.ripples.retain(|r| r.age < RIPPLE_LIFE);
    }

    pub fn draw(&self, width: u32, height: u32, frame: u64, paused: bool) -> DrawList {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let mut quads = Vec::with_capacity(16);

        // 背景: 横帯のグラデーション。単色との差が出るので描画経路の確認になる。
        let bands = 12;
        for i in 0..bands {
            let t = i as f32 / (bands - 1) as f32;
            let y = h * i as f32 / bands as f32;
            quads.push(Quad::new(
                0.0,
                y,
                w,
                h / bands as f32 + 1.0,
                [
                    (24.0 + 26.0 * t) as u8,
                    (28.0 + 34.0 * t) as u8,
                    (46.0 + 60.0 * t) as u8,
                    255,
                ],
            ));
        }

        // 波紋: 中空の枠を 4 辺の矩形で描く。
        for r in &self.ripples {
            let t = (r.age / RIPPLE_LIFE).clamp(0.0, 1.0);
            let radius = 24.0 + 90.0 * t;
            let alpha = (220.0 * (1.0 - t)) as u8;
            let thickness = 4.0;
            let cx = r.x * w;
            let cy = r.y * h;
            let color = [120, 220, 255, alpha];
            quads.push(Quad::new(
                cx - radius,
                cy - radius,
                radius * 2.0,
                thickness,
                color,
            ));
            quads.push(Quad::new(
                cx - radius,
                cy + radius - thickness,
                radius * 2.0,
                thickness,
                color,
            ));
            quads.push(Quad::new(
                cx - radius,
                cy - radius,
                thickness,
                radius * 2.0,
                color,
            ));
            quads.push(Quad::new(
                cx + radius - thickness,
                cy - radius,
                thickness,
                radius * 2.0,
                color,
            ));
        }

        // プレイヤー: 影 → 本体 → ハイライト。
        let size = PLAYER_SIZE * (w.min(h) / 720.0).clamp(0.5, 2.0);
        let cx = self.player_x * (w - size);
        let cy = self.player_y * (h - size);
        quads.push(Quad::new(cx + 6.0, cy + 8.0, size, size, [0, 0, 0, 90]));
        let pulse = 0.5 + 0.5 * (self.elapsed * 3.0).sin();
        quads.push(Quad::new(
            cx,
            cy,
            size,
            size,
            if paused {
                [140, 140, 150, 255]
            } else {
                [255, (150.0 + 80.0 * pulse) as u8, 90, 255]
            },
        ));
        quads.push(Quad::new(
            cx + size * 0.18,
            cy + size * 0.18,
            size * 0.28,
            size * 0.28,
            [255, 255, 255, 200],
        ));

        // 下端のフレームカウンタ（60 フレームで 1 往復する目盛り）。
        let bar_h = 6.0;
        quads.push(Quad::new(0.0, h - bar_h, w, bar_h, [0, 0, 0, 120]));
        let progress = (frame % 60) as f32 / 60.0;
        quads.push(Quad::new(
            0.0,
            h - bar_h,
            w * progress,
            bar_h,
            [90, 230, 160, 255],
        ));

        DrawList {
            clear: [16, 18, 28, 255],
            quads,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_moves_player_right() {
        let mut sc = DemoScene::default();
        let before = sc.player_x;
        for _ in 0..30 {
            sc.update(1280, 720, (1.0, 0.0), &[]);
        }
        assert!(sc.player_x > before);
        assert!(sc.player_x <= 1.0);
    }

    #[test]
    fn player_stays_in_bounds() {
        let mut sc = DemoScene::default();
        for _ in 0..600 {
            sc.update(1280, 720, (-1.0, -1.0), &[]);
        }
        assert_eq!(sc.player_x, 0.0);
        assert_eq!(sc.player_y, 0.0);
    }

    #[test]
    fn ripples_expire() {
        let mut sc = DemoScene::default();
        sc.update(800, 600, (0.0, 0.0), &[(100.0, 100.0)]);
        assert_eq!(sc.ripples.len(), 1);
        for _ in 0..(RIPPLE_LIFE / FIXED_DT) as usize + 2 {
            sc.update(800, 600, (0.0, 0.0), &[]);
        }
        assert!(sc.ripples.is_empty());
    }

    #[test]
    fn draw_list_covers_screen_and_stays_inside() {
        let sc = DemoScene::default();
        let dl = sc.draw(640, 360, 7, false);
        assert!(!dl.quads.is_empty());
        // 背景帯が画面全幅を覆う
        assert!(dl.quads.iter().any(|q| q.w >= 640.0));
        // プレイヤーが画面内に収まる
        assert!(dl
            .quads
            .iter()
            .all(|q| q.x > -200.0 && q.y > -200.0 && q.x < 640.0 + 200.0));
    }

    #[test]
    fn paused_player_is_desaturated() {
        let sc = DemoScene::default();
        let running = sc.draw(640, 360, 0, false);
        let paused = sc.draw(640, 360, 0, true);
        assert_ne!(running.quads, paused.quads);
    }
}
