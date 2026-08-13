//! 共有セッション状態（ネイティブシェルが毎フレーム駆動）。

use crate::input::{KeyEvent, PointerEvent, PointerPhase, VirtualPad};
use crate::scene::{DemoScene, DrawList};
use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize)]
pub struct FrameStats {
    pub frame: u64,
    pub width: u32,
    pub height: u32,
    pub paused: bool,
    pub pointer_count: u32,
}

#[derive(Debug)]
pub struct SharedSession {
    pub width: u32,
    pub height: u32,
    pub paused: bool,
    pub frame: u64,
    pub asset_root: String,
    pub pad: VirtualPad,
    pub scene: DemoScene,
    pointers: Vec<PointerEvent>,
    pending_keys: Vec<KeyEvent>,
    /// このフレームで新しく触れた座標。`poll_pointers` の意味を変えずに
    /// 「押した瞬間」をシーンへ渡すための別キュー。
    pending_taps: Vec<(f32, f32)>,
    #[cfg(feature = "render")]
    renderer: Option<crate::render::Renderer>,
}

impl Default for SharedSession {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            paused: false,
            frame: 0,
            asset_root: String::new(),
            pad: VirtualPad::new(0.25),
            scene: DemoScene::default(),
            pointers: Vec::new(),
            pending_keys: Vec::new(),
            pending_taps: Vec::new(),
            #[cfg(feature = "render")]
            renderer: None,
        }
    }
}

impl SharedSession {
    pub fn create_surface(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        #[cfg(feature = "render")]
        if let Some(r) = self.renderer.as_mut() {
            r.resize(self.width, self.height);
        }
    }

    pub fn set_asset_root(&mut self, root: impl Into<String>) {
        self.asset_root = root.into();
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn push_pointer(&mut self, ev: PointerEvent) {
        if matches!(ev.phase, PointerPhase::Begin) {
            self.pending_taps.push((ev.x, ev.y));
        }
        // 同一 id は最新で置換、begin は追加
        if let Some(slot) = self.pointers.iter_mut().find(|p| p.id == ev.id) {
            *slot = ev;
        } else {
            self.pointers.push(ev);
        }
        // end/cancel は次フレームで掃除してもよいが、ここでは保持して poll で返す
    }

    pub fn set_pad(&mut self, x: f32, y: f32) {
        self.pad.set_stick(x, y);
        self.pending_keys.extend(self.pad.drain_key_events());
    }

    pub fn poll_pointers(&mut self) -> Vec<PointerEvent> {
        let out = self.pointers.clone();
        self.pointers.retain(|p| {
            matches!(
                p.phase,
                crate::input::PointerPhase::Begin | crate::input::PointerPhase::Move
            )
        });
        out
    }

    pub fn poll_keys(&mut self) -> Vec<KeyEvent> {
        std::mem::take(&mut self.pending_keys)
    }

    pub fn request_frame(&mut self) -> FrameStats {
        if !self.paused {
            self.frame = self.frame.saturating_add(1);
            let taps = std::mem::take(&mut self.pending_taps);
            let pad = self.pad.stick();
            self.scene.update(self.width, self.height, pad, &taps);
        }
        FrameStats {
            frame: self.frame,
            width: self.width,
            height: self.height,
            paused: self.paused,
            pointer_count: self.pointers.len() as u32,
        }
    }

    /// 現在の状態から描画内容を作る。GPU に触らないので単体テスト可能。
    pub fn draw_list(&self) -> DrawList {
        self.scene
            .draw(self.width, self.height, self.frame, self.paused)
    }

    pub fn stats_json(&self) -> String {
        let s = FrameStats {
            frame: self.frame,
            width: self.width,
            height: self.height,
            paused: self.paused,
            pointer_count: self.pointers.len() as u32,
        };
        serde_json::to_string(&s).unwrap_or_else(|_| "{}".into())
    }
}

#[cfg(feature = "render")]
impl SharedSession {
    /// レンダラを束ねる。セッション側の画面サイズに合わせ直す。
    pub fn attach_renderer(&mut self, mut renderer: crate::render::Renderer) {
        renderer.resize(self.width, self.height);
        self.renderer = Some(renderer);
    }

    pub fn detach_renderer(&mut self) {
        self.renderer = None;
    }

    pub fn has_renderer(&self) -> bool {
        self.renderer.is_some()
    }

    /// 現在のシーンを 1 枚描く。レンダラ未接続ならエラー。
    pub fn render(&mut self) -> Result<(), String> {
        let list = self.draw_list();
        match self.renderer.as_mut() {
            Some(r) => r.render(&list),
            None => Err("no renderer attached".into()),
        }
    }

    /// オフスクリーンレンダラの内容を RGBA8 で読み出す。
    pub fn render_readback(&self) -> Result<Vec<u8>, String> {
        match self.renderer.as_ref() {
            Some(r) => r.read_rgba(),
            None => Err("no renderer attached".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::PointerPhase;

    #[test]
    fn frame_advances_and_scene_follows_pad() {
        let mut s = SharedSession::default();
        s.create_surface(800, 600);
        s.set_pad(1.0, 0.0);
        let start = s.scene.player_x;
        for _ in 0..30 {
            s.request_frame();
        }
        assert_eq!(s.frame, 30);
        assert!(s.scene.player_x > start);
    }

    #[test]
    fn paused_session_freezes_scene() {
        let mut s = SharedSession::default();
        s.set_pad(1.0, 0.0);
        s.pause();
        let before = s.scene.player_x;
        for _ in 0..30 {
            s.request_frame();
        }
        assert_eq!(s.frame, 0);
        assert_eq!(s.scene.player_x, before);
    }

    #[test]
    fn tap_is_consumed_once() {
        let mut s = SharedSession::default();
        s.push_pointer(PointerEvent {
            id: 0,
            x: 10.0,
            y: 20.0,
            phase: PointerPhase::Begin,
            pressure: 1.0,
        });
        // 押しっぱなしでも波紋は 1 回だけ増える
        let first = s.request_frame();
        assert_eq!(first.frame, 1);
        let before = s.draw_list().quads.len();
        s.request_frame();
        let after = s.draw_list().quads.len();
        assert!(after <= before, "held pointer kept spawning ripples");
    }

    #[test]
    fn draw_list_is_not_empty() {
        let s = SharedSession::default();
        assert!(!s.draw_list().quads.is_empty());
    }
}
