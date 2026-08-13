//! フォント無しの共通 UI。矩形ヒットだけでポーズメニューを扱う。

use crate::scene::Quad;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiMode {
    #[default]
    Hud,
    Pause,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiAction {
    Resume,
    Restart,
    ToggleMute,
}

#[derive(Clone, Copy, Debug)]
struct Button {
    id: UiAction,
    quad: Quad,
}

/// ポーズメニューのレイアウトとヒットテスト。
#[derive(Clone, Debug)]
pub struct PauseMenu {
    buttons: Vec<Button>,
}

impl PauseMenu {
    pub fn layout(width: u32, height: u32) -> Self {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
        let bw = 280.0 * scale;
        let bh = 52.0 * scale;
        let gap = 18.0 * scale;
        let cx = (w - bw) * 0.5;
        let total_h = bh * 3.0 + gap * 2.0;
        let mut y = (h - total_h) * 0.5;
        let mut buttons = Vec::with_capacity(3);
        for (id, color) in [
            (UiAction::Resume, [70, 160, 110, 230]),
            (UiAction::Restart, [70, 120, 180, 230]),
            (UiAction::ToggleMute, [150, 110, 70, 230]),
        ] {
            buttons.push(Button {
                id,
                quad: Quad::new(cx, y, bw, bh, color),
            });
            y += bh + gap;
        }
        Self { buttons }
    }

    pub fn quads(&self) -> Vec<Quad> {
        self.buttons.iter().map(|b| b.quad).collect()
    }

    pub fn hit(&self, x: f32, y: f32) -> Option<UiAction> {
        self.buttons
            .iter()
            .find(|b| b.quad.contains(x, y))
            .map(|b| b.id)
    }
}

/// ミッション進捗バナー用のクアッド。
pub fn mission_banner(width: u32, height: u32, progress: f32, complete: bool) -> Vec<Quad> {
    let w = width.max(1) as f32;
    let scale = (w.min(height.max(1) as f32) / 720.0).clamp(0.5, 2.0);
    let pad = 18.0 * scale;
    let bar_w = 240.0 * scale;
    let bar_h = 10.0 * scale;
    let y = pad + 20.0 * scale;
    let fill = if complete {
        [120, 220, 140, 255]
    } else {
        [255, 200, 90, 255]
    };
    vec![
        Quad::new(pad, y, bar_w, bar_h, [0, 0, 0, 130]),
        Quad::new(pad, y, bar_w * progress.clamp(0.0, 1.0), bar_h, fill),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_button_is_centered_and_hittable() {
        let menu = PauseMenu::layout(1280, 720);
        let resume = &menu.buttons[0].quad;
        let cx = resume.x + resume.w * 0.5;
        let cy = resume.y + resume.h * 0.5;
        assert_eq!(menu.hit(cx, cy), Some(UiAction::Resume));
        assert_eq!(menu.hit(0.0, 0.0), None);
    }
}
