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
}

impl Quad {
    pub fn new(x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) -> Self {
        Self { x, y, w, h, color }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

/// 1 フレームぶんの描画内容。
#[derive(Clone, Debug, Default)]
pub struct DrawList {
    pub clear: [u8; 4],
    pub quads: Vec<Quad>,
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
