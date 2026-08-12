//! 共有セッション状態（ネイティブシェルが毎フレーム駆動）。

use crate::input::{KeyEvent, PointerEvent, VirtualPad};
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
    pointers: Vec<PointerEvent>,
    pending_keys: Vec<KeyEvent>,
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
            pointers: Vec::new(),
            pending_keys: Vec::new(),
        }
    }
}

impl SharedSession {
    pub fn create_surface(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
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
        self.pointers
            .retain(|p| matches!(p.phase, crate::input::PointerPhase::Begin | crate::input::PointerPhase::Move));
        out
    }

    pub fn poll_keys(&mut self) -> Vec<KeyEvent> {
        std::mem::take(&mut self.pending_keys)
    }

    pub fn request_frame(&mut self) -> FrameStats {
        if !self.paused {
            self.frame = self.frame.saturating_add(1);
        }
        FrameStats {
            frame: self.frame,
            width: self.width,
            height: self.height,
            paused: self.paused,
            pointer_count: self.pointers.len() as u32,
        }
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
