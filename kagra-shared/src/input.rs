//! ポインタ／仮想パッド（Python `kagra.touch` と契約を揃える）。

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum PointerPhase {
    Begin = 0,
    Move = 1,
    End = 2,
    Cancel = 3,
}

impl PointerPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Begin => "begin",
            Self::Move => "move",
            Self::End => "end",
            Self::Cancel => "cancel",
        }
    }

    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Begin),
            1 => Some(Self::Move),
            2 => Some(Self::End),
            3 => Some(Self::Cancel),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PointerEvent {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub phase: PointerPhase,
    pub pressure: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyEvent {
    pub name: String,
    pub down: bool,
}

#[derive(Clone, Debug, Default)]
pub struct VirtualPad {
    pub deadzone: f32,
    lx: f32,
    ly: f32,
    held: Vec<&'static str>,
}

impl VirtualPad {
    pub fn new(deadzone: f32) -> Self {
        Self {
            deadzone: deadzone.max(0.0),
            ..Default::default()
        }
    }

    pub fn set_stick(&mut self, x: f32, y: f32) {
        self.lx = x.clamp(-1.0, 1.0);
        self.ly = y.clamp(-1.0, 1.0);
    }

    pub fn clear(&mut self) {
        self.lx = 0.0;
        self.ly = 0.0;
    }

    /// デッドゾーンを適用したスティック値。
    pub fn stick(&self) -> (f32, f32) {
        let dz = self.deadzone;
        let f = |v: f32| if v.abs() >= dz { v } else { 0.0 };
        (f(self.lx), f(self.ly))
    }

    pub fn desired_keys(&self) -> Vec<&'static str> {
        let mut keys = Vec::new();
        if self.lx.abs() >= self.deadzone {
            keys.push(if self.lx > 0.0 { "D" } else { "A" });
        }
        if self.ly.abs() >= self.deadzone {
            // 画面座標: +y 下 → S
            keys.push(if self.ly > 0.0 { "S" } else { "W" });
        }
        keys
    }

    /// 差分を KeyEvent として返す。
    pub fn drain_key_events(&mut self) -> Vec<KeyEvent> {
        let want = self.desired_keys();
        let mut out = Vec::new();
        for k in &want {
            if !self.held.iter().any(|h| h == k) {
                out.push(KeyEvent {
                    name: (*k).to_string(),
                    down: true,
                });
            }
        }
        for h in &self.held {
            if !want.iter().any(|k| k == h) {
                out.push(KeyEvent {
                    name: (*h).to_string(),
                    down: false,
                });
            }
        }
        self.held = want;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_maps_to_wasd() {
        let mut pad = VirtualPad::new(0.2);
        pad.set_stick(0.9, 0.0);
        let ev = pad.drain_key_events();
        assert!(ev.iter().any(|e| e.name == "D" && e.down));
        pad.set_stick(0.0, 0.0);
        let ev2 = pad.drain_key_events();
        assert!(ev2.iter().any(|e| e.name == "D" && !e.down));
    }
}
